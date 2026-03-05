//! Audit Logging
//!
//! Comprehensive security event logging for compliance and monitoring.
//!
//! Features:
//! - SIEM-compatible event schemas
//! - Kafka-based event streaming
//! - Event correlation and tracing
//! - Authentication and authorization auditing
//! - Session lifecycle tracking
//! - Security incident logging

pub mod events;
pub mod logger;
pub mod correlation;
pub mod middleware;

pub use events::*;
pub use logger::{AuditLogger, AuditLoggerConfig};
pub use correlation::{CorrelationId, TraceContext};
pub use middleware::AuditMiddleware;
