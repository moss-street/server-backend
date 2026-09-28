use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc,
    },
};

use anyhow::anyhow;
use rust_models::common::{
    create_trade_response::CreateTradeStatus, delete_trade_response::DeleteTradeStatus,
    get_trade_response::GetTradeStatus, trade_request::TradeType, trade_service_server::*,
    CreateTradeRequest, CreateTradeResponse, DeleteTradeRequest, DeleteTradeResponse,
    GetTradeRequest, GetTradeResponse, TradeId, TradeRequest,
};
use tokio::sync::{mpsc, oneshot, RwLock};
use tonic::Status;

use crate::{
    db::models::user::User,
    http::dependencies::ServerDependencies,
    trading::{
        backend::TradeBackend, ledger::AssetLedger, market::SwapPair,
        models::market_order::MarketOrder,
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
    asset_ledger: AssetLedger,
    trade_submission_guard: UserTradeSubmissionGuard,
    trade_state_store: TradeStateStore,
}

impl TradeServiceImpl {
    pub fn new(
        dependencies: ServerDependencies,
        trade_backend: TradeBackend,
        trade_submission_guard: UserTradeSubmissionGuard,
    ) -> Self {
        let asset_ledger = AssetLedger::new(dependencies.db_manager.clone());
        Self {
            _dependencies: dependencies,
            trade_backend,
            asset_ledger,
            trade_submission_guard,
            trade_state_store: TradeStateStore::default(),
        }
    }
}

#[allow(clippy::result_large_err)]
fn validate_trade_request(request: &TradeRequest) -> Result<(), Status> {
    let trade_type = TradeType::try_from(request.trade_type)
        .map_err(|_| Status::invalid_argument("Trade type is invalid"))?;

    if trade_type == TradeType::Unspecified {
        return Err(Status::invalid_argument("Trade type is required"));
    }
    if rust_models::common::trade_request::TransactionType::try_from(request.transaction_type)
        .map(|transaction_type| {
            transaction_type == rust_models::common::trade_request::TransactionType::Unspecified
        })
        .unwrap_or(true)
    {
        return Err(Status::invalid_argument("Transaction type is invalid"));
    }
    if request.symbol_source.trim().is_empty()
        || request.symbol_dest.trim().is_empty()
        || request.symbol_source == request.symbol_dest
    {
        return Err(Status::invalid_argument(
            "Source and destination symbols must be distinct and non-empty",
        ));
    }
    if !request.source_quantity.is_finite() || request.source_quantity <= 0.0 {
        return Err(Status::invalid_argument(
            "Source quantity must be a positive finite number",
        ));
    }

    match (trade_type, request.price) {
        (TradeType::Limit, Some(price)) if price.is_finite() && price > 0.0 => Ok(()),
        (TradeType::Limit, None) => Err(Status::invalid_argument(
            "Limit orders require a positive finite price",
        )),
        (TradeType::Limit, Some(_)) => Err(Status::invalid_argument(
            "Limit orders require a positive finite price",
        )),
        (TradeType::Market, None) => Ok(()),
        (TradeType::Market, Some(_)) => Err(Status::invalid_argument(
            "Market orders must not include a price",
        )),
        (TradeType::Unspecified, _) => unreachable!(),
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
            let create_trade_request = request
                .into_inner()
                .trade_request
                .ok_or_else(|| Status::invalid_argument("Trade request is required"))?;
            validate_trade_request(&create_trade_request)?;
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

            let trade_id = self.trade_state_store.allocate_trade_id();
            self.asset_ledger
                .ensure_default_accounts(user_id)
                .map_err(|err| {
                    Status::internal(format!("Failed to initialize user assets: {err}"))
                })?;
            self.asset_ledger
                .reserve_order(
                    trade_id,
                    user_id,
                    &create_trade_request.symbol_source,
                    &create_trade_request.symbol_dest,
                    create_trade_request.source_quantity,
                )
                .map_err(|_| Status::failed_precondition("Insufficient source asset balance"))?;

            let market_order = MarketOrder::new_with_order_id(
                create_trade_request.clone(),
                crate::trading::models::user::User::from(user),
                i64::from(trade_id),
            );
            let submission = match market.submit_order_with_settlement(market_order, |fills| {
                self.asset_ledger.settle_fills(fills)
            }) {
                Ok(submission) => submission,
                Err(err) => {
                    self.asset_ledger
                        .refund_cancelled_order(trade_id, user_id)
                        .map_err(|refund_err| {
                            Status::internal(format!(
                                "Order failed ({err}) and reservation cleanup failed ({refund_err})"
                            ))
                        })?;
                    return Err(Status::internal(format!("Error submitting order: {err}")));
                }
            };

            if !submission.rests_on_book && submission.remaining_quantity > 0.0 {
                self.asset_ledger
                    .refund_cancelled_order(trade_id, user_id)
                    .map_err(|err| {
                        Status::internal(format!("Failed to refund unfilled order amount: {err}"))
                    })?;
            }

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

        let tracked_trade = self.trade_state_store.get_owned(trade_id, user_id).await;
        let mut deleted = false;
        if let Some(trade_request) = tracked_trade {
            let swap_pair = SwapPair::new(
                trade_request.symbol_source.clone(),
                trade_request.symbol_dest.clone(),
            );
            let market = self
                .trade_backend
                .get_market(swap_pair)
                .ok_or_else(|| Status::internal("Trade market is no longer available"))?;
            market
                .cancel_order_with_settlement(trade_id as u64, |_| {
                    match self
                        .asset_ledger
                        .refund_cancelled_order(trade_id, user_id)?
                    {
                        Some(_) => Ok(()),
                        None => Err(anyhow!("Active order reservation was not found")),
                    }
                })
                .map_err(|err| Status::internal(format!("Failed to cancel order: {err}")))?;
            deleted = self.trade_state_store.delete_owned(trade_id, user_id).await;
        }

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

    #[test]
    fn trade_request_validation_accepts_market_and_limit_orders() {
        let limit_request = fake_trade_request();
        assert!(validate_trade_request(&limit_request).is_ok());

        let mut market_request = fake_trade_request();
        market_request.trade_type = TradeType::Market as i32;
        market_request.price = None;
        assert!(validate_trade_request(&market_request).is_ok());
    }

    #[test]
    fn trade_request_validation_rejects_invalid_fields() {
        fn assert_invalid(request: TradeRequest) {
            assert_eq!(
                validate_trade_request(&request)
                    .expect_err("malformed trade request should be rejected")
                    .code(),
                tonic::Code::InvalidArgument
            );
        }

        let mut request = fake_trade_request();
        request.source_quantity = 0.0;
        assert_invalid(request);

        let mut request = fake_trade_request();
        request.source_quantity = f64::INFINITY;
        assert_invalid(request);

        let mut request = fake_trade_request();
        request.price = Some(f64::NAN);
        assert_invalid(request);

        let mut request = fake_trade_request();
        request.price = None;
        assert_invalid(request);

        let mut request = fake_trade_request();
        request.trade_type = 99;
        assert_invalid(request);

        let mut request = fake_trade_request();
        request.transaction_type = 99;
        assert_invalid(request);

        let mut request = fake_trade_request();
        request.symbol_dest = request.symbol_source.clone();
        assert_invalid(request);

        let mut request = fake_trade_request();
        request.symbol_source.clear();
        assert_invalid(request);

        let mut request = fake_trade_request();
        request.trade_type = TradeType::Market as i32;
        assert_invalid(request);
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
    async fn create_trade_rejects_missing_trade_request_without_panicking() {
        let manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = diesel::r2d2::Pool::builder()
            .max_size(1)
            .build(manager)
            .expect("sqlite pool should build for tests");
        let dependencies = ServerDependencies::new(
            Arc::new(DBManager::new(pool)),
            Arc::new(SessionManager::default()),
        );
        let service = TradeServiceImpl::new(
            dependencies,
            TradeBackend::new(),
            UserTradeSubmissionGuard::default(),
        );
        let mut request = tonic::Request::new(CreateTradeRequest {
            trade_request: None,
        });
        request
            .extensions_mut()
            .insert(crate::db::models::user::User {
                id: Some(42),
                email: "user-42@example.com".to_string(),
                password: "pw".to_string(),
                first_name: "first".to_string(),
                last_name: "last".to_string(),
            });

        let result = service.create_trade(request).await;

        assert_eq!(
            result
                .expect_err("missing trade request should be rejected")
                .code(),
            tonic::Code::InvalidArgument
        );
    }

    #[tokio::test]
    async fn deleting_resting_trade_refunds_reserved_source_balance() {
        let manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = diesel::r2d2::Pool::builder()
            .max_size(1)
            .build(manager)
            .expect("sqlite pool should build for tests");
        let dependencies = ServerDependencies::new(
            Arc::new(DBManager::new(pool)),
            Arc::new(SessionManager::default()),
        );
        let mut connection = dependencies
            .db_manager
            .connection_pool
            .get()
            .expect("sqlite connection should be available");
        AssetLedger::initialize_database(&mut connection).expect("ledger schema should initialize");
        drop(connection);

        let ledger = AssetLedger::new(dependencies.db_manager.clone());
        let mut trade_backend = TradeBackend::new();
        trade_backend.add_market(Market::new("USD", "BTC"));
        let service = TradeServiceImpl::new(
            dependencies,
            trade_backend,
            UserTradeSubmissionGuard::default(),
        );

        let create = service
            .create_trade({
                let mut request = tonic::Request::new(CreateTradeRequest {
                    trade_request: Some(fake_trade_request()),
                });
                request
                    .extensions_mut()
                    .insert(crate::db::models::user::User {
                        id: Some(42),
                        email: "user-42@example.com".to_string(),
                        password: "pw".to_string(),
                        first_name: "first".to_string(),
                        last_name: "last".to_string(),
                    });
                request
            })
            .await
            .expect("resting trade should be accepted")
            .into_inner();
        let trade_id = create
            .trade_id
            .expect("trade id should be returned")
            .trade_id;

        assert_eq!(ledger.balance(42, "USD").unwrap(), Some((45.0, 5.0)));

        let delete = service
            .delete_trade({
                let mut request = tonic::Request::new(DeleteTradeRequest {
                    trade_id: Some(TradeId { trade_id }),
                });
                request
                    .extensions_mut()
                    .insert(crate::db::models::user::User {
                        id: Some(42),
                        email: "user-42@example.com".to_string(),
                        password: "pw".to_string(),
                        first_name: "first".to_string(),
                        last_name: "last".to_string(),
                    });
                request
            })
            .await
            .expect("resting trade should be cancellable")
            .into_inner();

        assert_eq!(delete.status, DeleteTradeStatus::Ok as i32);
        assert_eq!(ledger.balance(42, "USD").unwrap(), Some((50.0, 0.0)));
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
        let mut connection = dependencies
            .db_manager
            .connection_pool
            .get()
            .expect("sqlite connection should be available");
        AssetLedger::initialize_database(&mut connection).expect("ledger schema should initialize");
        drop(connection);

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
