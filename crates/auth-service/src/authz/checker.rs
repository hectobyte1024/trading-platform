//! Permission checker and authorization middleware

use std::sync::Arc;

use super::{
    policy::PolicyEngine,
    types::{Action, AuthzContext, AuthzError, Resource, ResourceType, Role, Subject},
};
use crate::domain::{AccessClaims, UserId, UserDomain};

/// Permission checker for programmatic authorization checks
pub struct PermissionChecker {
    /// Policy engine
    engine: Arc<PolicyEngine>,
}

impl PermissionChecker {
    /// Create a new permission checker
    pub fn new(engine: Arc<PolicyEngine>) -> Self {
        Self { engine }
    }

    /// Create with default policy engine
    pub fn with_default_engine() -> Self {
        Self::new(Arc::new(PolicyEngine::with_default_store()))
    }

    /// Check if subject can perform action on resource
    pub async fn check(
        &self,
        subject: &Subject,
        action: Action,
        resource: Resource,
    ) -> Result<bool, AuthzError> {
        let context = AuthzContext::new(subject.clone(), action, resource);
        self.engine.is_allowed(&context).await
    }

    /// Require permission (returns error if denied)
    pub async fn require(
        &self,
        subject: &Subject,
        action: Action,
        resource: Resource,
    ) -> Result<(), AuthzError> {
        let context = AuthzContext::new(subject.clone(), action, resource);
        self.engine.require(&context).await
    }

    /// Check permission from JWT claims
    pub async fn check_from_claims(
        &self,
        claims: &AccessClaims,
        action: Action,
        resource: Resource,
    ) -> Result<bool, AuthzError> {
        let subject = Self::subject_from_claims(claims);
        self.check(&subject, action, resource).await
    }

    /// Require permission from JWT claims
    pub async fn require_from_claims(
        &self,
        claims: &AccessClaims,
        action: Action,
        resource: Resource,
    ) -> Result<(), AuthzError> {
        let subject = Self::subject_from_claims(claims);
        self.require(&subject, action, resource).await
    }

    /// Convert JWT claims to Subject
    fn subject_from_claims(claims: &AccessClaims) -> Subject {
        // Parse user ID from subject claim
        let user_id = uuid::Uuid::parse_str(&claims.standard.sub)
            .map(UserId)
            .unwrap_or_else(|_| UserId::new());

        let mut subject = Subject::new(user_id, claims.domain);

        // Add role based on domain (simplified - in production, store roles in claims)
        let role = match claims.domain {
            UserDomain::Admin => Role::admin(),
            UserDomain::Retail => Role::retail_trader(),
            UserDomain::Institutional => Role::institutional_trader(),
            UserDomain::Compliance => Role::compliance(),
            UserDomain::Service => Role::service(),
        };

        subject = subject.with_role(role);

        // Parse session ID
        if let Ok(session_uuid) = uuid::Uuid::parse_str(&claims.session_id) {
            use crate::domain::SessionId;
            subject = subject.with_session(SessionId(session_uuid));
        }

        subject
    }

    /// Batch check permissions
    pub async fn check_batch(
        &self,
        subject: &Subject,
        actions: Vec<(Action, Resource)>,
    ) -> Result<Vec<bool>, AuthzError> {
        let mut results = Vec::new();
        for (action, resource) in actions {
            let allowed = self.check(subject, action, resource).await?;
            results.push(allowed);
        }
        Ok(results)
    }

    /// Get allowed actions for resource
    pub async fn get_allowed_actions(
        &self,
        subject: &Subject,
        resource_type: ResourceType,
    ) -> Result<Vec<Action>, AuthzError> {
        let actions = vec![
            Action::Read,
            Action::Create,
            Action::Update,
            Action::Delete,
            Action::Execute,
            Action::Approve,
            Action::Reject,
            Action::Cancel,
            Action::Export,
        ];

        let mut allowed = Vec::new();
        for action in actions {
            let resource = Resource::new(resource_type.clone());
            if self.check(subject, action.clone(), resource).await? {
                allowed.push(action);
            }
        }

        Ok(allowed)
    }
}

/// Authorization middleware (for use in API handlers)
pub struct AuthorizationMiddleware {
    /// Permission checker
    checker: Arc<PermissionChecker>,
}

impl AuthorizationMiddleware {
    /// Create new authorization middleware
    pub fn new(checker: Arc<PermissionChecker>) -> Self {
        Self { checker }
    }

