//! Audit Event Types
//!
//! SIEM-compatible event schemas for security monitoring and compliance.

use crate::domain::{UserId, SessionId, UserDomain};
use crate::authz::{Action, ResourceType, Resource};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use uuid::Uuid;

/// Audit event severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    /// Informational event
    Info,
    /// Low severity
    Low,
    /// Medium severity
    Medium,
    /// High severity
    High,
    /// Critical event
    Critical,
}

/// Audit event category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
    /// Authentication events
    Authentication,
    /// Authorization events
    Authorization,
    /// Session management
    Session,
    /// Security incidents
    Security,
    /// Administrative actions
    Admin,
    /// Compliance events
    Compliance,
}

/// Base audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique event ID
    pub event_id: Uuid,
    /// Event timestamp (UTC)
    pub timestamp: DateTime<Utc>,
    /// Event category
    pub category: EventCategory,
    /// Event severity
    pub severity: Severity,
    /// Event type (specific action)
    pub event_type: String,
    /// Correlation ID for related events
    pub correlation_id: Option<Uuid>,
    /// Trace ID for distributed tracing
    pub trace_id: Option<Uuid>,
    /// User ID (if applicable)
    pub user_id: Option<UserId>,
    /// Session ID (if applicable)
    pub session_id: Option<SessionId>,
    /// User domain (if applicable)
    pub domain: Option<UserDomain>,
    /// Source IP address
    pub ip_address: Option<IpAddr>,
    /// User agent
    pub user_agent: Option<String>,
    /// Event-specific data
    pub data: EventData,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Event outcome (success/failure)
    pub outcome: EventOutcome,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Event outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventOutcome {
    Success,
    Failure,
    Partial,
}

/// Event-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventData {
    /// Authentication event
    Authentication(AuthenticationEvent),
    /// Authorization event
    Authorization(AuthorizationEvent),
    /// Session event
    Session(SessionEvent),
    /// Security event
    Security(SecurityEvent),
    /// Admin event
    Admin(AdminEvent),
    /// Compliance event
    Compliance(ComplianceEvent),
}

/// Authentication event data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AuthenticationEvent {
    /// JWT token validation
    TokenValidation {
        token_type: String,
        validation_result: bool,
        reason: Option<String>,
    },
    /// Token refresh
    TokenRefresh {
        old_token_id: Option<String>,
        new_token_id: Option<String>,
    },
    /// Multi-factor authentication
    MfaChallenge {
        mfa_type: String,
        challenge_sent: bool,
    },
    /// MFA verification
    MfaVerification {
        mfa_type: String,
        verified: bool,
        attempts: u32,
    },
    /// WebAuthn registration
    WebAuthnRegistration {
        credential_id: String,
        authenticator_type: String,
        attestation_format: Option<String>,
    },
    /// WebAuthn authentication
    WebAuthnAuthentication {
        credential_id: String,
        user_verified: bool,
        counter: u32,
    },
    /// Password-based login (if supported)
    PasswordLogin {
        username: String,
        failed_attempts: u32,
    },
    /// Logout
    Logout {
        reason: String,
    },
}

/// Authorization event data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_action", rename_all = "snake_case")]
pub enum AuthorizationEvent {
    /// Permission check
    PermissionCheck {
        resource_type: ResourceType,
        resource_id: Option<String>,
        action: Action,
        decision: String,
        policy_evaluated: String,
    },
    /// Access denied
    AccessDenied {
        resource_type: ResourceType,
        resource_id: Option<String>,
        action: Action,
        reason: String,
    },
    /// Role assignment
    RoleAssignment {
        target_user_id: UserId,
        role: String,
        assigned_by: UserId,
    },
    /// Role revocation
    RoleRevocation {
        target_user_id: UserId,
        role: String,
        revoked_by: UserId,
    },
    /// Policy update
    PolicyUpdate {
        policy_type: String,
        policy_id: String,
        updated_by: UserId,
    },
}

/// Session event data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SessionEvent {
    /// Session created
    Created {
        session_type: String,
        ttl_seconds: u64,
    },
    /// Session extended
    Extended {
        old_expiry: DateTime<Utc>,
        new_expiry: DateTime<Utc>,
    },
    /// Session expired
    Expired {
        expiry_time: DateTime<Utc>,
        reason: String,
    },
    /// Session revoked
    Revoked {
        reason: String,
        revoked_by: Option<UserId>,
    },
    /// Session hijack detected
    HijackDetected {
        reason: String,
        original_ip: Option<IpAddr>,
        suspicious_ip: IpAddr,
    },
}

/// Security event data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SecurityEvent {
    /// Suspicious activity detected
    SuspiciousActivity {
        activity_type: String,
        risk_score: f64,
        indicators: Vec<String>,
    },
    /// Rate limit exceeded
    RateLimitExceeded {
        limit_type: String,
        limit: u64,
        actual: u64,
        window_seconds: u64,
    },
    /// Brute force attempt
    BruteForceAttempt {
        target_resource: String,
        attempt_count: u32,
        time_window_seconds: u64,
    },
    /// Credential compromise detected
    CredentialCompromise {
        credential_type: String,
        detection_method: String,
    },
    /// Token replay detected
    TokenReplay {
        token_id: String,
        first_use: DateTime<Utc>,
        replay_count: u32,
    },
    /// Security policy violation
    PolicyViolation {
        policy_name: String,
        violation_details: String,
    },
}

