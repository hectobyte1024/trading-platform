//! Adaptive Authentication Policies
//!
//! Determine when to require step-up authentication based on risk.

use crate::risk::anomaly::{Anomaly, AnomalyType};
use crate::risk::behavioral::UserBehavior;
use crate::risk::device_reputation::DeviceReputation;
use crate::risk::scoring::{RiskLevel, RiskScore};
use serde::{Deserialize, Serialize};

/// Authentication requirement level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticationRequirement {
    /// Basic authentication (password)
    Basic,
    /// Require MFA
    Mfa,
    /// Require WebAuthn
    WebAuthn,
    /// Require both MFA and WebAuthn
    StrongMfa,
    /// Deny access
    Deny,
}

/// Reason for step-up requirement
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepUpReason {
    /// High risk score
    HighRisk,
    /// Critical risk score
    CriticalRisk,
    /// Anomaly detected
    AnomalyDetected(AnomalyType),
    /// New device
    NewDevice,
    /// Suspicious device
    SuspiciousDevice,
    /// Geographic anomaly
    GeographicAnomaly,
    /// High value operation
    HighValueOperation,
    /// Compliance requirement
    ComplianceRequired,
}

/// Trigger for step-up authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepUpTrigger {
    /// Required authentication level
    pub requirement: AuthenticationRequirement,
    /// Reasons for requirement
    pub reasons: Vec<StepUpReason>,
    /// User-facing message
    pub message: String,
}

impl StepUpTrigger {
    /// Create a new trigger
    pub fn new(requirement: AuthenticationRequirement, reason: StepUpReason) -> Self {
        let message = match requirement {
            AuthenticationRequirement::Basic => "Standard authentication required".to_string(),
            AuthenticationRequirement::Mfa => {
                "Additional verification required. Please use your authenticator app.".to_string()
            }
            AuthenticationRequirement::WebAuthn => {
                "Hardware security key required for this login.".to_string()
            }
            AuthenticationRequirement::StrongMfa => {
                "Enhanced security required. Please use both MFA and security key.".to_string()
            }
            AuthenticationRequirement::Deny => {
                "Access denied due to security concerns. Please contact support.".to_string()
            }
        };

        Self {
            requirement,
            reasons: vec![reason],
            message,
        }
    }

    /// Add additional reason
    pub fn with_reason(mut self, reason: StepUpReason) -> Self {
        self.reasons.push(reason);
        self
    }

    /// Check if access should be denied
    pub fn is_denied(&self) -> bool {
        self.requirement == AuthenticationRequirement::Deny
    }
}

/// Adaptive authentication policy
pub struct AdaptivePolicy {
    /// Risk threshold for MFA requirement
    mfa_threshold: f32,
    /// Risk threshold for WebAuthn requirement
    webauthn_threshold: f32,
    /// Risk threshold for denial
    deny_threshold: f32,
    /// Always require MFA for new devices
    require_mfa_new_device: bool,
    /// Always require WebAuthn for suspicious devices
    require_webauthn_suspicious: bool,
}

impl AdaptivePolicy {
    /// Create a new policy with thresholds
    pub fn new(
        mfa_threshold: f32,
        webauthn_threshold: f32,
        deny_threshold: f32,
    ) -> Self {
        Self {
            mfa_threshold,
            webauthn_threshold,
            deny_threshold,
            require_mfa_new_device: true,
            require_webauthn_suspicious: true,
        }
    }

    /// Create default policy
    ///
    /// - MFA required at risk >= 0.4
    /// - WebAuthn required at risk >= 0.7
    /// - Deny at risk >= 0.9
    pub fn default_policy() -> Self {
        Self::new(0.4, 0.7, 0.9)
    }

    /// Create strict policy for high-security environments
    pub fn strict() -> Self {
        Self {
            mfa_threshold: 0.2,
            webauthn_threshold: 0.5,
            deny_threshold: 0.85,
            require_mfa_new_device: true,
            require_webauthn_suspicious: true,
        }
    }

    /// Create permissive policy for low-security environments
    pub fn permissive() -> Self {
        Self {
            mfa_threshold: 0.6,
            webauthn_threshold: 0.8,
            deny_threshold: 0.95,
            require_mfa_new_device: false,
            require_webauthn_suspicious: false,
        }
    }

