//! Role-Based Access Control (RBAC)
//!
//! Implements hierarchical roles with inherited permissions

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::types::{AuthzError, Permission, Role, ResourceType, Action};
use crate::domain::UserDomain;

/// Role definition with permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDefinition {
    /// Role
    pub role: Role,
    /// Direct permissions
    pub permissions: HashSet<Permission>,
    /// Parent roles (for inheritance)
    pub inherits_from: Vec<Role>,
    /// Description
    pub description: Option<String>,
}

impl RoleDefinition {
    /// Create a new role definition
    pub fn new(role: Role) -> Self {
        Self {
            role,
            permissions: HashSet::new(),
            inherits_from: Vec::new(),
            description: None,
        }
    }

    /// Add a permission
    pub fn add_permission(mut self, permission: Permission) -> Self {
        self.permissions.insert(permission);
        self
    }

    /// Add multiple permissions
    pub fn add_permissions(mut self, permissions: Vec<Permission>) -> Self {
        self.permissions.extend(permissions);
        self
    }

    /// Inherit from another role
    pub fn inherit_from(mut self, role: Role) -> Self {
        self.inherits_from.push(role);
        self
    }

    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
}

/// Role hierarchy manager
pub struct RoleHierarchy {
    /// Role definitions by role
    roles: HashMap<Role, RoleDefinition>,
}

impl RoleHierarchy {
    /// Create a new role hierarchy
    pub fn new() -> Self {
        Self {
            roles: HashMap::new(),
        }
    }

    /// Add a role definition
    pub fn add_role(&mut self, definition: RoleDefinition) -> Result<(), AuthzError> {
        // Check for circular dependencies
        self.check_circular_dependency(&definition)?;

        self.roles.insert(definition.role.clone(), definition);
        Ok(())
    }

    /// Get a role definition
    pub fn get_role(&self, role: &Role) -> Option<&RoleDefinition> {
        self.roles.get(role)
    }

    /// Get all permissions for a role (including inherited)
    pub fn get_all_permissions(&self, role: &Role) -> Result<HashSet<Permission>, AuthzError> {
        let mut permissions = HashSet::new();
        let mut visited = HashSet::new();
        self.collect_permissions(role, &mut permissions, &mut visited)?;
        Ok(permissions)
    }

    /// Recursively collect permissions (with cycle detection)
    fn collect_permissions(
        &self,
        role: &Role,
        permissions: &mut HashSet<Permission>,
        visited: &mut HashSet<Role>,
    ) -> Result<(), AuthzError> {
        if visited.contains(role) {
            return Err(AuthzError::CircularDependency);
        }

        visited.insert(role.clone());

        let definition = self
            .roles
            .get(role)
            .ok_or_else(|| AuthzError::RoleNotFound(role.name.clone()))?;

        // Add direct permissions
        permissions.extend(definition.permissions.clone());

        // Add inherited permissions
        for parent in &definition.inherits_from {
            self.collect_permissions(parent, permissions, visited)?;
        }

        Ok(())
    }

    /// Check for circular dependencies
    fn check_circular_dependency(&self, definition: &RoleDefinition) -> Result<(), AuthzError> {
        let mut visited = HashSet::new();
        visited.insert(definition.role.clone());
        
        for parent in &definition.inherits_from {
            self.check_circular_rec(&definition.role, parent, &mut visited)?;
        }
        
        Ok(())
    }

    fn check_circular_rec(
        &self,
        original: &Role,
        current: &Role,
        visited: &mut HashSet<Role>,
    ) -> Result<(), AuthzError> {
        // If we've seen this role before, we have a cycle
        if visited.contains(current) {
            return Err(AuthzError::CircularDependency);
        }

        visited.insert(current.clone());

        // Check parents of current role
        if let Some(current_def) = self.roles.get(current) {
            for parent in &current_def.inherits_from {
                self.check_circular_rec(original, parent, visited)?;
            }
        }

        Ok(())
    }
}

impl Default for RoleHierarchy {
    fn default() -> Self {
        Self::new()
    }
}

/// RBAC policy (role-based access control)
pub struct RbacPolicy {
    /// Role hierarchy
    hierarchy: RoleHierarchy,
}

impl RbacPolicy {
    /// Create a new RBAC policy
    pub fn new(hierarchy: RoleHierarchy) -> Self {
        Self { hierarchy }
    }

