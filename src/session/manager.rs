use bcrypt::hash;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use prost_types::Timestamp;

use crate::db::models::user::User;

const DEFAULT_TOKEN_TIMEOUT_DURATION: Duration = Duration::seconds(30);

#[derive(Debug, Clone)]
#[allow(unused)]
pub struct Session {
    pub token: SessionToken,
    pub expire_time: DateTime<Utc>,
    pub create_time: DateTime<Utc>,
    pub user: User,
}

impl Session {
    fn new(user: User, user_id: i32, expire_duration: Duration) -> Self {
        let token = SessionToken::new(user_id);
        let create_time = get_time();
        let expire_time = create_time + expire_duration;
        Self {
            token,
            user: user.clone(),
            expire_time,
            create_time,
        }
    }

    fn is_valid(&self) -> bool {
        let now = get_time();
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
        let now = get_time();
        let timestamp = now.to_rfc3339();

        // Concatenate the user ID and timestamp
        let input = format!("{}:{}", user_id, timestamp);

        // Hash the input using bcrypt
        // 4 is the lowest cost of the hash function making this faster
        // Since we're generating a sesion token it's ok for this to be
        // not as fast as a password hash. Changing this value to default cost
        // can take up to 600ms instead of 5ms.
        match hash(input, 4) {
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
        let session = Session::new(user.clone(), user.id?, DEFAULT_TOKEN_TIMEOUT_DURATION);

        let mut sessions = self.sessions.write().unwrap();
        sessions.insert(session.token.clone(), session.clone());
        Some(session)
    }

    fn cleanup(&self) {
        self.sessions
            .write()
            .unwrap()
            .retain(|_, session| session.is_valid());
    }
}

/// Returns a time based on Utc now, this provides a special way to return the time when testing is
/// enabled to accelerate time for Utc. Since Utc is not compatible with tokio::Instant this is
/// necessary to mock the chrono timestamps. If testing is enabled then this will always return the
/// same time
fn get_time() -> DateTime<Utc> {
    #[cfg(not(test))]
    {
        Utc::now()
    }
    #[cfg(test)]
    {
        use chrono::TimeZone;
        Utc.timestamp_opt(test_utils::get_mock_time(), 0).unwrap()
    }
}

#[cfg(test)]
mod test_utils {
    use std::sync::atomic::{AtomicI64, Ordering};
    // Atomic clock mock
    static MOCK_TIME: AtomicI64 = AtomicI64::new(0);

    pub fn set_mock_time(seconds: i64) {
        MOCK_TIME.store(seconds, Ordering::SeqCst);
    }

    pub fn get_mock_time() -> i64 {
        MOCK_TIME.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn fake_user() -> User {
        User {
            id: Some(123),
            email: "abd".to_owned(),
            password: "123".to_owned(),
            first_name: "bob".to_owned(),
            last_name: "bob".to_owned(),
        }
    }

    #[test]
    fn test_session_is_valid() {
        let user = fake_user();

        let expire_duration = Duration::seconds(1);
        let session = Session::new(user, 123, expire_duration);

        assert!(session.is_valid());

        test_utils::set_mock_time(test_utils::get_mock_time() + 1); // Fast-forward time
        assert!(!session.is_valid());
    }

    #[test]
    fn test_new_session() {
        let manager = SessionManager::default();
        let user = fake_user();

        let session = manager
            .new_session(user.clone())
            .expect("Session should be created");
        assert_eq!(session.user, user);
        assert!(session.is_valid());
    }

    #[test]
    fn test_get_session() {
        let manager = SessionManager::default();
        let user = fake_user();
        let session = manager
            .new_session(user.clone())
            .expect("Session should be created");

        let retrieved = manager
            .get_session(session.token.clone())
            .expect("Session should exist");
        assert_eq!(retrieved.user, user);
    }

    #[test]
    fn test_validate_session() {
        let manager = SessionManager::default();
        let user = fake_user();
        let session = manager
            .new_session(user.clone())
            .expect("Session should be created");

        let validated_user = manager
            .validate_session(session.clone())
            .expect("Session should be valid");
        assert_eq!(validated_user, user);
    }

    #[test]
    fn test_expired_session_cleanup() {
        let manager = SessionManager::default();
        let user = fake_user();
        test_utils::set_mock_time(Utc::now().timestamp());
        let session = manager
            .new_session(user.clone())
            .expect("Session should be created");

        test_utils::set_mock_time(test_utils::get_mock_time() + 35); // Fast-forward time
        manager.cleanup();

        assert!(
            manager.get_session(session.token).is_none(),
            "Expired session should be removed"
        );
    }
}
