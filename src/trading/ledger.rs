#![allow(clippy::redundant_field_names)]

use std::sync::Arc;

use anyhow::{anyhow, Result};
use diesel::{
    connection::Connection,
    sql_query,
    sql_types::{Double, Integer, Text},
    sqlite::SqliteConnection,
    OptionalExtension, QueryableByName, RunQueryDsl,
};

use crate::{db::manager::DBManager, trading::trade_engine::Fill};

const DEFAULT_ASSETS: [&str; 3] = ["USD", "BTC", "ETH"];
const DEFAULT_BALANCE: f64 = 50.0;
const BALANCE_EPSILON: f64 = 1e-9;

#[allow(dead_code)]
#[derive(Debug)]
pub struct AssetLedger {
    db_manager: Arc<DBManager>,
}

#[derive(Debug, QueryableByName)]
struct OrderReservation {
    #[diesel(sql_type = Integer)]
    user_id: i32,
    #[diesel(sql_type = Text)]
    source_symbol: String,
    #[diesel(sql_type = Text)]
    destination_symbol: String,
    #[diesel(sql_type = Double)]
    remaining_source: f64,
}

#[allow(dead_code)]
#[derive(Debug, QueryableByName)]
struct AssetBalance {
    #[diesel(sql_type = Double)]
    available: f64,
    #[diesel(sql_type = Double)]
    reserved: f64,
}

#[allow(dead_code)]
impl AssetLedger {
    pub fn new(db_manager: Arc<DBManager>) -> Self {
        Self { db_manager }
    }

