//! Anomaly Detection
//!
//! Detect anomalous authentication events and patterns.

use crate::domain::{UserId, SessionId};
use crate::risk::behavioral::UserBehavior;
use crate::risk::scoring::{RiskScore, RiskLevel};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Type of anomaly detected
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyType {
    /// Impossible travel (login from distant location too quickly)
    ImpossibleTravel,
    /// Login velocity too high
    HighVelocity,
    /// Unusual time of day
    UnusualTime,
    /// Unknown device
    UnknownDevice,
    /// Unknown IP address
    UnknownIp,
    /// Geographic anomaly
    GeographicAnomaly,
    /// Credential stuffing pattern
    CredentialStuffing,
    /// Session hijacking indicators
    SessionHijacking,
    /// Brute force attempt
    BruteForce,
}

/// Detected anomaly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    /// Type of anomaly
    pub anomaly_type: AnomalyType,
    /// When detected
    pub detected_at: DateTime<Utc>,
    /// User ID (if known)
    pub user_id: Option<UserId>,
    /// Session ID (if applicable)
    pub session_id: Option<SessionId>,
    /// Description
    pub description: String,
    /// Risk score contribution
    pub risk_contribution: f32,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

impl Anomaly {
    /// Create a new anomaly
    pub fn new(
        anomaly_type: AnomalyType,
        user_id: Option<UserId>,
        description: impl Into<String>,
        risk_contribution: f32,
    ) -> Self {
        Self {
            anomaly_type,
            detected_at: Utc::now(),
            user_id,
            session_id: None,
            description: description.into(),
            risk_contribution,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set session ID
    pub fn with_session(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }
}

/// Anomaly detector
pub struct AnomalyDetector {
    /// Recent anomalies (for pattern detection)
    anomalies: Arc<RwLock<Vec<Anomaly>>>,
    /// Configuration for detection thresholds
    config: DetectorConfig,
}

/// Detector configuration
#[derive(Debug, Clone)]
pub struct DetectorConfig {
    /// Velocity threshold (logins per hour)
    pub velocity_threshold: usize,
    /// Impossible travel speed (km/h)
    pub impossible_travel_speed_kmh: f64,
    /// Brute force threshold (attempts in window)
    pub brute_force_attempts: usize,
    /// Brute force window (seconds)
    pub brute_force_window_secs: u64,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            velocity_threshold: 10,
            impossible_travel_speed_kmh: 1000.0, // Speed of commercial aircraft
            brute_force_attempts: 5,
            brute_force_window_secs: 300, // 5 minutes
        }
    }
}

