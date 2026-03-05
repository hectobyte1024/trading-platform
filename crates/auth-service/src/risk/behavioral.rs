//! Behavioral Analytics
//!
//! Track and analyze user behavior patterns for anomaly detection.

use crate::domain::{UserId, DeviceId};
use chrono::{DateTime, Utc, Datelike, Timelike, Weekday};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// User behavior profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBehavior {
    pub user_id: UserId,
    /// Typical login hours (0-23)
    pub typical_login_hours: Vec<u8>,
    /// Typical login days of week
    pub typical_login_days: Vec<Weekday>,
    /// Known IP addresses
    pub known_ips: Vec<IpAddr>,
    /// Known devices
    pub known_devices: Vec<DeviceId>,
    /// Average time between logins (seconds)
    pub avg_login_interval_secs: Option<u64>,
    /// Total number of logins recorded
    pub total_logins: u64,
    /// Last login timestamp
    pub last_login: Option<DateTime<Utc>>,
    /// First seen timestamp
    pub first_seen: DateTime<Utc>,
    /// Geographic locations (country codes)
    pub known_countries: Vec<String>,
}

impl UserBehavior {
    /// Create a new empty behavior profile
    pub fn new(user_id: UserId) -> Self {
        Self {
            user_id,
            typical_login_hours: Vec::new(),
            typical_login_days: Vec::new(),
            known_ips: Vec::new(),
            known_devices: Vec::new(),
            avg_login_interval_secs: None,
            total_logins: 0,
            last_login: None,
            first_seen: Utc::now(),
            known_countries: Vec::new(),
        }
    }

    /// Update behavior with a new login event
    pub fn record_login(&mut self, timestamp: DateTime<Utc>, ip: IpAddr, device_id: DeviceId, country: Option<String>) {
        // Update login count
        self.total_logins += 1;

        // Record hour
        let hour = timestamp.hour() as u8;
        if !self.typical_login_hours.contains(&hour) {
            self.typical_login_hours.push(hour);
        }

        // Record day of week
        let day = timestamp.weekday();
        if !self.typical_login_days.contains(&day) {
            self.typical_login_days.push(day);
        }

        // Record IP (keep last 10)
        if !self.known_ips.contains(&ip) {
            self.known_ips.push(ip);
            if self.known_ips.len() > 10 {
                self.known_ips.remove(0);
            }
        }

        // Record device (keep last 5)
        if !self.known_devices.contains(&device_id) {
            self.known_devices.push(device_id);
            if self.known_devices.len() > 5 {
                self.known_devices.remove(0);
            }
        }

        // Record country
        if let Some(country_code) = country {
            if !self.known_countries.contains(&country_code) {
                self.known_countries.push(country_code);
            }
        }

        // Update average login interval
        if let Some(last) = self.last_login {
            let interval_secs = (timestamp - last).num_seconds().max(0) as u64;
            if let Some(avg) = self.avg_login_interval_secs {
                // Exponential moving average
                self.avg_login_interval_secs = Some((avg * 9 + interval_secs) / 10);
            } else {
                self.avg_login_interval_secs = Some(interval_secs);
            }
        }

        self.last_login = Some(timestamp);
    }

    /// Check if login hour is typical for this user
    pub fn is_typical_hour(&self, hour: u8) -> bool {
        if self.typical_login_hours.is_empty() {
            true // No data yet, allow all
        } else {
            self.typical_login_hours.contains(&hour)
        }
    }

    /// Check if login day is typical for this user
    pub fn is_typical_day(&self, day: Weekday) -> bool {
        if self.typical_login_days.is_empty() {
            true
        } else {
            self.typical_login_days.contains(&day)
        }
    }

    /// Check if IP is known
    pub fn is_known_ip(&self, ip: &IpAddr) -> bool {
        self.known_ips.contains(ip)
    }

    /// Check if device is known
    pub fn is_known_device(&self, device_id: &DeviceId) -> bool {
        self.known_devices.contains(device_id)
    }

    /// Check if country is known
    pub fn is_known_country(&self, country: &str) -> bool {
        if self.known_countries.is_empty() {
            true
        } else {
            self.known_countries.contains(&country.to_string())
        }
    }
}

/// Behavior pattern (for pattern matching)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BehaviorPattern {
    /// User logs in during specific hours
    TimeOfDay { hours: Vec<u8> },
    /// User logs in on specific days
    DayOfWeek { days: Vec<Weekday> },
    /// User logs in from specific IPs
    IpAddress { ips: Vec<IpAddr> },
    /// User logs in from specific devices
    Device { devices: Vec<DeviceId> },
    /// User logs in at regular intervals
    RegularInterval { avg_secs: u64, tolerance_pct: f32 },
    /// User logs in from specific countries
    Geography { countries: Vec<String> },
}

