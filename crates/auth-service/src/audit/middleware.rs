//! Audit Middleware
//!
//! Automatic audit logging for API requests and authentication events.

use crate::audit::events::*;
use crate::audit::logger::AuditLogger;
use crate::audit::correlation::{CorrelationId, TraceContext, CorrelationContext};
use crate::domain::{UserId, SessionId, UserDomain, AccessClaims};
use std::net::IpAddr;
use std::sync::Arc;
use tracing::error;

/// Audit middleware for automatic event logging
#[derive(Clone)]
pub struct AuditMiddleware {
    logger: Arc<AuditLogger>,
    correlation_context: CorrelationContext,
}

impl AuditMiddleware {
    /// Create new audit middleware
    pub fn new(logger: Arc<AuditLogger>) -> Self {
        Self {
            logger,
            correlation_context: CorrelationContext::new(),
        }
    }

    /// Set correlation ID
    pub async fn set_correlation_id(&self, id: CorrelationId) {
        self.correlation_context.set_correlation_id(id).await;
    }

    /// Set trace context
    pub async fn set_trace_context(&self, ctx: TraceContext) {
        self.correlation_context.set_trace_context(ctx).await;
    }

    /// Get or create correlation ID
    pub async fn get_correlation_id(&self) -> CorrelationId {
        self.correlation_context.get_or_create_correlation_id().await
    }

    /// Get or create trace context
    pub async fn get_trace_context(&self) -> TraceContext {
        self.correlation_context.get_or_create_trace_context().await
    }

    /// Log authentication event
    pub async fn log_authentication(
        &self,
        event_type: impl Into<String>,
        auth_event: AuthenticationEvent,
        outcome: EventOutcome,
        user_id: Option<UserId>,
        domain: Option<UserDomain>,
        session_id: Option<SessionId>,
        ip: Option<IpAddr>,
        user_agent: Option<String>,
        error: Option<String>,
    ) {
        let severity = match outcome {
            EventOutcome::Success => Severity::Info,
            EventOutcome::Failure => Severity::Medium,
            EventOutcome::Partial => Severity::Low,
        };

        let mut event = AuditEvent::new(
            EventCategory::Authentication,
            severity,
            event_type,
            EventData::Authentication(auth_event),
            outcome,
        );

        // Add context
        if let Some(uid) = user_id {
            if let Some(d) = domain {
                event = event.with_user(uid, d);
            }
        }

        if let Some(sid) = session_id {
            event = event.with_session(sid);
        }

        if let Some(ip_addr) = ip {
            event = event.with_ip(ip_addr);
        }

        if let Some(ua) = user_agent {
            event = event.with_user_agent(ua);
        }

        if let Some(err) = error {
            event = event.with_error(err);
        }

        // Add correlation/trace IDs
        if let Some(corr_id) = self.correlation_context.get_correlation_id().await {
            event = event.with_correlation_id(corr_id.as_uuid());
        }

        if let Some(trace) = self.correlation_context.get_trace_context().await {
            event = event.with_trace_id(trace.trace_id);
        }

        // Log event
        if let Err(e) = self.logger.log(event).await {
            error!("Failed to log authentication event: {}", e);
        }
    }

    /// Log authorization event
    pub async fn log_authorization(
        &self,
        event_type: impl Into<String>,
        authz_event: AuthorizationEvent,
        outcome: EventOutcome,
        claims: Option<&AccessClaims>,
        ip: Option<IpAddr>,
        user_agent: Option<String>,
    ) {
        let severity = match outcome {
            EventOutcome::Success => Severity::Info,
            EventOutcome::Failure => Severity::Medium,
            EventOutcome::Partial => Severity::Low,
        };

        let mut event = AuditEvent::new(
            EventCategory::Authorization,
            severity,
            event_type,
            EventData::Authorization(authz_event),
            outcome,
        );

        // Extract user context from claims
        if let Some(claims) = claims {
            if let Ok(user_id) = uuid::Uuid::parse_str(&claims.standard.sub) {
                event = event.with_user(UserId(user_id), claims.domain);
            }

            if let Ok(session_uuid) = uuid::Uuid::parse_str(&claims.session_id) {
                event = event.with_session(SessionId(session_uuid));
            }
        }

        if let Some(ip_addr) = ip {
            event = event.with_ip(ip_addr);
        }

        if let Some(ua) = user_agent {
            event = event.with_user_agent(ua);
        }

        // Add correlation/trace IDs
        if let Some(corr_id) = self.correlation_context.get_correlation_id().await {
            event = event.with_correlation_id(corr_id.as_uuid());
        }

        if let Some(trace) = self.correlation_context.get_trace_context().await {
            event = event.with_trace_id(trace.trace_id);
        }

        // Log event
        if let Err(e) = self.logger.log(event).await {
            error!("Failed to log authorization event: {}", e);
        }
    }

