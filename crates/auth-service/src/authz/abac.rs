//! Attribute-Based Access Control (ABAC)
//!
//! Policy engine for fine-grained, context-aware authorization

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

use super::types::{AuthzContext, AuthzError, Action, ResourceType};

/// Policy effect (allow or deny)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicyEffect {
    /// Allow the action
    Allow,
    /// Deny the action
    Deny,
}

/// Condition operator for policy rules
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    /// Equals
    Equals,
    /// Not equals
    NotEquals,
    /// Greater than
    GreaterThan,
    /// Greater than or equal
    GreaterThanOrEqual,
    /// Less than
    LessThan,
    /// Less than or equal
    LessThanOrEqual,
    /// In (array contains)
    In,
    /// Not in
    NotIn,
    /// String contains
    Contains,
    /// String starts with
    StartsWith,
    /// String ends with
    EndsWith,
    /// Exists (attribute is present)
    Exists,
    /// Not exists
    NotExists,
}

/// Condition for policy rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    /// Attribute path (e.g., "subject.domain", "resource.owner_id")
    pub attribute: String,
    /// Operator
    pub operator: ConditionOperator,
    /// Value to compare against
    pub value: JsonValue,
}

impl Condition {
    /// Create a new condition
    pub fn new(attribute: String, operator: ConditionOperator, value: JsonValue) -> Self {
        Self {
            attribute,
            operator,
            value,
        }
    }

    /// Evaluate condition against context
    pub fn evaluate(&self, context: &EvaluationContext) -> bool {
        let actual = match self.extract_value(context) {
            Some(v) => v,
            None => {
                // Attribute doesn't exist
                return self.operator == ConditionOperator::NotExists;
            }
        };

        if self.operator == ConditionOperator::Exists {
            return true;
        }

        match self.operator {
            ConditionOperator::Equals => actual == self.value,
            ConditionOperator::NotEquals => actual != self.value,
            ConditionOperator::GreaterThan => {
                self.compare_numbers(&actual, &self.value, |a, b| a > b)
            }
            ConditionOperator::GreaterThanOrEqual => {
                self.compare_numbers(&actual, &self.value, |a, b| a >= b)
            }
            ConditionOperator::LessThan => {
                self.compare_numbers(&actual, &self.value, |a, b| a < b)
            }
            ConditionOperator::LessThanOrEqual => {
                self.compare_numbers(&actual, &self.value, |a, b| a <= b)
            }
            ConditionOperator::In => {
                if let JsonValue::Array(arr) = &self.value {
                    arr.contains(&actual)
                } else {
                    false
                }
            }
            ConditionOperator::NotIn => {
                if let JsonValue::Array(arr) = &self.value {
                    !arr.contains(&actual)
                } else {
                    true
                }
            }
            ConditionOperator::Contains => {
                if let (JsonValue::String(s1), JsonValue::String(s2)) =
                    (&actual, &self.value)
                {
                    s1.contains(s2)
                } else {
                    false
                }
            }
            ConditionOperator::StartsWith => {
                if let (JsonValue::String(s1), JsonValue::String(s2)) =
                    (&actual, &self.value)
                {
                    s1.starts_with(s2)
                } else {
                    false
                }
            }
            ConditionOperator::EndsWith => {
                if let (JsonValue::String(s1), JsonValue::String(s2)) =
                    (&actual, &self.value)
                {
                    s1.ends_with(s2)
                } else {
                    false
                }
            }
            ConditionOperator::Exists | ConditionOperator::NotExists => unreachable!(),
        }
    }

    /// Extract value from context using attribute path
    fn extract_value(&self, context: &EvaluationContext) -> Option<JsonValue> {
        let parts: Vec<&str> = self.attribute.split('.').collect();
        if parts.is_empty() {
            return None;
        }

        match parts[0] {
            "subject" => self.extract_from_map(&context.subject_attributes, &parts[1..]),
            "resource" => self.extract_from_map(&context.resource_attributes, &parts[1..]),
            "environment" => self.extract_from_map(&context.environment_attributes, &parts[1..]),
            _ => None,
        }
    }

