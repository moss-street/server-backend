use rust_models::common::TradeRequest;
use std::cmp::Ordering;
use crate::trading::market::Market;

use super::user::User;

#[derive(Debug, PartialEq)]
pub struct MarketOrder {
    pub trade_request: TradeRequest,
    pub user: User,
    pub order: i64, // Used to break ties in priority
    pub rem_quantity: f64,
    pub is_buy: bool, // Determines Min-Heap (sells) or Max-Heap (buys)
}

impl MarketOrder {
    pub fn new(request: TradeRequest, user: User) -> Self {
        Self {
            trade_request: request.clone(),
            user,
            order : 0,
            rem_quantity: request.source_quantity,
            is_buy : false,
        }
    }
}
impl Eq for MarketOrder {}

// Implement ordering for MarketOrder
impl Ord for MarketOrder {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.trade_request.price, other.trade_request.price) {
            (Some(p1), Some(p2)) => {
                let price_cmp = if self.is_buy {
                    p2.partial_cmp(&p1).unwrap_or(Ordering::Equal) // MaxHeap for buys
                } else {
                    p1.partial_cmp(&p2).unwrap_or(Ordering::Equal) // MinHeap for sells
                };

                if price_cmp == Ordering::Equal {
                    self.order.cmp(&other.order) // Break tie using order ID
                } else {
                    price_cmp
                }
            }
            (Some(_), None) => Ordering::Less,  // Market orders (None) are prioritized
            (None, Some(_)) => Ordering::Greater,
            (None, None) => self.order.cmp(&other.order), // If both are market orders, use order ID
        }
    }
}

// PartialOrd must be consistent with Ord
impl PartialOrd for MarketOrder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
