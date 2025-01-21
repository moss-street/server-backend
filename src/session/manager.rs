use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub struct Session {
    token: String,
    user_id: i32,
    expire_time: DateTime<Utc>,
}

pub trait SessionManagerImpl {
    fn get_session(&mut self, user_id: i32) -> Option<&Session>;
    fn _new_session(&mut self, user_id: i32) -> Option<&Session>;
    fn generate_token(&self, user_id: i32) -> String;
    fn cleanup(&mut self);
}

#[derive(Debug)]
pub struct SessionManager {
    pub sessions: Vec<Session>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
        }
    }
}

impl SessionManagerImpl for SessionManager {
    fn generate_token(&self, user_id: i32) -> String {
        // Get the current timestamp
        let now: DateTime<Utc> = Utc::now();
        let timestamp = now.to_rfc3339();

        // Concatenate the user ID and timestamp
        let input = format!("{}:{}", user_id, timestamp);

        // Hash the input using SHA-256
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let hash = hasher.finalize();

        // Convert the hash to a hexadecimal string
        format!("{:x}", hash)
    }

    fn get_session(&mut self, user_id: i32) -> Option<&Session> {
        // Look for the damn thing
        if let Some(session_index) = self
            .sessions
            .iter()
            .position(|session| session.user_id == user_id)
        {
            let now = Utc::now();

            // Check if time is good if not, remove and make a new one
            if self.sessions[session_index].expire_time <= now {
                self.sessions.remove(session_index);
                self._new_session(user_id)
            } else {
                Some(&self.sessions[session_index])
            }
        } else {
            self._new_session(user_id)
        }
    }

    fn _new_session(&mut self, user_id: i32) -> Option<&Session> {
        let token: String = self.generate_token(user_id);
        let expire_time: DateTime<Utc> = Utc::now() + Duration::minutes(5);

        let session = Session {
            token,
            user_id,
            expire_time,
        };

        // Add the session to the list
        self.sessions.push(session);

        // Return a reference to the newly created session
        self.sessions.last()
    }

    fn cleanup(&mut self) {
        // Periodically call this I guess.
        let now = Utc::now();

        self.sessions.retain(|session| session.expire_time > now);
    }
}