    /// Create default policy with standard roles
    pub fn default_policy() -> Result<Self, AuthzError> {
        let mut hierarchy = RoleHierarchy::new();

        // Admin role (all permissions)
        let admin = RoleDefinition::new(Role::admin())
            .add_permissions(vec![
                Permission::new(ResourceType::Order, Action::Read),
                Permission::new(ResourceType::Order, Action::Create),
                Permission::new(ResourceType::Order, Action::Update),
                Permission::new(ResourceType::Order, Action::Delete),
                Permission::new(ResourceType::Account, Action::Read),
                Permission::new(ResourceType::Account, Action::Create),
                Permission::new(ResourceType::Account, Action::Update),
                Permission::new(ResourceType::Account, Action::Delete),
                Permission::new(ResourceType::User, Action::Read),
                Permission::new(ResourceType::User, Action::Create),
                Permission::new(ResourceType::User, Action::Update),
                Permission::new(ResourceType::User, Action::Delete),
                Permission::new(ResourceType::Config, Action::Read),
                Permission::new(ResourceType::Config, Action::Update),
                Permission::new(ResourceType::AuditLog, Action::Read),
                Permission::new(ResourceType::AuditLog, Action::Export),
            ])
            .with_description("Administrator with full access".to_string());

        // Compliance role (audit and review)
        let compliance = RoleDefinition::new(Role::compliance())
            .add_permissions(vec![
                Permission::new(ResourceType::Order, Action::Read),
                Permission::new(ResourceType::Order, Action::Approve),
                Permission::new(ResourceType::Order, Action::Reject),
                Permission::new(ResourceType::Position, Action::Read),
                Permission::new(ResourceType::Account, Action::Read),
                Permission::new(ResourceType::AuditLog, Action::Read),
                Permission::new(ResourceType::AuditLog, Action::Export),
            ])
            .with_description("Compliance team with audit permissions".to_string());

        // Retail trader role (create/manage own orders)
        let retail_trader = RoleDefinition::new(Role::retail_trader())
            .add_permissions(vec![
                Permission::new(ResourceType::Order, Action::Read),
                Permission::new(ResourceType::Order, Action::Create),
                Permission::new(ResourceType::Order, Action::Update),
                Permission::new(ResourceType::Order, Action::Cancel),
                Permission::new(ResourceType::Position, Action::Read),
                Permission::new(ResourceType::MarketData, Action::Read),
                Permission::new(ResourceType::Account, Action::Read),
            ])
            .with_description("Retail trader with order management permissions".to_string());

        // Institutional trader role (enhanced permissions)
        let institutional_trader = RoleDefinition::new(Role::institutional_trader())
            .add_permissions(vec![
                Permission::new(ResourceType::Order, Action::Read),
                Permission::new(ResourceType::Order, Action::Create),
                Permission::new(ResourceType::Order, Action::Update),
                Permission::new(ResourceType::Order, Action::Cancel),
                Permission::new(ResourceType::Position, Action::Read),
                Permission::new(ResourceType::MarketData, Action::Read),
                Permission::new(ResourceType::MarketData, Action::Export),
                Permission::new(ResourceType::Account, Action::Read),
                Permission::new(ResourceType::Account, Action::Update),
            ])
            .with_description("Institutional trader with enhanced permissions".to_string());

        hierarchy.add_role(admin)?;
        hierarchy.add_role(compliance)?;
        hierarchy.add_role(retail_trader)?;
        hierarchy.add_role(institutional_trader)?;

        Ok(Self::new(hierarchy))
    }

    /// Check if roles have permission
    pub fn has_permission(&self, roles: &[Role], permission: &Permission) -> bool {
        for role in roles {
            if let Ok(permissions) = self.hierarchy.get_all_permissions(role) {
                if permissions.contains(permission) {
                    return true;
                }
            }
        }
        false
    }

    /// Get all permissions for roles
    pub fn get_permissions(&self, roles: &[Role]) -> Result<HashSet<Permission>, AuthzError> {
        let mut all_permissions = HashSet::new();
        for role in roles {
            let permissions = self.hierarchy.get_all_permissions(role)?;
            all_permissions.extend(permissions);
        }
        Ok(all_permissions)
    }
}

/// Role manager for assigning roles to users
pub struct RoleManager {
    /// User role assignments (user_id -> roles)
    assignments: HashMap<String, Vec<Role>>,
}

