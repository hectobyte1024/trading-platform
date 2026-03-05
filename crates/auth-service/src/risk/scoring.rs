//! Risk Scoring
//!
//! Real-time risk score calculation based on multiple factors.

// Re-export RiskScore from domain for convenience
pub use crate::domain::RiskScore;

use crate::domain::{UserId, SessionId, DeviceId};
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Risk level categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    /// Convert from risk score
    pub fn from_score(score: RiskScore) -> Self {
        let value = score.value();
        if value < 0.3 {
            Self::Low
        } else if value < 0.6 {
            Self::Medium
        } else if value < 0.85 {
            Self::High
        } else {
            Self::Critical
        }
    }

    /// Convert to risk score (midpoint of range)
    pub fn to_score(&self) -> RiskScore {
        match self {
            Self::Low => RiskScore::new(0.15),
            Self::Medium => RiskScore::new(0.45),
            Self::High => RiskScore::new(0.72),
            Self::Critical => RiskScore::new(0.92),
        }
    }
}

/// Factors contributing to risk score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactors {
    /// IP address changed from previous session
    pub ip_change: bool,
    /// IP address from known bad list
    pub ip_reputation_bad: bool,
    /// Device fingerprint doesn't match
    pub device_mismatch: bool,
    /// Device is new/unknown
    pub device_new: bool,
    /// Geographic location changed significantly
    pub location_anomaly: bool,
    /// Login time is unusual for this user
    pub time_anomaly: bool,
    /// Failed authentication attempts
    pub failed_attempts: u32,
    /// Velocity: logins per hour above threshold
    pub velocity_high: bool,
    /// Account age (days)
    pub account_age_days: u32,
    /// User has MFA enabled
    pub mfa_enabled: bool,
    /// Last successful login timestamp
    pub last_login: Option<DateTime<Utc>>,
    /// Days since last login
    pub days_since_last_login: Option<u32>,
    /// User domain (institutional users = lower risk)
    pub is_institutional: bool,
}

impl RiskFactors {
    /// Create default risk factors (all safe)
    pub fn default_safe() -> Self {
        Self {
            ip_change: false,
            ip_reputation_bad: false,
            device_mismatch: false,
            device_new: false,
            location_anomaly: false,
            time_anomaly: false,
            failed_attempts: 0,
            velocity_high: false,
            account_age_days: 365,
            mfa_enabled: true,
            last_login: Some(Utc::now() - Duration::hours(1)),
            days_since_last_login: Some(0),
            is_institutional: false,
        }
    }

    /// Create high-risk factors
    pub fn default_high_risk() -> Self {
        Self {
            ip_change: true,
            ip_reputation_bad: true,
            device_mismatch: true,
            device_new: true,
            location_anomaly: true,
            time_anomaly: true,
            failed_attempts: 5,
            velocity_high: true,
            account_age_days: 1,
            mfa_enabled: false,
            last_login: Some(Utc::now() - Duration::days(90)),
            days_since_last_login: Some(90),
            is_institutional: false,
        }
    }
}

/// Risk scorer
pub struct RiskScorer {
    /// Weights for each risk factor (0.0 - 1.0)
    weights: RiskWeights,
    /// Recent login attempts (for velocity detection)
    login_attempts: Arc<RwLock<HashMap<UserId, Vec<DateTime<Utc>>>>>,
}

/// Configurable weights for risk factors
#[derive(Debug, Clone)]
pub struct RiskWeights {
    pub ip_change: f32,
    pub ip_reputation_bad: f32,
    pub device_mismatch: f32,
    pub device_new: f32,
    pub location_anomaly: f32,
    pub time_anomaly: f32,
    pub failed_attempts_per: f32,
    pub velocity_high: f32,
    pub account_age_bonus: f32,
    pub mfa_enabled_bonus: f32,
    pub days_since_login_penalty: f32,
    pub institutional_bonus: f32,
}

impl Default for RiskWeights {
    fn default() -> Self {
        Self {
            ip_change: 0.15,
            ip_reputation_bad: 0.30,
            device_mismatch: 0.20,
            device_new: 0.10,
            location_anomaly: 0.25,
            time_anomaly: 0.10,
            failed_attempts_per: 0.10,
            velocity_high: 0.20,
            account_age_bonus: -0.10,
            mfa_enabled_bonus: -0.15,
            days_since_login_penalty: 0.05,
            institutional_bonus: -0.10,
        }
    }
}

