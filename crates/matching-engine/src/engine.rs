use crate::orderbook::OrderBook;
use common::{Order, OrderId, OrderStatus, OrderType, Price, Quantity, RiskCheck, Result, Symbol, TradingError, UserId};
use event_journal::{
    EventJournal, MatchingEngineEvent, OrderPlacedData, TradeExecutedData,
};
use chrono::Utc;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

/// Production-grade matching engine with event sourcing and risk checks
/// 
/// Features:
/// - Deterministic orderbook matching with price-time priority
/// - Event-sourced architecture for replayability
/// - Pluggable risk checks via trait
/// - Concurrent access via RwLock and DashMap
/// - Full audit trail via event journal
pub struct MatchingEngine<J: EventJournal, R: RiskCheck> {
    /// Orderbooks indexed by symbol
    orderbooks: DashMap<Symbol, Arc<RwLock<OrderBook>>>,
    /// Event journal for durable event log
    journal: Arc<J>,
    /// Risk checker
    risk_check: Arc<R>,
    /// Current sequence number for events
    sequence: Arc<RwLock<u64>>,
}

impl<J: EventJournal, R: RiskCheck> MatchingEngine<J, R> {
    pub fn new(journal: Arc<J>, risk_check: Arc<R>) -> Self {
        Self {
            orderbooks: DashMap::new(),
            journal,
            risk_check,
            sequence: Arc::new(RwLock::new(0)),
        }
    }

    /// Create a new matching engine and replay from event journal
    pub async fn new_with_replay(journal: Arc<J>, risk_check: Arc<R>) -> Result<Self> {
        let engine = Self::new(journal.clone(), risk_check);
        
        info!("Replaying events from journal");
        let events = journal.replay_all().await?;
        
        for event in events {
            engine.apply_event(&event).await?;
        }
        
        // Set sequence to current journal sequence
        let current_seq = journal.current_sequence().await?;
        *engine.sequence.write().await = current_seq;
        
        info!("Replay complete, sequence: {}", current_seq);
        Ok(engine)
    }

    /// Get or create orderbook for a symbol
    async fn get_or_create_orderbook(&self, symbol: &Symbol) -> Arc<RwLock<OrderBook>> {
        self.orderbooks
            .entry(symbol.clone())
            .or_insert_with(|| Arc::new(RwLock::new(OrderBook::new(symbol.clone()))))
            .clone()
    }

    /// Place a new order
    /// This will:
    /// 1. Validate the order
    /// 2. Run risk checks
    /// 3. Emit OrderPlaced event to journal
    /// 4. Attempt to match against orderbook
    /// 5. Emit TradeExecuted events for any matches
    /// 6. Add remaining quantity to orderbook if not fully filled
    pub async fn place_order(&self, mut order: Order) -> Result<()> {
        // Validate order
        self.validate_order(&order)?;

        // Run risk checks
        if let Err(e) = self.risk_check.check_order(&order).await {
            warn!("Order {} failed risk check: {}", order.id, e);
            self.reject_order(order, format!("Risk check failed: {}", e)).await?;
            return Err(e);
        }

        // Get next sequence number
        let seq = self.next_sequence().await;
        let timestamp = Utc::now();
        order.sequence_number = seq;
        order.timestamp = timestamp;

        // Emit OrderPlaced event
        let event = MatchingEngineEvent::OrderPlaced {
            sequence_number: seq,
            timestamp,
            order: OrderPlacedData {
                order_id: order.id,
                user_id: order.user_id,
                symbol: order.symbol.clone(),
                side: order.side,
                order_type: order.order_type,
                price: order.price,
                quantity: order.quantity,
                time_in_force: order.time_in_force,
            },
        };

        self.journal.append(&event).await?;
        info!("Order placed: {} {} {} @ {}", order.id, order.side, order.quantity, order.price);

        // Apply the event (match and update orderbook)
        self.apply_event(&event).await?;

        Ok(())
    }

