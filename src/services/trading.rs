use std::collections::HashSet;

use rust_models::common::{
    create_trade_response::CreateTradeStatus, trade_service_server::*, CreateTradeRequest,
    CreateTradeResponse, DeleteTradeRequest, DeleteTradeResponse, GetTradeRequest,
    GetTradeResponse, TradeId,
};
use tokio::sync::{mpsc, oneshot};
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

#[derive(Debug)]
pub struct TradeServiceImpl {
    _dependencies: ServerDependencies,
    trade_backend: TradeBackend,
    trade_submission_guard: UserTradeSubmissionGuard,
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
                .unwrap_or_else(|err| {
                    eprintln!("Error sending order to channel: {}", err);
                });

            // package up user and swap pair and send it to the market for processing
            let response = CreateTradeResponse {
                status: CreateTradeStatus::Ok.into(),
                trade_id: Some(TradeId { trade_id: 1 }),
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
        _request: tonic::Request<GetTradeRequest>,
    ) -> Result<tonic::Response<GetTradeResponse>, tonic::Status> {
        unimplemented!("Not yet implemeneted")
    }

    async fn delete_trade(
        &self,
        _request: tonic::Request<DeleteTradeRequest>,
    ) -> Result<tonic::Response<DeleteTradeResponse>, tonic::Status> {
        unimplemented!("Not yet implemeneted")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
