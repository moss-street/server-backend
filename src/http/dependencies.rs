use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;

pub struct ServerDependencies {
    db_pool: Pool<SqliteConnectionManager>,
}

impl ServerDependencies {
    pub fn new(db_pool: Pool<SqliteConnectionManager>) -> Self {
        Self { db_pool }
    }
    pub fn get_connection(&self) -> Option<PooledConnection<SqliteConnectionManager>> {
        self.db_pool.try_get()
    }
}
