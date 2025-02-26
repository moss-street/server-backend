use anyhow::{anyhow, Result};
use derive_builder::Builder;
use diesel::prelude::*;
use diesel::prelude::{Insertable, Queryable, Selectable};
use diesel::sqlite::SqliteConnection;

pub(crate) mod schema {
    diesel::table! {
        stock (id) {
            id -> Nullable<Integer>,
            name -> Text,
            symbol -> Text,
            exchange_name -> Text,
        }
    }
}

#[derive(Debug, Builder, Clone, Queryable, Selectable, Insertable)]
#[diesel(table_name = schema::stock)]
pub struct Stock {
    // id is optinal because when we create a new item in the db, we don't actually set the id, we
    // let sqlite do that. We only set this field when we read from the db.
    pub id: Option<i32>,
    pub name: String,
    pub symbol: String,
    pub exchange_name: String,
}

impl Stock {
    pub fn initialize_database(conn: &mut SqliteConnection) -> Result<()> {
        diesel::sql_query(
            r#"
            CREATE TABLE IF NOT EXISTS stock (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            symbol TEXT NOT NULL,
            exchange_name TEXT NOT NULL,
        );
        "#,
        )
        .execute(conn)
        .map_err(|e| anyhow!("Failed to create table: {e:#?}"))?;

        Ok(())
    }
}

// impl From<Stock> for rust_models::common::Stock {
//     fn from(val: Stock) -> Self {
//         let creation_date: Option<Timestamp> = Some(Timestamp::default());
//         rust_models::common::User {
//             uuid: val.id.unwrap_or_default(),
//             username: val.email,
//             token: None,
//             creation_date,
//         }
//     }
// }
