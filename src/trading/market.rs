use std::{
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::Result;

use rust_models::common::TradeRequest;

use crate::db;

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
    pub market_book: Arc<Mutex<Vec<TradeRequest>>>,
    print_order_book_handle: JoinHandle<()>,
}

pub trait MarketProcessor {
    fn process_trade(&self, user: db::models::user::User, request: TradeRequest) -> Result<()>;
}

impl Market {
    pub fn new(a: impl Into<String>, b: impl Into<String>) -> Self {
        let swap_pair = SwapPair(a.into(), b.into());
        let market_book = Arc::new(Mutex::new(Vec::new()));
        let print_order_book_handle = thread::spawn({
            let market_book = market_book.clone();
            let swap_pair = swap_pair.clone();
            move || loop {
                {
                    let guard = market_book.lock().unwrap();
                    println!(
                    "============\n\n\n===CURRENT MARKET BOOK for {:#}===\n\n{:#?}\n\n============",
                    swap_pair, *guard
                );
                }
                thread::sleep(Duration::from_secs(10));
            }
        });
        Self {
            swap_pair,
            market_book,
            print_order_book_handle,
        }
    }
}

impl MarketProcessor for Market {
    fn process_trade(&self, user: db::models::user::User, request: TradeRequest) -> Result<()> {
        // convert from db user to trading user model
        let user = super::models::user::User::from(user);
        let _source_wallet = user.ledger.get(&request.symbol_source).ok_or_else(|| {
            tonic::Status::not_found(format!(
                "Wallet {} not found for user",
                &request.symbol_source
            ))
        })?;
        let _dest_wallet = user.ledger.get(&request.symbol_dest).ok_or_else(|| {
            tonic::Status::not_found(format!(
                "Wallet {} not found for user",
                &request.symbol_dest
            ))
        })?;

        let mut guard = self.market_book.lock().unwrap();
        guard.push(request);

        Ok(())
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
