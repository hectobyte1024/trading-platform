/// Authorization (RBAC + ABAC)
///
/// Combines role-based and attribute-based access control for fine-grained
/// authorization decisions across multi-domain trading platform.

mod types;
mod rbac;
mod abac;
mod policy;
mod checker;

pub use types::{
    AuthzError, Permission, Resource, Action, ResourceType,
    Role, RoleAssignment, Subject, AuthzContext,
};

pub use rbac::{
    RbacPolicy, RoleHierarchy, RoleDefinition, RoleManager,
};

pub use abac::{
    AbacPolicy, PolicyRule, PolicyEffect, Condition, ConditionOperator,
    PolicyEvaluator, EvaluationContext, EvaluationResult,
};

pub use policy::{
    PolicyStore, PolicyEngine, CombinedPolicy, PolicyDecision,
};

pub use checker::{
    PermissionChecker, AuthorizationMiddleware,
};