    pub fn initialize_database(connection: &mut SqliteConnection) -> Result<()> {
        diesel::sql_query(
            "CREATE TABLE IF NOT EXISTS asset_balances (
                user_id INTEGER NOT NULL,
                asset_symbol TEXT NOT NULL,
                available DOUBLE NOT NULL,
                reserved DOUBLE NOT NULL DEFAULT 0,
                PRIMARY KEY (user_id, asset_symbol)
            )",
        )
        .execute(connection)?;
        diesel::sql_query(
            "CREATE TABLE IF NOT EXISTS order_reservations (
                order_id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                source_symbol TEXT NOT NULL,
                destination_symbol TEXT NOT NULL,
                remaining_source DOUBLE NOT NULL,
                active INTEGER NOT NULL DEFAULT 1
            )",
        )
        .execute(connection)?;
        Ok(())
    }

    pub fn ensure_default_accounts(&self, user_id: i32) -> Result<()> {
        let mut connection = self.db_manager.connection_pool.get()?;
        connection.transaction::<_, anyhow::Error, _>(|connection| {
            for asset in DEFAULT_ASSETS {
                sql_query(
                    "INSERT OR IGNORE INTO asset_balances \
                     (user_id, asset_symbol, available, reserved) VALUES (?, ?, ?, 0)",
                )
                .bind::<Integer, _>(user_id)
                .bind::<Text, _>(asset)
                .bind::<Double, _>(DEFAULT_BALANCE)
                .execute(connection)?;
            }
            Ok(())
        })
    }

    pub fn reserve_order(
        &self,
        order_id: i32,
        user_id: i32,
        source_symbol: &str,
        destination_symbol: &str,
        source_quantity: f64,
    ) -> Result<()> {
        let mut connection = self.db_manager.connection_pool.get()?;
        connection.transaction::<_, anyhow::Error, _>(|connection| {
            let updated = sql_query(
                "UPDATE asset_balances \
                 SET available = available - ?, reserved = reserved + ? \
                 WHERE user_id = ? AND asset_symbol = ? AND available >= ?",
            )
            .bind::<Double, _>(source_quantity)
            .bind::<Double, _>(source_quantity)
            .bind::<Integer, _>(user_id)
            .bind::<Text, _>(source_symbol)
            .bind::<Double, _>(source_quantity)
            .execute(connection)?;

            if updated != 1 {
                return Err(anyhow!(
                    "Missing source account or insufficient available balance"
                ));
            }

            sql_query(
                "INSERT INTO order_reservations \
                 (order_id, user_id, source_symbol, destination_symbol, remaining_source, active) \
                 VALUES (?, ?, ?, ?, ?, 1)",
            )
            .bind::<Integer, _>(order_id)
            .bind::<Integer, _>(user_id)
            .bind::<Text, _>(source_symbol)
            .bind::<Text, _>(destination_symbol)
            .bind::<Double, _>(source_quantity)
            .execute(connection)?;
            Ok(())
        })
    }

    pub fn settle_fills(&self, fills: &[Fill]) -> Result<()> {
        let mut connection = self.db_manager.connection_pool.get()?;
        connection.transaction::<_, anyhow::Error, _>(|connection| {
            for fill in fills {
                settle_order(
                    connection,
                    fill.maker_order_id,
                    fill.maker_source_quantity,
                    fill.taker_source_quantity,
                )?;
                settle_order(
                    connection,
                    fill.taker_order_id,
                    fill.taker_source_quantity,
                    fill.maker_source_quantity,
                )?;
            }
            Ok(())
        })
    }

    pub fn refund_cancelled_order(&self, order_id: i32, user_id: i32) -> Result<Option<f64>> {
        let mut connection = self.db_manager.connection_pool.get()?;
        connection.transaction::<_, anyhow::Error, _>(|connection| {
            let reservation = sql_query(
                "SELECT user_id, source_symbol, destination_symbol, remaining_source \
                 FROM order_reservations WHERE order_id = ? AND active = 1",
            )
            .bind::<Integer, _>(order_id)
            .get_result::<OrderReservation>(connection)
            .optional()?;
            let Some(reservation) = reservation else {
                return Ok(None);
            };
            if reservation.user_id != user_id {
                return Ok(None);
            }

            let refunded = reservation.remaining_source;
            sql_query(
                "UPDATE asset_balances \
                 SET available = available + ?, reserved = MAX(reserved - ?, 0) \
                 WHERE user_id = ? AND asset_symbol = ?",
            )
            .bind::<Double, _>(refunded)
            .bind::<Double, _>(refunded)
            .bind::<Integer, _>(user_id)
            .bind::<Text, _>(&reservation.source_symbol)
            .execute(connection)?;
            sql_query(
                "UPDATE order_reservations SET remaining_source = 0, active = 0 WHERE order_id = ?",
            )
            .bind::<Integer, _>(order_id)
            .execute(connection)?;
            Ok(Some(refunded))
        })
    }

    pub fn balance(&self, user_id: i32, asset_symbol: &str) -> Result<Option<(f64, f64)>> {
        let mut connection = self.db_manager.connection_pool.get()?;
        let balance = sql_query(
            "SELECT available, reserved FROM asset_balances \
             WHERE user_id = ? AND asset_symbol = ?",
        )
        .bind::<Integer, _>(user_id)
        .bind::<Text, _>(asset_symbol)
        .get_result::<AssetBalance>(&mut *connection)
        .optional()?;
        Ok(balance.map(|balance| (balance.available, balance.reserved)))
    }
}

