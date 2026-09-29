use std::cmp::Ordering;

use super::models::internal_order::{InternalOrder, OrderSide};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct Fill {
    pub price: f64,
    pub quantity: f64,
    pub maker_order_id: u64,
    pub taker_order_id: u64,
    pub maker_source_quantity: f64,
    pub taker_source_quantity: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderSubmission {
    pub order_id: u64,
    pub fills: Vec<Fill>,
    pub remaining_quantity: f64,
    pub rests_on_book: bool,
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
        self.submit(OrderSide::Buy, Some(price), quantity).fills
    }

    pub fn submit_buy_with_id(
        &mut self,
        order_id: u64,
        quantity: f64,
        price: f64,
    ) -> OrderSubmission {
        self.submit_with_id(order_id, OrderSide::Buy, Some(price), quantity)
    }

    pub fn submit_sell(&mut self, quantity: f64, price: f64) -> Vec<Fill> {
        self.submit(OrderSide::Sell, Some(price), quantity).fills
    }

    pub fn submit_sell_with_id(
        &mut self,
        order_id: u64,
        quantity: f64,
        price: f64,
    ) -> OrderSubmission {
        self.submit_with_id(order_id, OrderSide::Sell, Some(price), quantity)
    }

    #[allow(dead_code)]
    pub fn submit_market_buy(&mut self, quantity: f64) -> Vec<Fill> {
        self.submit(OrderSide::Buy, None, quantity).fills
    }

    pub fn submit_market_buy_with_id(&mut self, order_id: u64, quantity: f64) -> OrderSubmission {
        self.submit_with_id(order_id, OrderSide::Buy, None, quantity)
    }

    #[allow(dead_code)]
    pub fn submit_market_sell(&mut self, quantity: f64) -> Vec<Fill> {
        self.submit(OrderSide::Sell, None, quantity).fills
    }

    pub fn submit_market_sell_with_id(&mut self, order_id: u64, quantity: f64) -> OrderSubmission {
        self.submit_with_id(order_id, OrderSide::Sell, None, quantity)
    }

    pub fn cancel_order(&mut self, order_id: u64) -> Option<InternalOrder> {
        Self::remove_order(&mut self.buy_orders, order_id)
            .or_else(|| Self::remove_order(&mut self.sell_orders, order_id))
    }

    fn submit(&mut self, side: OrderSide, price: Option<f64>, quantity: f64) -> OrderSubmission {
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
    ) -> OrderSubmission {
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
            let maker_source_quantity =
                (incoming.remaining_qty / execution_price).min(resting.remaining_qty);
            let taker_source_quantity = maker_source_quantity * execution_price;

            incoming.fill(taker_source_quantity);
            resting.fill(maker_source_quantity);
            fills.push(Fill {
                price: execution_price,
                quantity: maker_source_quantity,
                maker_order_id: resting.order_id,
                taker_order_id: incoming.order_id,
                maker_source_quantity,
                taker_source_quantity,
            });

            if resting.is_filled() {
                opposite.remove(best_index);
            }
        }

        let remaining_quantity = incoming.remaining_qty;
        let rests_on_book = remaining_quantity > 0.0 && incoming.price.is_some();
        if rests_on_book {
            match side {
                OrderSide::Buy => self.buy_orders.push(incoming),
                OrderSide::Sell => self.sell_orders.push(incoming),
            }
        }

        OrderSubmission {
            order_id,
            fills,
            remaining_quantity,
            rests_on_book,
        }
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
                        .then_with(|| right.order_id.cmp(&left.order_id))
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
                        .then_with(|| right.order_id.cmp(&left.order_id))
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
                incoming_price * resting_price >= 1.0
            }
            OrderSide::Sell => {
                if incoming.price.is_none() {
                    return true;
                }

                let incoming_price = incoming.price.unwrap();
                let resting_price = resting.price.unwrap_or(f64::NEG_INFINITY);
                incoming_price * resting_price >= 1.0
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

        engine.submit_sell(5.0, 2.0);
        let fills = engine.submit_buy(10.0, 0.6);

        assert_eq!(
            fills,
            vec![Fill {
                price: 2.0,
                quantity: 5.0,
                maker_order_id: 0,
                taker_order_id: 1,
                maker_source_quantity: 5.0,
                taker_source_quantity: 10.0,
            }]
        );
        assert_eq!(engine.buy_book_len(), 0);
        assert_eq!(engine.sell_book_len(), 0);
    }

    #[test]
    fn partial_fill_leaves_resting_order() {
        let mut engine = TradeEngine::new();

        engine.submit_sell(10.0, 2.0);
        let fills = engine.submit_buy(8.0, 0.6);

        assert_eq!(
            fills,
            vec![Fill {
                price: 2.0,
                quantity: 4.0,
                maker_order_id: 0,
                taker_order_id: 1,
                maker_source_quantity: 4.0,
                taker_source_quantity: 8.0,
            }]
        );
        assert_eq!(engine.buy_book_len(), 0);
        assert_eq!(engine.sell_book_len(), 1);
        assert_eq!(engine.sell_orders[0].remaining_qty, 6.0);
    }

    #[test]
    fn sell_direction_matches_using_reciprocal_source_quantities() {
        let mut engine = TradeEngine::new();
        engine.submit_buy_with_id(17, 8.0, 0.5);

        let submission = engine.submit_sell_with_id(18, 6.0, 3.0);

        assert_eq!(submission.fills.len(), 1);
        let fill = &submission.fills[0];
        assert_eq!(fill.maker_order_id, 17);
        assert_eq!(fill.taker_order_id, 18);
        assert_eq!(fill.price, 0.5);
        assert_eq!(fill.maker_source_quantity, 8.0);
        assert_eq!(fill.taker_source_quantity, 4.0);
        assert_eq!(submission.remaining_quantity, 2.0);
        assert!(submission.rests_on_book);
    }

    #[test]
    fn sell_matches_oldest_equal_price_buy_order_first() {
        let mut engine = TradeEngine::new();
        engine.submit_buy_with_id(1, 5.0, 0.5);
        engine.submit_buy_with_id(2, 5.0, 0.5);

        let submission = engine.submit_sell_with_id(3, 2.0, 3.0);

        assert_eq!(submission.fills.len(), 1);
        assert_eq!(submission.fills[0].maker_order_id, 1);
    }

    #[test]
    fn non_crossing_order_remains_resting() {
        let mut engine = TradeEngine::new();

        engine.submit_sell(10.0, 2.0);
        let fills = engine.submit_buy(5.0, 0.4);

        assert!(fills.is_empty());
        assert_eq!(engine.buy_book_len(), 1);
        assert_eq!(engine.buy_orders[0].remaining_qty, 5.0);
        assert_eq!(engine.sell_book_len(), 1);
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
        engine.submit_sell_with_id(51, 10.0, 1.0);
        let fills = engine.submit_buy(4.0, 1.0);
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
        engine.submit_sell_with_id(61, 4.0, 1.0);
        engine.submit_buy(4.0, 1.0);

        assert!(engine.cancel_order(61).is_none());
    }

    #[test]
    fn unfilled_market_order_does_not_rest_on_book() {
        let mut engine = TradeEngine::new();

        let submission = engine.submit_market_buy_with_id(71, 5.0);

        assert_eq!(submission.remaining_quantity, 5.0);
        assert!(!submission.rests_on_book);
        assert_eq!(engine.buy_book_len(), 0);
    }
}
