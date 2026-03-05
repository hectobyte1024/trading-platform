use crate::events::MatchingEngineEvent;
use async_trait::async_trait;
use common::Result;

/// Trait for event journal - provides append-only event log with replay capability
#[async_trait]
pub trait EventJournal: Send + Sync {
    /// Append an event to the journal
    /// Returns the sequence number assigned to the event
    async fn append(&self, event: &MatchingEngineEvent) -> Result<u64>;

    /// Read events from a starting sequence number
    /// Returns an iterator of events
    async fn read_from(&self, sequence_number: u64) -> Result<Vec<MatchingEngineEvent>>;

    /// Read events in a range [start, end)
    async fn read_range(
        &self,
        start_sequence: u64,
        end_sequence: u64,
    ) -> Result<Vec<MatchingEngineEvent>>;

    /// Get the current sequence number (last written event)
    async fn current_sequence(&self) -> Result<u64>;

    /// Flush any buffered events to durable storage
    async fn flush(&self) -> Result<()>;

    /// Replay all events from the beginning
    async fn replay_all(&self) -> Result<Vec<MatchingEngineEvent>> {
        self.read_from(0).await
    }
}

/// In-memory journal for testing
pub struct InMemoryJournal {
    events: tokio::sync::RwLock<Vec<MatchingEngineEvent>>,
}

impl InMemoryJournal {
    pub fn new() -> Self {
        Self {
            events: tokio::sync::RwLock::new(Vec::new()),
        }
    }
}

impl Default for InMemoryJournal {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventJournal for InMemoryJournal {
    async fn append(&self, event: &MatchingEngineEvent) -> Result<u64> {
        let mut events = self.events.write().await;
        events.push(event.clone());
        Ok(events.len() as u64 - 1)
    }

    async fn read_from(&self, sequence_number: u64) -> Result<Vec<MatchingEngineEvent>> {
        let events = self.events.read().await;
        Ok(events
            .iter()
            .skip(sequence_number as usize)
            .cloned()
            .collect())
    }

    async fn read_range(
        &self,
        start_sequence: u64,
        end_sequence: u64,
    ) -> Result<Vec<MatchingEngineEvent>> {
        let events = self.events.read().await;
        Ok(events
            .iter()
            .skip(start_sequence as usize)
            .take((end_sequence - start_sequence) as usize)
            .cloned()
            .collect())
    }

    async fn current_sequence(&self) -> Result<u64> {
        let events = self.events.read().await;
        Ok(events.len() as u64)
    }

    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{MatchingEngineEvent, OrderPlacedData};
    use chrono::Utc;
    use common::{OrderId, UserId, Symbol, Side, Price, Quantity, OrderType, TimeInForce};
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn test_in_memory_journal() {
        let journal = InMemoryJournal::new();

        let event = MatchingEngineEvent::OrderPlaced {
            sequence_number: 0,
            timestamp: Utc::now(),
            order: OrderPlacedData {
                order_id: OrderId::new(),
                user_id: UserId::new(),
                symbol: Symbol::new("BTC/USD"),
                side: Side::Buy,
                order_type: OrderType::Limit,
                price: Price::new(dec!(50000.00)),
                quantity: Quantity::new(dec!(1.0)),
                time_in_force: TimeInForce::GTC,
            },
        };

        let seq = journal.append(&event).await.unwrap();
        assert_eq!(seq, 0);

        let events = journal.read_from(0).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], event);
    }

    #[tokio::test]
    async fn test_journal_read_range() {
        let journal = InMemoryJournal::new();

        for i in 0..10 {
            let event = MatchingEngineEvent::OrderPlaced {
                sequence_number: i,
                timestamp: Utc::now(),
                order: OrderPlacedData {
                    order_id: OrderId::new(),
                    user_id: UserId::new(),
                    symbol: Symbol::new("BTC/USD"),
                    side: Side::Buy,
                    order_type: OrderType::Limit,
                    price: Price::new(dec!(50000.00)),
                    quantity: Quantity::new(dec!(1.0)),
                    time_in_force: TimeInForce::GTC,
                },
            };
            journal.append(&event).await.unwrap();
        }

        let events = journal.read_range(3, 7).await.unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].sequence_number(), 3);
        assert_eq!(events[3].sequence_number(), 6);
    }
}