    /// Cancel an order
    pub async fn cancel_order(&self, order_id: OrderId, user_id: UserId, symbol: Symbol) -> Result<()> {
        // Get orderbook
        let orderbook = self.get_or_create_orderbook(&symbol).await;
        let mut book = orderbook.write().await;

        // Get the order
        let order = book.get_order(&order_id)
            .ok_or_else(|| TradingError::OrderNotFound(order_id.to_string()))?;

        // Verify user owns this order
        if order.user_id != user_id {
            return Err(TradingError::AuthorizationError(
                "User does not own this order".to_string(),
            ));
        }

        let remaining_qty = order.remaining_quantity();

        // Remove from orderbook
        let cancelled_order = book.cancel_order(&order_id)?;
        drop(book); // Release lock

        // Emit OrderCancelled event
        let seq = self.next_sequence().await;
        let timestamp = Utc::now();
        let event = MatchingEngineEvent::OrderCancelled {
            sequence_number: seq,
            timestamp,
            order_id,
            user_id,
            symbol,
            remaining_quantity: remaining_qty,
        };

        self.journal.append(&event).await?;
        info!("Order cancelled: {}", order_id);

        // Notify risk engine
        self.risk_check.on_order_cancelled(&cancelled_order).await?;

        Ok(())
    }

    /// Reject an order (internal)
    async fn reject_order(&self, order: Order, reason: String) -> Result<()> {
        let seq = self.next_sequence().await;
        let timestamp = Utc::now();

        let event = MatchingEngineEvent::OrderRejected {
            sequence_number: seq,
            timestamp,
            order_id: order.id,
            user_id: order.user_id,
            symbol: order.symbol,
            reason: reason.clone(),
        };

        self.journal.append(&event).await?;
        error!("Order rejected: {} - {}", order.id, reason);

        Ok(())
    }

    /// Apply an event to the orderbook (used during replay and live processing)
    async fn apply_event(&self, event: &MatchingEngineEvent) -> Result<()> {
        match event {
            MatchingEngineEvent::OrderPlaced {
                sequence_number,
                timestamp,
                order,
            } => {
                let full_order = order.to_order(*sequence_number, *timestamp);
                self.process_order_placement(full_order, *sequence_number, *timestamp).await?;
            }
            MatchingEngineEvent::OrderCancelled { .. } => {
                // Already handled in cancel_order
            }
            MatchingEngineEvent::TradeExecuted { trade, .. } => {
                let full_trade = trade.to_trade(event.sequence_number(), event.timestamp());
                // Notify risk engine
                self.risk_check.on_trade_executed(&full_trade).await?;
            }
            MatchingEngineEvent::OrderRejected { .. } => {
                // No action needed
            }
            MatchingEngineEvent::OrderExpired { .. } => {
                // TODO: Implement expiry handling
            }
        }
        Ok(())
    }

    /// Process order placement (matching and orderbook update)
    async fn process_order_placement(
        &self,
        order: Order,
        sequence_number: u64,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        let orderbook = self.get_or_create_orderbook(&order.symbol).await;
        let mut book = orderbook.write().await;

        // Attempt to match
        let (trades, mut remaining_order) = book.match_order(order, sequence_number, timestamp)?;

        // Emit TradeExecuted events for each trade
        for trade in trades {
            // Run risk check on trade
            if let Err(e) = self.risk_check.check_trade(&trade).await {
                warn!("Trade {} failed risk check: {}", trade.id, e);
                // In production, you might want to unwind the trade or handle this differently
                return Err(e);
            }

            let event = MatchingEngineEvent::TradeExecuted {
                sequence_number: trade.sequence_number,
                timestamp: trade.timestamp,
                trade: TradeExecutedData {
                    trade_id: trade.id,
                    symbol: trade.symbol.clone(),
                    price: trade.price,
                    quantity: trade.quantity,
                    buy_order_id: trade.buy_order_id,
                    sell_order_id: trade.sell_order_id,
                    buyer_user_id: trade.buyer_user_id,
                    seller_user_id: trade.seller_user_id,
                },
            };

            self.journal.append(&event).await?;
            info!(
                "Trade executed: {} - {} {} @ {}",
                trade.id, trade.symbol, trade.quantity, trade.price
            );

            // Notify risk engine
            self.risk_check.on_trade_executed(&trade).await?;
        }

        // If order has remaining quantity, add to orderbook
        if remaining_order.remaining_quantity() > Quantity::zero() {
            remaining_order.status = if remaining_order.filled_quantity > Quantity::zero() {
                OrderStatus::PartiallyFilled
            } else {
                OrderStatus::Open
            };
            book.add_order(remaining_order);
        } else {
            remaining_order.status = OrderStatus::Filled;
        }

        Ok(())
    }

