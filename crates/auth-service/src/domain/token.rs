use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use super::{DeviceId, UserId};

/// Unique token identifier (JTI - JWT ID)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenId(pub Uuid);

impl TokenId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl fmt::Display for TokenId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for TokenId {
    fn default() -> Self {
        Self::new()
    }
}

/// Access token for short-lived API access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessToken {
    /// JWT ID for revocation and replay detection
    pub jti: TokenId,
    /// User this token belongs to
    pub user_id: UserId,
    /// Device this token is bound to
    pub device_id: DeviceId,
    /// When the token was issued
    pub issued_at: DateTime<Utc>,
    /// When the token expires
    pub expires_at: DateTime<Utc>,
    /// Nonce for replay protection
    pub nonce: String,
    /// Token signature algorithm (should be "RS256" or "ES256" from KMS)
    pub algorithm: String,
    /// Key ID used for signing (for key rotation)
    pub key_id: String,
}

impl AccessToken {
    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if token is valid (not expired and issue time is in the past)
    pub fn is_valid(&self) -> bool {
        let now = Utc::now();
        now >= self.issued_at && now <= self.expires_at
    }

    /// Time until expiration (in seconds)
    pub fn time_until_expiration(&self) -> i64 {
        (self.expires_at - Utc::now()).num_seconds()
    }
}

/// Refresh token for obtaining new access tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshToken {
    /// JWT ID for revocation
    pub jti: TokenId,
    /// User this token belongs to
    pub user_id: UserId,
    /// Device this token is bound to
    pub device_id: DeviceId,
    /// When the token was issued
    pub issued_at: DateTime<Utc>,
    /// When the token expires
    pub expires_at: DateTime<Utc>,
    /// Parent refresh token ID (for rotation tracking)
    pub parent_jti: Option<TokenId>,
    /// Generation number (increments on each rotation)
    pub generation: u32,
    /// Maximum number of times this token can be refreshed
    pub max_rotations: u32,
    /// Current rotation count
    pub rotation_count: u32,
    /// Token signature algorithm
    pub algorithm: String,
    /// Key ID used for signing
    pub key_id: String,
}

impl RefreshToken {
    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if token can be rotated
    pub fn can_rotate(&self) -> bool {
        !self.is_expired() && self.rotation_count < self.max_rotations
    }

    /// Check if token is valid
    pub fn is_valid(&self) -> bool {
        let now = Utc::now();
        now >= self.issued_at && now <= self.expires_at
    }

    /// Create a rotated version of this token
    pub fn rotate(&self, new_jti: TokenId, ttl: i64) -> Self {
        let now = Utc::now();
        Self {
            jti: new_jti,
            user_id: self.user_id,
            device_id: self.device_id,
            issued_at: now,
            expires_at: now + chrono::Duration::seconds(ttl),
            parent_jti: Some(self.jti),
            generation: self.generation + 1,
            max_rotations: self.max_rotations,
            rotation_count: self.rotation_count + 1,
            algorithm: self.algorithm.clone(),
            key_id: self.key_id.clone(),
        }
    }
}

/// Token pair returned to client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    /// Short-lived access token (JWT string)
    pub access_token: String,
    /// Long-lived refresh token (JWT string)
    pub refresh_token: Option<String>,
    /// Token type (always "Bearer")
    pub token_type: String,
    /// Seconds until access token expires
    pub expires_in: i64,
    /// Access token metadata
    #[serde(skip_serializing)]
    pub access_token_metadata: AccessToken,
    /// Refresh token metadata
    #[serde(skip_serializing)]
    pub refresh_token_metadata: Option<RefreshToken>,
}

impl TokenPair {
    /// Create a new token pair
    pub fn new(
        access_token: String,
        refresh_token: Option<String>,
        access_metadata: AccessToken,
        refresh_metadata: Option<RefreshToken>,
    ) -> Self {
        let expires_in = access_metadata.time_until_expiration();
        Self {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in,
            access_token_metadata: access_metadata,
            refresh_token_metadata: refresh_metadata,
        }
    }
}

/// Token revocation reason
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevocationReason {
    /// User logged out
    UserLogout,
    /// Administrative revocation
    AdminRevoke,
    /// Token compromised
    Compromised,
    /// Device lost/stolen
    DeviceLost,
    /// Suspicious activity detected
    SuspiciousActivity,
    /// Password changed
    PasswordChange,
    /// Permission downgrade
    PermissionChange,
    /// Session expired naturally
    Expired,
}

/// Token revocation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRevocation {
    /// The revoked token ID
    pub jti: TokenId,
    /// User the token belonged to
    pub user_id: UserId,
    /// When the token was revoked
    pub revoked_at: DateTime<Utc>,
    /// Why the token was revoked
    pub reason: RevocationReason,
    /// Who revoked the token (user_id or system)
    pub revoked_by: Option<UserId>,
    /// The original expiration time (for cleanup)
    pub original_expires_at: DateTime<Utc>,
}

impl TokenRevocation {
    /// Check if this revocation record can be cleaned up
    /// (token would have expired anyway)
    pub fn can_cleanup(&self) -> bool {
        Utc::now() > self.original_expires_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_token_expiration() {
        let now = Utc::now();
        let token = AccessToken {
            jti: TokenId::new(),
            user_id: UserId::new(),
            device_id: DeviceId::new(),
            issued_at: now - chrono::Duration::seconds(100),
            expires_at: now + chrono::Duration::seconds(900),
            nonce: "test-nonce".to_string(),
            algorithm: "RS256".to_string(),
            key_id: "key-1".to_string(),
        };

        assert!(!token.is_expired());
        assert!(token.is_valid());
    }

    #[test]
    fn test_refresh_token_rotation() {
        let now = Utc::now();
        let token = RefreshToken {
            jti: TokenId::new(),
            user_id: UserId::new(),
            device_id: DeviceId::new(),
            issued_at: now,
            expires_at: now + chrono::Duration::days(30),
            parent_jti: None,
            generation: 0,
            max_rotations: 10,
            rotation_count: 0,
            algorithm: "RS256".to_string(),
            key_id: "key-1".to_string(),
        };

        assert!(token.can_rotate());

        let rotated = token.rotate(TokenId::new(), 2592000);
        assert_eq!(rotated.generation, 1);
        assert_eq!(rotated.rotation_count, 1);
        assert_eq!(rotated.parent_jti, Some(token.jti));
    }

    #[test]
    fn test_refresh_token_max_rotations() {
        let now = Utc::now();
        let mut token = RefreshToken {
            jti: TokenId::new(),
            user_id: UserId::new(),
            device_id: DeviceId::new(),
            issued_at: now,
            expires_at: now + chrono::Duration::days(30),
            parent_jti: None,
            generation: 0,
            max_rotations: 3,
            rotation_count: 3,
            algorithm: "RS256".to_string(),
            key_id: "key-1".to_string(),
        };

        assert!(!token.can_rotate());

        token.rotation_count = 2;
        assert!(token.can_rotate());
    }

    #[test]
    fn test_token_revocation_cleanup() {
        let now = Utc::now();
        let revocation = TokenRevocation {
            jti: TokenId::new(),
            user_id: UserId::new(),
            revoked_at: now - chrono::Duration::days(1),
            reason: RevocationReason::UserLogout,
            revoked_by: None,
            original_expires_at: now - chrono::Duration::seconds(1),
        };

        assert!(revocation.can_cleanup());
    }
}
