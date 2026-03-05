//! Token revocation storage abstraction
//!
//! Provides a trait-based interface for storing and checking revoked tokens.
//! Supports multiple backends (Redis, PostgreSQL) for different deployment scenarios.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::token::TokenId;
use crate::domain::user::UserId;

/// Errors that can occur during revocation operations
#[derive(Debug, Error)]
pub enum RevocationError {
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Token not found: {0}")]
    NotFound(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

/// Reason for token revocation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevocationReason {
    /// User explicitly logged out
    UserLogout,
    /// Password was changed
    PasswordChange,
    /// User account was deactivated
    UserDeactivated,
    /// Security compromise detected
    SecurityCompromise,
    /// Administrative action
    AdminRevocation,
    /// Token expired naturally
    Expiration,
    /// Maximum rotation count reached
    MaxRotations,
    /// Suspicious activity detected
    SuspiciousActivity,
}

impl RevocationReason {
    /// Check if this reason requires immediate revocation of all user tokens
    pub fn requires_full_revocation(&self) -> bool {
        matches!(
            self,
            Self::PasswordChange
                | Self::UserDeactivated
                | Self::SecurityCompromise
                | Self::AdminRevocation
        )
    }

    /// Get a human-readable description
    pub fn description(&self) -> &str {
        match self {
            Self::UserLogout => "User logged out",
            Self::PasswordChange => "Password changed",
            Self::UserDeactivated => "User account deactivated",
            Self::SecurityCompromise => "Security compromise",
            Self::AdminRevocation => "Administrative revocation",
            Self::Expiration => "Token expired",
            Self::MaxRotations => "Maximum rotations reached",
            Self::SuspiciousActivity => "Suspicious activity detected",
        }
    }
}

/// Information about a revoked token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokedToken {
    /// Token ID (JTI)
    pub jti: TokenId,
    /// User who owned the token
    pub user_id: UserId,
    /// When the token was revoked
    pub revoked_at: DateTime<Utc>,
    /// Reason for revocation
    pub reason: RevocationReason,
    /// When the token would have expired naturally
    pub expires_at: DateTime<Utc>,
    /// Optional notes about the revocation
    pub notes: Option<String>,
}

impl RevokedToken {
    /// Create a new revoked token entry
    pub fn new(
        jti: TokenId,
        user_id: UserId,
        reason: RevocationReason,
        expires_at: DateTime<Utc>,
        notes: Option<String>,
    ) -> Self {
        Self {
            jti,
            user_id,
            revoked_at: Utc::now(),
            reason,
            expires_at,
            notes,
        }
    }

    /// Check if this revocation entry is still needed
    /// (can be cleaned up after natural expiration)
    pub fn can_cleanup(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

/// Statistics about revoked tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevocationStats {
    /// Total revoked tokens currently stored
    pub total_revoked: usize,
    /// Tokens revoked in last 24 hours
    pub revoked_24h: usize,
    /// Tokens that can be cleaned up
    pub cleanable: usize,
    /// Breakdown by reason
    pub by_reason: std::collections::HashMap<String, usize>,
}

/// Trait for token revocation storage
#[async_trait]
pub trait RevocationStore: Send + Sync {
    /// Check if a token is revoked
    async fn is_revoked(&self, jti: &TokenId) -> Result<bool, RevocationError>;

    /// Revoke a single token
    async fn revoke_token(
        &self,
        jti: TokenId,
        user_id: UserId,
        reason: RevocationReason,
        expires_at: DateTime<Utc>,
        notes: Option<String>,
    ) -> Result<(), RevocationError>;

    /// Revoke all tokens for a user
    /// Used when password changes or account is compromised
    async fn revoke_all_user_tokens(
        &self,
        user_id: UserId,
        reason: RevocationReason,
        notes: Option<String>,
    ) -> Result<usize, RevocationError>;

    /// Get revocation details for a token
    async fn get_revocation(
        &self,
        jti: &TokenId,
    ) -> Result<Option<RevokedToken>, RevocationError>;

    /// Clean up expired revocation entries
    /// Returns the number of entries removed
    async fn cleanup_expired(&self) -> Result<usize, RevocationError>;

    /// Get statistics about revoked tokens
    async fn stats(&self) -> Result<RevocationStats, RevocationError>;

    /// List all revoked tokens for a user
    async fn list_user_revocations(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<RevokedToken>, RevocationError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_revocation_reason_full_revocation() {
        assert!(RevocationReason::PasswordChange.requires_full_revocation());
        assert!(RevocationReason::UserDeactivated.requires_full_revocation());
        assert!(RevocationReason::SecurityCompromise.requires_full_revocation());
        assert!(RevocationReason::AdminRevocation.requires_full_revocation());

        assert!(!RevocationReason::UserLogout.requires_full_revocation());
        assert!(!RevocationReason::Expiration.requires_full_revocation());
        assert!(!RevocationReason::MaxRotations.requires_full_revocation());
    }

    #[test]
    fn test_revocation_reason_descriptions() {
        assert_eq!(RevocationReason::UserLogout.description(), "User logged out");
        assert_eq!(
            RevocationReason::PasswordChange.description(),
            "Password changed"
        );
    }

    #[test]
    fn test_revoked_token_creation() {
        let jti = TokenId::new();
        let user_id = UserId::new();
        let expires_at = Utc::now() + chrono::Duration::hours(1);

        let revoked = RevokedToken::new(
            jti,
            user_id,
            RevocationReason::UserLogout,
            expires_at,
            Some("Test revocation".to_string()),
        );

        assert_eq!(revoked.jti, jti);
        assert_eq!(revoked.user_id, user_id);
        assert_eq!(revoked.reason, RevocationReason::UserLogout);
        assert!(!revoked.can_cleanup());
    }

    #[test]
    fn test_revoked_token_cleanup_eligible() {
        let jti = TokenId::new();
        let user_id = UserId::new();
        let expires_at = Utc::now() - chrono::Duration::hours(1); // Already expired

        let revoked = RevokedToken::new(
            jti,
            user_id,
            RevocationReason::Expiration,
            expires_at,
            None,
        );

        assert!(revoked.can_cleanup());
    }
}