fn settle_order(
    connection: &mut SqliteConnection,
    order_id: u64,
    source_spent: f64,
    destination_received: f64,
) -> Result<()> {
    let order_id = i32::try_from(order_id).map_err(|_| anyhow!("Order id is out of range"))?;
    let reservation = sql_query(
        "SELECT user_id, source_symbol, destination_symbol, remaining_source \
         FROM order_reservations WHERE order_id = ? AND active = 1",
    )
    .bind::<Integer, _>(order_id)
    .get_result::<OrderReservation>(connection)?;

    if source_spent > reservation.remaining_source + BALANCE_EPSILON {
        return Err(anyhow!("Fill exceeds the order's reserved source amount"));
    }

    let balance_updated = sql_query(
        "UPDATE asset_balances SET reserved = MAX(reserved - ?, 0) \
         WHERE user_id = ? AND asset_symbol = ? AND reserved + ? >= ?",
    )
    .bind::<Double, _>(source_spent)
    .bind::<Integer, _>(reservation.user_id)
    .bind::<Text, _>(&reservation.source_symbol)
    .bind::<Double, _>(BALANCE_EPSILON)
    .bind::<Double, _>(source_spent)
    .execute(&mut *connection)?;
    if balance_updated != 1 {
        return Err(anyhow!("Reserved source balance does not cover fill"));
    }

    let credited = sql_query(
        "UPDATE asset_balances SET available = available + ? \
         WHERE user_id = ? AND asset_symbol = ?",
    )
    .bind::<Double, _>(destination_received)
    .bind::<Integer, _>(reservation.user_id)
    .bind::<Text, _>(&reservation.destination_symbol)
    .execute(&mut *connection)?;
    if credited != 1 {
        return Err(anyhow!("Destination asset account is missing"));
    }

    let remaining_source = (reservation.remaining_source - source_spent).max(0.0);
    sql_query("UPDATE order_reservations SET remaining_source = ?, active = ? WHERE order_id = ?")
        .bind::<Double, _>(remaining_source)
        .bind::<Integer, _>(i32::from(remaining_source > BALANCE_EPSILON))
        .bind::<Integer, _>(order_id)
        .execute(&mut *connection)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::r2d2::{ConnectionManager, Pool};

    fn build_ledger() -> AssetLedger {
        let manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(manager)
            .expect("sqlite pool should build for tests");
        let db_manager = Arc::new(DBManager::new(pool));
        let mut connection = db_manager
            .connection_pool
            .get()
            .expect("sqlite connection should be available");
        AssetLedger::initialize_database(&mut connection).expect("ledger schema should initialize");
        drop(connection);

        let ledger = AssetLedger::new(db_manager);
        ledger
            .ensure_default_accounts(1)
            .expect("user one accounts should be initialized");
        ledger
            .ensure_default_accounts(2)
            .expect("user two accounts should be initialized");
        ledger
    }

    #[test]
    fn reserve_and_cancel_refunds_the_full_unfilled_amount() {
        let ledger = build_ledger();
        ledger
            .reserve_order(1, 1, "USD", "BTC", 12.0)
            .expect("order amount should be reserved");

        assert_eq!(ledger.balance(1, "USD").unwrap(), Some((38.0, 12.0)));
        assert_eq!(ledger.refund_cancelled_order(1, 1).unwrap(), Some(12.0));
        assert_eq!(ledger.balance(1, "USD").unwrap(), Some((50.0, 0.0)));
    }

    #[test]
    fn partial_fill_then_cancel_refunds_only_the_remaining_reservation() {
        let ledger = build_ledger();
        ledger
            .reserve_order(11, 1, "USD", "BTC", 10.0)
            .expect("maker funds should be reserved");
        ledger
            .reserve_order(12, 2, "BTC", "USD", 4.0)
            .expect("taker funds should be reserved");

        ledger
            .settle_fills(&[Fill {
                price: 2.0,
                quantity: 2.0,
                maker_order_id: 11,
                taker_order_id: 12,
                maker_source_quantity: 2.0,
                taker_source_quantity: 4.0,
            }])
            .expect("matched amounts should settle atomically");

        assert_eq!(ledger.balance(1, "USD").unwrap(), Some((40.0, 8.0)));
        assert_eq!(ledger.balance(1, "BTC").unwrap(), Some((54.0, 0.0)));
        assert_eq!(ledger.balance(2, "BTC").unwrap(), Some((46.0, 0.0)));
        assert_eq!(ledger.balance(2, "USD").unwrap(), Some((52.0, 0.0)));

        assert_eq!(ledger.refund_cancelled_order(11, 1).unwrap(), Some(8.0));
        assert_eq!(ledger.balance(1, "USD").unwrap(), Some((48.0, 0.0)));
        assert_eq!(ledger.refund_cancelled_order(12, 2).unwrap(), None);
    }

    #[test]
    fn reserve_rejects_insufficient_funds_without_changing_balance() {
        let ledger = build_ledger();
        assert!(ledger.reserve_order(21, 1, "USD", "BTC", 51.0).is_err());
        assert_eq!(ledger.balance(1, "USD").unwrap(), Some((50.0, 0.0)));
    }
}
