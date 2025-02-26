use bcrypt::{hash, DEFAULT_COST};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
#[allow(unused)]
pub struct Session {
    pub token: String,
    pub user_id: i32,
    pub expire_time: DateTime<Utc>,
    pub create_time: DateTime<Utc>,
}

pub trait SessionManagerImpl: Send + Sync {
    fn get_session(&self, user_id: i32) -> Option<Session>;
    fn new_session(&self, user_id: i32) -> Option<Session>;
    fn generate_token(&self, user_id: i32) -> String;
    fn cleanup(&self);
}

#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<i32, Session>>>,
}

impl SessionManagerImpl for SessionManager {
    fn generate_token(&self, user_id: i32) -> String {
        // Get the current timestamp
        let now: DateTime<Utc> = Utc::now();
        let timestamp = now.to_rfc3339();

        // Concatenate the user ID and timestamp
        let input = format!("{}:{}", user_id, timestamp);

        // Hash the input using bcrypt
        match hash(input, DEFAULT_COST) {
            Ok(hashed) => hashed,
            Err(e) => {
                eprintln!("Failed to generate bcrypt hash: {}", e);
                String::new() // Return an empty string on failure
            }
        }
    }

    fn get_session(&self, user_id: i32) -> Option<Session> {
        if let Some(session) = self.sessions.read().unwrap().get(&user_id) {
            let now = Utc::now();
            if session.expire_time <= now {
                let mut sessions = self.sessions.write().unwrap();
                sessions.remove(&user_id);
                return self.new_session(user_id);
            }
            return Some(session.clone()); // ✅ Clone to return owned Session
        }

        self.new_session(user_id)
    }

    fn new_session(&self, user_id: i32) -> Option<Session> {
        let token: String = self.generate_token(user_id);
        let create_time = Utc::now();
        let expire_time: DateTime<Utc> = create_time + Duration::minutes(5);

        let session = Session {
            token,
            user_id,
            expire_time,
            create_time,
        };

        let mut sessions = self.sessions.write().unwrap();

        sessions.insert(user_id, session);

        // Return a reference to the newly created session
        sessions.get(&user_id).cloned()
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
