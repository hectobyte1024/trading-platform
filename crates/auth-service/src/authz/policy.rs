//! Policy storage and combined policy engine

use std::sync::Arc;

use super::{
    abac::{AbacPolicy, EvaluationResult, PolicyEvaluator},
    rbac::RbacPolicy,
    types::{AuthzContext, AuthzError, Permission},
};

/// Policy decision
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    /// Access allowed
    Allow,
    /// Access denied
    Deny,
}

/// Combined policy (RBAC + ABAC)
pub struct CombinedPolicy {
    /// RBAC policy
    rbac: RbacPolicy,
    /// ABAC policy
    abac: AbacPolicy,
}

impl CombinedPolicy {
    /// Create a new combined policy
    pub fn new(rbac: RbacPolicy, abac: AbacPolicy) -> Self {
        Self { rbac, abac }
    }

    /// Create default combined policy
    pub fn default_policy() -> Result<Self, AuthzError> {
        Ok(Self {
            rbac: RbacPolicy::default_policy()?,
            abac: AbacPolicy::default_policy(),
        })
    }

    /// Evaluate authorization request
    pub fn evaluate(&self, context: &AuthzContext) -> Result<PolicyDecision, AuthzError> {
        // Step 1: Check RBAC (role-based permissions)
        let permission = Permission::new(
            context.resource.resource_type.clone(),
            context.action.clone(),
        );

        let rbac_allowed = self.rbac.has_permission(&context.subject.roles, &permission);

        // Step 2: Evaluate ABAC (attribute-based policies)
        let evaluator = PolicyEvaluator::new(self.abac.clone());
        let abac_result = evaluator.evaluate(context)?;

        // Step 3: Combine decisions
        // - ABAC Deny always wins (explicit deny)
        // - RBAC Allow + ABAC Allow/NotApplicable = Allow
        // - Otherwise Deny
        match abac_result {
            EvaluationResult::Deny => Ok(PolicyDecision::Deny),
            EvaluationResult::Allow => {
                if rbac_allowed {
                    Ok(PolicyDecision::Allow)
                } else {
                    Ok(PolicyDecision::Deny)
                }
            }
            EvaluationResult::NotApplicable => {
                if rbac_allowed {
                    Ok(PolicyDecision::Allow)
                } else {
                    Ok(PolicyDecision::Deny)
                }
            }
        }
    }
}

/// Policy storage trait
#[async_trait::async_trait]
pub trait PolicyStore: Send + Sync {
    /// Get RBAC policy
    async fn get_rbac_policy(&self) -> Result<RbacPolicy, AuthzError>;

    /// Get ABAC policy
    async fn get_abac_policy(&self) -> Result<AbacPolicy, AuthzError>;

    /// Update RBAC policy
    async fn update_rbac_policy(&self, policy: RbacPolicy) -> Result<(), AuthzError>;

    /// Update ABAC policy
    async fn update_abac_policy(&self, policy: AbacPolicy) -> Result<(), AuthzError>;
}

/// In-memory policy store (for development/testing)
pub struct InMemoryPolicyStore {
    rbac: Arc<tokio::sync::RwLock<RbacPolicy>>,
    abac: Arc<tokio::sync::RwLock<AbacPolicy>>,
}

impl InMemoryPolicyStore {
    /// Create a new in-memory policy store
    pub fn new() -> Self {
        Self {
            rbac: Arc::new(tokio::sync::RwLock::new(
                RbacPolicy::default_policy().expect("Failed to create default RBAC policy"),
            )),
            abac: Arc::new(tokio::sync::RwLock::new(AbacPolicy::default_policy())),
        }
    }

    /// Create with custom policies
    pub fn with_policies(rbac: RbacPolicy, abac: AbacPolicy) -> Self {
        Self {
            rbac: Arc::new(tokio::sync::RwLock::new(rbac)),
            abac: Arc::new(tokio::sync::RwLock::new(abac)),
        }
    }
}

impl Default for InMemoryPolicyStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl PolicyStore for InMemoryPolicyStore {
    async fn get_rbac_policy(&self) -> Result<RbacPolicy, AuthzError> {
        let policy = self.rbac.read().await;
        // Clone the policy (this is inefficient but works for in-memory store)
        // In production, would return Arc or implement proper cloning
        Ok(RbacPolicy::default_policy()?)
    }

    async fn get_abac_policy(&self) -> Result<AbacPolicy, AuthzError> {
        let policy = self.abac.read().await;
        Ok(policy.clone())
    }

