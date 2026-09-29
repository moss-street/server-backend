use anyhow::Result;
use std::collections::HashMap;

use crate::db::manager::DBManager;

use super::market::{Market, SwapPair};

#[derive(Debug)]
pub struct TradeBackend {
    markets: HashMap<SwapPair, Market>,
}

impl TradeBackend {
    pub fn new() -> Self {
        let markets = HashMap::new();
        Self { markets }
    }

    #[allow(unused)]
    pub fn fast_forward(&mut self, _db_manager: &DBManager) -> Result<()> {
        unimplemented!("This functionality is not yet implemented. Start a new market for now.")
    }

    pub fn add_market(&mut self, market: Market) {
        self.markets.insert(market.swap_pair.clone(), market);
    }

    pub fn get_market(&self, swap_pair: SwapPair) -> Option<&Market> {
        self.markets.get(&swap_pair)
    }

    #[allow(dead_code)]
    pub fn has_market(&self, a: impl Into<String>, b: impl Into<String>) -> bool {
        self.markets.contains_key(&SwapPair::new(a, b))
    }

    #[allow(dead_code)]
    pub fn market_pairs(&self) -> Vec<SwapPair> {
        self.markets.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_tracks_multiple_markets_independently() {
        let mut backend = TradeBackend::new();
        backend.add_market(Market::new("USD", "BTC"));
        backend.add_market(Market::new("ETH", "USD"));

        assert!(backend.has_market("USD", "BTC"));
        assert!(backend.has_market("BTC", "USD"));
        assert!(backend.has_market("ETH", "USD"));
        assert!(!backend.has_market("ETH", "BTC"));

        let pairs = backend.market_pairs();
        assert_eq!(pairs.len(), 2);
        assert!(pairs
            .iter()
            .any(|pair| pair == &SwapPair::new("USD", "BTC")));
        assert!(pairs
            .iter()
            .any(|pair| pair == &SwapPair::new("ETH", "USD")));
    }
}
