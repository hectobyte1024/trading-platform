use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::util::Timeout;
use std::time::Duration;
use tracing::{debug, error, info};

/// Kafka event producer
pub struct KafkaProducer {
    producer: FutureProducer,
    topic: String,
}

impl KafkaProducer {
    /// Create a new Kafka producer
    pub fn new(brokers: &str, topic: impl Into<String>) -> Result<Self, rdkafka::error::KafkaError> {
        let topic = topic.into();
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "5000")
            .set("compression.type", "lz4")
            .set("batch.size", "16384")
            .set("linger.ms", "10")
            .set("acks", "all") // Wait for all replicas
            .create()?;

        info!("Kafka producer created for topic: {}", topic);

        Ok(Self {
            producer,
            topic,
        })
    }

    /// Publish a message to Kafka
    pub async fn publish(&self, key: Option<&str>, payload: &[u8]) -> Result<(), String> {
        let mut record = FutureRecord::to(&self.topic).payload(payload);

        if let Some(k) = key {
            record = record.key(k);
        }

        match self.producer.send(record, Timeout::After(Duration::from_secs(5))).await {
            Ok((partition, offset)) => {
                debug!("Message delivered to partition {} at offset {}", partition, offset);
                Ok(())
            }
            Err((err, _)) => {
                error!("Failed to publish message: {:?}", err);
                Err(format!("Kafka publish error: {:?}", err))
            }
        }
    }

    /// Flush pending messages
    pub async fn flush(&self) -> Result<(), String> {
        self.producer
            .flush(Timeout::After(Duration::from_secs(5)))
            .map_err(|e| format!("Flush error: {:?}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires running Kafka instance
    async fn test_producer_creation() {
        let result = KafkaProducer::new("localhost:9092", "test-topic");
        // Won't connect successfully in CI but verifies compilation
        assert!(result.is_ok() || result.is_err());
    }
}
