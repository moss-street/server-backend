use std::collections::HashSet;

use crate::db::manager::DBManager;

use super::market::Market;

#[derive(Debug)]
pub struct TradeBackend {
    markets: HashSet<Market>,
}

impl TradeBackend {
    pub fn new(_db_manager: &DBManager) -> Self {
        // TODO: make this init markets from database
        let markets = HashSet::new();
        Self { markets }
    }
}
