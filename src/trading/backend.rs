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
}
