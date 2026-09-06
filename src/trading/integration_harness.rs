#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rust_models::common::{
        trade_request::{TradeType, TransactionType},
        TradeRequest,
    };

    use crate::trading::{
        market::{Market, MarketProcessor},
        models::{
            market_order::MarketOrder,
            user::{User, Wallet},
        },
    };

    fn fake_user(user_id: i32) -> User {
        User {
            _id: user_id,
            _email: format!("user-{user_id}@example.com"),
            ledger: HashMap::from([
                ("USD".to_string(), Wallet::new("USD")),
                ("BTC".to_string(), Wallet::new("BTC")),
            ]),
        }
    }

    fn limit_order(
        user_id: i32,
        source_symbol: &str,
        dest_symbol: &str,
        source_qty: f64,
        price: f64,
    ) -> MarketOrder {
        MarketOrder::new(
            TradeRequest {
                trade_type: TradeType::Limit as i32,
                transaction_type: TransactionType::Buy as i32,
                symbol_source: source_symbol.to_string(),
                symbol_dest: dest_symbol.to_string(),
                source_quantity: source_qty,
                price: Some(price),
                valid_from: None,
                valid_to: None,
            },
            fake_user(user_id),
        )
    }

    #[test]
    fn harness_runs_trade_sequence_and_tracks_book_state() {
        let market = Market::new("USD", "BTC");

        // Step 1: Seller posts 10 USD @ 100 BTC/USD equivalent.
        market
            .send_order(limit_order(1, "USD", "BTC", 10.0, 100.0))
            .expect("sell order should be accepted");
        assert_eq!(market.book_depths(), (0, 1));

        // Step 2: Buyer posts non-crossing bid 3 BTC @ 90.
        market
            .send_order(limit_order(2, "BTC", "USD", 3.0, 90.0))
            .expect("non-crossing buy should be accepted");
        assert_eq!(market.book_depths(), (1, 1));

        // Step 3: Crossing buy at 100 fills 4 units against resting sell.
        let fills = market
            .send_order_and_collect_fills(limit_order(3, "BTC", "USD", 4.0, 100.0))
            .expect("crossing buy should execute");
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].quantity, 4.0);
        assert_eq!(fills[0].price, 100.0);
        assert_eq!(market.book_depths(), (1, 1));

        // Step 4: Crossing buy at 110 consumes the remaining 6 units.
        let fills = market
            .send_order_and_collect_fills(limit_order(4, "BTC", "USD", 6.0, 110.0))
            .expect("crossing buy should execute remaining sell quantity");
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].quantity, 6.0);
        assert_eq!(fills[0].price, 100.0);

        // The non-crossing bid from step 2 should still be resting.
        assert_eq!(market.book_depths(), (1, 0));
    }

    #[test]
    fn harness_rejects_symbols_outside_market_pair() {
        let market = Market::new("USD", "BTC");

        let result = market.send_order(limit_order(9, "ETH", "USD", 1.0, 100.0));

        assert!(result.is_err());
    }
}