impl RiskScorer {
    /// Create a new risk scorer with default weights
    pub fn new() -> Self {
        Self {
            weights: RiskWeights::default(),
            login_attempts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with custom weights
    pub fn with_weights(weights: RiskWeights) -> Self {
        Self {
            weights,
            login_attempts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Calculate risk score from factors
    pub fn calculate_score(&self, factors: &RiskFactors) -> RiskScore {
        let mut score = 0.0f32;

        // Binary factors (add weight if true)
        if factors.ip_change {
            score += self.weights.ip_change;
        }
        if factors.ip_reputation_bad {
            score += self.weights.ip_reputation_bad;
        }
        if factors.device_mismatch {
            score += self.weights.device_mismatch;
        }
        if factors.device_new {
            score += self.weights.device_new;
        }
        if factors.location_anomaly {
            score += self.weights.location_anomaly;
        }
        if factors.time_anomaly {
            score += self.weights.time_anomaly;
        }
        if factors.velocity_high {
            score += self.weights.velocity_high;
        }

        // Failed attempts (cumulative)
        score += (factors.failed_attempts as f32) * self.weights.failed_attempts_per;

        // Account age bonus (older accounts = lower risk)
        if factors.account_age_days > 30 {
            score += self.weights.account_age_bonus;
        }

        // MFA enabled bonus
        if factors.mfa_enabled {
            score += self.weights.mfa_enabled_bonus;
        }

        // Days since last login penalty
        if let Some(days) = factors.days_since_last_login {
            if days > 30 {
                score += self.weights.days_since_login_penalty * (days as f32 / 30.0);
            }
        }

        // Institutional user bonus
        if factors.is_institutional {
            score += self.weights.institutional_bonus;
        }

        // Base score (everyone starts with some risk)
        score += 0.2;

        RiskScore::new(score)
    }

    /// Record a login attempt for velocity tracking
    pub async fn record_login_attempt(&self, user_id: UserId) {
        let mut attempts = self.login_attempts.write().await;
        let now = Utc::now();
        
        // Get or create attempt list
        let user_attempts = attempts.entry(user_id).or_insert_with(Vec::new);
        
        // Add current attempt
        user_attempts.push(now);
        
        // Remove attempts older than 1 hour
        let hour_ago = now - Duration::hours(1);
        user_attempts.retain(|&timestamp| timestamp > hour_ago);
    }

    /// Check if velocity is high (too many logins in short time)
    pub async fn is_velocity_high(&self, user_id: UserId, threshold: usize) -> bool {
        let attempts = self.login_attempts.read().await;
        if let Some(user_attempts) = attempts.get(&user_id) {
            user_attempts.len() > threshold
        } else {
            false
        }
    }

    /// Get login attempt count in last hour
    pub async fn get_login_attempts(&self, user_id: UserId) -> usize {
        let attempts = self.login_attempts.read().await;
        attempts.get(&user_id).map(|v| v.len()).unwrap_or(0)
    }

    /// Clear old login attempts (cleanup)
    pub async fn cleanup_old_attempts(&self) {
        let mut attempts = self.login_attempts.write().await;
        let hour_ago = Utc::now() - Duration::hours(1);
        
        for user_attempts in attempts.values_mut() {
            user_attempts.retain(|&timestamp| timestamp > hour_ago);
        }
        
        // Remove users with no recent attempts
        attempts.retain(|_, v| !v.is_empty());
    }
}

impl Default for RiskScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_level_from_score() {
        assert_eq!(RiskLevel::from_score(RiskScore::new(0.1)), RiskLevel::Low);
        assert_eq!(RiskLevel::from_score(RiskScore::new(0.4)), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_score(RiskScore::new(0.7)), RiskLevel::High);
        assert_eq!(RiskLevel::from_score(RiskScore::new(0.9)), RiskLevel::Critical);
    }

    #[test]
    fn test_risk_level_to_score() {
        assert!(RiskLevel::Low.to_score().value() < 0.3);
        assert!(RiskLevel::Medium.to_score().value() >= 0.3);
        assert!(RiskLevel::High.to_score().value() >= 0.6);
        assert!(RiskLevel::Critical.to_score().value() >= 0.85);
    }

    #[test]
    fn test_safe_factors_score() {
        let scorer = RiskScorer::new();
        let factors = RiskFactors::default_safe();
        let score = scorer.calculate_score(&factors);
        
        // Safe factors should result in low risk
        assert!(score.value() < 0.4, "Safe factors should be low risk, got {}", score.value());
    }

    #[test]
    fn test_high_risk_factors_score() {
        let scorer = RiskScorer::new();
        let factors = RiskFactors::default_high_risk();
        let score = scorer.calculate_score(&factors);
        
        // High risk factors should result in high score
        assert!(score.value() > 0.7, "High risk factors should be high risk, got {}", score.value());
    }

    #[test]
    fn test_ip_change_increases_risk() {
        let scorer = RiskScorer::new();
        let mut factors = RiskFactors::default_safe();
        
        let score_before = scorer.calculate_score(&factors);
        factors.ip_change = true;
        let score_after = scorer.calculate_score(&factors);
        
        assert!(score_after.value() > score_before.value());
    }

    #[test]
    fn test_mfa_reduces_risk() {
        let scorer = RiskScorer::new();
        let mut factors = RiskFactors::default_safe();
        factors.mfa_enabled = true;
        
        let score_with_mfa = scorer.calculate_score(&factors);
        
        factors.mfa_enabled = false;
        let score_without_mfa = scorer.calculate_score(&factors);
        
        assert!(score_with_mfa.value() < score_without_mfa.value());
    }

    #[test]
    fn test_failed_attempts_increase_risk() {
        let scorer = RiskScorer::new();
        let mut factors = RiskFactors::default_safe();
        
        factors.failed_attempts = 0;
        let score_no_failures = scorer.calculate_score(&factors);
        
        factors.failed_attempts = 5;
        let score_with_failures = scorer.calculate_score(&factors);
        
        assert!(score_with_failures.value() > score_no_failures.value());
    }

    #[tokio::test]
    async fn test_velocity_tracking() {
        let scorer = RiskScorer::new();
        let user_id = UserId::new();
        
        // Record multiple login attempts
        for _ in 0..5 {
            scorer.record_login_attempt(user_id).await;
        }
        
        let count = scorer.get_login_attempts(user_id).await;
        assert_eq!(count, 5);
        
        let is_high = scorer.is_velocity_high(user_id, 3).await;
        assert!(is_high);
    }

    #[tokio::test]
    async fn test_cleanup_old_attempts() {
        let scorer = RiskScorer::new();
        let user_id = UserId::new();
        
        scorer.record_login_attempt(user_id).await;
        
        let count_before = scorer.get_login_attempts(user_id).await;
        assert_eq!(count_before, 1);
        
        // Cleanup should not remove recent attempts
        scorer.cleanup_old_attempts().await;
        
        let count_after = scorer.get_login_attempts(user_id).await;
        assert_eq!(count_after, 1);
    }

    #[test]
    fn test_institutional_user_bonus() {
        let scorer = RiskScorer::new();
        let mut factors = RiskFactors::default_safe();
        
        // Add some minor risk so we can see the bonus effect
        factors.ip_change = true; // Adds 0.15
        factors.mfa_enabled = false; // Removes -0.15 bonus
        
        // Retail user: 0.15 (IP change) + 0.2 (base) - 0.10 (account age) = 0.25
        factors.is_institutional = false;
        let score_retail = scorer.calculate_score(&factors);
        
        // Institutional: 0.15 + 0.2 - 0.10 - 0.10 (institutional) = 0.15
        factors.is_institutional = true;
        let score_institutional = scorer.calculate_score(&factors);
        
        assert!(score_institutional.value() < score_retail.value());
    }

    #[test]
    fn test_custom_weights() {
        let mut weights = RiskWeights::default();
        weights.ip_change = 0.5; // Increase IP change weight
        
        let scorer = RiskScorer::with_weights(weights);
        let mut factors = RiskFactors::default_safe();
        factors.ip_change = true;
        
        let score = scorer.calculate_score(&factors);
        
        // Should have higher score due to increased weight
        assert!(score.value() > 0.4);
    }
}
