use anyhow::{anyhow, Result};
use derive_builder::Builder;
use diesel::{sqlite::SqliteConnection, RunQueryDsl};
use prost_types::Timestamp;

use crate::passwords::Password;
use diesel::prelude::{Insertable, Queryable, Selectable};

pub(crate) mod schema {
    diesel::table! {
        users (id) {
            id -> Nullable<Integer>,
            email -> Text,
            password -> Text,
            first_name -> Text,
            last_name -> Text,
        }
    }
}

#[derive(Debug, Builder, Clone, Queryable, Selectable, Insertable, PartialEq, Eq)]
#[diesel(table_name = schema::users)]
pub struct User {
    // id is optinal because when we create a new item in the db, we don't actually set the id, we
    // let sqlite do that. We only set this field when we read from the db.
    pub id: Option<i32>,
    pub email: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
}

impl User {
    pub fn verify_password(&self, plaintext: &str) -> Result<bool, bcrypt::BcryptError> {
        Password::from_hash(&self.password).verify(plaintext)
    }

    pub fn initialize_database(conn: &mut SqliteConnection) -> Result<()> {
        diesel::sql_query(
            r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT NOT NULL UNIQUE,
            password TEXT NOT NULL,
            first_name TEXT NOT NULL,
            last_name TEXT NOT NULL
        );
        "#,
        )
        .execute(conn)
        .map_err(|e| anyhow!("Failed to create table: {e:#?}"))?;

        Ok(())
    }
}

impl From<User> for rust_models::common::User {
    fn from(val: User) -> Self {
        let creation_date: Option<Timestamp> = Some(Timestamp::default());
        rust_models::common::User {
            uuid: val.id.unwrap_or_default(),
            username: val.email,
            token: None,
            creation_date,
        }
    }
}
