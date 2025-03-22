use std::sync::Arc;

use crate::db::manager::DBManager;
use crate::session::manager::SessionManager;

#[derive(Debug, Clone)]
pub struct ServerDependencies {
    pub db_manager: Arc<DBManager>,
    pub session_manager: Arc<SessionManager>,
}

impl ServerDependencies {
    pub fn new(db_manager: Arc<DBManager>, session_manager: Arc<SessionManager>) -> Self {
        Self {
            db_manager,
            session_manager,
        }
    }
}
