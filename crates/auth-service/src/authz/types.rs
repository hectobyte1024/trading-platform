//! Authorization core types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

use crate::domain::{UserId, UserDomain, SessionId};

/// Authorization errors
#[derive(Debug, Error)]
pub enum AuthzError {
    #[error("Access denied")]
    AccessDenied,

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Role not found: {0}")]
    RoleNotFound(String),

    #[error("Invalid policy: {0}")]
    InvalidPolicy(String),

    #[error("Policy evaluation error: {0}")]
    EvaluationError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Invalid resource: {0}")]
    InvalidResource(String),

    #[error("Circular role dependency detected")]
    CircularDependency,
}

/// Resource type in the system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    /// Trading orders
    Order,
    /// User accounts
    Account,
    /// Trading positions
    Position,
    /// Market data
    MarketData,
    /// System configuration
    Config,
    /// Audit logs
    AuditLog,
    /// User management
    User,
    /// API keys
    ApiKey,
    /// WebAuthn credentials
    Credential,
    /// Sessions
    Session,
    /// Custom resource
    Custom(String),
}

impl ResourceType {
    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s {
            "order" => Self::Order,
            "account" => Self::Account,
            "position" => Self::Position,
            "market_data" => Self::MarketData,
            "config" => Self::Config,
            "audit_log" => Self::AuditLog,
            "user" => Self::User,
            "api_key" => Self::ApiKey,
            "credential" => Self::Credential,
            "session" => Self::Session,
            _ => Self::Custom(s.to_string()),
        }
    }
}

/// Resource being accessed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// Resource type
    pub resource_type: ResourceType,
    /// Resource identifier (optional)
    pub resource_id: Option<String>,
    /// Resource owner (optional)
    pub owner_id: Option<UserId>,
    /// Additional attributes
    pub attributes: HashMap<String, serde_json::Value>,
}

impl Resource {
    /// Create a new resource
    pub fn new(resource_type: ResourceType) -> Self {
        Self {
            resource_type,
            resource_id: None,
            owner_id: None,
            attributes: HashMap::new(),
        }
    }

    /// Set resource ID
    pub fn with_id(mut self, id: String) -> Self {
        self.resource_id = Some(id);
        self
    }

    /// Set owner
    pub fn with_owner(mut self, owner_id: UserId) -> Self {
        self.owner_id = Some(owner_id);
        self
    }

    /// Add attribute
    pub fn with_attribute(mut self, key: String, value: serde_json::Value) -> Self {
        self.attributes.insert(key, value);
        self
    }
}

/// Action being performed
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Read/view
    Read,
    /// Create/write
    Create,
    /// Update/modify
    Update,
    /// Delete
    Delete,
    /// Execute/run
    Execute,
    /// Approve
    Approve,
    /// Reject
    Reject,
    /// Cancel
    Cancel,
    /// Export
    Export,
    /// Custom action
    Custom(String),
}

impl Action {
    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s {
            "read" => Self::Read,
            "create" => Self::Create,
            "update" => Self::Update,
            "delete" => Self::Delete,
            "execute" => Self::Execute,
            "approve" => Self::Approve,
            "reject" => Self::Reject,
            "cancel" => Self::Cancel,
            "export" => Self::Export,
            _ => Self::Custom(s.to_string()),
        }
    }
}

/// Permission = Action on ResourceType
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Permission {
    /// Resource type
    pub resource_type: ResourceType,
    /// Action
    pub action: Action,
}

impl Permission {
    /// Create a new permission
    pub fn new(resource_type: ResourceType, action: Action) -> Self {
        Self {
            resource_type,
            action,
        }
    }

    /// Parse from string format "resource:action"
    pub fn from_string(s: &str) -> Result<Self, AuthzError> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err(AuthzError::InvalidPolicy(format!(
                "Invalid permission format: {}",
                s
            )));
        }

        Ok(Self {
            resource_type: ResourceType::from_str(parts[0]),
            action: Action::from_str(parts[1]),
        })
    }

    /// Convert to string format "resource:action"
    pub fn to_string(&self) -> String {
        format!("{:?}:{:?}", self.resource_type, self.action)
    }
}

/// Role in the system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Role {
    /// Role name
    pub name: String,
    /// User domain this role applies to
    pub domain: UserDomain,
}

impl Role {
    /// Create a new role
    pub fn new(name: String, domain: UserDomain) -> Self {
        Self { name, domain }
    }

