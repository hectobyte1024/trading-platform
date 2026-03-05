//! Device Reputation Tracking
//!
//! Track device trust scores based on authentication history.

use crate::domain::{DeviceId, UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Device reputation score
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ReputationScore(f32);

impl ReputationScore {
    /// Create a new reputation score (clamped 0.0-1.0)
    pub fn new(score: f32) -> Self {
        Self(score.clamp(0.0, 1.0))
    }

    /// Get the score value
    pub fn value(&self) -> f32 {
        self.0
    }

    /// Check if trusted (>= 0.7)
    pub fn is_trusted(&self) -> bool {
        self.0 >= 0.7
    }

    /// Check if suspicious (<= 0.3)
    pub fn is_suspicious(&self) -> bool {
        self.0 <= 0.3
    }
}

impl Default for ReputationScore {
    fn default() -> Self {
        Self(0.5) // Neutral score for new devices
    }
}

/// Device reputation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceReputation {
    /// Device ID
    pub device_id: DeviceId,
    /// Associated user IDs
    pub users: Vec<UserId>,
    /// Reputation score
    pub score: ReputationScore,
    /// First seen
    pub first_seen: DateTime<Utc>,
    /// Last seen
    pub last_seen: DateTime<Utc>,
    /// Total successful authentications
    pub successful_auths: u32,
    /// Total failed authentications
    pub failed_auths: u32,
    /// Times flagged as suspicious
    pub flags: u32,
    /// Notes about the device
    pub notes: Vec<String>,
}

impl DeviceReputation {
    /// Create a new device reputation
    pub fn new(device_id: DeviceId, user_id: UserId) -> Self {
        let now = Utc::now();
        Self {
            device_id,
            users: vec![user_id],
            score: ReputationScore::default(),
            first_seen: now,
            last_seen: now,
            successful_auths: 0,
            failed_auths: 0,
            flags: 0,
            notes: Vec::new(),
        }
    }

    /// Record a successful authentication
    pub fn record_success(&mut self, user_id: UserId) {
        self.successful_auths += 1;
        self.last_seen = Utc::now();
        
        if !self.users.contains(&user_id) {
            self.users.push(user_id);
        }

        // Improve score for successful auth
        let current = self.score.value();
        self.score = ReputationScore::new(current + 0.05);
    }

    /// Record a failed authentication
    pub fn record_failure(&mut self) {
        self.failed_auths += 1;
        self.last_seen = Utc::now();
        
        // Decrease score for failed auth
        let current = self.score.value();
        self.score = ReputationScore::new(current - 0.10);
    }

    /// Flag the device as suspicious
    pub fn flag(&mut self, reason: impl Into<String>) {
        self.flags += 1;
        self.notes.push(reason.into());
        
        // Significantly decrease score when flagged
        let current = self.score.value();
        self.score = ReputationScore::new(current - 0.20);
    }

    /// Calculate trust multiplier for risk scoring
    pub fn trust_multiplier(&self) -> f32 {
        if self.score.is_trusted() {
            0.7 // Reduce risk by 30%
        } else if self.score.is_suspicious() {
            1.5 // Increase risk by 50%
        } else {
            1.0 // No adjustment
        }
    }

    /// Check if device has multiple users (potential account sharing)
    pub fn has_multiple_users(&self) -> bool {
        self.users.len() > 1
    }

    /// Age of device in days
    pub fn age_days(&self) -> i64 {
        (Utc::now() - self.first_seen).num_days()
    }
}

/// Device reputation tracker
pub struct DeviceReputationTracker {
    /// Device reputations
    reputations: Arc<RwLock<HashMap<DeviceId, DeviceReputation>>>,
}

