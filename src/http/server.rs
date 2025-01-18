use rust_models::common;

use common::authorization_service_server::AuthorizationServiceServer;

use std::net::SocketAddr;

use crate::{
    db::{
        manager::DatabaseImpl,
        schemas::{stock::Stock, user::User},
    },
    services::auth::AuthService,
};

use super::dependencies::ServerDependencies;

pub struct Server {
    pub server_handle: tokio::task::JoinHandle<()>,
}

impl Server {
    pub async fn new(addr: SocketAddr, dependencies: ServerDependencies) -> Self {
        dependencies
            .db_manager
            .create_table::<User>()
            .await
            .expect("Error creating stock table");
        dependencies
            .db_manager
            .create_table::<Stock>()
            .await
            .expect("Error creating stock table");

        let auth_service = AuthService::new(dependencies);

        let service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(common::FILE_DESCRIPTOR_SET)
            .build_v1()
            .expect("Failed to create tonic reflecion");

        let handle = tokio::task::spawn({
            async move {
                tonic::transport::Server::builder()
                    .add_service(service)
                    .add_service(AuthorizationServiceServer::new(auth_service))
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
