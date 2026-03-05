use crate::price_level::PriceLevel;
use common::{Order, OrderId, Price, Quantity, Side, Symbol, Trade, TradeId, TradingError, Result};
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;

/// Deterministic orderbook with price-time priority matching
/// 
/// Bid (buy) side: sorted descending by price (highest first)
/// Ask (sell) side: sorted ascending by price (lowest first)
/// 
/// Within each price level, orders are matched by time priority (FIFO)
/// 
/// This implementation is fully deterministic - given the same sequence of operations,
/// it will always produce the same results, enabling replay from event log.
#[derive(Debug)]
pub struct OrderBook {
    symbol: Symbol,
    /// Bid side (buy orders) - sorted descending by price
    /// BTreeMap maintains sorted order, highest price first
    bids: BTreeMap<Price, PriceLevel>,
    /// Ask side (sell orders) - sorted ascending by price
    /// BTreeMap maintains sorted order, lowest price first
    asks: BTreeMap<Price, PriceLevel>,
    /// Fast lookup of orders by ID to their price level
    order_index: std::collections::HashMap<OrderId, (Side, Price)>,
}

impl OrderBook {
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_index: std::collections::HashMap::new(),
        }
    }

    pub fn symbol(&self) -> &Symbol {
        &self.symbol
    }

    /// Add an order to the orderbook
    /// This does NOT perform matching - use match_order for that
    pub fn add_order(&mut self, order: Order) {
        let price = order.price;
        let side = order.side;
        let order_id = order.id;

        let price_levels = match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };

        price_levels
            .entry(price)
            .or_insert_with(PriceLevel::new)
            .add_order(order);

        self.order_index.insert(order_id, (side, price));
    }

    /// Remove an order from the orderbook
    pub fn cancel_order(&mut self, order_id: &OrderId) -> Result<Order> {
        let (side, price) = self.order_index.remove(order_id)
            .ok_or_else(|| TradingError::OrderNotFound(order_id.to_string()))?;

        let price_levels = match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };

        let level = price_levels.get_mut(&price)
            .ok_or_else(|| TradingError::OrderbookError("Price level not found".to_string()))?;

        let order = level.remove_order(order_id)
            .ok_or_else(|| TradingError::OrderNotFound(order_id.to_string()))?;

        // Remove empty price level
        if level.is_empty() {
            price_levels.remove(&price);
        }

        Ok(order)
    }

    /// Get an order by ID
    pub fn get_order(&self, order_id: &OrderId) -> Option<&Order> {
        let (side, price) = self.order_index.get(order_id)?;
        let price_levels = match side {
            Side::Buy => &self.bids,
            Side::Sell => &self.asks,
        };
        price_levels.get(price)?.get_order(order_id)
    }

    /// Match an incoming order against the orderbook
    /// Returns a vector of trades executed and the updated order
    /// This implements price-time priority matching
    pub fn match_order(
        &mut self,
        mut order: Order,
        sequence_number: u64,
        timestamp: DateTime<Utc>,
    ) -> Result<(Vec<Trade>, Order)> {
        let mut trades = Vec::new();
        let mut current_sequence = sequence_number;

        // Keep matching until order is filled or no more matches available
        loop {
            if order.remaining_quantity() <= Quantity::zero() {
                break;
            }

            // Determine which side to match against and get best price
            let (best_price, can_match) = if order.side == Side::Buy {
                // For buy orders, match against lowest ask
                let best_price = self.asks.keys().next().copied();
                let can_match = best_price.map_or(false, |p| order.price >= p);
                (best_price, can_match)
            } else {
                // For sell orders, match against highest bid
                let best_price = self.bids.keys().next_back().copied();
                let can_match = best_price.map_or(false, |p| order.price <= p);
                (best_price, can_match)
            };

            let Some(best_price) = best_price else {
                break; // No more opposing orders
            };

            if !can_match {
                break; // No match possible
            }

            // Get the price level
            let opposing_levels = match order.side {
                Side::Buy => &mut self.asks,
                Side::Sell => &mut self.bids,
            };

            let level = opposing_levels.get_mut(&best_price).unwrap();

            // Match against orders at this price level in time priority order
            while order.remaining_quantity() > Quantity::zero() {
                let Some(resting_order) = level.first_order_mut() else {
                    break;
                };

                let resting_order_id = resting_order.id;
                let resting_order_user_id = resting_order.user_id;
                
                // Calculate match quantity
                let match_qty = std::cmp::min(
                    order.remaining_quantity(),
                    resting_order.remaining_quantity(),
                );

                // Execute trade at the resting order's price (price-time priority)
                let trade_price = best_price;

                // Create trade record
                let (buy_order_id, sell_order_id, buyer_id, seller_id) = match order.side {
                    Side::Buy => (
                        order.id,
                        resting_order_id,
                        order.user_id,
                        resting_order_user_id,
                    ),
                    Side::Sell => (
                        resting_order_id,
                        order.id,
                        resting_order_user_id,
                        order.user_id,
                    ),
                };

                let trade = Trade {
                    id: TradeId::new(),
                    symbol: self.symbol.clone(),
                    price: trade_price,
                    quantity: match_qty,
                    buy_order_id,
                    sell_order_id,
                    buyer_user_id: buyer_id,
                    seller_user_id: seller_id,
                    timestamp,
                    sequence_number: current_sequence,
                };

                trades.push(trade);
                current_sequence += 1;

                // Update filled quantities
                order.filled_quantity = Quantity(order.filled_quantity.0 + match_qty.0);
                level.update_filled_quantity(&resting_order_id, match_qty);

                // If resting order is fully filled, remove it
                if level.first_order().unwrap().is_fully_filled() {
                    let filled_order = level.remove_order(&resting_order_id).unwrap();
                    self.order_index.remove(&filled_order.id);
                }
            }

            // Remove empty price level
            if level.is_empty() {
                let opposing_levels = match order.side {
                    Side::Buy => &mut self.asks,
                    Side::Sell => &mut self.bids,
                };
                opposing_levels.remove(&best_price);
            }
        }

        Ok((trades, order))
    }

    /// Get the best bid price
    pub fn best_bid(&self) -> Option<Price> {
        self.bids.keys().next_back().copied()
    }

    /// Get the best ask price
    pub fn best_ask(&self) -> Option<Price> {
        self.asks.keys().next().copied()
    }

    /// Get market depth for bid side (top N levels)
    pub fn bid_depth(&self, levels: usize) -> Vec<(Price, Quantity)> {
        self.bids
            .iter()
            .rev()
            .take(levels)
            .map(|(price, level)| (*price, level.total_quantity()))
            .collect()
    }

    /// Get market depth for ask side (top N levels)
    pub fn ask_depth(&self, levels: usize) -> Vec<(Price, Quantity)> {
        self.asks
            .iter()
            .take(levels)
            .map(|(price, level)| (*price, level.total_quantity()))
            .collect()
    }

    /// Get the spread (difference between best bid and best ask)
    pub fn spread(&self) -> Option<Price> {
        let best_bid = self.best_bid()?;
        let best_ask = self.best_ask()?;
        Some(best_ask - best_bid)
    }

    /// Get total number of active orders
    pub fn order_count(&self) -> usize {
        self.order_index.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{OrderId, UserId, Symbol, Side, Price, Quantity, OrderType, TimeInForce, OrderStatus};
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use rust_decimal::Decimal;

    fn create_order(
        side: Side,
        price: Decimal,
        quantity: Decimal,
        sequence: u64,
    ) -> Order {
        Order {
            id: OrderId::new(),
            user_id: UserId::new(),
            symbol: Symbol::new("BTC/USD"),
            side,
            order_type: OrderType::Limit,
            price: Price::new(price),
            quantity: Quantity::new(quantity),
            filled_quantity: Quantity::zero(),
            time_in_force: TimeInForce::GTC,
            status: OrderStatus::Open,
            timestamp: Utc::now(),
            sequence_number: sequence,
        }
    }

    #[test]
    fn test_orderbook_add_and_cancel() {
        let mut book = OrderBook::new(Symbol::new("BTC/USD"));
        let order = create_order(Side::Buy, dec!(50000), dec!(1.0), 1);
        let order_id = order.id;

        book.add_order(order);
        assert_eq!(book.order_count(), 1);
        assert_eq!(book.best_bid(), Some(Price::new(dec!(50000))));

        let cancelled = book.cancel_order(&order_id).unwrap();
        assert_eq!(cancelled.id, order_id);
        assert_eq!(book.order_count(), 0);
    }

    #[test]
    fn test_orderbook_matching() {
        let mut book = OrderBook::new(Symbol::new("BTC/USD"));

        // Add resting sell order
        let sell_order = create_order(Side::Sell, dec!(50000), dec!(2.0), 1);
        book.add_order(sell_order);

        // Match with buy order
        let buy_order = create_order(Side::Buy, dec!(50000), dec!(1.5), 2);
        let (trades, remaining_order) = book.match_order(buy_order, 100, Utc::now()).unwrap();

        // Should execute one trade
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, Quantity::new(dec!(1.5)));
        assert_eq!(trades[0].price, Price::new(dec!(50000)));

        // Buy order should be fully filled
        assert_eq!(remaining_order.filled_quantity, Quantity::new(dec!(1.5)));
        assert!(remaining_order.is_fully_filled());

        // Sell order should have 0.5 remaining
        assert_eq!(book.best_ask(), Some(Price::new(dec!(50000))));
        let depth = book.ask_depth(1);
        assert_eq!(depth[0].1, Quantity::new(dec!(0.5)));
    }

    #[test]
    fn test_orderbook_price_time_priority() {
        let mut book = OrderBook::new(Symbol::new("BTC/USD"));

        // Add three sell orders at same price
        let sell1 = create_order(Side::Sell, dec!(50000), dec!(1.0), 1);
        let sell2 = create_order(Side::Sell, dec!(50000), dec!(1.0), 2);
        let sell3 = create_order(Side::Sell, dec!(50000), dec!(1.0), 3);

        let sell1_id = sell1.id;

        book.add_order(sell1);
        book.add_order(sell2);
        book.add_order(sell3);

        // Match with buy order for 1.0
        let buy_order = create_order(Side::Buy, dec!(50000), dec!(1.0), 4);
        let (trades, _) = book.match_order(buy_order, 100, Utc::now()).unwrap();

        // Should match against first order (time priority)
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].sell_order_id, sell1_id);

        // First order should be removed, others remain
        assert_eq!(book.order_count(), 2);
    }

    #[test]
    fn test_orderbook_best_bid_ask() {
        let mut book = OrderBook::new(Symbol::new("BTC/USD"));

        book.add_order(create_order(Side::Buy, dec!(49000), dec!(1.0), 1));
        book.add_order(create_order(Side::Buy, dec!(49500), dec!(1.0), 2));
        book.add_order(create_order(Side::Buy, dec!(49200), dec!(1.0), 3));

        book.add_order(create_order(Side::Sell, dec!(50000), dec!(1.0), 4));
        book.add_order(create_order(Side::Sell, dec!(50500), dec!(1.0), 5));
        book.add_order(create_order(Side::Sell, dec!(50200), dec!(1.0), 6));

        assert_eq!(book.best_bid(), Some(Price::new(dec!(49500))));
        assert_eq!(book.best_ask(), Some(Price::new(dec!(50000))));
        assert_eq!(book.spread(), Some(Price::new(dec!(500))));
    }

    #[test]
    fn test_orderbook_depth() {
        let mut book = OrderBook::new(Symbol::new("BTC/USD"));

        book.add_order(create_order(Side::Buy, dec!(49500), dec!(1.0), 1));
        book.add_order(create_order(Side::Buy, dec!(49500), dec!(0.5), 2));
        book.add_order(create_order(Side::Buy, dec!(49000), dec!(2.0), 3));

        let depth = book.bid_depth(2);
        assert_eq!(depth.len(), 2);
        assert_eq!(depth[0], (Price::new(dec!(49500)), Quantity::new(dec!(1.5))));
        assert_eq!(depth[1], (Price::new(dec!(49000)), Quantity::new(dec!(2.0))));
    }
}
