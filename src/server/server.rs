use rust_models::common;

use common::authorization_service_server::AuthorizationServiceServer;

use std::{net::SocketAddr, sync::Arc};

use crate::services::auth::AuthService;

use super::dependencies::ServerDependencies;

pub struct Server {
    addr: SocketAddr,
    dependencies: Arc<ServerDependencies>,
    pub server_handle: tokio::task::JoinHandle<()>,
}

impl Server {
    pub async fn new(addr: SocketAddr, dependencies: ServerDependencies) -> Self {
        let auth_service = AuthService::default();

        let service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(common::FILE_DESCRIPTOR_SET)
            .build_v1()
            .expect("Failed to create tonic reflecion");

        let handle = tokio::task::spawn({
            let addr = addr.clone();
            async move {
                tonic::transport::Server::builder()
                    .add_service(service)
                    .add_service(AuthorizationServiceServer::new(auth_service))
                    .serve(addr)
                    .await
                    .expect("Failed to create server!");
            }
        });

        let dependencies = Arc::new(dependencies);

        Server {
            addr,
            dependencies,
            server_handle: handle,
        }
    }
}
