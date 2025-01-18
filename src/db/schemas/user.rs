use derive_builder::Builder;
use rusqlite::Row;
use tokio::time::Instant;

use crate::{db::manager::TableImpl, passwords::Password};

#[derive(Debug, Builder)]
pub struct User {
    // id is optinal because when we create a new item in the db, we don't actually set the id, we
    // let sqlite do that. We only set this field when we read from the db.
    _id: Option<i32>,
    email: String,
    password: Password,
    first_name: String,
    last_name: String,
    created_at: Instant,
}

impl TableImpl for User {
    fn create_table_query() -> String {
        String::from(
            r#"CREATE TABLE IF NOT EXISTS User (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email TEXT NOT NULL UNIQUE,
    password TEXT NOT NULL,
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    created_at TEXT NOT NULL
);"#,
        )
    }

    fn generate_db_load_query(&self) -> String {
        format!(
            "INSERT INTO User (email, password, first_name, last_name, created_at) VALUES ('{}', '{}', '{}', '{}', '{:#?}')",
            self.email.replace('\'', "''"), // Escape single quotes
            self.password.hashed(),
            self.first_name,
            self.last_name,
            self.created_at
        )
    }

    fn generate_db_lookup_query(_id: i32) -> String {
        todo!()
    }

    fn deserialize_query_result(_result: &Row) -> Result<Self, rusqlite::Error> {
        todo!()
    }
}
