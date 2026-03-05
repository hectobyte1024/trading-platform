//! Event Correlation and Tracing
//!
//! Support for correlating related audit events and distributed tracing.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Correlation ID for grouping related events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(pub Uuid);

impl CorrelationId {
    /// Create a new correlation ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Get inner UUID
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Parse from string
    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Trace context for distributed tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    /// Trace ID
    pub trace_id: Uuid,
    /// Parent span ID (if any)
    pub parent_span_id: Option<Uuid>,
    /// Current span ID
    pub span_id: Uuid,
    /// Sampling decision
    pub sampled: bool,
}

impl TraceContext {
    /// Create a new root trace context
    pub fn new() -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            parent_span_id: None,
            span_id: Uuid::new_v4(),
            sampled: true,
        }
    }

    /// Create a child span
    pub fn child_span(&self) -> Self {
        Self {
            trace_id: self.trace_id,
            parent_span_id: Some(self.span_id),
            span_id: Uuid::new_v4(),
            sampled: self.sampled,
        }
    }

    /// Parse from W3C traceparent header
    pub fn from_traceparent(header: &str) -> Option<Self> {
        // Format: 00-{trace_id}-{parent_id}-{flags}
        let parts: Vec<&str> = header.split('-').collect();
        if parts.len() != 4 || parts[0] != "00" {
            return None;
        }

        // Parse trace_id (32 hex chars without dashes)
        let trace_id = Uuid::parse_str(&format!(
            "{}-{}-{}-{}-{}",
            &parts[1][0..8],
            &parts[1][8..12],
            &parts[1][12..16],
            &parts[1][16..20],
            &parts[1][20..32]
        ))
        .ok()?;

        // Parse parent_span_id (32 hex chars without dashes)
        let parent_span_id = Uuid::parse_str(&format!(
            "{}-{}-{}-{}-{}",
            &parts[2][0..8],
            &parts[2][8..12],
            &parts[2][12..16],
            &parts[2][16..20],
            &parts[2][20..32]
        ))
        .ok()?;

        let flags = u8::from_str_radix(parts[3], 16).ok()?;
        let sampled = (flags & 0x01) == 0x01;

        Some(Self {
            trace_id,
            parent_span_id: Some(parent_span_id),
            span_id: Uuid::new_v4(),
            sampled,
        })
    }

    /// Convert to W3C traceparent header
    pub fn to_traceparent(&self) -> String {
        let flags = if self.sampled { "01" } else { "00" };
        let parent_id = self.parent_span_id.unwrap_or_else(Uuid::nil);
        
        // Format UUIDs without dashes for W3C traceparent
        let trace_str = self.trace_id.simple().to_string();
        let parent_str = parent_id.simple().to_string();
        
        format!("00-{}-{}-{}", trace_str, parent_str, flags)
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Correlation context manager
#[derive(Debug, Clone)]
pub struct CorrelationContext {
    inner: Arc<RwLock<CorrelationContextInner>>,
}

#[derive(Debug)]
struct CorrelationContextInner {
    correlation_id: Option<CorrelationId>,
    trace_context: Option<TraceContext>,
}

impl CorrelationContext {
    /// Create a new correlation context
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(CorrelationContextInner {
                correlation_id: None,
                trace_context: None,
            })),
        }
    }

    /// Set correlation ID
    pub async fn set_correlation_id(&self, id: CorrelationId) {
        let mut inner = self.inner.write().await;
        inner.correlation_id = Some(id);
    }

    /// Get correlation ID
    pub async fn get_correlation_id(&self) -> Option<CorrelationId> {
        let inner = self.inner.read().await;
        inner.correlation_id
    }

    /// Get or create correlation ID
    pub async fn get_or_create_correlation_id(&self) -> CorrelationId {
        {
            let inner = self.inner.read().await;
            if let Some(id) = inner.correlation_id {
                return id;
            }
        }

        let new_id = CorrelationId::new();
        self.set_correlation_id(new_id).await;
        new_id
    }

    /// Set trace context
    pub async fn set_trace_context(&self, ctx: TraceContext) {
        let mut inner = self.inner.write().await;
        inner.trace_context = Some(ctx);
    }

    /// Get trace context
    pub async fn get_trace_context(&self) -> Option<TraceContext> {
        let inner = self.inner.read().await;
        inner.trace_context.clone()
    }

    /// Get or create trace context
    pub async fn get_or_create_trace_context(&self) -> TraceContext {
        {
            let inner = self.inner.read().await;
            if let Some(ctx) = &inner.trace_context {
                return ctx.clone();
            }
        }

        let new_ctx = TraceContext::new();
        self.set_trace_context(new_ctx.clone()).await;
        new_ctx
    }

    /// Clear context
    pub async fn clear(&self) {
        let mut inner = self.inner.write().await;
        inner.correlation_id = None;
        inner.trace_context = None;
    }
}