    /// Authorize request with claims
    pub async fn authorize(
        &self,
        claims: &AccessClaims,
        action: Action,
        resource: Resource,
    ) -> Result<(), AuthzError> {
        self.checker
            .require_from_claims(claims, action, resource)
            .await
    }

    /// Get subject from claims
    pub fn get_subject(&self, claims: &AccessClaims) -> Subject {
        PermissionChecker::subject_from_claims(claims)
    }
}

/// Convenience macro for permission checks
#[macro_export]
macro_rules! check_permission {
    ($checker:expr, $subject:expr, $action:expr, $resource:expr) => {
        $checker.check($subject, $action, $resource).await?
    };
}

/// Convenience macro for requiring permissions
#[macro_export]
macro_rules! require_permission {
    ($checker:expr, $subject:expr, $action:expr, $resource:expr) => {
        $checker.require($subject, $action, $resource).await?
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SessionId;

    #[tokio::test]
    async fn test_permission_checker() {
        let checker = PermissionChecker::with_default_engine();

        let admin = Subject::new(UserId::new(), UserDomain::Admin).with_role(Role::admin());

        let can_delete = checker
            .check(&admin, Action::Delete, Resource::new(ResourceType::User))
            .await
            .unwrap();

        assert!(can_delete);
    }

    #[tokio::test]
    async fn test_permission_checker_deny() {
        let checker = PermissionChecker::with_default_engine();

        let compliance = Subject::new(UserId::new(), UserDomain::Compliance).with_role(Role::compliance());

        let can_delete = checker
            .check(&compliance, Action::Delete, Resource::new(ResourceType::Order))
            .await
            .unwrap();

        assert!(!can_delete);
    }

    #[tokio::test]
    async fn test_require_permission() {
        let checker = PermissionChecker::with_default_engine();

        let compliance = Subject::new(UserId::new(), UserDomain::Compliance).with_role(Role::compliance());

        let result = checker
            .require(&compliance, Action::Delete, Resource::new(ResourceType::Order))
            .await;

        assert!(matches!(result, Err(AuthzError::AccessDenied)));
    }

    #[tokio::test]
    async fn test_check_from_claims() {
        let checker = PermissionChecker::with_default_engine();

        let user_id = UserId::new();
        let session_id = SessionId::new();

        let claims = AccessClaims {
            standard: crate::domain::StandardClaims {
                iss: "trading-platform".to_string(),
                sub: user_id.to_string(),
                aud: vec!["api".to_string()],
                exp: chrono::Utc::now().timestamp() + 3600,
                nbf: chrono::Utc::now().timestamp(),
                iat: chrono::Utc::now().timestamp(),
                jti: crate::domain::TokenId::new().to_string(),
            },
            domain: UserDomain::Retail,
            device_id: "device-123".to_string(),
            session_id: session_id.to_string(),
            scopes: Default::default(),
            nonce: "nonce-123".to_string(),
            kid: "key-1".to_string(),
            ip: "127.0.0.1".to_string(),
            risk_score: 0.1,
            mfa_verified: false,
            webauthn_verified: false,
            token_version: 1,
        };

        let can_create = checker
            .check_from_claims(&claims, Action::Create, Resource::new(ResourceType::Order))
            .await
            .unwrap();

        assert!(can_create);
    }

    #[tokio::test]
    async fn test_get_allowed_actions() {
        let checker = PermissionChecker::with_default_engine();

        let trader = Subject::new(UserId::new(), UserDomain::Retail).with_role(Role::retail_trader());

        let allowed = checker
            .get_allowed_actions(&trader, ResourceType::Order)
            .await
            .unwrap();

        assert!(allowed.contains(&Action::Read));
        assert!(allowed.contains(&Action::Create));
        assert!(allowed.contains(&Action::Update));
        assert!(allowed.contains(&Action::Cancel));
        assert!(!allowed.contains(&Action::Delete));
    }

    #[tokio::test]
    async fn test_batch_check() {
        let checker = PermissionChecker::with_default_engine();

        let admin = Subject::new(UserId::new(), UserDomain::Admin).with_role(Role::admin());

        let checks = vec![
            (Action::Read, Resource::new(ResourceType::Order)),
            (Action::Delete, Resource::new(ResourceType::User)),
            (Action::Export, Resource::new(ResourceType::AuditLog)),
        ];

        let results = checker.check_batch(&admin, checks).await.unwrap();

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|&r| r)); // Admin should have all permissions
    }
}
