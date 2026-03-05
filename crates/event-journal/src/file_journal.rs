use crate::events::{EventEnvelope, MatchingEngineEvent};
use crate::journal::EventJournal;
use async_trait::async_trait;
use common::{Result, TradingError};
use std::path::{Path, PathBuf};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;

/// File-based event journal for durable storage
/// Events are written to an append-only file as newline-delimited JSON
/// This provides durability and allows for deterministic replay
pub struct FileJournal {
    path: PathBuf,
    file: RwLock<Option<File>>,
    sequence: RwLock<u64>,
}

impl FileJournal {
    /// Create a new file journal at the specified path
    pub async fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| TradingError::EventJournalError(format!("Failed to create directory: {}", e)))?;
        }

        // Load existing events to determine current sequence
        let sequence = if path.exists() {
            Self::count_events(&path).await?
        } else {
            0
        };

        // Open file in append mode
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|e| TradingError::EventJournalError(format!("Failed to open journal file: {}", e)))?;

        Ok(Self {
            path,
            file: RwLock::new(Some(file)),
            sequence: RwLock::new(sequence),
        })
    }

    async fn count_events(path: &Path) -> Result<u64> {
        let file = File::open(path)
            .await
            .map_err(|e| TradingError::EventJournalError(format!("Failed to open journal file: {}", e)))?;
        
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut count = 0u64;

        while lines.next_line().await.map_err(|e| TradingError::EventJournalError(e.to_string()))?.is_some() {
            count += 1;
        }

        Ok(count)
    }
}

#[async_trait]
impl EventJournal for FileJournal {
    async fn append(&self, event: &MatchingEngineEvent) -> Result<u64> {
        let envelope = EventEnvelope::new(event)
            .map_err(|e| TradingError::EventJournalError(format!("Failed to serialize event: {}", e)))?;

        let json = serde_json::to_string(&envelope)
            .map_err(|e| TradingError::EventJournalError(format!("Failed to serialize envelope: {}", e)))?;

        let mut file_guard = self.file.write().await;
        if let Some(file) = file_guard.as_mut() {
            file.write_all(json.as_bytes())
                .await
                .map_err(|e| TradingError::EventJournalError(format!("Failed to write event: {}", e)))?;
            file.write_all(b"\n")
                .await
                .map_err(|e| TradingError::EventJournalError(format!("Failed to write newline: {}", e)))?;
        }

        let mut seq = self.sequence.write().await;
        let current_seq = *seq;
        *seq += 1;

        Ok(current_seq)
    }

    async fn read_from(&self, sequence_number: u64) -> Result<Vec<MatchingEngineEvent>> {
        let file = File::open(&self.path)
            .await
            .map_err(|e| TradingError::EventJournalError(format!("Failed to open journal file: {}", e)))?;

        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut events = Vec::new();
        let mut current_seq = 0u64;

        while let Some(line) = lines.next_line()
            .await
            .map_err(|e| TradingError::EventJournalError(e.to_string()))? {
            if current_seq >= sequence_number {
                let envelope: EventEnvelope = serde_json::from_str(&line)
                    .map_err(|e| TradingError::EventJournalError(format!("Failed to deserialize envelope: {}", e)))?;
                let event = envelope.into_event()
                    .map_err(|e| TradingError::EventJournalError(format!("Failed to deserialize event: {}", e)))?;
                events.push(event);
            }
            current_seq += 1;
        }

        Ok(events)
    }

    async fn read_range(
        &self,
        start_sequence: u64,
        end_sequence: u64,
    ) -> Result<Vec<MatchingEngineEvent>> {
        let all_events = self.read_from(start_sequence).await?;
        let count = (end_sequence - start_sequence) as usize;
        Ok(all_events.into_iter().take(count).collect())
    }

    async fn current_sequence(&self) -> Result<u64> {
        Ok(*self.sequence.read().await)
    }

    async fn flush(&self) -> Result<()> {
        let mut file_guard = self.file.write().await;
        if let Some(file) = file_guard.as_mut() {
            file.sync_all()
                .await
                .map_err(|e| TradingError::EventJournalError(format!("Failed to flush: {}", e)))?;
        }
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
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_file_journal_persistence() {
        let dir = tempdir().unwrap();
        let journal_path = dir.path().join("events.jsonl");

        // Create journal and write events
        {
            let journal = FileJournal::new(&journal_path).await.unwrap();

            for i in 0..5 {
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

            journal.flush().await.unwrap();
        }

        // Reopen journal and verify events are persisted
        {
            let journal = FileJournal::new(&journal_path).await.unwrap();
            assert_eq!(journal.current_sequence().await.unwrap(), 5);

            let events = journal.read_from(0).await.unwrap();
            assert_eq!(events.len(), 5);
        }
    }

    #[tokio::test]
    async fn test_file_journal_replay() {
        let dir = tempdir().unwrap();
        let journal_path = dir.path().join("events.jsonl");
        let journal = FileJournal::new(&journal_path).await.unwrap();

        let order_id = OrderId::new();
        let event = MatchingEngineEvent::OrderPlaced {
            sequence_number: 0,
            timestamp: Utc::now(),
            order: OrderPlacedData {
                order_id,
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
        journal.flush().await.unwrap();

        let replayed = journal.replay_all().await.unwrap();
        assert_eq!(replayed.len(), 1);

        match &replayed[0] {
            MatchingEngineEvent::OrderPlaced { order, .. } => {
                assert_eq!(order.order_id, order_id);
            }
            _ => panic!("Expected OrderPlaced event"),
        }
    }
}
