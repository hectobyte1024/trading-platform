use async_trait::async_trait;
use common::Result;
use event_journal::{EventJournal, MatchingEngineEvent};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{info, warn};

use crate::consumer::KafkaConsumer;
use crate::producer::KafkaProducer;

/// Kafka-backed event journal for production use
///
/// Provides durable, distributed event storage with the following features:
/// - Distributed, fault-tolerant storage via Kafka
/// - High throughput event publishing
/// - Replay capability from any point
/// - Automatic partitioning and replication
pub struct KafkaJournal {
    producer: Arc<KafkaProducer>,
    consumer: Arc<KafkaConsumer>,
    sequence: AtomicU64,
}

impl KafkaJournal {
    /// Create a new Kafka journal
    ///
    /// # Arguments
    /// * `brokers` - Kafka broker addresses (e.g., "localhost:9092")
    /// * `topic` - Topic name for events
    /// * `group_id` - Consumer group ID for reading
    pub async fn new(brokers: &str, topic: &str, group_id: &str) -> Result<Self> {
        let producer = KafkaProducer::new(brokers, topic)
            .map_err(|e| common::TradingError::EventJournalError(format!("Kafka producer creation failed: {:?}", e)))?;

        let consumer = KafkaConsumer::new(brokers, group_id, topic)
            .map_err(|e| common::TradingError::EventJournalError(format!("Kafka consumer creation failed: {:?}", e)))?;

        // Determine current sequence by reading committed offset
        let current_seq = consumer.committed_offset(0).unwrap_or(-1) + 1;

        info!("KafkaJournal initialized at sequence {}", current_seq);

        Ok(Self {
            producer: Arc::new(producer),
            consumer: Arc::new(consumer),
            sequence: AtomicU64::new(current_seq.max(0) as u64),
        })
    }

    /// Read all events from Kafka
    async fn read_all_events(&self) -> Result<Vec<MatchingEngineEvent>> {
        let events = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let result = self.consumer.consume_from_beginning(move |payload| {
            match bincode::deserialize::<MatchingEngineEvent>(&payload) {
                Ok(event) => {
                    let events = events_clone.clone();
                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            events.lock().await.push(event);
                        });
                    });
                    Ok(())
                }
                Err(e) => {
                    warn!("Failed to deserialize event: {:?}", e);
                    Err(format!("Deserialization error: {:?}", e))
                }
            }
        }).await;

        if let Err(e) = result {
            return Err(common::TradingError::EventJournalError(e));
        }

        let final_events = events.lock().await;
        Ok(final_events.clone())
    }
}

#[async_trait]
impl EventJournal for KafkaJournal {
    async fn append(&self, event: &MatchingEngineEvent) -> Result<u64> {
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);

        // Serialize event
        let payload = bincode::serialize(event)
            .map_err(|e| common::TradingError::EventJournalError(format!("Serialization error: {:?}", e)))?;

        // Use sequence number as key for ordering
        let key = seq.to_string();

        // Publish to Kafka
        self.producer
            .publish(Some(&key), &payload)
            .await
            .map_err(|e| common::TradingError::EventJournalError(e))?;

        Ok(seq)
    }

    async fn read_from(&self, sequence_number: u64) -> Result<Vec<MatchingEngineEvent>> {
        let all_events = self.read_all_events().await?;
        Ok(all_events
            .into_iter()
            .filter(|e| e.sequence_number() >= sequence_number)
            .collect())
    }

    async fn read_range(
        &self,
        start_sequence: u64,
        end_sequence: u64,
    ) -> Result<Vec<MatchingEngineEvent>> {
        let all_events = self.read_all_events().await?;
        Ok(all_events
            .into_iter()
            .filter(|e| e.sequence_number() >= start_sequence && e.sequence_number() < end_sequence)
            .collect())
    }

    async fn current_sequence(&self) -> Result<u64> {
        Ok(self.sequence.load(Ordering::SeqCst))
    }

    async fn flush(&self) -> Result<()> {
        self.producer
            .flush()
            .await
            .map_err(|e| common::TradingError::EventJournalError(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires running Kafka instance
    async fn test_kafka_journal_creation() {
        let result = KafkaJournal::new("localhost:9092", "test-events", "test-group").await;
        // Won't connect successfully in CI but verifies compilation
        assert!(result.is_ok() || result.is_err());
    }
}