    /// Evaluate authentication requirements
    pub fn evaluate(
        &self,
        risk_score: RiskScore,
        risk_level: RiskLevel,
        anomalies: &[Anomaly],
        device_reputation: Option<&DeviceReputation>,
        _behavior: Option<&UserBehavior>,
    ) -> Option<StepUpTrigger> {
        let score = risk_score.value();

        // Check for denial threshold
        if score >= self.deny_threshold {
            return Some(StepUpTrigger::new(
                AuthenticationRequirement::Deny,
                StepUpReason::CriticalRisk,
            ));
        }

        // Check for critical anomalies
        let critical_anomalies: Vec<_> = anomalies
            .iter()
            .filter(|a| matches!(
                a.anomaly_type,
                AnomalyType::ImpossibleTravel
                    | AnomalyType::SessionHijacking
                    | AnomalyType::BruteForce
            ))
            .collect();

        if !critical_anomalies.is_empty() {
            let mut trigger = StepUpTrigger::new(
                AuthenticationRequirement::WebAuthn,
                StepUpReason::AnomalyDetected(critical_anomalies[0].anomaly_type.clone()),
            );
            for anomaly in &critical_anomalies[1..] {
                trigger = trigger.with_reason(StepUpReason::AnomalyDetected(anomaly.anomaly_type.clone()));
            }
            return Some(trigger);
        }

        // Check device reputation
        if let Some(device) = device_reputation {
            if device.score.is_suspicious() && self.require_webauthn_suspicious {
                return Some(StepUpTrigger::new(
                    AuthenticationRequirement::WebAuthn,
                    StepUpReason::SuspiciousDevice,
                ));
            }
        }

        // Check WebAuthn threshold
        if score >= self.webauthn_threshold {
            return Some(StepUpTrigger::new(
                AuthenticationRequirement::WebAuthn,
                StepUpReason::HighRisk,
            ));
        }

        // Check for geographic anomalies
        if anomalies.iter().any(|a| {
            matches!(
                a.anomaly_type,
                AnomalyType::GeographicAnomaly | AnomalyType::ImpossibleTravel
            )
        }) {
            return Some(StepUpTrigger::new(
                AuthenticationRequirement::Mfa,
                StepUpReason::GeographicAnomaly,
            ));
        }

        // Check MFA threshold
        if score >= self.mfa_threshold {
            return Some(StepUpTrigger::new(
                AuthenticationRequirement::Mfa,
                StepUpReason::HighRisk,
            ));
        }

        // Check for new device
        if let Some(device) = device_reputation {
            if device.successful_auths == 0 && self.require_mfa_new_device {
                return Some(StepUpTrigger::new(
                    AuthenticationRequirement::Mfa,
                    StepUpReason::NewDevice,
                ));
            }
        }

        // No step-up required
        None
    }

    /// Evaluate for high-value operations (transfers, etc.)
    pub fn evaluate_high_value_operation(
        &self,
        risk_score: RiskScore,
    ) -> Option<StepUpTrigger> {
        let score = risk_score.value();

        if score >= 0.5 {
            Some(StepUpTrigger::new(
                AuthenticationRequirement::WebAuthn,
                StepUpReason::HighValueOperation,
            ))
        } else if score >= 0.3 {
            Some(StepUpTrigger::new(
                AuthenticationRequirement::Mfa,
                StepUpReason::HighValueOperation,
            ))
        } else {
            None
        }
    }
}

impl Default for AdaptivePolicy {
    fn default() -> Self {
        Self::default_policy()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DeviceId, UserId};

    #[test]
    fn test_authentication_requirement_ordering() {
        assert!(AuthenticationRequirement::Mfa > AuthenticationRequirement::Basic);
        assert!(AuthenticationRequirement::WebAuthn > AuthenticationRequirement::Mfa);
        assert!(AuthenticationRequirement::StrongMfa > AuthenticationRequirement::WebAuthn);
        assert!(AuthenticationRequirement::Deny > AuthenticationRequirement::StrongMfa);
    }

    #[test]
    fn test_step_up_trigger() {
        let trigger = StepUpTrigger::new(
            AuthenticationRequirement::Mfa,
            StepUpReason::HighRisk,
        );

        assert_eq!(trigger.requirement, AuthenticationRequirement::Mfa);
        assert_eq!(trigger.reasons.len(), 1);
        assert!(!trigger.is_denied());
    }

    #[test]
    fn test_step_up_with_reason() {
        let trigger = StepUpTrigger::new(
            AuthenticationRequirement::WebAuthn,
            StepUpReason::HighRisk,
        )
        .with_reason(StepUpReason::NewDevice);

        assert_eq!(trigger.reasons.len(), 2);
    }

    #[test]
    fn test_policy_low_risk() {
        let policy = AdaptivePolicy::default_policy();
        let risk_score = RiskScore::new(0.2);
        let risk_level = RiskLevel::from_score(risk_score);

        let result = policy.evaluate(risk_score, risk_level, &[], None, None);
        assert!(result.is_none()); // No step-up for low risk
    }

