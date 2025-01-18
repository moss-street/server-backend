use std::sync::Arc;

use crate::db::manager::DBManager;

#[derive(Debug)]
pub struct ServerDependencies {
    pub db_manager: Arc<DBManager>,
}

impl ServerDependencies {
    pub fn new(db_manager: Arc<DBManager>) -> Self {
        Self { db_manager }
    }
}