    /// Log session event
    pub async fn log_session(
        &self,
        event_type: impl Into<String>,
        session_event: SessionEvent,
        outcome: EventOutcome,
        user_id: UserId,
        domain: UserDomain,
        session_id: SessionId,
        ip: Option<IpAddr>,
    ) {
        let severity = match outcome {
            EventOutcome::Success => Severity::Info,
            EventOutcome::Failure => Severity::Medium,
            EventOutcome::Partial => Severity::Low,
        };

        let mut event = AuditEvent::new(
            EventCategory::Session,
            severity,
            event_type,
            EventData::Session(session_event),
            outcome,
        )
        .with_user(user_id, domain)
        .with_session(session_id);

        if let Some(ip_addr) = ip {
            event = event.with_ip(ip_addr);
        }

        // Add correlation/trace IDs
        if let Some(corr_id) = self.correlation_context.get_correlation_id().await {
            event = event.with_correlation_id(corr_id.as_uuid());
        }

        if let Some(trace) = self.correlation_context.get_trace_context().await {
            event = event.with_trace_id(trace.trace_id);
        }

        // Log event
        if let Err(e) = self.logger.log(event).await {
            error!("Failed to log session event: {}", e);
        }
    }

    /// Log security event
    pub async fn log_security(
        &self,
        event_type: impl Into<String>,
        security_event: SecurityEvent,
        severity: Severity,
        user_id: Option<UserId>,
        domain: Option<UserDomain>,
        session_id: Option<SessionId>,
        ip: Option<IpAddr>,
        user_agent: Option<String>,
    ) {
        let mut event = AuditEvent::new(
            EventCategory::Security,
            severity,
            event_type,
            EventData::Security(security_event),
            EventOutcome::Failure,
        );

        if let Some(uid) = user_id {
            if let Some(d) = domain {
                event = event.with_user(uid, d);
            }
        }

        if let Some(sid) = session_id {
            event = event.with_session(sid);
        }

        if let Some(ip_addr) = ip {
            event = event.with_ip(ip_addr);
        }

        if let Some(ua) = user_agent {
            event = event.with_user_agent(ua);
        }

        // Add correlation/trace IDs
        if let Some(corr_id) = self.correlation_context.get_correlation_id().await {
            event = event.with_correlation_id(corr_id.as_uuid());
        }

        if let Some(trace) = self.correlation_context.get_trace_context().await {
            event = event.with_trace_id(trace.trace_id);
        }

        // Log event
        if let Err(e) = self.logger.log(event).await {
            error!("Failed to log security event: {}", e);
        }
    }

    /// Log admin event
    pub async fn log_admin(
        &self,
        event_type: impl Into<String>,
        admin_event: AdminEvent,
        outcome: EventOutcome,
        admin_user_id: UserId,
        domain: UserDomain,
        ip: Option<IpAddr>,
    ) {
        let mut event = AuditEvent::new(
            EventCategory::Admin,
            Severity::Medium,
            event_type,
            EventData::Admin(admin_event),
            outcome,
        )
        .with_user(admin_user_id, domain);

        if let Some(ip_addr) = ip {
            event = event.with_ip(ip_addr);
        }

        // Add correlation/trace IDs
        if let Some(corr_id) = self.correlation_context.get_correlation_id().await {
            event = event.with_correlation_id(corr_id.as_uuid());
        }

        if let Some(trace) = self.correlation_context.get_trace_context().await {
            event = event.with_trace_id(trace.trace_id);
        }

        // Log event
        if let Err(e) = self.logger.log(event).await {
            error!("Failed to log admin event: {}", e);
        }
    }

    /// Log compliance event
    pub async fn log_compliance(
        &self,
        event_type: impl Into<String>,
        compliance_event: ComplianceEvent,
        user_id: UserId,
        domain: UserDomain,
        ip: Option<IpAddr>,
    ) {
        let mut event = AuditEvent::new(
            EventCategory::Compliance,
            Severity::Info,
            event_type,
            EventData::Compliance(compliance_event),
            EventOutcome::Success,
        )
        .with_user(user_id, domain);

        if let Some(ip_addr) = ip {
            event = event.with_ip(ip_addr);
        }

        // Add correlation/trace IDs
        if let Some(corr_id) = self.correlation_context.get_correlation_id().await {
            event = event.with_correlation_id(corr_id.as_uuid());
        }

        if let Some(trace) = self.correlation_context.get_trace_context().await {
            event = event.with_trace_id(trace.trace_id);
        }

        // Log event
        if let Err(e) = self.logger.log(event).await {
            error!("Failed to log compliance event: {}", e);
        }
    }

