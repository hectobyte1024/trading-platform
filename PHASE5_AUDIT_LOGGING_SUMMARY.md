# Phase 5: Audit Logging with Kafka

**Status:** ✅ Complete - 137 tests passing (32 new)  
**Code:** 1,797 lines across 5 files  
**Test Coverage:** 32 audit logging tests (100% passing)

## Overview

Production-ready security audit logging system with Kafka event streaming, SIEM-compatible schemas, distributed tracing, and comprehensive event correlation.

## Architecture

### Core Components

```
audit/
├── mod.rs          (19 lines)   - Module structure
├── events.rs       (550 lines)  - SIEM-compatible event types
├── correlation.rs  (330 lines)  - Event correlation & tracing
├── logger.rs       (432 lines)  - Kafka audit logger
└── middleware.rs   (466 lines)  - Audit middleware
```

### Event Types (events.rs)

**Event Categories:**
- Authentication - Login, logout, MFA, WebAuthn, token operations
- Authorization - Permission checks, access denials, role changes
- Session - Created, extended, expired, revoked, hijack detection
- Security - Suspicious activity, rate limits, brute force, compromises
- Admin - User management, configuration changes, API keys
- Compliance - Data access, exports, audit log queries, reports

**Severity Levels:**
- INFO - Normal operations
- LOW - Minor events
- MEDIUM - Notable events
- HIGH - Important security events
- CRITICAL - Critical security incidents

**Event Structure:**
```rust
pub struct AuditEvent {
    pub event_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub category: EventCategory,
    pub severity: Severity,
    pub event_type: String,
    pub correlation_id: Option<Uuid>,
    pub trace_id: Option<Uuid>,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub domain: Option<UserDomain>,
    pub ip_address: Option<IpAddr>,
    pub user_agent: Option<String>,
    pub data: EventData,
    pub metadata: HashMap<String, serde_json::Value>,
    pub outcome: EventOutcome,
    pub error: Option<String>,
}
```

**Authentication Events:**
- Token validation (JWT validation results)
- Token refresh (old/new token tracking)
- MFA challenge/verification (type, attempts, success)
- WebAuthn registration (credential ID, authenticator type, attestation)
- WebAuthn authentication (user verification, counter)
- Password login (username, failed attempts)
- Logout (reason)

**Authorization Events:**
- Permission check (resource, action, decision, policy)
- Access denied (resource, action, reason)
- Role assignment/revocation (target user, role, actor)
- Policy update (policy type, ID, updated by)

**Session Events:**
- Created (type, TTL)
- Extended (old/new expiry)
- Expired (time, reason)
- Revoked (reason, revoked by)
- Hijack detected (IPs, indicators)

**Security Events:**
- Suspicious activity (type, risk score, indicators)
- Rate limit exceeded (limit type, threshold, actual)
- Brute force attempt (target, count, window)
- Credential compromise (type, detection method)
- Token replay (token ID, first use, replay count)
- Policy violation (policy name, details)

**Admin Events:**
- User created/deleted/suspended (target user, reason, duration)
- Configuration changed (key, old/new values)
- API key created/revoked (key ID, permissions, reason)

**Compliance Events:**
- Data access (type, count, purpose)
- Data export (type, count, destination)
- Audit log access (query, count)
- Report generated (type, time range)

### Event Correlation (correlation.rs)

**CorrelationId:**
- UUID-based correlation for related events
- Track multi-step operations
- Group authentication flows
- Connect request/response pairs

**TraceContext:**
- W3C traceparent header support
- Distributed tracing integration
- Trace ID + Span ID + Parent Span ID
- Sampling decision propagation
- Format: `00-{trace_id}-{parent_span_id}-{flags}`

**CorrelationContext:**
- Thread-safe context management
- Async-aware with RwLock
- Auto-generate IDs if not set
- Clear context between requests
- Child span creation

**W3C Traceparent:**
- Parse incoming traceparent headers
- Generate outgoing headers
- UUID simple format (no dashes)
- Hex flag encoding (sampled bit)

### Kafka Logger (logger.rs)

**AuditLogger:**
- Kafka producer integration
- Asynchronous event publishing
- Synchronous mode (wait for ack)
- Fallback to structured logs
- Graceful degradation

**Configuration:**
```rust
pub struct AuditLoggerConfig {
    pub kafka_brokers: String,
    pub topic: String,
    pub synchronous: bool,
    pub buffer_size: usize,
}
```

**Features:**
- JSON event serialization
- Event key = event_id (for Kafka partitioning)
- Batch logging support
- Flush on shutdown
- Initialization retry logic
- Fallback logging when Kafka unavailable

**AuditLoggerBuilder:**
```rust
let logger = AuditLoggerBuilder::new()
    .with_kafka_brokers("kafka:9092")
    .with_topic("auth-audit-events")
    .synchronous()
    .with_buffer_size(1000)
    .build();
```

### Audit Middleware (middleware.rs)

