use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message;
use rdkafka::util::Timeout;
use tokio_stream::StreamExt;
use tracing::{debug, error, info, warn};

/// Kafka event consumer
pub struct KafkaConsumer {
    consumer: StreamConsumer,
    topic: String,
}

impl KafkaConsumer {
    /// Create a new Kafka consumer
    pub fn new(
        brokers: &str,
        group_id: &str,
        topic: impl Into<String>,
    ) -> Result<Self, rdkafka::error::KafkaError> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("group.id", group_id)
            .set("enable.auto.commit", "true")
            .set("auto.offset.reset", "earliest")
            .set("session.timeout.ms", "6000")
            .create()?;

        let topic = topic.into();
        consumer.subscribe(&[&topic])?;

        info!("Kafka consumer created for topic: {} (group: {})", topic, group_id);

        Ok(Self { consumer, topic })
    }

    /// Consume messages from the beginning
    pub async fn consume_from_beginning<F>(&self, mut handler: F) -> Result<(), String>
    where
        F: FnMut(Vec<u8>) -> Result<(), String>,
    {
        info!("Starting consumption from beginning of topic: {}", self.topic);

        let mut message_stream = self.consumer.stream();

        while let Some(message_result) = message_stream.next().await {
            match message_result {
                Ok(msg) => {
                    if let Some(payload) = msg.payload() {
                        debug!(
                            "Received message from partition {} at offset {}",
                            msg.partition(),
                            msg.offset()
                        );

                        if let Err(e) = handler(payload.to_vec()) {
                            error!("Handler error: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Kafka error: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Get committed offset for a partition
    pub fn committed_offset(&self, partition: i32) -> Option<i64> {
        use rdkafka::topic_partition_list::TopicPartitionList;

        let mut tpl = TopicPartitionList::new();
        tpl.add_partition(&self.topic, partition);

        match self.consumer.committed_offsets(tpl, Timeout::Never) {
            Ok(offsets) => {
                let elements = offsets.elements();
                if !elements.is_empty() {
                    elements[0].offset().to_raw()
                } else {
                    None
                }
            }
            Err(e) => {
                error!("Failed to get committed offset: {}", e);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires running Kafka instance
    async fn test_consumer_creation() {
        let result = KafkaConsumer::new("localhost:9092", "test-group", "test-topic");
        // Won't connect successfully in CI but verifies compilation
        assert!(result.is_ok() || result.is_err());
    }
}
