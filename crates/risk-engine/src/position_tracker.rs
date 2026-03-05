use common::{Order, Symbol, Trade, UserId};
use dashmap::DashMap;
use rust_decimal::Decimal;
use std::sync::Arc;

/// Tracks open positions and exposures for users
#[derive(Debug, Clone)]
pub struct Position {
    pub user_id: UserId,
    pub symbol: Symbol,
    /// Net position (positive = long, negative = short)
    pub net_quantity: Decimal,
    /// Total buy quantity
    pub total_buy: Decimal,
    /// Total sell quantity
    pub total_sell: Decimal,
    /// Number of open orders
    pub open_orders: usize,
}

impl Position {
    pub fn new(user_id: UserId, symbol: Symbol) -> Self {
        Self {
            user_id,
            symbol,
            net_quantity: Decimal::ZERO,
            total_buy: Decimal::ZERO,
            total_sell: Decimal::ZERO,
            open_orders: 0,
        }
    }

    pub fn is_long(&self) -> bool {
        self.net_quantity > Decimal::ZERO
    }

    pub fn is_short(&self) -> bool {
        self.net_quantity < Decimal::ZERO
    }

    pub fn exposure(&self) -> Decimal {
        self.net_quantity.abs()
    }
}

/// Position tracker for managing user positions
pub struct PositionTracker {
    /// Positions indexed by (UserId, Symbol)
    positions: Arc<DashMap<(UserId, Symbol), Position>>,
}

impl PositionTracker {
    pub fn new() -> Self {
        Self {
            positions: Arc::new(DashMap::new()),
        }
    }

    /// Record an order placement (reserves position)
    pub fn on_order_placed(&self, order: &Order) {
        let key = (order.user_id, order.symbol.clone());
        let mut entry = self.positions.entry(key.clone()).or_insert_with(|| {
            Position::new(order.user_id, order.symbol.clone())
        });
        
        entry.open_orders += 1;
    }

    /// Record a trade execution
    pub fn on_trade(&self, trade: &Trade) {
        // Update buyer position
        {
            let key = (trade.buyer_user_id, trade.symbol.clone());
            let mut entry = self.positions.entry(key).or_insert_with(|| {
                Position::new(trade.buyer_user_id, trade.symbol.clone())
            });
            
            entry.net_quantity += trade.quantity.0;
            entry.total_buy += trade.quantity.0;
        }

        // Update seller position
        {
            let key = (trade.seller_user_id, trade.symbol.clone());
            let mut entry = self.positions.entry(key).or_insert_with(|| {
                Position::new(trade.seller_user_id, trade.symbol.clone())
            });
            
            entry.net_quantity -= trade.quantity.0;
            entry.total_sell += trade.quantity.0;
        }
    }

    /// Record an order cancellation
    pub fn on_order_cancelled(&self, order: &Order) {
        let key = (order.user_id, order.symbol.clone());
        if let Some(mut entry) = self.positions.get_mut(&key) {
            entry.open_orders = entry.open_orders.saturating_sub(1);
        }
    }

    /// Get position for a user and symbol
    pub fn get_position(&self, user_id: UserId, symbol: &Symbol) -> Option<Position> {
        self.positions.get(&(user_id, symbol.clone())).map(|p| p.clone())
    }

    /// Get all positions for a user
    pub fn get_user_positions(&self, user_id: UserId) -> Vec<Position> {
        self.positions
            .iter()
            .filter(|entry| entry.key().0 == user_id)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get net exposure across all positions for a user
    pub fn get_total_exposure(&self, user_id: UserId) -> Decimal {
        self.positions
            .iter()
            .filter(|entry| entry.key().0 == user_id)
            .map(|entry| entry.value().exposure())
            .sum()
    }
}

impl Default for PositionTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{OrderId, Price, Quantity, TradeId};
    use chrono::Utc;
    use rust_decimal_macros::dec;

    #[test]
    fn test_position_tracker() {
        let tracker = PositionTracker::new();
        let user_id = UserId::new();
        let symbol = Symbol::new("BTC/USD");

        // Simulate a trade
        let trade = Trade {
            id: TradeId::new(),
            symbol: symbol.clone(),
            price: Price::new(dec!(50000)),
            quantity: Quantity::new(dec!(1.5)),
            buy_order_id: OrderId::new(),
            sell_order_id: OrderId::new(),
            buyer_user_id: user_id,
            seller_user_id: UserId::new(),
            timestamp: Utc::now(),
            sequence_number: 1,
        };

        tracker.on_trade(&trade);

        let position = tracker.get_position(user_id, &symbol).unwrap();
        assert_eq!(position.net_quantity, dec!(1.5));
        assert!(position.is_long());
        assert_eq!(position.exposure(), dec!(1.5));
    }

    #[test]
    fn test_position_tracking_both_sides() {
        let tracker = PositionTracker::new();
        let user_id = UserId::new();
        let symbol = Symbol::new("ETH/USD");

        // Buy 2.0
        let buy_trade = Trade {
            id: TradeId::new(),
            symbol: symbol.clone(),
            price: Price::new(dec!(3000)),
            quantity: Quantity::new(dec!(2.0)),
            buy_order_id: OrderId::new(),
            sell_order_id: OrderId::new(),
            buyer_user_id: user_id,
            seller_user_id: UserId::new(),
            timestamp: Utc::now(),
            sequence_number: 1,
        };

        tracker.on_trade(&buy_trade);

        // Sell 0.5
        let sell_trade = Trade {
            id: TradeId::new(),
            symbol: symbol.clone(),
            price: Price::new(dec!(3100)),
            quantity: Quantity::new(dec!(0.5)),
            buy_order_id: OrderId::new(),
            sell_order_id: OrderId::new(),
            buyer_user_id: UserId::new(),
            seller_user_id: user_id,
            timestamp: Utc::now(),
            sequence_number: 2,
        };

        tracker.on_trade(&sell_trade);

        let position = tracker.get_position(user_id, &symbol).unwrap();
        assert_eq!(position.net_quantity, dec!(1.5));
        assert_eq!(position.total_buy, dec!(2.0));
        assert_eq!(position.total_sell, dec!(0.5));
    }
}
