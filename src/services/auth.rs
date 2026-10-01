use rust_models::common::{
    authorization_service_server::AuthorizationService, CreateUserRequest, CreateUserResponse,
    LoginUserRequest, LoginUserResponse,
};

use diesel::{prelude::*, OptionalExtension};
use tonic::Request;
use tracing::{debug, info, warn};

use crate::{
    db::models::user::{self, UserBuilder},
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
                let mut connection = self
                    .server_deps
                    .db_manager
                    .connection_pool
                    .get()
                    .map_err(|e| tonic::Status::internal(format!("Server Error: {e:#}")))?;
                let created_user = connection
                    .transaction::<_, anyhow::Error, _>(|connection| {
                        diesel::insert_into(user::schema::users::table)
                            .values(&user)
                            .execute(connection)?;
                        let created_user = user::schema::users::table
                            .filter(user::schema::users::email.eq(&user.email))
                            .first::<user::User>(connection)?;
                        Ok(created_user)
                    })
                    .map_err(|err| {
                        tonic::Status::internal(format!("Failed to create user accounts: {err:#}"))
                    })?;
                let user_id = created_user
                    .id
                    .ok_or_else(|| tonic::Status::internal("Created user id is missing"))?;
                info!(user_id, "Created user");
                Ok(tonic::Response::new(CreateUserResponse {
                    status: rust_models::common::create_user_response::Status::Ok.into(),
                    message: "User created".to_string(),
                }))
            }
            Err(e) => Ok(tonic::Response::new(CreateUserResponse {
                status: rust_models::common::create_user_response::Status::Error.into(),
                message: format!("Failed to create user with error: {e:#}"),
            })),
        }
    }

    async fn login_user(
        &self,
        request: Request<LoginUserRequest>,
    ) -> Result<tonic::Response<LoginUserResponse>, tonic::Status> {
        let request = request.get_ref();

        let mut connection = self
            .server_deps
            .db_manager
            .connection_pool
            .get()
            .map_err(|e| tonic::Status::internal(format!("Server Error: {e:#}")))?;
        let user = user::schema::users::table
            .filter(user::schema::users::email.eq(&request.email))
            .first::<user::User>(&mut connection)
            .optional()
            .map_err(|e| tonic::Status::internal(format!("Server Error: {e:#}")))?;

        if let Some(user) = user {
            if !user.verify_password(&request.password).map_err(|e| {
                tonic::Status::invalid_argument(format!("Interal Error occured {e}"))
            })? {
                warn!("Rejected login with invalid password");
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
            debug!(user_id = user.id, "User login succeeded");

            Ok(tonic::Response::new(LoginUserResponse {
                status: rust_models::common::login_user_response::Status::Ok.into(),
                user: Some(proto_user),
            }))
        } else {
            warn!("Rejected login for unknown user");
            Err(tonic::Status::internal("No user found".to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use diesel::{r2d2::ConnectionManager, sqlite::SqliteConnection};
    use rust_models::common::{
        create_user_response::Status as CreateUserStatus,
        login_user_response::Status as LoginUserStatus, CreateUserRequest, LoginUserRequest,
    };

    use super::*;
    use crate::{db::manager::DBManager, session::manager::SessionManager};

    fn service() -> AuthService {
        let manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = diesel::r2d2::Pool::builder()
            .max_size(1)
            .build(manager)
            .expect("sqlite pool should build for tests");
        let db_manager = Arc::new(DBManager::new(pool));
        let mut connection = db_manager
            .connection_pool
            .get()
            .expect("sqlite connection should be available");
        user::User::initialize_database(&mut connection).expect("user schema should initialize");
        AuthService::new(ServerDependencies::new(
            db_manager,
            Arc::new(SessionManager::default()),
        ))
    }

    #[tokio::test]
    async fn create_and_login_return_ok_statuses() {
        let service = service();
        let create = service
            .create_user(Request::new(CreateUserRequest {
                email: "trader@example.com".to_string(),
                password: "correct-horse-battery-staple".to_string(),
                first_name: "Trade".to_string(),
                last_name: "Tester".to_string(),
            }))
            .await
            .expect("user should be created")
            .into_inner();
        assert_eq!(create.status, CreateUserStatus::Ok as i32);

        let login = service
            .login_user(Request::new(LoginUserRequest {
                email: "trader@example.com".to_string(),
                password: "correct-horse-battery-staple".to_string(),
            }))
            .await
            .expect("user should be able to log in")
            .into_inner();
        assert_eq!(login.status, LoginUserStatus::Ok as i32);
        assert!(login.user.and_then(|user| user.token).is_some());
    }
}
