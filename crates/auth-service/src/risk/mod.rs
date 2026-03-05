//! Risk Engine
//!
//! Adaptive authentication risk engine with behavioral analytics,
//! anomaly detection, and step-up authentication triggers.
//!
//! Features:
//! - Real-time risk scoring
//! - Behavioral pattern analysis
//! - Anomaly detection
//! - Device reputation tracking
//! - Adaptive security policies
//! - Step-up authentication triggers

pub mod scoring;
pub mod behavioral;
pub mod anomaly;
pub mod device_reputation;
pub mod adaptive_policy;

pub use scoring::{RiskScorer, RiskFactors, RiskLevel, RiskScore};
pub use behavioral::{BehavioralAnalyzer, UserBehavior, BehaviorPattern};
pub use anomaly::{AnomalyDetector, Anomaly, AnomalyType};
pub use device_reputation::{DeviceReputationTracker, DeviceReputation, ReputationScore};
pub use adaptive_policy::{AdaptivePolicy, StepUpTrigger, AuthenticationRequirement, StepUpReason};