    /// Extract value from map using path
    fn extract_from_map(&self, map: &HashMap<String, JsonValue>, path: &[&str]) -> Option<JsonValue> {
        if path.is_empty() {
            return None;
        }

        let mut current = map.get(path[0])?.clone();

        for key in &path[1..] {
            if let JsonValue::Object(obj) = current {
                current = obj.get(*key)?.clone();
            } else {
                return None;
            }
        }

        Some(current)
    }

    /// Compare numbers
    fn compare_numbers<F>(&self, a: &JsonValue, b: &JsonValue, f: F) -> bool
    where
        F: Fn(f64, f64) -> bool,
    {
        if let (Some(n1), Some(n2)) = (a.as_f64(), b.as_f64()) {
            f(n1, n2)
        } else {
            false
        }
    }
}

/// Policy rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    /// Rule ID
    pub id: String,
    /// Effect
    pub effect: PolicyEffect,
    /// Resource types this rule applies to
    pub resources: Vec<ResourceType>,
    /// Actions this rule applies to
    pub actions: Vec<Action>,
    /// Conditions (must all be true)
    pub conditions: Vec<Condition>,
    /// Description
    pub description: Option<String>,
}

impl PolicyRule {
    /// Create a new policy rule
    pub fn new(id: String, effect: PolicyEffect) -> Self {
        Self {
            id,
            effect,
            resources: Vec::new(),
            actions: Vec::new(),
            conditions: Vec::new(),
            description: None,
        }
    }

    /// Add resource type
    pub fn add_resource(mut self, resource: ResourceType) -> Self {
        self.resources.push(resource);
        self
    }

    /// Add action
    pub fn add_action(mut self, action: Action) -> Self {
        self.actions.push(action);
        self
    }

    /// Add condition
    pub fn add_condition(mut self, condition: Condition) -> Self {
        self.conditions.push(condition);
        self
    }

    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Check if rule applies to context
    pub fn applies_to(&self, context: &AuthzContext) -> bool {
        // Check resource type
        if !self.resources.is_empty()
            && !self.resources.contains(&context.resource.resource_type)
        {
            return false;
        }

        // Check action
        if !self.actions.is_empty() && !self.actions.contains(&context.action) {
            return false;
        }

        true
    }

    /// Evaluate rule against context
    pub fn evaluate(&self, eval_context: &EvaluationContext) -> bool {
        // All conditions must be true
        self.conditions.iter().all(|c| c.evaluate(eval_context))
    }
}

/// ABAC policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbacPolicy {
    /// Policy rules
    pub rules: Vec<PolicyRule>,
}

impl AbacPolicy {
    /// Create a new ABAC policy
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a rule
    pub fn add_rule(mut self, rule: PolicyRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Create default policy with common rules
    pub fn default_policy() -> Self {
        let mut policy = Self::new();

        // Rule: Deny access during maintenance window
        let maintenance_rule = PolicyRule::new("maintenance".to_string(), PolicyEffect::Deny)
            .add_condition(Condition::new(
                "environment.maintenance_mode".to_string(),
                ConditionOperator::Equals,
                JsonValue::Bool(true),
            ))
            .with_description("Deny all access during maintenance".to_string());

        policy = policy.add_rule(maintenance_rule);
        policy
    }
}

impl Default for AbacPolicy {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluation context (flattened for ABAC)
#[derive(Debug, Clone)]
pub struct EvaluationContext {
    /// Subject attributes
    pub subject_attributes: HashMap<String, JsonValue>,
    /// Resource attributes
    pub resource_attributes: HashMap<String, JsonValue>,
    /// Environment attributes
    pub environment_attributes: HashMap<String, JsonValue>,
}

impl EvaluationContext {
    /// Create from authorization context
    pub fn from_authz_context(context: &AuthzContext) -> Self {
        let mut subject_attrs = HashMap::new();
        subject_attrs.insert(
            "user_id".to_string(),
            JsonValue::String(context.subject.user_id.to_string()),
        );
        subject_attrs.insert(
            "domain".to_string(),
            JsonValue::String(format!("{:?}", context.subject.domain)),
        );
        subject_attrs.extend(context.subject.attributes.clone());

        let mut resource_attrs = HashMap::new();
        if let Some(owner_id) = &context.resource.owner_id {
            resource_attrs.insert(
                "owner_id".to_string(),
                JsonValue::String(owner_id.to_string()),
            );
        }
        resource_attrs.extend(context.resource.attributes.clone());

        Self {
            subject_attributes: subject_attrs,
            resource_attributes: resource_attrs,
            environment_attributes: context.environment.clone(),
        }
    }
}

/// Evaluation result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluationResult {
    /// Explicitly allowed
    Allow,
    /// Explicitly denied
    Deny,
    /// No applicable rules (default deny)
    NotApplicable,
}

/// Policy evaluator
pub struct PolicyEvaluator {
    /// ABAC policy
    policy: AbacPolicy,
}

impl PolicyEvaluator {
    /// Create a new policy evaluator
    pub fn new(policy: AbacPolicy) -> Self {
        Self { policy }
    }

