use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc,
    },
};

use rust_models::common::{
    create_trade_response::CreateTradeStatus, delete_trade_response::DeleteTradeStatus,
    get_trade_response::GetTradeStatus, trade_service_server::*, CreateTradeRequest,
    CreateTradeResponse, DeleteTradeRequest, DeleteTradeResponse, GetTradeRequest,
    GetTradeResponse, TradeId, TradeRequest,
};
use tokio::sync::{mpsc, oneshot, RwLock};
use tonic::Status;

use crate::{
    db::models::user::User,
    http::dependencies::ServerDependencies,
    trading::{
        backend::TradeBackend,
        market::{MarketProcessor, SwapPair},
        models::market_order::MarketOrder,
        models::user::{User as trade_user, WalletOperations},
    },
};

#[derive(Debug, Clone)]
pub struct UserTradeSubmissionGuard {
    tx: mpsc::Sender<UserSubmissionCommand>,
}

impl Default for UserTradeSubmissionGuard {
    fn default() -> Self {
        Self::new()
    }
}

enum UserSubmissionCommand {
    Acquire {
        user_id: i32,
        responder: oneshot::Sender<bool>,
    },
    Release {
        user_id: i32,
    },
}

impl UserTradeSubmissionGuard {
    pub fn new() -> Self {
        let (tx, mut rx) = mpsc::channel(256);

        tokio::spawn(async move {
            let mut active_users = HashSet::new();

            while let Some(command) = rx.recv().await {
                match command {
                    UserSubmissionCommand::Acquire { user_id, responder } => {
                        let is_available = active_users.insert(user_id);
                        let _ = responder.send(is_available);
                    }
                    UserSubmissionCommand::Release { user_id } => {
                        active_users.remove(&user_id);
                    }
                }
            }
        });

        Self { tx }
    }

    #[allow(clippy::result_large_err)]
    pub async fn acquire(&self, user_id: i32) -> Result<(), Status> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(UserSubmissionCommand::Acquire {
                user_id,
                responder: tx,
            })
            .await
            .map_err(|_| Status::internal("Failed to acquire trade submission slot"))?;

        let is_available = rx
            .await
            .map_err(|_| Status::internal("Trade submission coordinator closed"))?;

        if !is_available {
            return Err(Status::failed_precondition(format!(
                "Another trade is already in progress for user {}",
                user_id
            )));
        }

