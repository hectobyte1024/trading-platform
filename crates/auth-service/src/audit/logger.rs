//! Audit Logger
//!
//! Kafka-based audit logging for security events.

use crate::audit::events::AuditEvent;
use crate::audit::correlation::{CorrelationId, TraceContext};
use kafka_adapter::KafkaProducer;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

/// Audit logger configuration
#[derive(Debug, Clone)]
pub struct AuditLoggerConfig {
    /// Kafka broker addresses
    pub kafka_brokers: String,
    /// Kafka topic for audit events
    pub topic: String,
    /// Enable synchronous logging (wait for Kafka ack)
    pub synchronous: bool,
    /// Buffer size for async logging
    pub buffer_size: usize,
}

impl Default for AuditLoggerConfig {
    fn default() -> Self {
        Self {
            kafka_brokers: "localhost:9092".to_string(),
            topic: "auth-audit-events".to_string(),
            synchronous: false,
            buffer_size: 1000,
        }
    }
}

/// Audit logger
#[derive(Clone)]
pub struct AuditLogger {
    producer: Arc<RwLock<Option<KafkaProducer>>>,
    config: AuditLoggerConfig,
    fallback_enabled: bool,
}

impl AuditLogger {
    /// Create a new audit logger
    pub fn new(config: AuditLoggerConfig) -> Self {
        Self {
            producer: Arc::new(RwLock::new(None)),
            config,
            fallback_enabled: true,
        }
    }

    /// Initialize Kafka producer
    pub async fn initialize(&self) -> Result<(), String> {
        match KafkaProducer::new(&self.config.kafka_brokers, &self.config.topic) {
            Ok(producer) => {
                let mut prod = self.producer.write().await;
                *prod = Some(producer);
                debug!("Audit logger initialized with Kafka");
                Ok(())
            }
            Err(e) => {
                error!("Failed to initialize Kafka producer: {:?}", e);
                if self.fallback_enabled {
                    warn!("Audit logger will use fallback (local logging only)");
                    Ok(())
                } else {
                    Err(format!("Kafka initialization failed: {:?}", e))
                }
            }
        }
    }

    /// Log an audit event
    pub async fn log(&self, event: AuditEvent) -> Result<(), String> {
        // Serialize event to JSON
        let json = event
            .to_json()
            .map_err(|e| format!("Event serialization failed: {}", e))?;

        // Try to send to Kafka
        let producer_guard = self.producer.read().await;
        if let Some(producer) = producer_guard.as_ref() {
            let key = event.event_id.to_string();

            if self.config.synchronous {
                // Synchronous: wait for Kafka acknowledgment
                producer.publish(Some(&key), json.as_bytes()).await?;
                debug!("Audit event {} logged to Kafka (sync)", event.event_id);
            } else {
                // Asynchronous: fire and forget
                match producer.publish(Some(&key), json.as_bytes()).await {
                    Ok(_) => {
                        debug!("Audit event {} logged to Kafka (async)", event.event_id);
                    }
                    Err(e) => {
                        error!("Kafka publish failed: {}, falling back to local log", e);
                        self.fallback_log(&event);
                    }
                }
            }
        } else {
            // Kafka not available, use fallback
            self.fallback_log(&event);
        }

        Ok(())
    }

    /// Log multiple events in batch
    pub async fn log_batch(&self, events: Vec<AuditEvent>) -> Result<(), String> {
        for event in events {
            self.log(event).await?;
        }
        Ok(())
    }

    /// Flush pending events
    pub async fn flush(&self) -> Result<(), String> {
        let producer_guard = self.producer.read().await;
        if let Some(producer) = producer_guard.as_ref() {
            producer.flush().await?;
            debug!("Audit logger flushed");
        }
        Ok(())
    }

    /// Fallback logging (to structured logs)
    fn fallback_log(&self, event: &AuditEvent) {
        match event.to_json_pretty() {
            Ok(json) => {
                tracing::warn!(
                    event_id = %event.event_id,
                    category = ?event.category,
                    severity = ?event.severity,
                    "AUDIT EVENT (fallback): {}",
                    json
                );
            }
            Err(e) => {
                error!("Failed to serialize audit event for fallback: {}", e);
            }
        }
    }

    /// Shutdown the audit logger
    pub async fn shutdown(&self) -> Result<(), String> {
        self.flush().await?;
        debug!("Audit logger shut down");
        Ok(())
    }
}

