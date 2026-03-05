//! Kafka Adapter
//!
//! Production-grade event journal backed by Apache Kafka
//!
//! Features:
//! - Distributed, fault-tolerant event storage
//! - High throughput event publishing
//! - Full replay capability
//! - Automatic partitioning and replication

pub mod producer;
pub mod consumer;
pub mod kafka_journal;

pub use producer::KafkaProducer;
pub use consumer::KafkaConsumer;
pub use kafka_journal::KafkaJournal;