impl RoleManager {
    /// Create a new role manager
    pub fn new() -> Self {
        Self {
            assignments: HashMap::new(),
        }
    }

    /// Assign a role to a user
    pub fn assign_role(&mut self, user_id: String, role: Role) {
        self.assignments
            .entry(user_id)
            .or_insert_with(Vec::new)
            .push(role);
    }

    /// Revoke a role from a user
    pub fn revoke_role(&mut self, user_id: &str, role: &Role) {
        if let Some(roles) = self.assignments.get_mut(user_id) {
            roles.retain(|r| r != role);
        }
    }

    /// Get user roles
    pub fn get_user_roles(&self, user_id: &str) -> Vec<Role> {
        self.assignments
            .get(user_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Check if user has role
    pub fn has_role(&self, user_id: &str, role: &Role) -> bool {
        self.get_user_roles(user_id).contains(role)
    }
}

impl Default for RoleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_definition() {
        let role = Role::new("test_role".to_string(), UserDomain::Retail);
        let perm = Permission::new(ResourceType::Order, Action::Read);

        let definition = RoleDefinition::new(role.clone())
            .add_permission(perm.clone())
            .with_description("Test role".to_string());

        assert_eq!(definition.role, role);
        assert!(definition.permissions.contains(&perm));
        assert_eq!(definition.description, Some("Test role".to_string()));
    }

    #[test]
    fn test_role_hierarchy() {
        let mut hierarchy = RoleHierarchy::new();

        let viewer = RoleDefinition::new(Role::new("viewer".to_string(), UserDomain::Retail))
            .add_permission(Permission::new(ResourceType::Order, Action::Read));

        let trader = RoleDefinition::new(Role::retail_trader())
            .add_permission(Permission::new(ResourceType::Order, Action::Create))
            .inherit_from(Role::new("viewer".to_string(), UserDomain::Retail));

        hierarchy.add_role(viewer).unwrap();
        hierarchy.add_role(trader).unwrap();

        // Trader should have both create and read (inherited)
        let permissions = hierarchy.get_all_permissions(&Role::retail_trader()).unwrap();
        assert_eq!(permissions.len(), 2);
        assert!(permissions.contains(&Permission::new(ResourceType::Order, Action::Read)));
        assert!(permissions.contains(&Permission::new(ResourceType::Order, Action::Create)));
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut hierarchy = RoleHierarchy::new();

        let role_a = Role::new("role_a".to_string(), UserDomain::Retail);
        let role_b = Role::new("role_b".to_string(), UserDomain::Retail);

        // Add role_a that inherits from role_b
        hierarchy
            .add_role(RoleDefinition::new(role_a.clone()).inherit_from(role_b.clone()))
            .unwrap();

        // Try to add role_b that inherits from role_a (circular)
        let result = hierarchy.add_role(RoleDefinition::new(role_b).inherit_from(role_a));

        assert!(matches!(result, Err(AuthzError::CircularDependency)));
    }

    #[test]
    fn test_default_rbac_policy() {
        let policy = RbacPolicy::default_policy().unwrap();

        // Admin should have user management permissions
        assert!(policy.has_permission(
            &[Role::admin()],
            &Permission::new(ResourceType::User, Action::Delete)
        ));

        // Retail trader should have order creation
        assert!(policy.has_permission(
            &[Role::retail_trader()],
            &Permission::new(ResourceType::Order, Action::Create)
        ));

        // Compliance should NOT have order creation
        assert!(!policy.has_permission(
            &[Role::compliance()],
            &Permission::new(ResourceType::Order, Action::Create)
        ));

        // Compliance should have audit log access
        assert!(policy.has_permission(
            &[Role::compliance()],
            &Permission::new(ResourceType::AuditLog, Action::Read)
        ));
    }

    #[test]
    fn test_role_manager() {
        let mut manager = RoleManager::new();
        let user_id = "user-123".to_string();

        manager.assign_role(user_id.clone(), Role::retail_trader());
        manager.assign_role(user_id.clone(), Role::compliance());

        assert_eq!(manager.get_user_roles(&user_id).len(), 2);
        assert!(manager.has_role(&user_id, &Role::retail_trader()));

        manager.revoke_role(&user_id, &Role::retail_trader());
        assert_eq!(manager.get_user_roles(&user_id).len(), 1);
        assert!(!manager.has_role(&user_id, &Role::retail_trader()));
    }
}