**AuditMiddleware:**
- Automatic event logging
- Correlation context management
- Helper methods for each event category
- JWT claims extraction
- IP and user agent tracking

**Methods:**
- `log_authentication()` - Auth events with outcome
- `log_authorization()` - Authz events with claims
- `log_session()` - Session lifecycle events
- `log_security()` - Security incidents
- `log_admin()` - Administrative actions
- `log_compliance()` - Compliance operations

**Integration:**
```rust
let middleware = AuditMiddleware::new(Arc::new(logger));

// Set correlation from request header
if let Some(corr_id) = request.headers().get("X-Correlation-Id") {
    middleware.set_correlation_id(CorrelationId::parse(corr_id)?).await;
}

// Log authentication event
middleware.log_authentication(
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
    Some(ip_address),
    Some(user_agent),
    None,
).await;
```

## Usage Examples

### Basic Event Logging

```rust
use auth_service::audit::*;

// Create audit logger
let logger = AuditLoggerBuilder::new()
    .with_kafka_brokers("localhost:9092")
    .with_topic("auth-audit")
    .build();

logger.initialize().await?;

// Create audit event
let event = AuditEvent::new(
    EventCategory::Authentication,
    Severity::Info,
    "token_validation",
    EventData::Authentication(AuthenticationEvent::TokenValidation {
        token_type: "access".to_string(),
        validation_result: true,
        reason: None,
    }),
    EventOutcome::Success,
)
.with_user(user_id, UserDomain::Retail)
.with_session(session_id)
.with_ip(ip_address)
.with_correlation_id(correlation_id.as_uuid());

// Log to Kafka
logger.log(event).await?;
```

### Using Middleware

```rust
let middleware = AuditMiddleware::new(Arc::new(logger));

// Set correlation and trace
middleware.set_correlation_id(CorrelationId::new()).await;
middleware.set_trace_context(TraceContext::new()).await;

// Log authorization event
middleware.log_authorization(
    "permission_check",
    AuthorizationEvent::PermissionCheck {
        resource_type: ResourceType::Order,
        resource_id: Some("order_123".to_string()),
        action: Action::Create,
        decision: "allow".to_string(),
        policy_evaluated: "rbac".to_string(),
    },
    EventOutcome::Success,
    Some(&claims),
    Some(ip_address),
    Some(user_agent),
).await;
```

### Session Tracking

```rust
// Session created
middleware.log_session(
    "session_created",
    SessionEvent::Created {
        session_type: "access_token".to_string(),
        ttl_seconds: 3600,
    },
    EventOutcome::Success,
    user_id,
    domain,
    session_id,
    Some(ip_address),
).await;

// Session hijack detected
middleware.log_session(
    "hijack_detected",
    SessionEvent::HijackDetected {
        reason: "IP address mismatch".to_string(),
        original_ip: Some(original_ip),
        suspicious_ip: current_ip,
    },
    EventOutcome::Failure,
    user_id,
    domain,
    session_id,
    Some(current_ip),
).await;
```

### Security Incident Logging

```rust
middleware.log_security(
    "brute_force_attempt",
    SecurityEvent::BruteForceAttempt {
        target_resource: "login".to_string(),
        attempt_count: 10,
        time_window_seconds: 60,
    },
    Severity::High,
    None,
    None,
    None,
    Some(ip_address),
    Some(user_agent),
).await;
```

### Compliance Reporting

```rust
middleware.log_compliance(
    "data_export",
    ComplianceEvent::DataExport {
        data_type: "user_orders".to_string(),
        record_count: 500,
        destination: "s3://compliance-exports/2026-03-03/".to_string(),
    },
    compliance_user_id,
    UserDomain::Compliance,
    Some(ip_address),
).await;
```

### Distributed Tracing

```rust
// Extract from incoming request
if let Some(traceparent) = request.headers().get("traceparent") {
    if let Some(trace_ctx) = TraceContext::from_traceparent(traceparent) {
        middleware.set_trace_context(trace_ctx).await;
    }
}

// Create child span for operation
let trace = middleware.get_trace_context().await;
let child_span = trace.child_span();
middleware.set_trace_context(child_span.clone()).await;

// Propagate to downstream service
request.headers_mut().insert(
    "traceparent",
    child_span.to_traceparent().parse()?,
);
```

## Test Coverage (32 tests)

### Event Tests (7):
- ✅ Audit event creation
- ✅ Event builder pattern
- ✅ Event serialization/deserialization
- ✅ Event correlation
- ✅ Severity levels
- ✅ Event categories
- ✅ Metadata attachment

### Correlation Tests (9):
- ✅ Correlation ID creation
- ✅ Correlation ID parsing
- ✅ Trace context creation
- ✅ Trace context child spans
- ✅ Traceparent serialization
- ✅ Traceparent parsing
- ✅ Correlation context management
- ✅ Trace context management
- ✅ Context clearing