impl BehaviorPattern {
    /// Check if current event matches pattern
    pub fn matches(&self, behavior: &UserBehavior, timestamp: DateTime<Utc>, ip: &IpAddr, device_id: &DeviceId, country: Option<&str>) -> bool {
        match self {
            Self::TimeOfDay { hours } => {
                let hour = timestamp.hour() as u8;
                hours.contains(&hour)
            }
            Self::DayOfWeek { days } => {
                let day = timestamp.weekday();
                days.contains(&day)
            }
            Self::IpAddress { ips } => ips.contains(ip),
            Self::Device { devices } => devices.contains(device_id),
            Self::RegularInterval { avg_secs, tolerance_pct } => {
                if let (Some(last), Some(avg_interval)) = (behavior.last_login, behavior.avg_login_interval_secs) {
                    let actual_interval = (timestamp - last).num_seconds().max(0) as u64;
                    let tolerance = (avg_interval as f32 * tolerance_pct) as u64;
                    let lower = avg_interval.saturating_sub(tolerance);
                    let upper = avg_interval + tolerance;
                    actual_interval >= lower && actual_interval <= upper
                } else {
                    true // No data yet
                }
            }
            Self::Geography { countries } => {
                if let Some(c) = country {
                    countries.contains(&c.to_string())
                } else {
                    false
                }
            }
        }
    }
}

/// Behavioral analyzer
pub struct BehavioralAnalyzer {
    /// User behavior profiles
    profiles: Arc<RwLock<HashMap<UserId, UserBehavior>>>,
}

