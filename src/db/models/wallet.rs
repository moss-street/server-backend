use anyhow::{anyhow, Result};

use derive_builder::Builder;
use diesel::{
    prelude::{Insertable, Queryable, Selectable},
    RunQueryDsl, SqliteConnection,
};

pub(crate) mod schema {
    diesel::table! {
        wallets (id) {
            id -> Nullable<Integer>,
            stock_id -> Integer,
            user_id -> Integer,
            balance -> Double,
        }
    }
}

#[derive(Debug, Builder, Clone, Queryable, Selectable, Insertable, PartialEq)]
#[diesel(table_name = schema::wallets)]
pub struct Wallet {
    id: Option<i32>,
    // the id of the stock that this wallet is set for
    stock_id: i32,
    // the owner id of the person this wallet belongs to
    user_id: i32,
    // amount of money in the wallet
    balance: f64,
}

impl Wallet {
    pub fn initialize_database(conn: &mut SqliteConnection) -> Result<()> {
        diesel::sql_query(
            r#"
        CREATE TABLE IF NOT EXISTS wallets (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        stock_id INTEGER NOT NULL,
        user_id INTEGER NOT NULL,
        balance DOUBLE NOT NULL
        );
        "#,
        )
        .execute(conn)
        .map_err(|e| anyhow!("Failed to create table: {e:#?}"))?;

        Ok(())
    }
}
