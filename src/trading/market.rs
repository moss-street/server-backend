use std::{
    sync::{
        mpsc::{self, SyncSender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use std::sync::mpsc::{sync_channel, Receiver, Sender};

use anyhow::{anyhow, Result};

use rust_models::common::{trade_request, TradeRequest};

use crate::db;

use super::models::market_order::MarketOrder;
use super::models::{
    market_order,
    user::{UpdateIndicator, User, WalletOperations, Wallet},
};
use std::collections::BinaryHeap;

use tokio::sync::MutexGuard;

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
    order_sender: SyncSender<MarketOrder>,
    process_orders_handle: JoinHandle<()>,
}

pub trait MarketProcessor {
    fn send_order(&self, market_order: MarketOrder) -> Result<()>;
}

enum OrderType {
    MarketSell,
    MarketBuy,
    LimitSell,
    LimitBuy
}

impl Market {
    pub fn new(a: impl Into<String>, b: impl Into<String>) -> Self {
        let swap_pair = SwapPair(a.into(), b.into());
        let (order_sender, _order_receiver): (SyncSender<MarketOrder>, Receiver<MarketOrder>) =
            mpsc::sync_channel(100);

        // Spawning a thread to consume orders from the channel
        let process_orders_handle = thread::spawn({
            let swap_pair = swap_pair.clone();
            let mut finished_orders: Vec<MarketOrder> = Vec::new();
            let mut src_sell_limits: BinaryHeap<MarketOrder> = BinaryHeap::new();
            let mut src_buy_limits: BinaryHeap<MarketOrder> = BinaryHeap::new();
            let mut limit_counter: i64 = 0;

            move || loop {
                if let Ok(mut order) = _order_receiver.recv() {

                    let process_trade_object = match (order.trade_request.trade_type(), order.trade_request.symbol_source == swap_pair.0) {
                        (trade_request::TradeType::Market, false) => todo!(), // Buy Market
                        (trade_request::TradeType::Market, true) => todo!(), // Sell Market
                        (trade_request::TradeType::Limit, false) => todo!(), // Buy Limit
                        (trade_request::TradeType::Limit, true) => todo!(), // Sell Limit
                        (_, _) => continue, // invalid order, also should never occur
                    };

                    let result = process_trade_object.process();

                    println!("{:#?}", result);

                    print!("processing order");

                    // This order is to sell the src symbol in exchange for the dst symbol
                    if order.trade_request.symbol_source == swap_pair.0 {
                        order.is_buy = false;

                        // If the order is a limit order

                        if let Some(price) = order.trade_request.price {
                            // Do we have a buy limit?
                            while let Some(mut buy_limit_order) = src_buy_limits.pop() {
                                // Then get its price
                                let best_limit_price = buy_limit_order.trade_request.price.unwrap();

                                // If the price is more than our sell
                                if best_limit_price >= price {
                                    // Fulfill
                                    process_transaction(&mut buy_limit_order, &mut order, OrderType::LimitSell);

                                    if buy_limit_order.rem_quantity >= 0.0 {
                                        // The incoming order is satisfied

                                        if buy_limit_order.rem_quantity > 0.0 {
                                            // Push the standing buy limit order back to the heap if it still has some left
                                            src_buy_limits.push(buy_limit_order);
                                        }

                                        break;
                                    } else {
                                        // The limit order is finished
                                        finished_orders.push(buy_limit_order);
                                    }
                                } else {
                                    // The existing buy orders are too cheap for us, break
                                    break;
                                }
                            }

                            if order.rem_quantity >= 0.0 {
                                // Seems the loop before didn't satisfy our order, push it up the sell heap
                                src_sell_limits.push(order);
                                break;
                            } else {
                                // Looks like the order is complete,
                                finished_orders.push(order);
                            }
                        } else {
                        }

                        // src_sell_orders.push(order.clone());
                    } else if order.trade_request.symbol_source == swap_pair.1 {
                        order.is_buy = true;
                        // order.rem_quantity /= ratio;
                        // src_buy_orders.push(order);
                    }

                    println!(
                        "============\n\n\n===CURRENT MARKET BOOK for {:#}===\n\n{:#?}\n\n============{:#?}\n\n============",
                        swap_pair, src_buy_limits, src_sell_limits
                    );
                }
            }
        });

        Self {
            swap_pair,
            order_sender,
            process_orders_handle,
        }
    }
}

fn process_market_sell(buyer: &mut MarketOrder, seller: &mut MarketOrder){
    let mut transaction_qty = 0.0;

    // It is a market sell, so the buyer has to be a limit with a price
    let price = buyer.trade_request.price.unwrap();

    if buyer.rem_quantity == seller.rem_quantity {
        transaction_qty = buyer.rem_quantity;

        buyer.rem_quantity = 0.0;
        seller.rem_quantity = 0.0;
    } else if buyer.rem_quantity > seller.rem_quantity {
        transaction_qty = seller.rem_quantity;

        buyer.rem_quantity -= transaction_qty;
        seller.rem_quantity = 0.0;
    } else {
        transaction_qty = buyer.rem_quantity;

        buyer.rem_quantity = 0.0;
        seller.rem_quantity -= transaction_qty;
    }

    buyer
        .user
        .ledger
        .get(&buyer.trade_request.symbol_source)
        .unwrap()
        .update_balance(UpdateIndicator::Add, transaction_qty * price);
    seller
        .user
        .ledger
        .get(&buyer.trade_request.symbol_dest)
        .unwrap()
        .update_balance(UpdateIndicator::Subtract, transaction_qty * price);
}

fn process_transaction(buyer: &mut MarketOrder, seller: &mut MarketOrder, order_type : OrderType) {
    let mut transaction_qty = 0.0;

    match order_type {
        OrderType::MarketSell => {
                todo!()
        },
        OrderType::MarketBuy => {
                todo!()
        },
        OrderType::LimitSell => {
            todo!()
        },
        OrderType::LimitBuy => {
            todo!()
        },
    }

    if buyer.rem_quantity == seller.rem_quantity {
        transaction_qty = buyer.rem_quantity;

        buyer.rem_quantity = 0.0;
        seller.rem_quantity = 0.0;
    } else if buyer.rem_quantity > seller.rem_quantity {
        transaction_qty = seller.rem_quantity;

        buyer.rem_quantity -= transaction_qty;
        seller.rem_quantity = 0.0;
    } else {
        transaction_qty = buyer.rem_quantity;

        buyer.rem_quantity = 0.0;
        seller.rem_quantity -= transaction_qty;
    }

    buyer
        .user
        .ledger
        .get(&buyer.trade_request.symbol_source)
        .unwrap()
        .update_balance(UpdateIndicator::Add, transaction_qty);
    seller
        .user
        .ledger
        .get(&buyer.trade_request.symbol_dest)
        .unwrap()
        .update_balance(UpdateIndicator::Subtract, transaction_qty);
}

impl MarketProcessor for Market {
    fn send_order(&self, market_order: MarketOrder) -> Result<()> {
        self.order_sender
            .send(market_order)
            .map_err(|err| anyhow!("Failed to send order: {}", err))
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



struct ProcessTradeObject<'a> {
    market_order: &'a MarketOrder,
    is_limit: bool,
    is_buy: bool,
}

impl<'a> ProcessTradeObject<'a> {
    pub fn new(market_order: &'a mut MarketOrder, is_limit: bool, is_buy: bool) -> Self {
        Self { market_order, is_limit, is_buy}
    }

    fn process_order(&mut self) {
        todo!()
    }
}