impl Default for CorrelationContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correlation_id() {
        let id1 = CorrelationId::new();
        let id2 = CorrelationId::new();

        assert_ne!(id1, id2);
        assert_eq!(id1, id1);

        let uuid_str = id1.to_string();
        let parsed = CorrelationId::parse(&uuid_str).unwrap();
        assert_eq!(id1, parsed);
    }

    #[test]
    fn test_trace_context_creation() {
        let ctx = TraceContext::new();

        assert!(ctx.parent_span_id.is_none());
        assert!(ctx.sampled);
        assert_ne!(ctx.trace_id, Uuid::nil());
        assert_ne!(ctx.span_id, Uuid::nil());
    }

    #[test]
    fn test_trace_context_child() {
        let parent = TraceContext::new();
        let child = parent.child_span();

        assert_eq!(child.trace_id, parent.trace_id);
        assert_eq!(child.parent_span_id, Some(parent.span_id));
        assert_ne!(child.span_id, parent.span_id);
        assert_eq!(child.sampled, parent.sampled);
    }

    #[test]
    fn test_traceparent_serialization() {
        let ctx = TraceContext::new();
        let header = ctx.to_traceparent();

        assert!(header.starts_with("00-"));
        // Traceparent uses simple UUID format (no dashes)
        assert!(header.contains(&ctx.trace_id.simple().to_string()));
    }

    #[test]
    fn test_traceparent_parsing() {
        let original = TraceContext::new();
        let header = original.to_traceparent();

        let parsed = TraceContext::from_traceparent(&header).unwrap();

        assert_eq!(parsed.trace_id, original.trace_id);
        assert_eq!(parsed.sampled, original.sampled);
    }

    #[tokio::test]
    async fn test_correlation_context() {
        let ctx = CorrelationContext::new();

        assert!(ctx.get_correlation_id().await.is_none());

        let id = CorrelationId::new();
        ctx.set_correlation_id(id).await;

        assert_eq!(ctx.get_correlation_id().await, Some(id));
    }

    #[tokio::test]
    async fn test_correlation_context_get_or_create() {
        let ctx = CorrelationContext::new();

        let id1 = ctx.get_or_create_correlation_id().await;
        let id2 = ctx.get_or_create_correlation_id().await;

        assert_eq!(id1, id2);
    }

    #[tokio::test]
    async fn test_trace_context_management() {
        let ctx = CorrelationContext::new();

        let trace = TraceContext::new();
        ctx.set_trace_context(trace.clone()).await;

        let retrieved = ctx.get_trace_context().await.unwrap();
        assert_eq!(retrieved.trace_id, trace.trace_id);
    }

    #[tokio::test]
    async fn test_context_clear() {
        let ctx = CorrelationContext::new();

        ctx.set_correlation_id(CorrelationId::new()).await;
        ctx.set_trace_context(TraceContext::new()).await;

        ctx.clear().await;

        assert!(ctx.get_correlation_id().await.is_none());
        assert!(ctx.get_trace_context().await.is_none());
    }
}
