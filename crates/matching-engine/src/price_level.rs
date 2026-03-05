use common::{Order, OrderId, Quantity};
use indexmap::IndexMap;

/// Represents a single price level in the orderbook
/// Orders at the same price level are sorted by time priority (FIFO)
/// Uses IndexMap to maintain insertion order (time priority) while allowing O(1) lookup by OrderId
#[derive(Debug, Clone)]
pub struct PriceLevel {
    /// Orders at this price level, indexed by OrderId
    /// IndexMap maintains insertion order for time priority
    orders: IndexMap<OrderId, Order>,
    /// Total quantity available at this price level
    total_quantity: Quantity,
}

impl PriceLevel {
    pub fn new() -> Self {
        Self {
            orders: IndexMap::new(),
            total_quantity: Quantity::zero(),
        }
    }

    /// Add an order to this price level
    pub fn add_order(&mut self, order: Order) {
        self.total_quantity = Quantity(self.total_quantity.0 + order.remaining_quantity().0);
        self.orders.insert(order.id, order);
    }

    /// Remove an order from this price level
    pub fn remove_order(&mut self, order_id: &OrderId) -> Option<Order> {
        if let Some(order) = self.orders.shift_remove(order_id) {
            self.total_quantity = Quantity(self.total_quantity.0 - order.remaining_quantity().0);
            Some(order)
        } else {
            None
        }
    }

    /// Get a mutable reference to an order
    pub fn get_order_mut(&mut self, order_id: &OrderId) -> Option<&mut Order> {
        self.orders.get_mut(order_id)
    }

    /// Get an order by ID
    pub fn get_order(&self, order_id: &OrderId) -> Option<&Order> {
        self.orders.get(order_id)
    }

    /// Get the first order (oldest by time priority)
    pub fn first_order(&self) -> Option<&Order> {
        self.orders.values().next()
    }

    /// Get a mutable reference to the first order
    pub fn first_order_mut(&mut self) -> Option<&mut Order> {
        self.orders.values_mut().next()
    }

    /// Get total quantity at this price level
    pub fn total_quantity(&self) -> Quantity {
        self.total_quantity
    }

    /// Check if this price level is empty
    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    /// Get the number of orders at this price level
    pub fn order_count(&self) -> usize {
        self.orders.len()
    }

    /// Iterate over all orders at this price level (in time priority order)
    pub fn orders(&self) -> impl Iterator<Item = &Order> {
        self.orders.values()
    }

    /// Update the filled quantity of an order and adjust total quantity
    pub fn update_filled_quantity(&mut self, order_id: &OrderId, filled_qty: Quantity) -> bool {
        if let Some(order) = self.orders.get_mut(order_id) {
            let old_remaining = order.remaining_quantity();
            order.filled_quantity = Quantity(order.filled_quantity.0 + filled_qty.0);
            let new_remaining = order.remaining_quantity();
            
            // Adjust total quantity
            self.total_quantity = Quantity(
                self.total_quantity.0 - old_remaining.0 + new_remaining.0
            );
            
            true
        } else {
            false
        }
    }
}

impl Default for PriceLevel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{OrderId, UserId, Symbol, Side, Price, Quantity, OrderType, TimeInForce, OrderStatus};
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use rust_decimal::Decimal;

    fn create_test_order(id: OrderId, quantity: Decimal, sequence: u64) -> Order {
        Order {
            id,
            user_id: UserId::new(),
            symbol: Symbol::new("BTC/USD"),
            side: Side::Buy,
            order_type: OrderType::Limit,
            price: Price::new(dec!(50000.00)),
            quantity: Quantity::new(quantity),
            filled_quantity: Quantity::zero(),
            time_in_force: TimeInForce::GTC,
            status: OrderStatus::Open,
            timestamp: Utc::now(),
            sequence_number: sequence,
        }
    }

    #[test]
    fn test_price_level_add_remove() {
        let mut level = PriceLevel::new();
        let order_id = OrderId::new();
        let order = create_test_order(order_id, dec!(1.5), 1);

        level.add_order(order.clone());
        assert_eq!(level.order_count(), 1);
        assert_eq!(level.total_quantity(), Quantity::new(dec!(1.5)));

        let removed = level.remove_order(&order_id);
        assert!(removed.is_some());
        assert!(level.is_empty());
        assert_eq!(level.total_quantity(), Quantity::zero());
    }

    #[test]
    fn test_price_level_time_priority() {
        let mut level = PriceLevel::new();
        
        let order1 = create_test_order(OrderId::new(), dec!(1.0), 1);
        let order2 = create_test_order(OrderId::new(), dec!(2.0), 2);
        let order3 = create_test_order(OrderId::new(), dec!(3.0), 3);

        level.add_order(order1.clone());
        level.add_order(order2.clone());
        level.add_order(order3.clone());

        // First order should be order1 (oldest)
        assert_eq!(level.first_order().unwrap().id, order1.id);
        assert_eq!(level.total_quantity(), Quantity::new(dec!(6.0)));
    }

    #[test]
    fn test_price_level_update_filled() {
        let mut level = PriceLevel::new();
        let order_id = OrderId::new();
        let order = create_test_order(order_id, dec!(10.0), 1);

        level.add_order(order);
        assert_eq!(level.total_quantity(), Quantity::new(dec!(10.0)));

        // Fill 3.0
        level.update_filled_quantity(&order_id, Quantity::new(dec!(3.0)));
        assert_eq!(level.total_quantity(), Quantity::new(dec!(7.0)));

        // Fill remaining 7.0
        level.update_filled_quantity(&order_id, Quantity::new(dec!(7.0)));
        assert_eq!(level.total_quantity(), Quantity::zero());
    }
}