### Logger Tests (8):
- ✅ Logger config defaults
- ✅ Logger builder pattern
- ✅ Logger initialization
- ✅ Log event without Kafka (fallback)
- ✅ Batch logging
- ✅ Flush events
- ✅ Shutdown gracefully
- ✅ Event serialization for Kafka

### Middleware Tests (8):
- ✅ Middleware creation
- ✅ Log authentication event
- ✅ Log authorization event
- ✅ Log session event
- ✅ Log security event
- ✅ Log admin event
- ✅ Log compliance event
- ✅ Correlation context
- ✅ Trace context

## Key Features

### SIEM Integration
- ✅ Industry-standard event schemas
- ✅ JSON serialization for all events
- ✅ Categorical event organization
- ✅ Severity-based filtering
- ✅ Structured metadata fields
- ✅ Timestamp normalization (UTC)

### Kafka Streaming
- ✅ High-throughput event publishing
- ✅ Durable event storage
- ✅ Event replay capability
- ✅ Partitioning by event ID
- ✅ Topic configuration
- ✅ Graceful fallback

### Distributed Tracing
- ✅ W3C traceparent standard
- ✅ Trace ID propagation
- ✅ Span hierarchy
- ✅ Sampling support
- ✅ Cross-service correlation
- ✅ Request flow tracking

### Event Correlation
- ✅ Correlation ID generation
- ✅ Multi-event grouping
- ✅ Authentication flow tracking
- ✅ Session lifecycle correlation
- ✅ Context management
- ✅ Async-safe operations

### Compliance
- ✅ Complete audit trail
- ✅ Immutable event log
- ✅ Privacy-safe data capture
- ✅ Export capabilities
- ✅ Report generation tracking
- ✅ Access logging

### Performance
- ✅ Asynchronous logging
- ✅ Batch operations
- ✅ Non-blocking writes
- ✅ Buffer management
- ✅ Graceful degradation
- ✅ Minimal overhead

## SIEM Query Examples

### Find Failed Login Attempts
```sql
SELECT * FROM audit_events
WHERE category = 'authentication'
  AND outcome = 'failure'
  AND event_type LIKE '%login%'
  AND timestamp > NOW() - INTERVAL '1 hour'
ORDER BY timestamp DESC;
```

### Track User Session
```sql
SELECT * FROM audit_events
WHERE session_id = 'session_uuid_here'
ORDER BY timestamp ASC;
```

### Security Incident Report
```sql
SELECT event_type, severity, COUNT(*) as count
FROM audit_events
WHERE category = 'security'
  AND severity IN ('high', 'critical')
  AND timestamp > NOW() - INTERVAL '24 hours'
GROUP BY event_type, severity
ORDER BY count DESC;
```

### Compliance Audit Trail
```sql
SELECT user_id, event_type, data, timestamp
FROM audit_events
WHERE category = 'compliance'
  AND timestamp BETWEEN '2026-03-01' AND '2026-03-31'
ORDER BY timestamp ASC;
```

### Correlated Events
```sql
SELECT * FROM audit_events
WHERE correlation_id = 'correlation_uuid_here'
ORDER BY timestamp ASC;
```

## Integration Points

### API Gateway
- Extract correlation ID from headers
- Set trace context from traceparent
- Log all authenticated requests
- Track permission checks
- Monitor rate limits

### Authentication Flow
- Log token validation
- Track MFA challenges
- Record WebAuthn operations
- Monitor failed attempts
- Detect suspicious patterns

### Authorization Layer
- Log permission checks
- Record access denials
- Track role changes
- Monitor policy updates
- Audit privilege escalation

### Session Management
- Log session creation
- Track session extensions
- Record session expiry
- Monitor revocations
- Detect hijack attempts

### Admin Operations
- Log user management
- Track configuration changes
- Record API key operations
- Monitor system changes
- Audit administrative access

## Technical Implementation

### Serde Tag Conflict Fix
- Changed `AuthorizationEvent` tag from `"action"` to `"event_action"`
- Prevents conflict with `action` field in variants
- Maintains clean JSON structure

### W3C Traceparent Format
- UUIDs in simple format (32 hex chars, no dashes)
- Parsing handles both formats
- Serialization uses simple format
- Flags in 2-digit hex

### Fallback Logging
- Kafka unavailable → structured logs
- JSON pretty-print for debugging
- Maintains audit trail integrity
- No event loss

### Async Safety
- RwLock for correlation context
- Arc-wrapped logger
- Thread-safe operations
- Tokio integration

## Next Steps

**Phase 6: Risk Engine Integration** (Future)
- Behavioral analytics
- Anomaly detection
- Risk scoring algorithms
- Step-up authentication triggers
- Device reputation tracking
- Adaptive security policies

---

**Phase 5 Complete**: Production-ready audit logging with Kafka streaming, SIEM-compatible events, distributed tracing, and comprehensive event correlation. 137 total tests passing, 1,797 lines of audit code.