        Ok(())
    }

    #[allow(clippy::result_large_err)]
    pub async fn release(&self, user_id: i32) -> Result<(), Status> {
        self.tx
            .send(UserSubmissionCommand::Release { user_id })
            .await
            .map_err(|_| Status::internal("Failed to release trade submission slot"))?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct TrackedTrade {
    owner_user_id: i32,
    trade_request: TradeRequest,
}

#[derive(Debug, Clone)]
struct TradeStateStore {
    next_trade_id: Arc<AtomicI32>,
    trades: Arc<RwLock<HashMap<i32, TrackedTrade>>>,
}

impl Default for TradeStateStore {
    fn default() -> Self {
        Self {
            next_trade_id: Arc::new(AtomicI32::new(1)),
            trades: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl TradeStateStore {
    fn allocate_trade_id(&self) -> i32 {
        self.next_trade_id.fetch_add(1, Ordering::SeqCst)
    }

    async fn insert(&self, trade_id: i32, owner_user_id: i32, trade_request: TradeRequest) {
        let mut trades = self.trades.write().await;
        trades.insert(
            trade_id,
            TrackedTrade {
                owner_user_id,
                trade_request,
            },
        );
    }

    async fn get_owned(&self, trade_id: i32, user_id: i32) -> Option<TradeRequest> {
        let trades = self.trades.read().await;
        trades
            .get(&trade_id)
            .filter(|tracked| tracked.owner_user_id == user_id)
            .map(|tracked| tracked.trade_request.clone())
    }

    async fn delete_owned(&self, trade_id: i32, user_id: i32) -> bool {
        let mut trades = self.trades.write().await;
        let is_owned = trades
            .get(&trade_id)
            .map(|tracked| tracked.owner_user_id == user_id)
            .unwrap_or(false);
        if is_owned {
            trades.remove(&trade_id);
            return true;
        }

        false
    }
}

#[derive(Debug)]
pub struct TradeServiceImpl {
    _dependencies: ServerDependencies,
    trade_backend: TradeBackend,
    trade_submission_guard: UserTradeSubmissionGuard,
    trade_state_store: TradeStateStore,
}

impl TradeServiceImpl {
    pub fn new(
        dependencies: ServerDependencies,
        trade_backend: TradeBackend,
        trade_submission_guard: UserTradeSubmissionGuard,
    ) -> Self {
        Self {
            _dependencies: dependencies,
            trade_backend,
            trade_submission_guard,
            trade_state_store: TradeStateStore::default(),
        }
    }
}

#[tonic::async_trait]
impl TradeService for TradeServiceImpl {
    async fn create_trade(
        &self,
        request: tonic::Request<CreateTradeRequest>,
    ) -> Result<tonic::Response<CreateTradeResponse>, tonic::Status> {
        // take user out of the request
        let user = request
            .extensions()
            .get::<User>()
            .ok_or_else(|| Status::not_found("User not found"))?
            .to_owned();
        let user_id = user.id.ok_or_else(|| Status::internal("User id missing"))?;

        self.trade_submission_guard
            .acquire(user_id)
            .await
            .map_err(|err| {
                eprintln!("Trade submission rejected for user {user_id}: {err}");
                err
            })?;

        let result = async {
            // Unwrap is fine since the trade_request being the request is validated by the server
            let create_trade_request = request.into_inner().trade_request.unwrap();
            // validate swap_pair is in the market
            let swap_pair = SwapPair::new(
                create_trade_request.symbol_source.clone(),
                create_trade_request.symbol_dest.clone(),
            );

            let market = self
                .trade_backend
                .get_market(swap_pair.clone())
                .ok_or_else(|| {
                    Status::not_found(format!("Market for: {:#} does not exist", swap_pair))
                })?;

            // transform the user from the tonic user to the internal user representation.
            let user: trade_user = trade_user::from(user);

            // Check if user has src and dst wallets, and also check if they have enough src amount
            user.check_order_prereqs(create_trade_request.clone())
                .await?;

            market
                .send_order(MarketOrder::new(create_trade_request.clone(), user))
                .map_err(|err| Status::internal(format!("Error submitting order: {}", err)))?;

            let trade_id = self.trade_state_store.allocate_trade_id();
            self.trade_state_store
                .insert(trade_id, user_id, create_trade_request.clone())
                .await;

            // package up user and swap pair and send it to the market for processing
            let response = CreateTradeResponse {
                status: CreateTradeStatus::Ok.into(),
                trade_id: Some(TradeId { trade_id }),
                trade_request: Some(create_trade_request),
            };

            Ok(tonic::Response::new(response))
        }
        .await;

        self.trade_submission_guard.release(user_id).await.ok();
        result
    }

    async fn get_trade(
        &self,
        request: tonic::Request<GetTradeRequest>,
    ) -> Result<tonic::Response<GetTradeResponse>, tonic::Status> {
        let user = request
            .extensions()
            .get::<User>()
            .ok_or_else(|| Status::not_found("User not found"))?
            .to_owned();
        let user_id = user.id.ok_or_else(|| Status::internal("User id missing"))?;

        let trade_id = request
            .into_inner()
            .trade_id
            .ok_or_else(|| Status::invalid_argument("Trade id is required"))?
            .trade_id;

        let maybe_trade = self.trade_state_store.get_owned(trade_id, user_id).await;

        let response = match maybe_trade {
            Some(trade_request) => GetTradeResponse {
                status: GetTradeStatus::Ok.into(),
                trade_id: Some(TradeId { trade_id }),
                trade_request: Some(trade_request),
            },
            None => GetTradeResponse {
                status: GetTradeStatus::NotFound.into(),
                trade_id: Some(TradeId { trade_id }),
                trade_request: None,
            },
        };

        Ok(tonic::Response::new(response))
    }

    async fn delete_trade(
        &self,
        request: tonic::Request<DeleteTradeRequest>,
    ) -> Result<tonic::Response<DeleteTradeResponse>, tonic::Status> {
        let user = request
            .extensions()
            .get::<User>()
            .ok_or_else(|| Status::not_found("User not found"))?
            .to_owned();
        let user_id = user.id.ok_or_else(|| Status::internal("User id missing"))?;

        let trade_id = request
            .into_inner()
            .trade_id
            .ok_or_else(|| Status::invalid_argument("Trade id is required"))?
            .trade_id;

        let deleted = self.trade_state_store.delete_owned(trade_id, user_id).await;

        let response = DeleteTradeResponse {
            status: if deleted {
                DeleteTradeStatus::Ok.into()
            } else {
                DeleteTradeStatus::NotFound.into()
            },
            trade_id: Some(TradeId { trade_id }),
        };

        Ok(tonic::Response::new(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::{r2d2::ConnectionManager, sqlite::SqliteConnection};
    use rust_models::common::{
        trade_request::{TradeType, TransactionType},
        CreateTradeRequest, TradeRequest,
    };

    use crate::{
        db::manager::DBManager,
        http::dependencies::ServerDependencies,
        session::manager::SessionManager,
        trading::{backend::TradeBackend, market::Market},
    };

    fn fake_trade_request() -> TradeRequest {
        TradeRequest {
            trade_type: TradeType::Limit as i32,
            transaction_type: TransactionType::Buy as i32,
            symbol_source: "USD".into(),
            symbol_dest: "BTC".into(),
            source_quantity: 5.0,
            price: Some(100.0),
            valid_from: None,
            valid_to: None,
        }
    }

    #[tokio::test]
    async fn same_user_requests_are_serialized() {
        let guard = UserTradeSubmissionGuard::new();

        guard
            .acquire(42)
            .await
            .expect("first user trade should pass");

        let second = guard.acquire(42).await;
        assert!(second.is_err());
        assert_eq!(second.unwrap_err().code(), tonic::Code::FailedPrecondition);

        guard.release(42).await.expect("release should succeed");
    }

    #[tokio::test]
    async fn different_users_can_submit_without_blocking_each_other() {
        let guard = UserTradeSubmissionGuard::new();

        guard.acquire(1).await.expect("first user should acquire");

        let second_user = guard.acquire(2).await;
        assert!(second_user.is_ok(), "a different user should not block");

        guard.release(1).await.expect("first user should release");
        guard.release(2).await.expect("second user should release");
    }

    #[tokio::test]
    async fn different_users_can_trade_across_distinct_markets() {
        let manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = diesel::r2d2::Pool::builder()
            .max_size(1)
            .build(manager)
            .expect("sqlite pool should build for tests");

        let dependencies = ServerDependencies::new(
            Arc::new(DBManager::new(pool)),
            Arc::new(SessionManager::default()),
        );

        let mut trade_backend = TradeBackend::new();
        trade_backend.add_market(Market::new("USD", "BTC"));
        trade_backend.add_market(Market::new("ETH", "USD"));

        let service = TradeServiceImpl::new(
            dependencies,
            trade_backend,
            UserTradeSubmissionGuard::default(),
        );

        let mut first_request = tonic::Request::new(CreateTradeRequest {
            trade_request: Some(TradeRequest {
                trade_type: TradeType::Limit as i32,
                transaction_type: TransactionType::Buy as i32,
                symbol_source: "USD".to_string(),
                symbol_dest: "BTC".to_string(),
                source_quantity: 1.0,
                price: Some(100.0),
                valid_from: None,
                valid_to: None,
            }),
        });
        first_request
            .extensions_mut()
            .insert(crate::db::models::user::User {
                id: Some(17),
                email: "user-17@example.com".to_string(),
                password: "pw".to_string(),
                first_name: "first".to_string(),
                last_name: "last".to_string(),
            });

        let mut second_request = tonic::Request::new(CreateTradeRequest {
            trade_request: Some(TradeRequest {
                trade_type: TradeType::Limit as i32,
                transaction_type: TransactionType::Sell as i32,
                symbol_source: "ETH".to_string(),
                symbol_dest: "USD".to_string(),
                source_quantity: 2.0,
                price: Some(90.0),
                valid_from: None,
                valid_to: None,
            }),
        });
        second_request
            .extensions_mut()
            .insert(crate::db::models::user::User {
                id: Some(18),
                email: "user-18@example.com".to_string(),
                password: "pw".to_string(),
                first_name: "first".to_string(),
                last_name: "last".to_string(),
            });

        let first = service
            .create_trade(first_request)
            .await
            .expect("first market trade should succeed")
            .into_inner();

        let second = service
            .create_trade(second_request)
            .await
            .expect("second market trade should succeed")
            .into_inner();

        assert_eq!(first.status, CreateTradeStatus::Ok as i32);
        assert_eq!(second.status, CreateTradeStatus::Ok as i32);
        assert_ne!(
            first
                .trade_id
                .expect("first trade id should be present")
                .trade_id,
            second
                .trade_id
                .expect("second trade id should be present")
                .trade_id
        );
    }

    #[tokio::test]
    async fn trade_state_store_tracks_and_filters_by_owner() {
        let store = TradeStateStore::default();
        let trade_id = store.allocate_trade_id();

        store.insert(trade_id, 7, fake_trade_request()).await;

        let owner_view = store.get_owned(trade_id, 7).await;
        let other_view = store.get_owned(trade_id, 8).await;

        assert!(owner_view.is_some());
        assert!(other_view.is_none());
    }

    #[tokio::test]
    async fn trade_state_store_deletes_only_for_owner() {
        let store = TradeStateStore::default();
        let trade_id = store.allocate_trade_id();

        store.insert(trade_id, 11, fake_trade_request()).await;

        let deleted_other = store.delete_owned(trade_id, 12).await;
        assert!(!deleted_other);

        let deleted_owner = store.delete_owned(trade_id, 11).await;
        assert!(deleted_owner);

        assert!(store.get_owned(trade_id, 11).await.is_none());
    }
}