/// Admin event data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AdminEvent {
    /// User created
    UserCreated {
        target_user_id: UserId,
        domain: UserDomain,
    },
    /// User deleted
    UserDeleted {
        target_user_id: UserId,
        reason: String,
    },
    /// User suspended
    UserSuspended {
        target_user_id: UserId,
        reason: String,
        duration_seconds: Option<u64>,
    },
    /// Configuration changed
    ConfigurationChanged {
        config_key: String,
        old_value: Option<String>,
        new_value: String,
    },
    /// API key created
    ApiKeyCreated {
        key_id: String,
        permissions: Vec<String>,
    },
    /// API key revoked
    ApiKeyRevoked {
        key_id: String,
        reason: String,
    },
}

/// Compliance event data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ComplianceEvent {
    /// Data access
    DataAccess {
        data_type: String,
        record_count: u64,
        purpose: String,
    },
    /// Data export
    DataExport {
        data_type: String,
        record_count: u64,
        destination: String,
    },
    /// Audit log access
    AuditLogAccess {
        query: String,
        record_count: u64,
    },
    /// Compliance report generated
    ReportGenerated {
        report_type: String,
        time_range_start: DateTime<Utc>,
        time_range_end: DateTime<Utc>,
    },
}

impl AuditEvent {
    /// Create a new audit event
    pub fn new(
        category: EventCategory,
        severity: Severity,
        event_type: impl Into<String>,
        data: EventData,
        outcome: EventOutcome,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            category,
            severity,
            event_type: event_type.into(),
            correlation_id: None,
            trace_id: None,
            user_id: None,
            session_id: None,
            domain: None,
            ip_address: None,
            user_agent: None,
            data,
            metadata: HashMap::new(),
            outcome,
            error: None,
        }
    }

    /// Set correlation ID
    pub fn with_correlation_id(mut self, id: Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Set trace ID
    pub fn with_trace_id(mut self, id: Uuid) -> Self {
        self.trace_id = Some(id);
        self
    }

    /// Set user context
    pub fn with_user(mut self, user_id: UserId, domain: UserDomain) -> Self {
        self.user_id = Some(user_id);
        self.domain = Some(domain);
        self
    }

    /// Set session ID
    pub fn with_session(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Set IP address
    pub fn with_ip(mut self, ip: IpAddr) -> Self {
        self.ip_address = Some(ip);
        self
    }

    /// Set user agent
    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Set error message
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Convert to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Convert to pretty JSON
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_creation() {
        let event = AuditEvent::new(
            EventCategory::Authentication,
            Severity::Info,
            "webauthn_authentication",
            EventData::Authentication(AuthenticationEvent::WebAuthnAuthentication {
                credential_id: "test_credential".to_string(),
                user_verified: true,
                counter: 1,
            }),
            EventOutcome::Success,
        );

        assert_eq!(event.category, EventCategory::Authentication);
        assert_eq!(event.severity, Severity::Info);
        assert_eq!(event.outcome, EventOutcome::Success);
        assert!(event.event_id != Uuid::nil());
    }

    #[test]
    fn test_audit_event_builder() {
        let user_id = UserId::new();
        let session_id = SessionId(Uuid::new_v4());
        let ip = "192.168.1.1".parse::<IpAddr>().unwrap();

        let event = AuditEvent::new(
            EventCategory::Authorization,
            Severity::Medium,
            "access_denied",
            EventData::Authorization(AuthorizationEvent::AccessDenied {
                resource_type: ResourceType::Order,
                resource_id: Some("order_123".to_string()),
                action: Action::Delete,
                reason: "Insufficient permissions".to_string(),
            }),
            EventOutcome::Failure,
        )
        .with_user(user_id, UserDomain::Retail)
        .with_session(session_id)
        .with_ip(ip)
        .with_user_agent("Mozilla/5.0")
        .with_error("Permission denied");

        assert_eq!(event.user_id, Some(user_id));
        assert_eq!(event.session_id, Some(session_id));
        assert_eq!(event.ip_address, Some(ip));
        assert_eq!(event.user_agent, Some("Mozilla/5.0".to_string()));
        assert_eq!(event.error, Some("Permission denied".to_string()));
    }

    #[test]
    fn test_event_serialization() {
        let event = AuditEvent::new(
            EventCategory::Security,
            Severity::Critical,
            "token_replay",
            EventData::Security(SecurityEvent::TokenReplay {
                token_id: "token_123".to_string(),
                first_use: Utc::now(),
                replay_count: 3,
            }),
            EventOutcome::Failure,
        );

        let json = event.to_json().unwrap();
        assert!(json.contains("token_replay"));
        assert!(json.contains("\"severity\":\"CRITICAL\""));

        // Test deserialization
        let _deserialized: AuditEvent = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_event_correlation() {
        let correlation_id = Uuid::new_v4();
        let trace_id = Uuid::new_v4();

        let event = AuditEvent::new(
            EventCategory::Session,
            Severity::Info,
            "session_created",
            EventData::Session(SessionEvent::Created {
                session_type: "access_token".to_string(),
                ttl_seconds: 3600,
            }),
            EventOutcome::Success,
        )
        .with_correlation_id(correlation_id)
        .with_trace_id(trace_id);

        assert_eq!(event.correlation_id, Some(correlation_id));
        assert_eq!(event.trace_id, Some(trace_id));
    }

    #[test]
    fn test_severity_levels() {
        assert_eq!(serde_json::to_string(&Severity::Info).unwrap(), "\"INFO\"");
        assert_eq!(serde_json::to_string(&Severity::Critical).unwrap(), "\"CRITICAL\"");
    }

    #[test]
    fn test_event_categories() {
        let categories = vec![
            EventCategory::Authentication,
            EventCategory::Authorization,
            EventCategory::Session,
            EventCategory::Security,
            EventCategory::Admin,
            EventCategory::Compliance,
        ];

        for category in categories {
            let json = serde_json::to_string(&category).unwrap();
            let _deserialized: EventCategory = serde_json::from_str(&json).unwrap();
        }
    }
}