impl DeviceReputationTracker {
    /// Create a new tracker
    pub fn new() -> Self {
        Self {
            reputations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create device reputation
    pub async fn get_or_create(&self, device_id: DeviceId, user_id: UserId) -> DeviceReputation {
        let mut reputations = self.reputations.write().await;
        reputations
            .entry(device_id)
            .or_insert_with(|| DeviceReputation::new(device_id, user_id))
            .clone()
    }

    /// Get device reputation
    pub async fn get(&self, device_id: DeviceId) -> Option<DeviceReputation> {
        let reputations = self.reputations.read().await;
        reputations.get(&device_id).cloned()
    }

    /// Record successful authentication
    pub async fn record_success(&self, device_id: DeviceId, user_id: UserId) {
        let mut reputations = self.reputations.write().await;
        if let Some(rep) = reputations.get_mut(&device_id) {
            rep.record_success(user_id);
        } else {
            let mut rep = DeviceReputation::new(device_id, user_id);
            rep.record_success(user_id);
            reputations.insert(device_id, rep);
        }
    }

    /// Record failed authentication
    pub async fn record_failure(&self, device_id: DeviceId) {
        let mut reputations = self.reputations.write().await;
        if let Some(rep) = reputations.get_mut(&device_id) {
            rep.record_failure();
        }
    }

    /// Flag device as suspicious
    pub async fn flag_device(&self, device_id: DeviceId, reason: impl Into<String>) {
        let mut reputations = self.reputations.write().await;
        if let Some(rep) = reputations.get_mut(&device_id) {
            rep.flag(reason);
        }
    }

    /// Get reputation score for risk calculation
    pub async fn get_score(&self, device_id: DeviceId) -> ReputationScore {
        let reputations = self.reputations.read().await;
        reputations
            .get(&device_id)
            .map(|r| r.score)
            .unwrap_or_default()
    }

    /// Get trust multiplier for device
    pub async fn get_trust_multiplier(&self, device_id: DeviceId) -> f32 {
        let reputations = self.reputations.read().await;
        reputations
            .get(&device_id)
            .map(|r| r.trust_multiplier())
            .unwrap_or(1.0)
    }

    /// Get all devices for a user
    pub async fn get_user_devices(&self, user_id: UserId) -> Vec<DeviceReputation> {
        let reputations = self.reputations.read().await;
        reputations
            .values()
            .filter(|r| r.users.contains(&user_id))
            .cloned()
            .collect()
    }

    /// Check if device is new for user
    pub async fn is_new_device(&self, device_id: DeviceId, user_id: UserId) -> bool {
        let reputations = self.reputations.read().await;
        match reputations.get(&device_id) {
            Some(rep) => !rep.users.contains(&user_id),
            None => true,
        }
    }
}

impl Default for DeviceReputationTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reputation_score() {
        let score = ReputationScore::new(0.8);
        assert_eq!(score.value(), 0.8);
        assert!(score.is_trusted());
        assert!(!score.is_suspicious());

        let low = ReputationScore::new(0.2);
        assert!(low.is_suspicious());
        assert!(!low.is_trusted());
    }

    #[test]
    fn test_reputation_score_clamping() {
        let high = ReputationScore::new(1.5);
        assert_eq!(high.value(), 1.0);

        let low = ReputationScore::new(-0.5);
        assert_eq!(low.value(), 0.0);
    }

    #[test]
    fn test_device_reputation_creation() {
        let device_id = DeviceId::new();
        let user_id = UserId::new();
        let rep = DeviceReputation::new(device_id, user_id);

        assert_eq!(rep.device_id, device_id);
        assert_eq!(rep.users.len(), 1);
        assert_eq!(rep.successful_auths, 0);
        assert_eq!(rep.failed_auths, 0);
    }

    #[test]
    fn test_record_success() {
        let device_id = DeviceId::new();
        let user_id = UserId::new();
        let mut rep = DeviceReputation::new(device_id, user_id);

        let initial_score = rep.score.value();
        rep.record_success(user_id);

        assert_eq!(rep.successful_auths, 1);
        assert!(rep.score.value() > initial_score);
    }

    #[test]
    fn test_record_failure() {
        let device_id = DeviceId::new();
        let user_id = UserId::new();
        let mut rep = DeviceReputation::new(device_id, user_id);

        let initial_score = rep.score.value();
        rep.record_failure();

        assert_eq!(rep.failed_auths, 1);
        assert!(rep.score.value() < initial_score);
    }

    #[test]
    fn test_flag_device() {
        let device_id = DeviceId::new();
        let user_id = UserId::new();
        let mut rep = DeviceReputation::new(device_id, user_id);

        let initial_score = rep.score.value();
        rep.flag("Suspicious activity");

        assert_eq!(rep.flags, 1);
        assert_eq!(rep.notes.len(), 1);
        assert!(rep.score.value() < initial_score - 0.15);
    }

    #[test]
    fn test_multiple_users() {
        let device_id = DeviceId::new();
        let user1 = UserId::new();
        let user2 = UserId::new();
        let mut rep = DeviceReputation::new(device_id, user1);

        assert!(!rep.has_multiple_users());

        rep.record_success(user2);
        assert!(rep.has_multiple_users());
    }

    #[test]
    fn test_trust_multiplier() {
        let device_id = DeviceId::new();
        let user_id = UserId::new();
        let mut rep = DeviceReputation::new(device_id, user_id);

        // Neutral (0.5 default)
        assert_eq!(rep.trust_multiplier(), 1.0);

        // Build trust
        rep.score = ReputationScore::new(0.8);
        assert_eq!(rep.trust_multiplier(), 0.7);

        // Make suspicious
        rep.score = ReputationScore::new(0.2);
        assert_eq!(rep.trust_multiplier(), 1.5);
    }

    #[tokio::test]
    async fn test_tracker_get_or_create() {
        let tracker = DeviceReputationTracker::new();
        let device_id = DeviceId::new();
        let user_id = UserId::new();

        let rep = tracker.get_or_create(device_id, user_id).await;
        assert_eq!(rep.device_id, device_id);

        // Should get same reputation
        let rep2 = tracker.get_or_create(device_id, user_id).await;
        assert_eq!(rep2.device_id, device_id);
    }

    #[tokio::test]
    async fn test_tracker_record_success() {
        let tracker = DeviceReputationTracker::new();
        let device_id = DeviceId::new();
        let user_id = UserId::new();

        tracker.record_success(device_id, user_id).await;

        let rep = tracker.get(device_id).await.unwrap();
        assert_eq!(rep.successful_auths, 1);
    }

    #[tokio::test]
    async fn test_tracker_is_new_device() {
        let tracker = DeviceReputationTracker::new();
        let device_id = DeviceId::new();
        let user_id = UserId::new();

        assert!(tracker.is_new_device(device_id, user_id).await);

        tracker.record_success(device_id, user_id).await;

        assert!(!tracker.is_new_device(device_id, user_id).await);
    }

    #[tokio::test]
    async fn test_tracker_get_user_devices() {
        let tracker = DeviceReputationTracker::new();
        let device1 = DeviceId::new();
        let device2 = DeviceId::new();
        let user_id = UserId::new();

        tracker.record_success(device1, user_id).await;
        tracker.record_success(device2, user_id).await;

        let devices = tracker.get_user_devices(user_id).await;
        assert_eq!(devices.len(), 2);
    }
}
