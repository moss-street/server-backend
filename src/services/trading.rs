use rust_models::common::{
    trade_service_server::*, CreateTradeRequest, CreateTradeResponse, DeleteTradeRequest,
    DeleteTradeResponse, GetTradeRequest, GetTradeResponse,
};

#[derive(Debug)]
pub struct TradeServiceImpl;

#[tonic::async_trait]
impl TradeService for TradeServiceImpl {
    async fn create_trade(
        &self,
        _request: tonic::Request<CreateTradeRequest>,
    ) -> Result<tonic::Response<CreateTradeResponse>, tonic::Status> {
        unimplemented!("Not yet implemeneted")
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