/// Audit logger builder with fluent API
pub struct AuditLoggerBuilder {
    config: AuditLoggerConfig,
    correlation_id: Option<CorrelationId>,
    trace_context: Option<TraceContext>,
}

impl AuditLoggerBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: AuditLoggerConfig::default(),
            correlation_id: None,
            trace_context: None,
        }
    }

    /// Set Kafka brokers
    pub fn with_kafka_brokers(mut self, brokers: impl Into<String>) -> Self {
        self.config.kafka_brokers = brokers.into();
        self
    }

    /// Set topic
    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.config.topic = topic.into();
        self
    }

    /// Enable synchronous logging
    pub fn synchronous(mut self) -> Self {
        self.config.synchronous = true;
        self
    }

    /// Set buffer size
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_size = size;
        self
    }

    /// Build the audit logger
    pub fn build(self) -> AuditLogger {
        AuditLogger::new(self.config)
    }
}

impl Default for AuditLoggerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::events::*;
    use crate::domain::{UserId, UserDomain};

    #[test]
    fn test_logger_config_default() {
        let config = AuditLoggerConfig::default();
        assert_eq!(config.kafka_brokers, "localhost:9092");
        assert_eq!(config.topic, "auth-audit-events");
        assert!(!config.synchronous);
    }

    #[test]
    fn test_logger_builder() {
        let logger = AuditLoggerBuilder::new()
            .with_kafka_brokers("kafka:9092")
            .with_topic("custom-audit")
            .synchronous()
            .with_buffer_size(500)
            .build();

        assert_eq!(logger.config.kafka_brokers, "kafka:9092");
        assert_eq!(logger.config.topic, "custom-audit");
        assert!(logger.config.synchronous);
        assert_eq!(logger.config.buffer_size, 500);
    }

    #[tokio::test]
    async fn test_logger_initialization() {
        let logger = AuditLogger::new(AuditLoggerConfig::default());

        // Will fail without Kafka but should fallback gracefully
        let result = logger.initialize().await;
        assert!(result.is_ok()); // Fallback enabled
    }

    #[tokio::test]
    async fn test_log_event_without_kafka() {
        let logger = AuditLogger::new(AuditLoggerConfig::default());
        logger.initialize().await.unwrap();

        let event = AuditEvent::new(
            EventCategory::Authentication,
            Severity::Info,
            "test_event",
            EventData::Authentication(AuthenticationEvent::TokenValidation {
                token_type: "access".to_string(),
                validation_result: true,
                reason: None,
            }),
            EventOutcome::Success,
        );

        // Should use fallback logging
        let result = logger.log(event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_log_batch() {
        let logger = AuditLogger::new(AuditLoggerConfig::default());
        logger.initialize().await.unwrap();

        let events = vec![
            AuditEvent::new(
                EventCategory::Authentication,
                Severity::Info,
                "event1",
                EventData::Authentication(AuthenticationEvent::Logout {
                    reason: "user_logout".to_string(),
                }),
                EventOutcome::Success,
            ),
            AuditEvent::new(
                EventCategory::Authorization,
                Severity::Medium,
                "event2",
                EventData::Authorization(AuthorizationEvent::AccessDenied {
                    resource_type: crate::authz::ResourceType::Order,
                    resource_id: None,
                    action: crate::authz::Action::Delete,
                    reason: "unauthorized".to_string(),
                }),
                EventOutcome::Failure,
            ),
        ];

        let result = logger.log_batch(events).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_flush() {
        let logger = AuditLogger::new(AuditLoggerConfig::default());
        logger.initialize().await.unwrap();

        let result = logger.flush().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_shutdown() {
        let logger = AuditLogger::new(AuditLoggerConfig::default());
        logger.initialize().await.unwrap();

        let result = logger.shutdown().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_event_serialization_for_kafka() {
        let event = AuditEvent::new(
            EventCategory::Security,
            Severity::Critical,
            "security_incident",
            EventData::Security(SecurityEvent::BruteForceAttempt {
                target_resource: "login".to_string(),
                attempt_count: 10,
                time_window_seconds: 60,
            }),
            EventOutcome::Failure,
        )
        .with_user(UserId::new(), UserDomain::Retail);

        let json = event.to_json().unwrap();
        assert!(json.contains("security_incident"));
        assert!(json.contains("brute_force_attempt"));

        // Verify it can be deserialized
        let _deserialized: AuditEvent = serde_json::from_str(&json).unwrap();
    }
}
