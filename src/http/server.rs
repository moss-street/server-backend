use rust_models::common;
use std::sync::Arc;

use common::authorization_service_server::AuthorizationServiceServer;
use common::trade_service_server::TradeServiceServer;
use std::net::SocketAddr;
use tonic::{Request, Status};

use crate::{
    services::{auth::AuthService, trading::TradeServiceImpl},
    session::manager::{SessionManager, SessionManagerImpl, SessionToken},
    trading::{backend::TradeBackend, market::Market},
};

use super::dependencies::ServerDependencies;

use anyhow::Result;

pub struct Server {
    pub server_handle: tokio::task::JoinHandle<()>,
}

impl Server {
    pub async fn new(addr: SocketAddr, dependencies: ServerDependencies) -> Self {
        let mut trade_backend = TradeBackend::new();
        let btc_usd_market = Market::new("USD", "BTC");
        let eth_usd_market = Market::new("USD", "ETH");
        trade_backend.add_market(btc_usd_market);
        trade_backend.add_market(eth_usd_market);

        let auth_service = AuthService::new(dependencies.clone());
        let trade_service = TradeServiceImpl::new(dependencies.clone(), trade_backend);

        let service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(common::FILE_DESCRIPTOR_SET)
            .build_v1()
            .expect("Failed to create tonic reflecion");

        let session_manager = dependencies.session_manager;
        let auth_interceptor =
            { move |request: Request<()>| verify_auth(request, session_manager.clone()) };
        let auth_server = AuthorizationServiceServer::new(auth_service);
        let trade_server = TradeServiceServer::with_interceptor(trade_service, auth_interceptor);

        let handle = tokio::task::spawn({
            async move {
                tonic::transport::Server::builder()
                    .add_service(service)
                    .add_service(auth_server)
                    .add_service(trade_server)
                    .serve(addr)
                    .await
                    .expect("Failed to create server!");
            }
        });

        Server {
            server_handle: handle,
        }
    }
}

fn verify_auth(
    mut req: Request<()>,
    session_manager: Arc<SessionManager>,
) -> Result<Request<()>, Status> {
    let token = req
        .metadata()
        .get("Auth")
        .and_then(|md| md.to_str().ok())
        .ok_or_else(|| tonic::Status::unknown("Header is missing `Auth` field with token"))?;

    let session = session_manager
        .get_session(SessionToken::from(token.to_owned()))
        .ok_or_else(|| tonic::Status::not_found("Invalid token"))?;

    let user = session_manager
        .validate_session(session)
        .ok_or_else(|| tonic::Status::unauthenticated("Token expired"))?;

    req.extensions_mut().insert(user);
    Ok(req)
}