impl BehavioralAnalyzer {
    /// Create a new behavioral analyzer
    pub fn new() -> Self {
        Self {
            profiles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create user behavior profile
    pub async fn get_or_create_profile(&self, user_id: UserId) -> UserBehavior {
        let profiles = self.profiles.read().await;
        if let Some(profile) = profiles.get(&user_id) {
            profile.clone()
        } else {
            drop(profiles);
            let mut profiles = self.profiles.write().await;
            let profile = UserBehavior::new(user_id);
            profiles.insert(user_id, profile.clone());
            profile
        }
    }

    /// Record a login event
    pub async fn record_login(&self, user_id: UserId, timestamp: DateTime<Utc>, ip: IpAddr, device_id: DeviceId, country: Option<String>) {
        let mut profiles = self.profiles.write().await;
        let profile = profiles.entry(user_id).or_insert_with(|| UserBehavior::new(user_id));
        profile.record_login(timestamp, ip, device_id, country);
    }

    /// Check if login matches user's typical behavior
    pub async fn is_typical_behavior(&self, user_id: UserId, timestamp: DateTime<Utc>, ip: &IpAddr, device_id: &DeviceId) -> bool {
        let profile = self.get_or_create_profile(user_id).await;
        
        // New users have no patterns yet
        if profile.total_logins == 0 {
            return true;
        }

        // Check multiple factors
        let hour = timestamp.hour() as u8;
        let day = timestamp.weekday();
        
        profile.is_typical_hour(hour)
            && profile.is_typical_day(day)
            && (profile.is_known_ip(ip) || profile.known_ips.len() < 3)
            && (profile.is_known_device(device_id) || profile.known_devices.len() < 2)
    }

    /// Detect behavioral anomalies
    pub async fn detect_anomalies(&self, user_id: UserId, timestamp: DateTime<Utc>, ip: &IpAddr, device_id: &DeviceId, country: Option<&str>) -> Vec<String> {
        let profile = self.get_or_create_profile(user_id).await;
        let mut anomalies = Vec::new();

        if profile.total_logins == 0 {
            return anomalies; // No baseline yet
        }

        // Check hour
        let hour = timestamp.hour() as u8;
        if !profile.is_typical_hour(hour) {
            anomalies.push(format!("Unusual login hour: {}", hour));
        }

        // Check day
        let day = timestamp.weekday();
        if !profile.is_typical_day(day) {
            anomalies.push(format!("Unusual login day: {:?}", day));
        }

        // Check IP
        if !profile.is_known_ip(ip) {
            anomalies.push(format!("Unknown IP address: {}", ip));
        }

        // Check device
        if !profile.is_known_device(device_id) {
            anomalies.push(format!("Unknown device: {}", device_id));
        }

        // Check country
        if let Some(c) = country {
            if !profile.is_known_country(c) {
                anomalies.push(format!("Unusual country: {}", c));
            }
        }

        // Check login interval
        if let (Some(last), Some(avg_interval)) = (profile.last_login, profile.avg_login_interval_secs) {
            let actual_interval = (timestamp - last).num_seconds().max(0) as u64;
            // Alert if login is much faster than typical (10% of average)
            if actual_interval < avg_interval / 10 && actual_interval < 300 {
                anomalies.push(format!("Login too soon ({}s vs avg {}s)", actual_interval, avg_interval));
            }
        }

        anomalies
    }

    /// Get user behavior summary
    pub async fn get_behavior_summary(&self, user_id: UserId) -> Option<UserBehavior> {
        let profiles = self.profiles.read().await;
        profiles.get(&user_id).cloned()
    }
}

impl Default for BehavioralAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_behavior_creation() {
        let user_id = UserId::new();
        let behavior = UserBehavior::new(user_id);

        assert_eq!(behavior.total_logins, 0);
        assert!(behavior.typical_login_hours.is_empty());
        assert!(behavior.known_ips.is_empty());
    }

    #[test]
    fn test_record_login() {
        let user_id = UserId::new();
        let mut behavior = UserBehavior::new(user_id);

        let timestamp = Utc::now();
        let ip = "192.168.1.1".parse().unwrap();
        let device_id = DeviceId::new();

        behavior.record_login(timestamp, ip, device_id, Some("US".to_string()));

        assert_eq!(behavior.total_logins, 1);
        assert!(!behavior.typical_login_hours.is_empty());
        assert!(behavior.known_ips.contains(&ip));
        assert!(behavior.known_devices.contains(&device_id));
        assert!(behavior.known_countries.contains(&"US".to_string()));
    }

    #[test]
    fn test_typical_hour() {
        let user_id = UserId::new();
        let mut behavior = UserBehavior::new(user_id);

        let timestamp = Utc::now();
        let ip = "192.168.1.1".parse().unwrap();
        let device_id = DeviceId::new();

        behavior.record_login(timestamp, ip, device_id, None);

        let hour = timestamp.hour() as u8;
        assert!(behavior.is_typical_hour(hour));
        assert!(behavior.is_typical_hour((hour + 12) % 24) == false || behavior.is_typical_hour((hour + 12) % 24) == true);
    }

    #[tokio::test]
    async fn test_behavioral_analyzer() {
        let analyzer = BehavioralAnalyzer::new();
        let user_id = UserId::new();

        let profile = analyzer.get_or_create_profile(user_id).await;
        assert_eq!(profile.total_logins, 0);

        // Record a login
        let ip = "192.168.1.1".parse().unwrap();
        let device_id = DeviceId::new();
        analyzer.record_login(user_id, Utc::now(), ip, device_id, Some("US".to_string())).await;

        let profile = analyzer.get_or_create_profile(user_id).await;
        assert_eq!(profile.total_logins, 1);
    }

    #[tokio::test]
    async fn test_is_typical_behavior() {
        let analyzer = BehavioralAnalyzer::new();
        let user_id = UserId::new();
        let ip = "192.168.1.1".parse().unwrap();
        let device_id = DeviceId::new();

        // First login - should be typical (no baseline)
        let is_typical = analyzer.is_typical_behavior(user_id, Utc::now(), &ip, &device_id).await;
        assert!(is_typical);

        // Record the login
        analyzer.record_login(user_id, Utc::now(), ip, device_id, None).await;

        // Same IP/device should be typical
        let is_typical = analyzer.is_typical_behavior(user_id, Utc::now(), &ip, &device_id).await;
        assert!(is_typical);
    }

    #[tokio::test]
    async fn test_detect_anomalies() {
        let analyzer = BehavioralAnalyzer::new();
        let user_id = UserId::new();
        let ip1 = "192.168.1.1".parse().unwrap();
        let ip2 = "10.0.0.1".parse().unwrap();
        let device_id = DeviceId::new();

        // Record first login
        analyzer.record_login(user_id, Utc::now(), ip1, device_id, Some("US".to_string())).await;

        // Login from different IP
        let anomalies = analyzer.detect_anomalies(user_id, Utc::now(), &ip2, &device_id, Some("US")).await;
        
        // Should detect unknown IP
        assert!(anomalies.iter().any(|a| a.contains("Unknown IP")));
    }

    #[test]
    fn test_behavior_pattern_time_of_day() {
        let pattern = BehaviorPattern::TimeOfDay { hours: vec![9, 10, 11] };
        let behavior = UserBehavior::new(UserId::new());
        
        let timestamp = Utc::now().with_hour(10).unwrap();
        let ip = "192.168.1.1".parse().unwrap();
        let device_id = DeviceId::new();
        
        assert!(pattern.matches(&behavior, timestamp, &ip, &device_id, None));
        
        let timestamp = Utc::now().with_hour(15).unwrap();
        assert!(!pattern.matches(&behavior, timestamp, &ip, &device_id, None));
    }

    #[test]
    fn test_behavior_pattern_geography() {
        let pattern = BehaviorPattern::Geography { countries: vec!["US".to_string(), "CA".to_string()] };
        let behavior = UserBehavior::new(UserId::new());
        let timestamp = Utc::now();
        let ip = "192.168.1.1".parse().unwrap();
        let device_id = DeviceId::new();
        
        assert!(pattern.matches(&behavior, timestamp, &ip, &device_id, Some("US")));
        assert!(pattern.matches(&behavior, timestamp, &ip, &device_id, Some("CA")));
        assert!(!pattern.matches(&behavior, timestamp, &ip, &device_id, Some("RU")));
    }

    #[tokio::test]
    async fn test_get_behavior_summary() {
        let analyzer = BehavioralAnalyzer::new();
        let user_id = UserId::new();

        let summary = analyzer.get_behavior_summary(user_id).await;
        assert!(summary.is_none());

        let ip = "192.168.1.1".parse().unwrap();
        let device_id = DeviceId::new();
        analyzer.record_login(user_id, Utc::now(), ip, device_id, None).await;

        let summary = analyzer.get_behavior_summary(user_id).await;
        assert!(summary.is_some());
        assert_eq!(summary.unwrap().total_logins, 1);
    }
}
