use bcrypt::{hash, DEFAULT_COST};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use prost_types::Timestamp;

use crate::db::models::user::User;

#[derive(Debug, Clone)]
#[allow(unused)]
pub struct Session {
    pub token: SessionToken,
    pub expire_time: DateTime<Utc>,
    pub create_time: DateTime<Utc>,
    pub user: User,
}

impl Session {
    fn is_valid(&self) -> bool {
        let now = Utc::now();
        now < self.expire_time
    }
}

impl From<Session> for rust_models::common::Token {
    fn from(val: Session) -> Self {
        let expire_ts: Option<Timestamp> = Some(Timestamp {
            seconds: val.expire_time.timestamp(),
            nanos: val.expire_time.timestamp_subsec_nanos() as i32,
        });
        let create_ts: Option<Timestamp> = Some(Timestamp {
            seconds: val.create_time.timestamp(),
            nanos: val.create_time.timestamp_subsec_nanos() as i32,
        });
        rust_models::common::Token {
            create_ts,
            expire_ts,
            token: val.token.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionToken(String);

impl SessionToken {
    fn new(user_id: i32) -> Self {
        // Get the current timestamp
        let now: DateTime<Utc> = Utc::now();
        let timestamp = now.to_rfc3339();

        // Concatenate the user ID and timestamp
        let input = format!("{}:{}", user_id, timestamp);

        // Hash the input using bcrypt
        match hash(input, DEFAULT_COST) {
            Ok(hashed) => Self(hashed),
            Err(e) => {
                eprintln!("Failed to generate bcrypt hash: {}", e);
                Self(String::new()) // Return an empty string on failure
            }
        }
    }
}

pub trait SessionManagerImpl: Send + Sync {
    fn new_session(&self, user: User) -> Option<Session>;
    fn get_session(&self, token: impl Into<SessionToken>) -> Option<Session>;
    fn validate_session(&self, session: Session) -> Option<User>;
    fn cleanup(&self);
}

#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<SessionToken, Session>>>,
}

impl SessionManagerImpl for SessionManager {
    fn get_session(&self, token: impl Into<SessionToken>) -> Option<Session> {
        self.sessions.read().unwrap().get(&token.into()).cloned()
    }

    fn validate_session(&self, session: Session) -> Option<User> {
        match session.is_valid().then_some(session.user) {
            Some(user) => Some(user),
            None => {
                let mut guard = self.sessions.write().unwrap();
                guard.remove(&session.token);
                None
            }
        }
    }

    fn new_session(&self, user: User) -> Option<Session> {
        let token = SessionToken::new(user.id?);
        let create_time = Utc::now();
        let expire_time: DateTime<Utc> = create_time + Duration::seconds(30);
        let session = Session {
            token: token.clone(),
            user,
            expire_time,
            create_time,
        };

        let mut sessions = self.sessions.write().unwrap();
        sessions.insert(token.clone(), session.clone());
        Some(session)
    }

    fn cleanup(&self) {
        // Periodically call this I guess.
        let now = Utc::now();

        self.sessions
            .write()
            .unwrap()
            .retain(|_, session| session.expire_time > now);
    }
}