    #[test]
    fn test_policy_mfa_threshold() {
        let policy = AdaptivePolicy::default_policy();
        let risk_score = RiskScore::new(0.5);
        let risk_level = RiskLevel::from_score(risk_score);

        let result = policy.evaluate(risk_score, risk_level, &[], None, None);
        assert!(result.is_some());
        let trigger = result.unwrap();
        assert_eq!(trigger.requirement, AuthenticationRequirement::Mfa);
    }

    #[test]
    fn test_policy_webauthn_threshold() {
        let policy = AdaptivePolicy::default_policy();
        let risk_score = RiskScore::new(0.75);
        let risk_level = RiskLevel::from_score(risk_score);

        let result = policy.evaluate(risk_score, risk_level, &[], None, None);
        assert!(result.is_some());
        let trigger = result.unwrap();
        assert_eq!(trigger.requirement, AuthenticationRequirement::WebAuthn);
    }

    #[test]
    fn test_policy_deny_threshold() {
        let policy = AdaptivePolicy::default_policy();
        let risk_score = RiskScore::new(0.95);
        let risk_level = RiskLevel::from_score(risk_score);

        let result = policy.evaluate(risk_score, risk_level, &[], None, None);
        assert!(result.is_some());
        let trigger = result.unwrap();
        assert_eq!(trigger.requirement, AuthenticationRequirement::Deny);
        assert!(trigger.is_denied());
    }

    #[test]
    fn test_policy_new_device() {
        let policy = AdaptivePolicy::default_policy();
        let risk_score = RiskScore::new(0.2);
        let risk_level = RiskLevel::from_score(risk_score);
        
        let device = DeviceReputation::new(DeviceId::new(), UserId::new());

        let result = policy.evaluate(risk_score, risk_level, &[], Some(&device), None);
        assert!(result.is_some());
        let trigger = result.unwrap();
        assert_eq!(trigger.requirement, AuthenticationRequirement::Mfa);
        assert!(trigger.reasons.contains(&StepUpReason::NewDevice));
    }

    #[test]
    fn test_policy_critical_anomaly() {
        let policy = AdaptivePolicy::default_policy();
        let risk_score = RiskScore::new(0.3);
        let risk_level = RiskLevel::from_score(risk_score);

        let anomaly = Anomaly::new(
            AnomalyType::ImpossibleTravel,
            Some(UserId::new()),
            "Travel too fast",
            0.3,
        );

        let result = policy.evaluate(risk_score, risk_level, &[anomaly], None, None);
        assert!(result.is_some());
        let trigger = result.unwrap();
        assert_eq!(trigger.requirement, AuthenticationRequirement::WebAuthn);
    }

    #[test]
    fn test_policy_geographic_anomaly() {
        let policy = AdaptivePolicy::default_policy();
        let risk_score = RiskScore::new(0.3);
        let risk_level = RiskLevel::from_score(risk_score);

        let anomaly = Anomaly::new(
            AnomalyType::GeographicAnomaly,
            Some(UserId::new()),
            "Unusual location",
            0.25,
        );

        let result = policy.evaluate(risk_score, risk_level, &[anomaly], None, None);
        assert!(result.is_some());
        let trigger = result.unwrap();
        assert_eq!(trigger.requirement, AuthenticationRequirement::Mfa);
    }

    #[test]
    fn test_strict_policy() {
        let policy = AdaptivePolicy::strict();
        let risk_score = RiskScore::new(0.3);
        let risk_level = RiskLevel::from_score(risk_score);

        let result = policy.evaluate(risk_score, risk_level, &[], None, None);
        assert!(result.is_some());
        let trigger = result.unwrap();
        assert_eq!(trigger.requirement, AuthenticationRequirement::Mfa);
    }

    #[test]
    fn test_permissive_policy() {
        let policy = AdaptivePolicy::permissive();
        let risk_score = RiskScore::new(0.5);
        let risk_level = RiskLevel::from_score(risk_score);

        let result = policy.evaluate(risk_score, risk_level, &[], None, None);
        assert!(result.is_none()); // Permissive policy allows higher risk
    }

    #[test]
    fn test_high_value_operation() {
        let policy = AdaptivePolicy::default_policy();
        
        let low_risk = RiskScore::new(0.2);
        assert!(policy.evaluate_high_value_operation(low_risk).is_none());

        let medium_risk = RiskScore::new(0.4);
        let result = policy.evaluate_high_value_operation(medium_risk);
        assert!(result.is_some());
        assert_eq!(result.unwrap().requirement, AuthenticationRequirement::Mfa);

        let high_risk = RiskScore::new(0.6);
        let result = policy.evaluate_high_value_operation(high_risk);
        assert!(result.is_some());
        assert_eq!(result.unwrap().requirement, AuthenticationRequirement::WebAuthn);
    }
}
