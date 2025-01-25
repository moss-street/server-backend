use derive_builder::Builder;
use prost_types::Timestamp;
use rusqlite::Row;
use tokio::time::Instant;

use crate::{db::manager::TableImpl, passwords::Password};

#[derive(Debug, Builder)]
pub struct User {
    // id is optinal because when we create a new item in the db, we don't actually set the id, we
    // let sqlite do that. We only set this field when we read from the db.
    pub id: Option<u64>,
    pub email: String,
    password: Password,
    pub first_name: String,
    pub last_name: String,
    pub created_at: Instant,
}

impl User {
    pub fn verify_password(&self, plaintext: &str) -> Result<bool, bcrypt::BcryptError> {
        self.password.verify(plaintext)
    }
}

impl From<User> for rust_models::common::User {
    fn from(val: User) -> Self {
        rust_models::common::User {
            uuid: val.id.unwrap_or_default(),
            username: val.email,
            token: None,
            creation_date: Some(Timestamp::default()),
        }
    }
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

    fn deserialize_query_result(result: &Row) -> Result<Self, rusqlite::Error> {
        println!("result was {result:#?}");
        let password: String = result.get("password")?;
        let password = Password::from_hash(&password);

        Ok(User {
            id: Some(result.get("id")?),
            email: result.get("email")?,
            first_name: result.get("first_name")?,
            last_name: result.get("last_name")?,
            password,
            created_at: Instant::now(),
        })
    }
}
