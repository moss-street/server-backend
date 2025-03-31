use std::{
    sync::{mpsc::{self, SyncSender}, Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

use std::sync::mpsc::{sync_channel, Receiver, Sender};

use anyhow::{Result, anyhow};

use rust_models::common::TradeRequest;

use crate::db;

use super::models::{market_order, user::{User, WalletOperations}};
use super::models::market_order::{MarketOrder};
use std::collections::BinaryHeap;

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
    order_sender : SyncSender<MarketOrder>,
    process_orders_handle : JoinHandle<()>,
}

pub trait MarketProcessor {
    fn send_order(&self, market_order : MarketOrder) -> Result<()>;
    fn process_transaction(&self, buyer : MarketOrder, seller : MarketOrder, ratio : f64);
}

impl Market {
    pub fn new(a: impl Into<String>, b: impl Into<String>) -> Self {
        let swap_pair = SwapPair(a.into(), b.into());
        let (order_sender, _order_receiver) : (SyncSender<MarketOrder>,  Receiver<MarketOrder>) = mpsc::sync_channel(100);

    // Spawning a thread to consume orders from the channel
        let process_orders_handle = thread::spawn({
            let swap_pair = swap_pair.clone();
            let mut src_buy_orders : Vec<MarketOrder> = Vec::new();
            let mut src_sell_orders : Vec<MarketOrder> = Vec::new();
            let mut finished_orders : Vec<MarketOrder> = Vec::new();
            let mut src_sell_limits : BinaryHeap<MarketOrder> = BinaryHeap::new();
            let mut src_buy_limits : BinaryHeap<MarketOrder> = BinaryHeap::new();
            let mut limit_counter : i64 = 0;

            // sell 1 src at 45
            // buy 1 src at 55

            // 1 src = 50 dst
            // 1 dst = 1/50 src
            let market_price : f64 = 50.0;

            move || loop {
                if let Ok(mut order) = _order_receiver.recv(){
                    print!("processing order");
            

                    // This order is to sell the src symbol in exchange for the dst symbol
                    if order.trade_request.symbol_source == swap_pair.0 {
                        order.is_buy = false;

                        // If the order is a limit order
                        
                        if let Some (price) = order.trade_request.price{

                            // Assign the limit order a order number
                            // In case we(the heap) have 2 with same price and need to determine prio
                            order.order = limit_counter;
                            limit_counter += 1;


                            // Then we can sell it to both market and limit seller, gotta find highest
                            let market_buy_iter : std::iter::Peekable<_> = src_buy_orders.iter().peekable();

                            loop{
                                // Nice check to see if the order is fulfilled
                                if order.rem_quantity == 0.0{
                                    finished_orders.push(order);
                                    break;
                                }

                                // Do we have a buy limit?
                                if let Some (sell_limit_order) = src_buy_limits.peek(){

                                    // Then get its price
                                    let best_limit_price = sell_limit_order.trade_request.price.unwrap();

                                    // If the buying limit is better than market (very sus, but possible)
                                    if best_limit_price > market_price {

                                        // And if the limit is better than asking price
                                        if best_limit_price > price{

                                        }
                                    }
                                    else{

                                    }
                                }
                                if price <= market_price{

                                }

                            }
                                // Compare price between market sell and limit sell 


                                

                                // If the sell is still not fulfilled by market buyers
                                // Then check limit orders that are cheaper than market_price
                                // This is not really probable, but is an edge case
                                 
                            
                        }

        

                        // src_sell_orders.push(order.clone());
                    } else if order.trade_request.symbol_source == swap_pair.1 {
                        order.is_buy = true;
                        // order.rem_quantity /= ratio;
                        src_buy_orders.push(order);
                    }
    
                    println!(
                        "============\n\n\n===CURRENT MARKET BOOK for {:#}===\n\n{:#?}\n\n============{:#?}\n\n============",
                        swap_pair, src_buy_orders, src_sell_orders
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
    fn process_transaction(&self, mut buyer : MarketOrder, mut seller : MarketOrder, ratio : f64) {
        let mut transaction_qty = 0.0;

        if buyer.rem_quantity  == seller.rem_quantity{
            transaction_qty = buyer.rem_quantity;

            buyer.rem_quantity = 0.0;
            seller.rem_quantity = 0.0;
        }
        else if buyer.rem_quantity > seller.rem_quantity{
            transaction_qty = seller.rem_quantity;

            buyer.rem_quantity -= transaction_qty;
            seller.rem_quantity = 0.0;
        }
        else{
            transaction_qty = buyer.rem_quantity; 

            buyer.rem_quantity = 0.0;
            seller.rem_quantity -= transaction_qty
        }

        // buyer.user.ledger.get(&buyer.trade_request.symbol_dest)
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
