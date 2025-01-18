use rust_models::common;

use common::{
    authorization_service_server::AuthorizationService, UserCreateRequest, UserCreateResponse,
    UserDeleteRequest, UserDeleteResponse, UserGetRequest, UserGetResponse, UserLoginRequest,
    UserLoginResponse, UserUpdateRequest, UserUpdateResponse,
};

use tokio::time::Instant;
use tonic::Request;

use crate::{
    db::{manager::DatabaseImpl, schemas::user::UserBuilder},
    http::dependencies::ServerDependencies,
    passwords::Password,
};

#[derive(Debug)]
pub struct AuthService {
    server_deps: ServerDependencies,
}

impl AuthService {
    pub fn new(server_deps: ServerDependencies) -> Self {
        Self { server_deps }
    }
}

#[tonic::async_trait]
impl AuthorizationService for AuthService {
    async fn create_user(
        &self,
        request: Request<UserCreateRequest>,
    ) -> Result<tonic::Response<UserCreateResponse>, tonic::Status> {
        let request = request.get_ref();
        let password_hash = Password::new(request.password.as_str()).map_err(|_| {
            tonic::Status::invalid_argument(
                "Password provided was invalid, please try again".to_owned(),
            )
        })?;

        match UserBuilder::default()
            ._id(None)
            .email(request.email.clone())
            .password(password_hash)
            .first_name(request.first_name.clone())
            .last_name(request.last_name.clone())
            .created_at(Instant::now())
            .build()
        {
            Ok(user) => {
                let user_write_result = self
                    .server_deps
                    .db_manager
                    .write_to_table(&user)
                    .await
                    .map_err(|e| tonic::Status::internal(format!("Server Error: {e}")))?;
                Ok(tonic::Response::new(UserCreateResponse {
                    status: 1,
                    message: format!("{:#?}", user_write_result),
                }))
            }
            Err(e) => Ok(tonic::Response::new(UserCreateResponse {
                status: 0,
                message: format!("Failed to create user with error: {e:#}"),
            })),
        }
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
