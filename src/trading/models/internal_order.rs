use std::cmp::Ordering;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[allow(dead_code)]
impl OrderSide {
    pub fn opposite(self) -> Self {
        match self {
            Self::Buy => Self::Sell,
            Self::Sell => Self::Buy,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct InternalOrder {
    pub order_id: u64,
    pub side: OrderSide,
    pub price: Option<f64>,
    pub original_qty: f64,
    pub remaining_qty: f64,
}

#[allow(dead_code)]
impl InternalOrder {
    pub fn new(order_id: u64, side: OrderSide, price: Option<f64>, quantity: f64) -> Self {
        Self {
            order_id,
            side,
            price,
            original_qty: quantity,
            remaining_qty: quantity,
        }
    }

    pub fn is_filled(&self) -> bool {
        self.remaining_qty <= 0.0
    }

    pub fn available_qty(&self) -> f64 {
        self.remaining_qty.max(0.0)
    }

    pub fn fill(&mut self, qty: f64) {
        self.remaining_qty = (self.remaining_qty - qty).max(0.0);
    }
}

impl Eq for InternalOrder {}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct BuyOrderComparator;

#[allow(dead_code)]
impl BuyOrderComparator {
    pub fn cmp(a: &InternalOrder, b: &InternalOrder) -> Ordering {
        match (a.price, b.price) {
            (Some(price_a), Some(price_b)) => {
                match price_a.partial_cmp(&price_b).unwrap_or(Ordering::Equal) {
                    Ordering::Equal => b.order_id.cmp(&a.order_id),
                    other => other,
                }
            }
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (None, None) => b.order_id.cmp(&a.order_id),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct SellOrderComparator;

#[allow(dead_code)]
impl SellOrderComparator {
    pub fn cmp(a: &InternalOrder, b: &InternalOrder) -> Ordering {
        match (a.price, b.price) {
            (Some(price_a), Some(price_b)) => {
                match price_b.partial_cmp(&price_a).unwrap_or(Ordering::Equal) {
                    Ordering::Equal => b.order_id.cmp(&a.order_id),
                    other => other,
                }
            }
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (None, None) => b.order_id.cmp(&a.order_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_order_invariants_are_defined() {
        let order = InternalOrder::new(1, OrderSide::Buy, Some(100.0), 10.0);
        assert_eq!(order.order_id, 1);
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.price, Some(100.0));
        assert_eq!(order.remaining_qty, 10.0);
        assert!(!order.is_filled());
    }

    #[test]
    fn buy_orders_prioritize_higher_price() {
        let higher = InternalOrder::new(1, OrderSide::Buy, Some(100.0), 10.0);
        let lower = InternalOrder::new(2, OrderSide::Buy, Some(50.0), 10.0);

        assert_eq!(BuyOrderComparator::cmp(&higher, &lower), Ordering::Greater);
    }

    #[test]
    fn same_price_orders_are_fifo() {
        let first = InternalOrder::new(1, OrderSide::Buy, Some(100.0), 10.0);
        let second = InternalOrder::new(2, OrderSide::Buy, Some(100.0), 10.0);

        assert_eq!(BuyOrderComparator::cmp(&first, &second), Ordering::Greater);
    }

    #[test]
    fn sell_orders_prioritize_lower_price() {
        let cheaper = InternalOrder::new(1, OrderSide::Sell, Some(50.0), 10.0);
        let dearer = InternalOrder::new(2, OrderSide::Sell, Some(100.0), 10.0);

        assert_eq!(
            SellOrderComparator::cmp(&cheaper, &dearer),
            Ordering::Greater
        );
    }

    #[test]
    fn market_orders_have_order_priority() {
        let market = InternalOrder::new(1, OrderSide::Buy, None, 10.0);
        let limit = InternalOrder::new(2, OrderSide::Buy, Some(100.0), 10.0);

        assert_eq!(BuyOrderComparator::cmp(&market, &limit), Ordering::Greater);
    }
}
