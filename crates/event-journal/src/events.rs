use chrono::{DateTime, Utc};
use common::{OrderId, Price, Quantity, Side, Symbol, TradeId, UserId, Order, Trade, OrderType, TimeInForce};
use serde::{Deserialize, Serialize};

/// Core event types for the matching engine
/// These events form the event-sourced log that allows deterministic replay
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchingEngineEvent {
    /// An order was placed into the matching engine
    OrderPlaced {
        sequence_number: u64,
        timestamp: DateTime<Utc>,
        order: OrderPlacedData,
    },

    /// An order was cancelled
    OrderCancelled {
        sequence_number: u64,
        timestamp: DateTime<Utc>,
        order_id: OrderId,
        user_id: UserId,
        symbol: Symbol,
        remaining_quantity: Quantity,
    },

    /// A trade was executed (match between buy and sell)
    TradeExecuted {
        sequence_number: u64,
        timestamp: DateTime<Utc>,
        trade: TradeExecutedData,
    },

    /// An order was rejected (failed risk check, validation, etc.)
    OrderRejected {
        sequence_number: u64,
        timestamp: DateTime<Utc>,
        order_id: OrderId,
        user_id: UserId,
        symbol: Symbol,
        reason: String,
    },

    /// An order expired (for GTD orders)
    OrderExpired {
        sequence_number: u64,
        timestamp: DateTime<Utc>,
        order_id: OrderId,
        user_id: UserId,
        symbol: Symbol,
    },
}

impl MatchingEngineEvent {
    pub fn sequence_number(&self) -> u64 {
        match self {
            Self::OrderPlaced { sequence_number, .. } => *sequence_number,
            Self::OrderCancelled { sequence_number, .. } => *sequence_number,
            Self::TradeExecuted { sequence_number, .. } => *sequence_number,
            Self::OrderRejected { sequence_number, .. } => *sequence_number,
            Self::OrderExpired { sequence_number, .. } => *sequence_number,
        }
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::OrderPlaced { timestamp, .. } => *timestamp,
            Self::OrderCancelled { timestamp, .. } => *timestamp,
            Self::TradeExecuted { timestamp, .. } => *timestamp,
            Self::OrderRejected { timestamp, .. } => *timestamp,
            Self::OrderExpired { timestamp, .. } => *timestamp,
        }
    }
}

/// Data for OrderPlaced event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderPlacedData {
    pub order_id: OrderId,
    pub user_id: UserId,
    pub symbol: Symbol,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Price,
    pub quantity: Quantity,
    pub time_in_force: TimeInForce,
}

impl OrderPlacedData {
    pub fn to_order(&self, sequence_number: u64, timestamp: DateTime<Utc>) -> Order {
        Order {
            id: self.order_id,
            user_id: self.user_id,
            symbol: self.symbol.clone(),
            side: self.side,
            order_type: self.order_type,
            price: self.price,
            quantity: self.quantity,
            filled_quantity: Quantity::zero(),
            time_in_force: self.time_in_force,
            status: common::OrderStatus::Open,
            timestamp,
            sequence_number,
        }
    }
}

/// Data for TradeExecuted event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeExecutedData {
    pub trade_id: TradeId,
    pub symbol: Symbol,
    pub price: Price,
    pub quantity: Quantity,
    pub buy_order_id: OrderId,
    pub sell_order_id: OrderId,
    pub buyer_user_id: UserId,
    pub seller_user_id: UserId,
}

impl TradeExecutedData {
    pub fn to_trade(&self, sequence_number: u64, timestamp: DateTime<Utc>) -> Trade {
        Trade {
            id: self.trade_id,
            symbol: self.symbol.clone(),
            price: self.price,
            quantity: self.quantity,
            buy_order_id: self.buy_order_id,
            sell_order_id: self.sell_order_id,
            buyer_user_id: self.buyer_user_id,
            seller_user_id: self.seller_user_id,
            timestamp,
            sequence_number,
        }
    }
}

/// Envelope for serialized events with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub sequence_number: u64,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub payload: Vec<u8>,
}

impl EventEnvelope {
    pub fn new(event: &MatchingEngineEvent) -> Result<Self, bincode::Error> {
        let event_type = match event {
            MatchingEngineEvent::OrderPlaced { .. } => "OrderPlaced",
            MatchingEngineEvent::OrderCancelled { .. } => "OrderCancelled",
            MatchingEngineEvent::TradeExecuted { .. } => "TradeExecuted",
            MatchingEngineEvent::OrderRejected { .. } => "OrderRejected",
            MatchingEngineEvent::OrderExpired { .. } => "OrderExpired",
        }
        .to_string();

        Ok(Self {
            sequence_number: event.sequence_number(),
            timestamp: event.timestamp(),
            event_type,
            payload: bincode::serialize(event)?,
        })
    }

    pub fn into_event(self) -> Result<MatchingEngineEvent, bincode::Error> {
        bincode::deserialize(&self.payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{OrderId, UserId, Symbol, Side, Price, Quantity, OrderType, TimeInForce};
    use rust_decimal_macros::dec;

    #[test]
    fn test_event_serialization() {
        let event = MatchingEngineEvent::OrderPlaced {
            sequence_number: 1,
            timestamp: Utc::now(),
            order: OrderPlacedData {
                order_id: OrderId::new(),
                user_id: UserId::new(),
                symbol: Symbol::new("BTC/USD"),
                side: Side::Buy,
                order_type: OrderType::Limit,
                price: Price::new(dec!(50000.00)),
                quantity: Quantity::new(dec!(1.5)),
                time_in_force: TimeInForce::GTC,
            },
        };

        let envelope = EventEnvelope::new(&event).unwrap();
        let restored = envelope.into_event().unwrap();
        assert_eq!(event, restored);
    }

    #[test]
    fn test_event_sequence_number() {
        let event = MatchingEngineEvent::TradeExecuted {
            sequence_number: 42,
            timestamp: Utc::now(),
            trade: TradeExecutedData {
                trade_id: TradeId::new(),
                symbol: Symbol::new("ETH/USD"),
                price: Price::new(dec!(3000.00)),
                quantity: Quantity::new(dec!(2.0)),
                buy_order_id: OrderId::new(),
                sell_order_id: OrderId::new(),
                buyer_user_id: UserId::new(),
                seller_user_id: UserId::new(),
            },
        };

        assert_eq!(event.sequence_number(), 42);
    }
}
