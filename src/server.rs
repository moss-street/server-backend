mod common {
    tonic::include_proto!("authorization");
    tonic::include_proto!("common");

    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("moss-street");
}

use crate::db::DBManager;
use std::net::SocketAddr;

use common::{
    authorization_service_server::{AuthorizationService, AuthorizationServiceServer},
    UserCreateRequest, UserCreateResponse, UserDeleteRequest, UserDeleteResponse, UserGetRequest,
    UserGetResponse, UserLoginRequest, UserLoginResponse, UserUpdateRequest, UserUpdateResponse,
};
use std::sync::{Arc, RwLock};

use tonic::Request;

#[derive(Debug)]
struct AuthService {
    db_manager: Arc<RwLock<DBManager>>,
}

impl AuthService {
    fn new(db_manager: DBManager) -> Self {
        Self {
            db_manager: Arc::new(RwLock::new(db_manager)),
        }
    }
}

#[tonic::async_trait]
impl AuthorizationService for AuthService {
    async fn create_user(
        &self,
        _request: Request<UserCreateRequest>,
    ) -> Result<tonic::Response<UserCreateResponse>, tonic::Status> {
        let response = UserCreateResponse {
            status: 0,
            message: "response".into(),
        };

        Ok(tonic::Response::new(response))
    }

    async fn get_user(
        &self,
        _request: Request<UserGetRequest>,
    ) -> Result<tonic::Response<UserGetResponse>, tonic::Status> {
        todo!()
    }

    async fn update_user(
        &self,
        _request: Request<UserUpdateRequest>,
    ) -> Result<tonic::Response<UserUpdateResponse>, tonic::Status> {
        todo!()
    }

    async fn delete_user(
        &self,
        _request: Request<UserDeleteRequest>,
    ) -> Result<tonic::Response<UserDeleteResponse>, tonic::Status> {
        todo!()
    }

    async fn login_user(
        &self,
        _request: Request<UserLoginRequest>,
    ) -> Result<tonic::Response<UserLoginResponse>, tonic::Status> {
        todo!()
    }
}

pub struct Server {
    addr: SocketAddr,
    db_manager: DBManager,
    pub server_handle: tokio::task::JoinHandle<()>,
}

impl Server {
    pub async fn new(addr: SocketAddr, db_manager: DBManager) -> Self {
        let auth_service = AuthService::new(db_manager);

        let service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(common::FILE_DESCRIPTOR_SET)
            .build_v1()
            .expect("Failed to create tonic reflecion");

        let server = tokio::task::spawn({
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

        Server {
            addr,
            db_manager,
            server_handle: server,
        }
    }
}