    /// Evaluate context against policy
    pub fn evaluate(&self, context: &AuthzContext) -> Result<EvaluationResult, AuthzError> {
        let eval_context = EvaluationContext::from_authz_context(context);

        let mut has_allow = false;
        let mut has_deny = false;

        for rule in &self.policy.rules {
            if !rule.applies_to(context) {
                continue;
            }

            if rule.evaluate(&eval_context) {
                match rule.effect {
                    PolicyEffect::Deny => {
                        has_deny = true;
                        // Explicit deny always wins - return immediately
                        return Ok(EvaluationResult::Deny);
                    }
                    PolicyEffect::Allow => {
                        has_allow = true;
                    }
                }
            }
        }

        if has_allow {
            Ok(EvaluationResult::Allow)
        } else if has_deny {
            Ok(EvaluationResult::Deny)
        } else {
            Ok(EvaluationResult::NotApplicable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authz::types::{Resource, Subject};
    use crate::domain::{UserId, UserDomain};

    #[test]
    fn test_condition_equals() {
        let condition = Condition::new(
            "subject.domain".to_string(),
            ConditionOperator::Equals,
            JsonValue::String("Retail".to_string()),
        );

        let mut subject_attrs = HashMap::new();
        subject_attrs.insert("domain".to_string(), JsonValue::String("Retail".to_string()));

        let context = EvaluationContext {
            subject_attributes: subject_attrs,
            resource_attributes: HashMap::new(),
            environment_attributes: HashMap::new(),
        };

        assert!(condition.evaluate(&context));
    }

    #[test]
    fn test_condition_greater_than() {
        let condition = Condition::new(
            "resource.amount".to_string(),
            ConditionOperator::GreaterThan,
            JsonValue::Number(serde_json::Number::from(1000)),
        );

        let mut resource_attrs = HashMap::new();
        resource_attrs.insert("amount".to_string(), JsonValue::Number(serde_json::Number::from(2000)));

        let context = EvaluationContext {
            subject_attributes: HashMap::new(),
            resource_attributes: resource_attrs,
            environment_attributes: HashMap::new(),
        };

        assert!(condition.evaluate(&context));
    }

    #[test]
    fn test_policy_evaluation() {
        let user_id = UserId::new();
        let subject = Subject::new(user_id, UserDomain::Retail);
        let resource = Resource::new(ResourceType::Order).with_owner(user_id);

        let context = AuthzContext::new(subject, Action::Update, resource);

        let policy = AbacPolicy::default_policy();
        let evaluator = PolicyEvaluator::new(policy);

        let result = evaluator.evaluate(&context).unwrap();
        // Should be NotApplicable since we don't have exact matching conditions
        assert!(matches!(
            result,
            EvaluationResult::NotApplicable | EvaluationResult::Allow
        ));
    }

    #[test]
    fn test_maintenance_mode_deny() {
        let user_id = UserId::new();
        let subject = Subject::new(user_id, UserDomain::Retail);
        let resource = Resource::new(ResourceType::Order);

        let context = AuthzContext::new(subject, Action::Create, resource).with_env(
            "maintenance_mode".to_string(),
            JsonValue::Bool(true),
        );

        let policy = AbacPolicy::default_policy();
        let evaluator = PolicyEvaluator::new(policy);

        let result = evaluator.evaluate(&context).unwrap();
        assert_eq!(result, EvaluationResult::Deny);
    }
}
