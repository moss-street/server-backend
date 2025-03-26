use std::{
    sync::{mpsc::{self, SyncSender}, Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

use std::sync::mpsc::{sync_channel, Receiver, Sender};

use anyhow::{Result, anyhow};

use rust_models::common::TradeRequest;

use crate::db;

use super::models::user::{User, WalletOperations};

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
#[derive(Debug)]
pub struct MarketOrder{
    pub trade_request : TradeRequest,
    pub user : User,
}

impl MarketOrder{
    pub fn new(request : TradeRequest, user : User) -> Self{
        Self{trade_request : request, user}
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
    order_sender : SyncSender<MarketOrder>,
    process_orders_handle : JoinHandle<()>,
}

pub trait MarketProcessor {
    fn send_order(&self, market_order : MarketOrder) -> Result<()>;
}

impl Market {
    pub fn new(a: impl Into<String>, b: impl Into<String>) -> Self {
        let swap_pair = SwapPair(a.into(), b.into());
        let (order_sender, _order_receiver) = mpsc::sync_channel(100);

    // Spawning a thread to consume orders from the channel
        let process_orders_handle = thread::spawn({
            let mut market_book =  Vec::new();
            let swap_pair = swap_pair.clone();
            move || loop {
                if let Ok(order) = _order_receiver.recv(){
                    print!("processing order");
                    market_book.push(order);
    
                    println!(
                        "============\n\n\n===CURRENT MARKET BOOK for {:#}===\n\n{:#?}\n\n============",
                        swap_pair, market_book
                    );
                }
            }
        });

        Self {
            swap_pair,
            order_sender,
            process_orders_handle
        }
    }

}

impl MarketProcessor for Market {
    fn send_order(&self, market_order : MarketOrder) -> Result<()> {
        
        self.order_sender.send(market_order).map_err(|err| {
            anyhow!("Failed to send order: {}", err)
        })
    }

}

#[cfg(test)]
mod test {
    use std::collections::{HashMap, HashSet};

    use super::*;

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
}