    /// Create admin role
    pub fn admin() -> Self {
        Self::new("admin".to_string(), UserDomain::Admin)
    }

    /// Create retail trader role
    pub fn retail_trader() -> Self {
        Self::new("retail_trader".to_string(), UserDomain::Retail)
    }

    /// Create institutional trader role
    pub fn institutional_trader() -> Self {
        Self::new("institutional_trader".to_string(), UserDomain::Institutional)
    }

    /// Create compliance role
    pub fn compliance() -> Self {
        Self::new("compliance".to_string(), UserDomain::Compliance)
    }

    /// Create service account role
    pub fn service() -> Self {
        Self::new("service".to_string(), UserDomain::Service)
    }
}

/// Role assignment to a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleAssignment {
    /// User ID
    pub user_id: UserId,
    /// Role
    pub role: Role,
    /// Scope (optional - e.g., specific account)
    pub scope: Option<String>,
    /// Assignment metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Subject performing the action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    /// User ID
    pub user_id: UserId,
    /// User domain
    pub domain: UserDomain,
    /// User roles
    pub roles: Vec<Role>,
    /// Session ID
    pub session_id: Option<SessionId>,
    /// Subject attributes
    pub attributes: HashMap<String, serde_json::Value>,
}

impl Subject {
    /// Create a new subject
    pub fn new(user_id: UserId, domain: UserDomain) -> Self {
        Self {
            user_id,
            domain,
            roles: Vec::new(),
            session_id: None,
            attributes: HashMap::new(),
        }
    }

    /// Add a role
    pub fn with_role(mut self, role: Role) -> Self {
        self.roles.push(role);
        self
    }

    /// Add session
    pub fn with_session(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Add attribute
    pub fn with_attribute(mut self, key: String, value: serde_json::Value) -> Self {
        self.attributes.insert(key, value);
        self
    }

    /// Check if subject has role
    pub fn has_role(&self, role: &Role) -> bool {
        self.roles.contains(role)
    }

    /// Check if subject has any role with given name
    pub fn has_role_name(&self, name: &str) -> bool {
        self.roles.iter().any(|r| r.name == name)
    }
}

/// Authorization context (full context for decision)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthzContext {
    /// Subject (who)
    pub subject: Subject,
    /// Action (what)
    pub action: Action,
    /// Resource (on what)
    pub resource: Resource,
    /// Environment attributes (when, where, how)
    pub environment: HashMap<String, serde_json::Value>,
}

impl AuthzContext {
    /// Create a new authorization context
    pub fn new(subject: Subject, action: Action, resource: Resource) -> Self {
        Self {
            subject,
            action,
            resource,
            environment: HashMap::new(),
        }
    }

    /// Add environment attribute
    pub fn with_env(mut self, key: String, value: serde_json::Value) -> Self {
        self.environment.insert(key, value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_parsing() {
        let perm = Permission::from_string("order:read").unwrap();
        assert_eq!(perm.resource_type, ResourceType::Order);
        assert_eq!(perm.action, Action::Read);
    }

    #[test]
    fn test_resource_builder() {
        let resource = Resource::new(ResourceType::Order)
            .with_id("order-123".to_string())
            .with_owner(UserId::new())
            .with_attribute("status".to_string(), serde_json::json!("pending"));

        assert_eq!(resource.resource_type, ResourceType::Order);
        assert_eq!(resource.resource_id, Some("order-123".to_string()));
        assert!(resource.owner_id.is_some());
        assert_eq!(
            resource.attributes.get("status"),
            Some(&serde_json::json!("pending"))
        );
    }

    #[test]
    fn test_subject_roles() {
        let user_id = UserId::new();
        let subject = Subject::new(user_id, UserDomain::Retail)
            .with_role(Role::retail_trader())
            .with_role(Role::new("risk_manager".to_string(), UserDomain::Retail));

        assert!(subject.has_role(&Role::retail_trader()));
        assert!(subject.has_role_name("risk_manager"));
        assert!(!subject.has_role_name("admin"));
    }

    #[test]
    fn test_role_equality() {
        let role1 = Role::new("admin".to_string(), UserDomain::Admin);
        let role2 = Role::new("admin".to_string(), UserDomain::Admin);
        let role3 = Role::new("admin".to_string(), UserDomain::Retail);

        assert_eq!(role1, role2);
        assert_ne!(role1, role3);
    }
}