    /// Validate order before processing
    fn validate_order(&self, order: &Order) -> Result<()> {
        if order.quantity <= Quantity::zero() {
            return Err(TradingError::InvalidQuantity("Quantity must be positive".to_string()));
        }

        // Market orders can have price 0 (will be filled at best available price)
        if order.order_type == OrderType::Limit && order.price <= Price::zero() {
            return Err(TradingError::InvalidPrice("Price must be positive for limit orders".to_string()));
        }

        Ok(())
    }

    /// Get next sequence number
    async fn next_sequence(&self) -> u64 {
        let mut seq = self.sequence.write().await;
        let current = *seq;
        *seq += 1;
        current
    }

    /// Get orderbook for a symbol
    pub async fn get_orderbook(&self, symbol: &Symbol) -> Option<Arc<RwLock<OrderBook>>> {
        self.orderbooks.get(symbol).map(|entry| entry.clone())
    }

    /// Flush journal to disk
    pub async fn flush(&self) -> Result<()> {
        self.journal.flush().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use common::{OrderType, Price, TimeInForce, Trade, Side};
    use event_journal::InMemoryJournal;
    use rust_decimal_macros::dec;

    // Mock risk check that always passes
    struct MockRiskCheck;

    #[async_trait]
    impl RiskCheck for MockRiskCheck {
        async fn check_order(&self, _order: &Order) -> Result<()> {
            Ok(())
        }

        async fn check_trade(&self, _trade: &Trade) -> Result<()> {
            Ok(())
        }

        async fn on_trade_executed(&self, _trade: &Trade) -> Result<()> {
            Ok(())
        }

        async fn on_order_cancelled(&self, _order: &Order) -> Result<()> {
            Ok(())
        }
    }

    fn create_order(side: Side, price: rust_decimal::Decimal, qty: rust_decimal::Decimal) -> Order {
        Order {
            id: OrderId::new(),
            user_id: UserId::new(),
            symbol: Symbol::new("BTC/USD"),
            side,
            order_type: OrderType::Limit,
            price: Price::new(price),
            quantity: Quantity::new(qty),
            filled_quantity: Quantity::zero(),
            time_in_force: TimeInForce::GTC,
            status: OrderStatus::Pending,
            timestamp: Utc::now(),
            sequence_number: 0,
        }
    }

    #[tokio::test]
    async fn test_matching_engine_place_and_match() {
        let journal = Arc::new(InMemoryJournal::new());
        let risk_check = Arc::new(MockRiskCheck);
        let engine = MatchingEngine::new(journal, risk_check);

        // Place sell order
        let sell_order = create_order(Side::Sell, dec!(50000), dec!(2.0));
        engine.place_order(sell_order).await.unwrap();

        // Place buy order that should match
        let buy_order = create_order(Side::Buy, dec!(50000), dec!(1.5));
        engine.place_order(buy_order).await.unwrap();

        // Check orderbook state
        let symbol = Symbol::new("BTC/USD");
        let book = engine.get_orderbook(&symbol).await.unwrap();
        let book = book.read().await;

        // Should have 0.5 remaining on sell side
        let depth = book.ask_depth(1);
        assert_eq!(depth.len(), 1);
        assert_eq!(depth[0].1, Quantity::new(dec!(0.5)));
    }

    #[tokio::test]
    async fn test_matching_engine_replay() {
        let journal = Arc::new(InMemoryJournal::new());
        let risk_check = Arc::new(MockRiskCheck);

        // Create engine and place orders
        {
            let engine = MatchingEngine::new(journal.clone(), risk_check.clone());
            
            let sell = create_order(Side::Sell, dec!(50000), dec!(1.0));
            engine.place_order(sell).await.unwrap();

            let buy = create_order(Side::Buy, dec!(50000), dec!(1.0));
            engine.place_order(buy).await.unwrap();

            engine.flush().await.unwrap();
        }

        // Create new engine and replay
        let engine = MatchingEngine::new_with_replay(journal, risk_check).await.unwrap();

        // Check that state was restored
        let symbol = Symbol::new("BTC/USD");
        let book_opt = engine.get_orderbook(&symbol).await;
        assert!(book_opt.is_some());
    }
}