    /// Clear correlation context
    pub async fn clear_context(&self) {
        self.correlation_context.clear().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::logger::{AuditLogger, AuditLoggerConfig};
    use crate::domain::StandardClaims;
    use chrono::Utc;

    #[tokio::test]
    async fn test_middleware_creation() {
        let logger = Arc::new(AuditLogger::new(AuditLoggerConfig::default()));
        let middleware = AuditMiddleware::new(logger);

        let corr_id = middleware.get_correlation_id().await;
        assert_ne!(corr_id.as_uuid(), uuid::Uuid::nil());
    }

    #[tokio::test]
    async fn test_log_authentication_event() {
        let logger = Arc::new(AuditLogger::new(AuditLoggerConfig::default()));
        logger.initialize().await.unwrap();
        let middleware = AuditMiddleware::new(logger);

        let user_id = UserId::new();
        let session_id = SessionId(uuid::Uuid::new_v4());
        let ip = "192.168.1.1".parse().unwrap();

        middleware
            .log_authentication(
                "webauthn_auth",
                AuthenticationEvent::WebAuthnAuthentication {
                    credential_id: "cred_123".to_string(),
                    user_verified: true,
                    counter: 1,
                },
                EventOutcome::Success,
                Some(user_id),
                Some(UserDomain::Retail),
                Some(session_id),
                Some(ip),
                Some("Mozilla/5.0".to_string()),
                None,
            )
            .await;

        // Should complete without error
    }

    #[tokio::test]
    async fn test_log_authorization_event() {
        let logger = Arc::new(AuditLogger::new(AuditLoggerConfig::default()));
        logger.initialize().await.unwrap();
        let middleware = AuditMiddleware::new(logger);

        middleware
            .log_authorization(
                "permission_check",
                AuthorizationEvent::PermissionCheck {
                    resource_type: crate::authz::ResourceType::Order,
                    resource_id: Some("order_123".to_string()),
                    action: crate::authz::Action::Create,
                    decision: "allow".to_string(),
                    policy_evaluated: "rbac".to_string(),
                },
                EventOutcome::Success,
                None,
                None,
                None,
            )
            .await;

        // Should complete without error
    }

    #[tokio::test]
    async fn test_log_session_event() {
        let logger = Arc::new(AuditLogger::new(AuditLoggerConfig::default()));
        logger.initialize().await.unwrap();
        let middleware = AuditMiddleware::new(logger);

        let user_id = UserId::new();
        let session_id = SessionId(uuid::Uuid::new_v4());

        middleware
            .log_session(
                "session_created",
                SessionEvent::Created {
                    session_type: "access_token".to_string(),
                    ttl_seconds: 3600,
                },
                EventOutcome::Success,
                user_id,
                UserDomain::Institutional,
                session_id,
                None,
            )
            .await;

        // Should complete without error
    }

    #[tokio::test]
    async fn test_log_security_event() {
        let logger = Arc::new(AuditLogger::new(AuditLoggerConfig::default()));
        logger.initialize().await.unwrap();
        let middleware = AuditMiddleware::new(logger);

        middleware
            .log_security(
                "rate_limit_exceeded",
                SecurityEvent::RateLimitExceeded {
                    limit_type: "login_attempts".to_string(),
                    limit: 5,
                    actual: 10,
                    window_seconds: 300,
                },
                Severity::High,
                None,
                None,
                None,
                Some("10.0.0.1".parse().unwrap()),
                None,
            )
            .await;

        // Should complete without error
    }

    #[tokio::test]
    async fn test_log_admin_event() {
        let logger = Arc::new(AuditLogger::new(AuditLoggerConfig::default()));
        logger.initialize().await.unwrap();
        let middleware = AuditMiddleware::new(logger);

        let admin_id = UserId::new();
        let target_id = UserId::new();

        middleware
            .log_admin(
                "user_suspended",
                AdminEvent::UserSuspended {
                    target_user_id: target_id,
                    reason: "policy_violation".to_string(),
                    duration_seconds: Some(86400),
                },
                EventOutcome::Success,
                admin_id,
                UserDomain::Admin,
                None,
            )
            .await;

        // Should complete without error
    }

    #[tokio::test]
    async fn test_log_compliance_event() {
        let logger = Arc::new(AuditLogger::new(AuditLoggerConfig::default()));
        logger.initialize().await.unwrap();
        let middleware = AuditMiddleware::new(logger);

        let user_id = UserId::new();

        middleware
            .log_compliance(
                "data_export",
                ComplianceEvent::DataExport {
                    data_type: "user_orders".to_string(),
                    record_count: 150,
                    destination: "s3://compliance-exports/".to_string(),
                },
                user_id,
                UserDomain::Compliance,
                None,
            )
            .await;

        // Should complete without error
    }

    #[tokio::test]
    async fn test_correlation_context() {
        let logger = Arc::new(AuditLogger::new(AuditLoggerConfig::default()));
        let middleware = AuditMiddleware::new(logger);

        let corr_id = CorrelationId::new();
        middleware.set_correlation_id(corr_id).await;

        let retrieved = middleware.get_correlation_id().await;
        assert_eq!(retrieved, corr_id);

        middleware.clear_context().await;
    }

    #[tokio::test]
    async fn test_trace_context() {
        let logger = Arc::new(AuditLogger::new(AuditLoggerConfig::default()));
        let middleware = AuditMiddleware::new(logger);

        let trace = TraceContext::new();
        middleware.set_trace_context(trace.clone()).await;

        let retrieved = middleware.get_trace_context().await;
        assert_eq!(retrieved.trace_id, trace.trace_id);
    }
}
