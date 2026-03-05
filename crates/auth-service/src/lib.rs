// Authentication and cybersecurity service
// Production-grade authentication with WebAuthn, KMS-backed JWT, multi-domain auth,
// RBAC+ABAC, risk-based authentication, audit logging, and zero-trust mTLS

pub mod domain;
pub mod crypto;
pub mod jwt;
pub mod replay;
pub mod revocation;
pub mod webauthn;
pub mod authz;
pub mod audit;
pub mod risk;
pub mod hardware_signer;

// Re-export key types
pub use domain::{
    AccessClaims, AccessToken, Claims, ClaimScope, DeviceFingerprint, DeviceId, RefreshClaims,
    RefreshToken, RiskScore, Session, SessionId, SessionState, StandardClaims, TokenId, TokenPair,
    User, UserDomain, UserId, UserType,
};

pub use crypto::{
    KmsClient, KmsClientTrait, KmsConfig, KmsError, KeyManager, KeyRotationPolicy, Signer,
    SignerTrait, SigningAlgorithm,
};

pub use jwt::{GeneratorConfig, JwtGenerator, JwtValidator, ValidatorConfig, ValidationError};

pub use replay::{ReplayDetector, ReplayDetectorConfig, NonceStore, InMemoryNonceStore};

pub use revocation::{
    RevocationStore, RevocationError, RevocationReason, RevokedToken, RevocationStats,
};

#[cfg(feature = "redis")]
pub use revocation::RedisRevocationStore;

#[cfg(feature = "postgres")]
pub use revocation::PostgresRevocationStore;

pub use webauthn::{
    WebAuthnError, AuthenticatorType, UserVerification, AttestationType,
    PublicKeyCredential, AuthenticatorData, ClientData, CredentialDescriptor,
    RegistrationOptions, RegistrationChallenge, RegistrationResponse, RegistrationVerifier,
    AuthenticationOptions, AuthenticationChallenge, AuthenticationResponse, AuthenticationVerifier,
    CredentialStore, StoredCredential, CredentialMetadata,
    ChallengeGenerator, ChallengeStore, InMemoryChallengeStore,
};

pub use authz::{
    AuthzError, Permission, Resource, Action, ResourceType,
    Role, RoleAssignment, Subject, AuthzContext,
    RbacPolicy, RoleHierarchy, RoleDefinition, RoleManager,
    AbacPolicy, PolicyRule, PolicyEffect, Condition, ConditionOperator,
    PolicyEvaluator, EvaluationContext, EvaluationResult,
    PolicyStore, PolicyEngine, CombinedPolicy, PolicyDecision,
    PermissionChecker, AuthorizationMiddleware,
};

pub use audit::{
    AuditEvent, EventCategory, EventData, EventOutcome, Severity,
    AuthenticationEvent, AuthorizationEvent, SessionEvent, SecurityEvent,
    AdminEvent, ComplianceEvent,
    AuditLogger, AuditLoggerConfig,
    CorrelationId, TraceContext,
    AuditMiddleware,
};

pub use risk::{
    RiskScorer, RiskLevel, BehavioralAnalyzer, UserBehavior, BehaviorPattern,
    AnomalyDetector, Anomaly, AnomalyType,
    DeviceReputationTracker, DeviceReputation, ReputationScore,
    AdaptivePolicy, StepUpTrigger, AuthenticationRequirement, StepUpReason,
};