impl AnomalyDetector {
    /// Create a new anomaly detector
    pub fn new(config: DetectorConfig) -> Self {
        Self {
            anomalies: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    /// Create with default config
    pub fn new_default() -> Self {
        Self::new(DetectorConfig::default())
    }

    /// Detect anomalies in authentication attempt
    pub async fn detect(
        &self,
        user_id: UserId,
        behavior: &UserBehavior,
        ip: &IpAddr,
        country: Option<&str>,
        failed_attempts: u32,
    ) -> Vec<Anomaly> {
        let mut detected = Vec::new();

        // Check velocity
        if behavior.total_logins > 10 {
            if let Some(last_login) = behavior.last_login {
                let time_since_last = (Utc::now() - last_login).num_seconds();
                if time_since_last < 60 {
                    detected.push(Anomaly::new(
                        AnomalyType::HighVelocity,
                        Some(user_id),
                        format!("Login only {}s after previous", time_since_last),
                        0.15,
                    ));
                }
            }
        }

        // Check unknown IP
        if !behavior.is_known_ip(ip) && behavior.total_logins > 5 {
            detected.push(Anomaly::new(
                AnomalyType::UnknownIp,
                Some(user_id),
                format!("Unknown IP address: {}", ip),
                0.20,
            ));
        }

        // Check geographic anomaly
        if let Some(c) = country {
            if !behavior.is_known_country(c) && behavior.total_logins > 3 {
                detected.push(Anomaly::new(
                    AnomalyType::GeographicAnomaly,
                    Some(user_id),
                    format!("Unusual country: {}", c),
                    0.25,
                ));
            }
        }

        // Check brute force
        if failed_attempts >= self.config.brute_force_attempts as u32 {
            detected.push(Anomaly::new(
                AnomalyType::BruteForce,
                Some(user_id),
                format!("{} failed attempts", failed_attempts),
                0.30,
            ));
        }

        // Record anomalies
        if !detected.is_empty() {
            let mut anomalies = self.anomalies.write().await;
            anomalies.extend(detected.clone());
            
            // Keep only last 1000 anomalies
            let len = anomalies.len();
            if len > 1000 {
                anomalies.drain(0..(len - 1000));
            }
        }

        detected
    }

    /// Get recent anomalies for a user
    pub async fn get_user_anomalies(&self, user_id: UserId, since: DateTime<Utc>) -> Vec<Anomaly> {
        let anomalies = self.anomalies.read().await;
        anomalies
            .iter()
            .filter(|a| a.user_id == Some(user_id) && a.detected_at >= since)
            .cloned()
            .collect()
    }

    /// Check if user has recent anomalies
    pub async fn has_recent_anomalies(&self, user_id: UserId, window_secs: u64) -> bool {
        let since = Utc::now() - chrono::Duration::seconds(window_secs as i64);
        let recent = self.get_user_anomalies(user_id, since).await;
        !recent.is_empty()
    }

    /// Calculate risk adjustment from anomalies
    pub async fn calculate_risk_adjustment(&self, user_id: UserId, window_secs: u64) -> f32 {
        let since = Utc::now() - chrono::Duration::seconds(window_secs as i64);
        let recent = self.get_user_anomalies(user_id, since).await;
        
        recent.iter().map(|a| a.risk_contribution).sum()
    }

    /// Clear old anomalies
    pub async fn cleanup(&self, older_than: DateTime<Utc>) {
        let mut anomalies = self.anomalies.write().await;
        anomalies.retain(|a| a.detected_at >= older_than);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DeviceId;

    #[test]
    fn test_anomaly_creation() {
        let anomaly = Anomaly::new(
            AnomalyType::UnknownIp,
            Some(UserId::new()),
            "Test anomaly",
            0.15,
        );

        assert_eq!(anomaly.anomaly_type, AnomalyType::UnknownIp);
        assert_eq!(anomaly.risk_contribution, 0.15);
    }

    #[test]
    fn test_anomaly_with_metadata() {
        let anomaly = Anomaly::new(
            AnomalyType::ImpossibleTravel,
            Some(UserId::new()),
            "Travel too fast",
            0.30,
        )
        .with_metadata("distance_km", "5000")
        .with_metadata("time_mins", "30");

        assert_eq!(anomaly.metadata.len(), 2);
        assert_eq!(anomaly.metadata.get("distance_km"), Some(&"5000".to_string()));
    }

    #[tokio::test]
    async fn test_detector_unknown_ip() {
        let detector = AnomalyDetector::new_default();
        let user_id = UserId::new();
        let mut behavior = UserBehavior::new(user_id);
        
        // Establish baseline with known IP
        let known_ip: IpAddr = "192.168.1.1".parse().unwrap();
        behavior.record_login(Utc::now(), known_ip, DeviceId::new(), None);
        behavior.total_logins = 10; // Set high enough to trigger detection

        // Login from unknown IP
        let unknown_ip: IpAddr = "10.0.0.1".parse().unwrap();
        let anomalies = detector.detect(user_id, &behavior, &unknown_ip, None, 0).await;

        assert!(!anomalies.is_empty());
        assert!(anomalies.iter().any(|a| a.anomaly_type == AnomalyType::UnknownIp));
    }

    #[tokio::test]
    async fn test_detector_brute_force() {
        let detector = AnomalyDetector::new_default();
        let user_id = UserId::new();
        let behavior = UserBehavior::new(user_id);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        let anomalies = detector.detect(user_id, &behavior, &ip, None, 5).await;

        assert!(anomalies.iter().any(|a| a.anomaly_type == AnomalyType::BruteForce));
    }

    #[tokio::test]
    async fn test_detector_geographic_anomaly() {
        let detector = AnomalyDetector::new_default();
        let user_id = UserId::new();
        let mut behavior = UserBehavior::new(user_id);
        behavior.total_logins = 5;
        behavior.known_countries.push("US".to_string());
        
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        let anomalies = detector.detect(user_id, &behavior, &ip, Some("RU"), 0).await;

        assert!(anomalies.iter().any(|a| a.anomaly_type == AnomalyType::GeographicAnomaly));
    }

    #[tokio::test]
    async fn test_get_user_anomalies() {
        let detector = AnomalyDetector::new_default();
        let user_id = UserId::new();
        let behavior = UserBehavior::new(user_id);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        // Trigger anomalies
        detector.detect(user_id, &behavior, &ip, None, 5).await;

        let since = Utc::now() - chrono::Duration::minutes(5);
        let recent = detector.get_user_anomalies(user_id, since).await;

        assert!(!recent.is_empty());
    }

    #[tokio::test]
    async fn test_has_recent_anomalies() {
        let detector = AnomalyDetector::new_default();
        let user_id = UserId::new();
        let behavior = UserBehavior::new(user_id);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        assert!(!detector.has_recent_anomalies(user_id, 300).await);

        // Trigger anomalies
        detector.detect(user_id, &behavior, &ip, None, 5).await;

        assert!(detector.has_recent_anomalies(user_id, 300).await);
    }

    #[tokio::test]
    async fn test_calculate_risk_adjustment() {
        let detector = AnomalyDetector::new_default();
        let user_id = UserId::new();
        let mut behavior = UserBehavior::new(user_id);
        behavior.total_logins = 10;
        
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        
        // Trigger multiple anomalies
        detector.detect(user_id, &behavior, &ip, Some("RU"), 5).await;

        let adjustment = detector.calculate_risk_adjustment(user_id, 300).await;
        assert!(adjustment > 0.0);
    }

    #[tokio::test]
    async fn test_cleanup() {
        let detector = AnomalyDetector::new_default();
        let user_id = UserId::new();
        let behavior = UserBehavior::new(user_id);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        // Create anomalies
        detector.detect(user_id, &behavior, &ip, None, 5).await;

        // Cleanup (keep all recent)
        detector.cleanup(Utc::now() - chrono::Duration::hours(1)).await;

        let recent = detector.get_user_anomalies(user_id, Utc::now() - chrono::Duration::hours(1)).await;
        assert!(!recent.is_empty());

        // Cleanup (remove all)
        detector.cleanup(Utc::now()).await;

        let recent = detector.get_user_anomalies(user_id, Utc::now() - chrono::Duration::hours(1)).await;
        assert!(recent.is_empty());
    }
}
