use rust_models::common::{
    authorization_service_server::AuthorizationService, CreateUserRequest, CreateUserResponse,
    LoginUserRequest, LoginUserResponse,
};

use tonic::Request;

use crate::{
    db::{
        manager::DatabaseImpl,
        models::user::{self, UserBuilder},
    },
    http::dependencies::ServerDependencies,
    passwords::Password,
    session::manager::SessionManagerImpl,
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
        request: Request<CreateUserRequest>,
    ) -> Result<tonic::Response<CreateUserResponse>, tonic::Status> {
        let request = request.get_ref();
        let password_hash = Password::new(request.password.as_str()).map_err(|_| {
            tonic::Status::invalid_argument(
                "Password provided was invalid, please try again".to_owned(),
            )
        })?;

        match UserBuilder::default()
            .id(None)
            .email(request.email.clone())
            .password(password_hash.hashed().to_owned())
            .first_name(request.first_name.clone())
            .last_name(request.last_name.clone())
            .build()
        {
            Ok(user) => {
                let user_write_result = self
                    .server_deps
                    .db_manager
                    .insert_row(user::schema::users::table, &user)
                    .map_err(|e| tonic::Status::internal(format!("Server Error: {e}")))?;
                Ok(tonic::Response::new(CreateUserResponse {
                    status: 1,
                    message: format!("{:#?}", user_write_result),
                }))
            }
            Err(e) => Ok(tonic::Response::new(CreateUserResponse {
                status: 0,
                message: format!("Failed to create user with error: {e:#}"),
            })),
        }
    }

    async fn login_user(
        &self,
        request: Request<LoginUserRequest>,
    ) -> Result<tonic::Response<LoginUserResponse>, tonic::Status> {
        let request = request.get_ref();

        let user: Vec<crate::db::models::user::User> = self
            .server_deps
            .db_manager
            .query_rows(user::schema::users::table, vec![("email", &request.email)])
            .await
            .map_err(|e| tonic::Status::internal(format!("Server Error: {e:#}")))?;

        if let Some(user) = user.first() {
            if !user.verify_password(&request.password).map_err(|e| {
                tonic::Status::invalid_argument(format!("Interal Error occured {e}"))
            })? {
                return Err(tonic::Status::invalid_argument(
                    "Invalid Password".to_owned(),
                ));
            }

            let mut proto_user = rust_models::common::User::from(user.clone());

            proto_user.token = Some(rust_models::common::Token::from(
                self.server_deps
                    .session_manager
                    .new_session(user.clone())
                    .ok_or_else(|| {
                        tonic::Status::not_found("Invalid token during generation".to_string())
                    })?,
            ));

            Ok(tonic::Response::new(LoginUserResponse {
                status: 1,
                user: Some(proto_user),
            }))
        } else {
            Err(tonic::Status::internal("No user found".to_string()))
        }
    }
}
