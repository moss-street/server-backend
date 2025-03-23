use rust_models::common::{
    create_trade_response::CreateTradeStatus, trade_service_server::*, CreateTradeRequest,
    CreateTradeResponse, DeleteTradeRequest, DeleteTradeResponse, GetTradeRequest,
    GetTradeResponse, TradeId,
};
use tonic::Status;

use crate::{
    db::models::user::User,
    http::dependencies::ServerDependencies,
    trading::{
        backend::TradeBackend,
        market::{MarketProcessor, SwapPair},
    },
};

#[derive(Debug)]
pub struct TradeServiceImpl {
    _dependencies: ServerDependencies,
    trade_backend: TradeBackend,
}

impl TradeServiceImpl {
    pub fn new(dependencies: ServerDependencies, trade_backend: TradeBackend) -> Self {
        Self {
            _dependencies: dependencies,
            trade_backend,
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
                Status::not_found(format!("Market for: {:#} does not exist", &swap_pair))
            })?;

        market.process_trade(user.clone(), create_trade_request.clone());

        // package up user and swap pair and send it to the market for processing

        let response = CreateTradeResponse {
            status: CreateTradeStatus::Ok.into(),
            trade_id: Some(TradeId { trade_id: 1 }),
            trade_request: Some(create_trade_request),
        };

        Ok(tonic::Response::new(response))
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
