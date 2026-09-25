use std::cmp::Ordering;

use super::models::internal_order::{InternalOrder, OrderSide};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct Fill {
    pub price: f64,
    pub quantity: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TradeEngine {
    buy_orders: Vec<InternalOrder>,
    sell_orders: Vec<InternalOrder>,
    next_order_id: u64,
}

#[allow(dead_code)]
impl TradeEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn submit_buy(&mut self, quantity: f64, price: f64) -> Vec<Fill> {
        self.submit(OrderSide::Buy, Some(price), quantity)
    }

    pub fn submit_buy_with_id(&mut self, order_id: u64, quantity: f64, price: f64) -> Vec<Fill> {
        self.submit_with_id(order_id, OrderSide::Buy, Some(price), quantity)
    }

    pub fn submit_sell(&mut self, quantity: f64, price: f64) -> Vec<Fill> {
        self.submit(OrderSide::Sell, Some(price), quantity)
    }

    pub fn submit_sell_with_id(&mut self, order_id: u64, quantity: f64, price: f64) -> Vec<Fill> {
        self.submit_with_id(order_id, OrderSide::Sell, Some(price), quantity)
    }

    #[allow(dead_code)]
    pub fn submit_market_buy(&mut self, quantity: f64) -> Vec<Fill> {
        self.submit(OrderSide::Buy, None, quantity)
    }

    pub fn submit_market_buy_with_id(&mut self, order_id: u64, quantity: f64) -> Vec<Fill> {
        self.submit_with_id(order_id, OrderSide::Buy, None, quantity)
    }

    #[allow(dead_code)]
    pub fn submit_market_sell(&mut self, quantity: f64) -> Vec<Fill> {
        self.submit(OrderSide::Sell, None, quantity)
    }

    pub fn submit_market_sell_with_id(&mut self, order_id: u64, quantity: f64) -> Vec<Fill> {
        self.submit_with_id(order_id, OrderSide::Sell, None, quantity)
    }

    pub fn cancel_order(&mut self, order_id: u64) -> Option<InternalOrder> {
        Self::remove_order(&mut self.buy_orders, order_id)
            .or_else(|| Self::remove_order(&mut self.sell_orders, order_id))
    }

    fn submit(&mut self, side: OrderSide, price: Option<f64>, quantity: f64) -> Vec<Fill> {
        let order_id = self.next_order_id;
        self.next_order_id += 1;
        self.submit_with_id(order_id, side, price, quantity)
    }

    fn submit_with_id(
        &mut self,
        order_id: u64,
        side: OrderSide,
        price: Option<f64>,
        quantity: f64,
    ) -> Vec<Fill> {
        self.next_order_id = self.next_order_id.max(order_id.saturating_add(1));
        let mut incoming = InternalOrder::new(order_id, side, price, quantity);

        let mut fills = Vec::new();
        let opposite = match side {
            OrderSide::Buy => &mut self.sell_orders,
            OrderSide::Sell => &mut self.buy_orders,
        };

        while incoming.remaining_qty > 0.0 {
            let Some(best_index) = Self::best_crossing_index(opposite, &incoming) else {
                break;
            };

            let resting = &mut opposite[best_index];
            let execution_price = resting.price.or(incoming.price).unwrap_or(0.0);
            let matched_qty = incoming.remaining_qty.min(resting.remaining_qty);

            incoming.fill(matched_qty);
            resting.fill(matched_qty);
            fills.push(Fill {
                price: execution_price,
                quantity: matched_qty,
            });

            if resting.is_filled() {
                opposite.remove(best_index);
            }
        }

        if incoming.remaining_qty > 0.0 {
            match side {
                OrderSide::Buy => self.buy_orders.push(incoming),
                OrderSide::Sell => self.sell_orders.push(incoming),
            }
        }

        fills
    }

    fn remove_order(orders: &mut Vec<InternalOrder>, order_id: u64) -> Option<InternalOrder> {
        orders
            .iter()
            .position(|order| order.order_id == order_id)
            .map(|index| orders.remove(index))
    }

    fn best_crossing_index(orders: &[InternalOrder], incoming: &InternalOrder) -> Option<usize> {
        match incoming.side {
            OrderSide::Buy => orders
                .iter()
                .enumerate()
                .filter(|(_, order)| Self::can_cross(order, incoming))
                .min_by(|(_, left), (_, right)| {
                    let left_price = left.price.unwrap_or(f64::INFINITY);
                    let right_price = right.price.unwrap_or(f64::INFINITY);
                    left_price
                        .partial_cmp(&right_price)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| left.order_id.cmp(&right.order_id))
                })
                .map(|(index, _)| index),
            OrderSide::Sell => orders
                .iter()
                .enumerate()
                .filter(|(_, order)| Self::can_cross(order, incoming))
                .max_by(|(_, left), (_, right)| {
                    let left_price = left.price.unwrap_or(f64::NEG_INFINITY);
                    let right_price = right.price.unwrap_or(f64::NEG_INFINITY);
                    left_price
                        .partial_cmp(&right_price)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| left.order_id.cmp(&right.order_id))
                })
                .map(|(index, _)| index),
        }
    }

    fn can_cross(resting: &InternalOrder, incoming: &InternalOrder) -> bool {
        match incoming.side {
            OrderSide::Buy => {
                if incoming.price.is_none() {
                    return true;
                }

                let incoming_price = incoming.price.unwrap();
                let resting_price = resting.price.unwrap_or(f64::NEG_INFINITY);
                incoming_price >= resting_price
            }
            OrderSide::Sell => {
                if incoming.price.is_none() {
                    return true;
                }

                let incoming_price = incoming.price.unwrap();
                let resting_price = resting.price.unwrap_or(f64::INFINITY);
                incoming_price <= resting_price
            }
        }
    }

    #[cfg(test)]
    pub fn buy_book_len(&self) -> usize {
        self.buy_orders.len()
    }

    #[cfg(test)]
    pub fn sell_book_len(&self) -> usize {
        self.sell_orders.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buy_order_matches_sell_order_at_same_price() {
        let mut engine = TradeEngine::new();

        engine.submit_sell(5.0, 100.0);
        let fills = engine.submit_buy(5.0, 100.0);

        assert_eq!(
            fills,
            vec![Fill {
                price: 100.0,
                quantity: 5.0
            }]
        );
        assert_eq!(engine.buy_book_len(), 0);
        assert_eq!(engine.sell_book_len(), 0);
    }

    #[test]
    fn partial_fill_leaves_resting_order() {
        let mut engine = TradeEngine::new();

        engine.submit_sell(10.0, 100.0);
        let fills = engine.submit_buy(6.0, 100.0);

        assert_eq!(
            fills,
            vec![Fill {
                price: 100.0,
                quantity: 6.0
            }]
        );
        assert_eq!(engine.buy_book_len(), 0);
        assert_eq!(engine.sell_book_len(), 1);
        assert_eq!(engine.sell_orders[0].remaining_qty, 4.0);
    }

    #[test]
    fn non_crossing_order_remains_resting() {
        let mut engine = TradeEngine::new();

        engine.submit_sell(10.0, 100.0);
        let fills = engine.submit_buy(5.0, 90.0);

        assert!(fills.is_empty());
        assert_eq!(engine.buy_book_len(), 1);
        assert_eq!(engine.buy_orders[0].remaining_qty, 5.0);
    }

    #[test]
    fn cancel_removes_only_the_requested_resting_order() {
        let mut engine = TradeEngine::new();
        engine.submit_sell_with_id(41, 10.0, 100.0);
        engine.submit_sell_with_id(42, 7.0, 110.0);

        let cancelled = engine
            .cancel_order(41)
            .expect("resting order should be cancellable");

        assert_eq!(cancelled.order_id, 41);
        assert_eq!(cancelled.remaining_qty, 10.0);
        assert_eq!(engine.sell_book_len(), 1);
        assert_eq!(engine.sell_orders[0].order_id, 42);
        assert!(engine.cancel_order(41).is_none());
    }

    #[test]
    fn cancelled_order_returns_only_its_unfilled_quantity() {
        let mut engine = TradeEngine::new();
        engine.submit_sell_with_id(51, 10.0, 100.0);
        let fills = engine.submit_buy(4.0, 100.0);
        assert_eq!(fills[0].quantity, 4.0);

        let cancelled = engine
            .cancel_order(51)
            .expect("partially filled resting order should be cancellable");

        assert_eq!(cancelled.remaining_qty, 6.0);
        assert_eq!(engine.sell_book_len(), 0);
    }

    #[test]
    fn fully_filled_order_is_not_cancellable() {
        let mut engine = TradeEngine::new();
        engine.submit_sell_with_id(61, 4.0, 100.0);
        engine.submit_buy(4.0, 100.0);

        assert!(engine.cancel_order(61).is_none());
    }
}
