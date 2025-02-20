use crate::db::{
    manager::{DBManager, DatabaseImpl},
    models::user::User,
};
use anyhow::{anyhow, Result};
use derive_builder::Builder;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use diesel::prelude::{Insertable, Queryable, Selectable};

pub(crate) mod schema {
    diesel::table! {
        stock_wallet (user_id, stock_id) {
            user_id -> Integer,
            stock_id -> Integer,
            amount -> Double,
        }
    }
}

#[derive(Debug, Builder, Clone, Queryable, Selectable, Insertable)]
#[diesel(table_name = schema::stock_wallet)]
pub struct StockWallet {
    pub user_id: i32,
    pub stock_id: i32,
    pub amount: f64,
}

impl StockWallet {
    pub fn initialize_database(conn: &mut SqliteConnection) -> Result<()> {
        diesel::sql_query(
            r#"
        CREATE TABLE IF NOT EXISTS stock_wallet (
            user_id INTEGER NOT NULL,
            stock_id INTEGER NOT NULL,
            amount DOUBLE NOT NULL,
        );
        "#,
        )
        .execute(conn)
        .map_err(|e| anyhow!("Failed to create table: {e:#?}"))?;

        Ok(())
    }

    // TODO: fix this mert
    // fn resolve_owner(&self, db_manager: DBManager) -> Result<User> {
    //     let fields = vec![("user_id", &self.user_id), (seller_id)];
    //     let transaction = db_manager.query_row(super::user::schema::users, fields);
    //     db_manager.query_row(
    //         super::user::schema::users,
    //         ["order_id", transaction.order_id],
    //     )
    // }
}
