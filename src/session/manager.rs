use bcrypt::{hash, DEFAULT_COST};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Session {
    token: String,
    user_id: i32,
    expire_time: DateTime<Utc>,
}

pub trait SessionManagerImpl {
    fn get_session(&mut self, user_id: i32) -> Option<&Session>;
    fn new_session(&mut self, user_id: i32) -> Option<&Session>;
    fn generate_token(&self, user_id: i32) -> String;
    fn cleanup(&mut self);
}

#[derive(Debug)]
pub struct SessionManager {
    pub sessions: HashMap<i32, Session>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
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

        // Hash the input using bcrypt
        match hash(input, DEFAULT_COST) {
            Ok(hashed) => hashed,
            Err(e) => {
                eprintln!("Failed to generate bcrypt hash: {}", e);
                String::new() // Return an empty string on failure
            }
        }
    }

    fn get_session(&mut self, user_id: i32) -> Option<&Session> {
        // Look for the damn thing
        if let Some(session) = self.sessions.get(&user_id) {
            let now = Utc::now();

            // Check if time is good if not, remove and make a new one
            if session.expire_time <= now {
                self.sessions.remove(&user_id);
                self.new_session(user_id)
            } else {
                //I can't just say Some(session) here because rust goes apeshit when I
                // Try to return an immutable instance after borrowing it mutably
                self.sessions.get(&user_id)
            }
        } else {
            self.new_session(user_id)
        }
    }

    fn new_session(&mut self, user_id: i32) -> Option<&Session> {
        let token: String = self.generate_token(user_id);
        let expire_time: DateTime<Utc> = Utc::now() + Duration::minutes(5);

        let session = Session {
            token,
            user_id,
            expire_time,
        };

        self.sessions.insert(user_id, session);

        // Return a reference to the newly created session
        self.sessions.get(&user_id)
    }

    fn cleanup(&mut self) {
        // Periodically call this I guess.
        let now = Utc::now();

        self.sessions.retain(|_, session| session.expire_time > now);
    }
}
