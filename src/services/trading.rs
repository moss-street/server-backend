use rust_models::common::{
    create_trade_response::CreateTradeStatus, trade_service_server::*, CreateTradeRequest,
    CreateTradeResponse, DeleteTradeRequest, DeleteTradeResponse, GetTradeRequest,
    GetTradeResponse, TradeId,
};
use tonic::Status;

use crate::{
    http::dependencies::ServerDependencies, session::manager::Session,
    trading::backend::TradeBackend,
};

#[derive(Debug)]
pub struct TradeServiceImpl {
    _dependencies: ServerDependencies,
    _trade_backend: TradeBackend,
}

impl TradeServiceImpl {
    pub fn new(dependencies: ServerDependencies) -> Self {
        let trade_backend = TradeBackend::new(&dependencies.db_manager);
        Self {
            _dependencies: dependencies,
            _trade_backend: trade_backend,
        }
    }
}

#[tonic::async_trait]
impl TradeService for TradeServiceImpl {
    async fn create_trade(
        &self,
        request: tonic::Request<CreateTradeRequest>,
    ) -> Result<tonic::Response<CreateTradeResponse>, tonic::Status> {
        let _session = request
            .extensions()
            .get::<Session>()
            .ok_or_else(|| Status::unauthenticated("Session not found"))?;

        let create_trade_request = request.into_inner().trade_request;
        let response = CreateTradeResponse {
            status: CreateTradeStatus::Ok.into(),
            trade_id: Some(TradeId { trade_id: 1 }),
            trade_request: create_trade_request,
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
