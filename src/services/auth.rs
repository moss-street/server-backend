use rust_models::common;

use common::{
    authorization_service_server::AuthorizationService, UserCreateRequest, UserCreateResponse,
    UserDeleteRequest, UserDeleteResponse, UserGetRequest, UserGetResponse, UserLoginRequest,
    UserLoginResponse, UserUpdateRequest, UserUpdateResponse,
};

use tonic::Request;

#[derive(Default)]
pub struct AuthService;

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