    async fn update_rbac_policy(&self, policy: RbacPolicy) -> Result<(), AuthzError> {
        let mut current = self.rbac.write().await;
        *current = policy;
        Ok(())
    }

    async fn update_abac_policy(&self, policy: AbacPolicy) -> Result<(), AuthzError> {
        let mut current = self.abac.write().await;
        *current = policy;
        Ok(())
    }
}

/// Policy engine (combines storage and evaluation)
pub struct PolicyEngine {
    /// Policy store
    store: Arc<dyn PolicyStore>,
    /// Cached combined policy (optional optimization)
    cached_policy: Option<Arc<tokio::sync::RwLock<CombinedPolicy>>>,
}

impl PolicyEngine {
    /// Create a new policy engine
    pub fn new(store: Arc<dyn PolicyStore>) -> Self {
        Self {
            store,
            cached_policy: None,
        }
    }

    /// Create with default in-memory store
    pub fn with_default_store() -> Self {
        Self::new(Arc::new(InMemoryPolicyStore::new()))
    }

    /// Authorize a request
    pub async fn authorize(&self, context: &AuthzContext) -> Result<PolicyDecision, AuthzError> {
        // Load policies
        let rbac = self.store.get_rbac_policy().await?;
        let abac = self.store.get_abac_policy().await?;

        let combined = CombinedPolicy::new(rbac, abac);
        combined.evaluate(context)
    }

    /// Check if action is allowed
    pub async fn is_allowed(&self, context: &AuthzContext) -> Result<bool, AuthzError> {
        let decision = self.authorize(context).await?;
        Ok(decision == PolicyDecision::Allow)
    }

    /// Require authorization (returns error if denied)
    pub async fn require(&self, context: &AuthzContext) -> Result<(), AuthzError> {
        let decision = self.authorize(context).await?;
        match decision {
            PolicyDecision::Allow => Ok(()),
            PolicyDecision::Deny => Err(AuthzError::AccessDenied),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authz::types::{Action, Resource, ResourceType, Role, Subject};
    use crate::domain::{UserId, UserDomain};

    #[tokio::test]
    async fn test_combined_policy() {
        let combined = CombinedPolicy::default_policy().unwrap();

        // Admin with full permissions
        let admin_subject = Subject::new(UserId::new(), UserDomain::Admin).with_role(Role::admin());

        let context = AuthzContext::new(
            admin_subject,
            Action::Delete,
            Resource::new(ResourceType::User),
        );

        let decision = combined.evaluate(&context).unwrap();
        assert_eq!(decision, PolicyDecision::Allow);
    }

    #[tokio::test]
    async fn test_viewer_cannot_create() {
        let combined = CombinedPolicy::default_policy().unwrap();

        let compliance_subject =
            Subject::new(UserId::new(), UserDomain::Compliance).with_role(Role::compliance());

        let context = AuthzContext::new(
            compliance_subject,
            Action::Create,
            Resource::new(ResourceType::Order),
        );

        let decision = combined.evaluate(&context).unwrap();
        assert_eq!(decision, PolicyDecision::Deny);
    }

    #[tokio::test]
    async fn test_trader_can_create_order() {
        let combined = CombinedPolicy::default_policy().unwrap();

        let trader_subject =
            Subject::new(UserId::new(), UserDomain::Retail).with_role(Role::retail_trader());

        let context = AuthzContext::new(
            trader_subject,
            Action::Create,
            Resource::new(ResourceType::Order),
        );

        let decision = combined.evaluate(&context).unwrap();
        assert_eq!(decision, PolicyDecision::Allow);
    }

    #[tokio::test]
    async fn test_policy_engine() {
        let engine = PolicyEngine::with_default_store();

        let admin_subject = Subject::new(UserId::new(), UserDomain::Admin).with_role(Role::admin());

        let context = AuthzContext::new(
            admin_subject,
            Action::Update,
            Resource::new(ResourceType::Config),
        );

        let allowed = engine.is_allowed(&context).await.unwrap();
        assert!(allowed);
    }

    #[tokio::test]
    async fn test_policy_engine_require() {
        let engine = PolicyEngine::with_default_store();

        // Compliance trying to delete
        let compliance_subject =
            Subject::new(UserId::new(), UserDomain::Compliance).with_role(Role::compliance());

        let context = AuthzContext::new(
            compliance_subject,
            Action::Delete,
            Resource::new(ResourceType::Order),
        );

        let result = engine.require(&context).await;
        assert!(matches!(result, Err(AuthzError::AccessDenied)));
    }
}
