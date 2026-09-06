use anyhow::{anyhow, Result};
use std::sync::Mutex;

use super::{
    models::market_order::MarketOrder,
    trade_engine::{Fill, TradeEngine},
};

/// A swap pair is the type of currency pairs that we are trading in the market.
/// This struct has a custom implementation of PartialEq that allows us to take two swap pairs and use == on them
/// e.g.  SwapPair('usd', 'btc') == SwapPair('btc', 'usd')
#[derive(Debug, Eq, Clone)]
pub struct SwapPair(String, String);

impl SwapPair {
    pub fn new(a: impl Into<String>, b: impl Into<String>) -> Self {
        Self(a.into(), b.into())
    }
}
impl std::cmp::PartialEq for SwapPair {
    fn eq(&self, other: &Self) -> bool {
        (self.0 == other.0 && self.1 == other.1) || (self.1 == other.0 && self.0 == other.1)
    }
}

impl std::hash::Hash for SwapPair {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let (a, b) = match self.0.cmp(&self.1) {
            std::cmp::Ordering::Less => (&self.0, &self.1),
            _ => (&self.1, &self.0),
        };
        a.hash(state);
        b.hash(state);
    }
}

impl std::fmt::Display for SwapPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.0, self.1)
    }
}

#[derive(Debug)]
pub struct Market {
    pub swap_pair: SwapPair,
    engine: Mutex<TradeEngine>,
}

pub trait MarketProcessor {
    fn send_order(&self, market_order: MarketOrder) -> Result<()>;
}

impl Market {
    pub fn new(a: impl Into<String>, b: impl Into<String>) -> Self {
        let swap_pair = SwapPair(a.into(), b.into());
        Self {
            swap_pair,
            engine: Mutex::new(TradeEngine::new()),
        }
    }

    fn process_with_engine(&self, market_order: MarketOrder) -> Result<Vec<Fill>> {
        let mut engine = self
            .engine
            .lock()
            .map_err(|_| anyhow!("Failed to lock market engine"))?;

        let source_symbol = market_order.trade_request.symbol_source.as_str();
        let price = market_order.trade_request.price;
        let quantity = market_order.rem_quantity;

        if source_symbol == self.swap_pair.0 {
            Ok(match price {
                Some(limit) => engine.submit_sell(quantity, limit),
                None => engine.submit_market_sell(quantity),
            })
        } else if source_symbol == self.swap_pair.1 {
            Ok(match price {
                Some(limit) => engine.submit_buy(quantity, limit),
                None => engine.submit_market_buy(quantity),
            })
        } else {
            Err(anyhow!(
                "Order source symbol {} is not part of market {}",
                source_symbol,
                self.swap_pair
            ))
        }
    }

    #[cfg(test)]
    pub(crate) fn book_depths(&self) -> (usize, usize) {
        let engine = self.engine.lock().expect("engine lock should succeed");
        (engine.buy_book_len(), engine.sell_book_len())
    }

    #[cfg(test)]
    pub(crate) fn send_order_and_collect_fills(
        &self,
        market_order: MarketOrder,
    ) -> Result<Vec<Fill>> {
        self.process_with_engine(market_order)
    }
}

impl MarketProcessor for Market {
    fn send_order(&self, market_order: MarketOrder) -> Result<()> {
        let _fills = self.process_with_engine(market_order)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use rust_models::common::{
        trade_request::{TradeType, TransactionType},
        TradeRequest,
    };
    use std::collections::{HashMap, HashSet};

    use super::*;
    use crate::trading::models::user::{User, Wallet};

    fn fake_user() -> User {
        User {
            _id: 1,
            _email: "market-test@example.com".into(),
            ledger: HashMap::from([
                ("USD".into(), Wallet::new("USD")),
                ("BTC".into(), Wallet::new("BTC")),
            ]),
        }
    }

    fn order(source: &str, dest: &str, qty: f64, price: Option<f64>) -> MarketOrder {
        MarketOrder::new(
            TradeRequest {
                trade_type: TradeType::Limit as i32,
                transaction_type: TransactionType::Buy as i32,
                symbol_source: source.to_string(),
                symbol_dest: dest.to_string(),
                source_quantity: qty,
                price,
                valid_from: None,
                valid_to: None,
            },
            fake_user(),
        )
    }

    #[test]
    fn test_cmp() {
        let a = SwapPair("usd".into(), "btc".into());
        let b = SwapPair("btc".into(), "usd".into());

        assert_eq!(
            a, b,
            "Swap Pairs did not match! {a:#?} is supposed to be equal to {b:#?}"
        );
    }

    #[test]
    fn test_hashes_equal() {
        let a = SwapPair("usd".into(), "btc".into());
        let b = SwapPair("btc".into(), "usd".into());

        let mut hs = HashSet::new();
        hs.insert(a);
        hs.insert(b);

        assert_eq!(hs.len(), 1);
    }

    #[test]
    fn test_swap_pair_in_hash_map() {
        let a = SwapPair("usd".into(), "btc".into());
        let b = SwapPair("btc".into(), "usd".into());

        let mut hm = HashMap::new();
        let a_val = "A Value";
        hm.insert(&a, a_val);
        assert_eq!(*hm.get(&a).unwrap(), a_val);

        let b_val = "B Value";
        let removed = hm.insert(&b, b_val).unwrap();
        assert_eq!(removed, a_val);
        assert_eq!(*hm.get(&b).unwrap(), b_val);
    }

    #[test]
    fn send_order_matches_crossing_orders() {
        let market = Market::new("USD", "BTC");

        market
            .send_order(order("USD", "BTC", 5.0, Some(100.0)))
            .expect("sell order should be accepted");
        market
            .send_order(order("BTC", "USD", 5.0, Some(100.0)))
            .expect("buy order should be accepted");

        assert_eq!(market.book_depths(), (0, 0));
    }

    #[test]
    fn send_order_partial_fill_leaves_resting_quantity() {
        let market = Market::new("USD", "BTC");

        market
            .send_order(order("USD", "BTC", 10.0, Some(100.0)))
            .expect("sell order should be accepted");
        market
            .send_order(order("BTC", "USD", 6.0, Some(100.0)))
            .expect("buy order should be accepted");

        assert_eq!(market.book_depths(), (0, 1));
    }

    #[test]
    fn send_order_non_crossing_orders_rest_on_book() {
        let market = Market::new("USD", "BTC");

        market
            .send_order(order("USD", "BTC", 10.0, Some(100.0)))
            .expect("sell order should be accepted");
        market
            .send_order(order("BTC", "USD", 5.0, Some(90.0)))
            .expect("buy order should be accepted");

        assert_eq!(market.book_depths(), (1, 1));
    }
}
