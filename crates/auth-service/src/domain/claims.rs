use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::{TokenId, UserDomain, UserId};

/// Scope for access tokens
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClaimScope {
    // Trading scopes
    #[serde(rename = "trade:read")]
    TradeRead,
    #[serde(rename = "trade:write")]
    TradeWrite,
    #[serde(rename = "trade:cancel")]
    TradeCancel,

    // Account scopes
    #[serde(rename = "account:read")]
    AccountRead,
    #[serde(rename = "account:write")]
    AccountWrite,

    // Position scopes
    #[serde(rename = "position:read")]
    PositionRead,

    // Market data scopes
    #[serde(rename = "market:read")]
    MarketRead,
    #[serde(rename = "market:subscribe")]
    MarketSubscribe,

    // Withdrawal scopes (high-value operations)
    #[serde(rename = "withdrawal:initiate")]
    WithdrawalInitiate,
    #[serde(rename = "withdrawal:approve")]
    WithdrawalApprove,

    // Admin scopes
    #[serde(rename = "admin:users")]
    AdminUsers,
    #[serde(rename = "admin:compliance")]
    AdminCompliance,
    #[serde(rename = "admin:system")]
    AdminSystem,
    #[serde(rename = "admin:audit")]
    AdminAudit,

    // API scopes
    #[serde(rename = "api:read")]
    ApiRead,
    #[serde(rename = "api:write")]
    ApiWrite,
}

impl ClaimScope {
    /// Check if scope requires elevated authentication
    pub fn is_sensitive(&self) -> bool {
        matches!(
            self,
            Self::WithdrawalInitiate
                | Self::WithdrawalApprove
                | Self::AdminUsers
                | Self::AdminCompliance
                | Self::AdminSystem
                | Self::AdminAudit
                | Self::AccountWrite
        )
    }

    /// Get default scopes for a user domain
    pub fn defaults_for_domain(domain: UserDomain) -> HashSet<Self> {
        match domain {
            UserDomain::Retail => {
                vec![
                    Self::TradeRead,
                    Self::TradeWrite,
                    Self::TradeCancel,
                    Self::AccountRead,
                    Self::PositionRead,
                    Self::MarketRead,
                    Self::MarketSubscribe,
                    Self::WithdrawalInitiate,
                ]
                .into_iter()
                .collect()
            }
            UserDomain::Institutional => {
                vec![
                    Self::TradeRead,
                    Self::TradeWrite,
                    Self::TradeCancel,
                    Self::AccountRead,
                    Self::AccountWrite,
                    Self::PositionRead,
                    Self::MarketRead,
                    Self::MarketSubscribe,
                    Self::WithdrawalInitiate,
                    Self::ApiRead,
                    Self::ApiWrite,
                ]
                .into_iter()
                .collect()
            }
            UserDomain::Admin => {
                vec![
                    Self::TradeRead,
                    Self::AccountRead,
                    Self::PositionRead,
                    Self::MarketRead,
                    Self::AdminUsers,
                    Self::AdminCompliance,
                    Self::AdminSystem,
                    Self::AdminAudit,
                ]
                .into_iter()
                .collect()
            }
            UserDomain::Compliance => {
                vec![
                    Self::TradeRead,
                    Self::AccountRead,
                    Self::PositionRead,
                    Self::AdminCompliance,
                    Self::AdminAudit,
                ]
                .into_iter()
                .collect()
            }
            UserDomain::Service => {
                vec![Self::ApiRead, Self::ApiWrite]
                    .into_iter()
                    .collect()
            }
        }
    }
}

/// Standard JWT claims (RFC 7519)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardClaims {
    /// Issuer
    pub iss: String,
    /// Subject (user ID)
    pub sub: String,
    /// Audience
    pub aud: Vec<String>,
    /// Expiration time (Unix timestamp)
    pub exp: i64,
    /// Not before (Unix timestamp)
    pub nbf: i64,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// JWT ID
    pub jti: String,
}

impl StandardClaims {
    /// Create standard claims
    pub fn new(
        issuer: String,
        subject: UserId,
        audience: Vec<String>,
        jti: TokenId,
        issued_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            iss: issuer,
            sub: subject.to_string(),
            aud: audience,
            exp: expires_at.timestamp(),
            nbf: issued_at.timestamp(),
            iat: issued_at.timestamp(),
            jti: jti.to_string(),
        }
    }

    /// Validate standard claims
    pub fn validate(&self, expected_issuer: &str, expected_audience: &str) -> Result<(), String> {
        // Check issuer
        if self.iss != expected_issuer {
            return Err(format!("Invalid issuer: expected {}, got {}", expected_issuer, self.iss));
        }

        // Check audience
        if !self.aud.contains(&expected_audience.to_string()) {
            return Err(format!("Invalid audience: expected {}", expected_audience));
        }

        // Check expiration
        let now = Utc::now().timestamp();
        if now >= self.exp {
            return Err("Token expired".to_string());
        }

        // Check not before
        if now < self.nbf {
            return Err("Token not yet valid".to_string());
        }

        Ok(())
    }
}

/// Access token claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessClaims {
    /// Standard claims
    #[serde(flatten)]
    pub standard: StandardClaims,

    /// User domain
    pub domain: UserDomain,

    /// Device ID this token is bound to
    pub device_id: String,

    /// Session ID
    pub session_id: String,

    /// Scopes granted
    pub scopes: HashSet<ClaimScope>,

    /// Nonce for replay protection
    pub nonce: String,

    /// Key ID used for signing
    pub kid: String,

    /// IP address when token was issued
    pub ip: String,

    /// Risk score at time of issuance
    pub risk_score: f32,

    /// Whether this token required MFA
    pub mfa_verified: bool,

    /// Whether this token required WebAuthn
    pub webauthn_verified: bool,

    /// Token version (for rolling key rotation)
    pub token_version: u32,
}

impl AccessClaims {
    /// Check if token has specific scope
    pub fn has_scope(&self, scope: &ClaimScope) -> bool {
        self.scopes.contains(scope)
    }

    /// Check if token has all required scopes
    pub fn has_all_scopes(&self, required: &[ClaimScope]) -> bool {
        required.iter().all(|s| self.scopes.contains(s))
    }

    /// Check if token has any of the required scopes
    pub fn has_any_scope(&self, required: &[ClaimScope]) -> bool {
        required.iter().any(|s| self.scopes.contains(s))
    }

    /// Validate the claims
    pub fn validate(&self, expected_issuer: &str, expected_audience: &str) -> Result<(), String> {
        self.standard.validate(expected_issuer, expected_audience)
    }
}

/// Refresh token claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshClaims {
    /// Standard claims
    #[serde(flatten)]
    pub standard: StandardClaims,

    /// User domain
    pub domain: UserDomain,

    /// Device ID this token is bound to
    pub device_id: String,

    /// Session ID
    pub session_id: String,

    /// Key ID used for signing
    pub kid: String,

    /// Parent refresh token JTI (for rotation tracking)
    pub parent_jti: Option<String>,

    /// Generation number
    pub generation: u32,

    /// Rotation count
    pub rotation_count: u32,

    /// Maximum rotations allowed
    pub max_rotations: u32,

    /// Token version
    pub token_version: u32,
}

impl RefreshClaims {
    /// Check if token can be rotated
    pub fn can_rotate(&self) -> bool {
        self.rotation_count < self.max_rotations
    }

    /// Validate the claims
    pub fn validate(&self, expected_issuer: &str, expected_audience: &str) -> Result<(), String> {
        self.standard.validate(expected_issuer, expected_audience)
    }
}

/// Complete claims structure for both access and refresh tokens
/// Uses untagged enum to avoid conflicts with flattened StandardClaims
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Claims {
    Access(AccessClaims),
    Refresh(RefreshClaims),
}

impl Claims {
    /// Get the JTI from either claim type
    pub fn jti(&self) -> &str {
        match self {
            Self::Access(claims) => &claims.standard.jti,
            Self::Refresh(claims) => &claims.standard.jti,
        }
    }

    /// Get the subject (user ID) from either claim type
    pub fn subject(&self) -> &str {
        match self {
            Self::Access(claims) => &claims.standard.sub,
            Self::Refresh(claims) => &claims.standard.sub,
        }
    }

    /// Get the device ID from either claim type
    pub fn device_id(&self) -> &str {
        match self {
            Self::Access(claims) => &claims.device_id,
            Self::Refresh(claims) => &claims.device_id,
        }
    }

    /// Get the session ID from either claim type
    pub fn session_id(&self) -> &str {
        match self {
            Self::Access(claims) => &claims.session_id,
            Self::Refresh(claims) => &claims.session_id,
        }
    }

    /// Validate the claims
    pub fn validate(&self, expected_issuer: &str, expected_audience: &str) -> Result<(), String> {
        match self {
            Self::Access(claims) => claims.validate(expected_issuer, expected_audience),
            Self::Refresh(claims) => claims.validate(expected_issuer, expected_audience),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DeviceId, SessionId};

    #[test]
    fn test_scope_sensitivity() {
        assert!(ClaimScope::WithdrawalApprove.is_sensitive());
        assert!(ClaimScope::AdminSystem.is_sensitive());
        assert!(!ClaimScope::MarketRead.is_sensitive());
    }

    #[test]
    fn test_default_scopes_retail() {
        let scopes = ClaimScope::defaults_for_domain(UserDomain::Retail);
        assert!(scopes.contains(&ClaimScope::TradeRead));
        assert!(scopes.contains(&ClaimScope::TradeWrite));
        assert!(!scopes.contains(&ClaimScope::AdminUsers));
    }

    #[test]
    fn test_default_scopes_admin() {
        let scopes = ClaimScope::defaults_for_domain(UserDomain::Admin);
        assert!(scopes.contains(&ClaimScope::AdminUsers));
        assert!(scopes.contains(&ClaimScope::AdminSystem));
        assert!(!scopes.contains(&ClaimScope::TradeWrite));
    }

    #[test]
    fn test_standard_claims_validation() {
        let now = Utc::now();
        let claims = StandardClaims::new(
            "test-issuer".to_string(),
            UserId::new(),
            vec!["test-audience".to_string()],
            TokenId::new(),
            now,
            now + chrono::Duration::seconds(3600),
        );

        assert!(claims.validate("test-issuer", "test-audience").is_ok());
        assert!(claims.validate("wrong-issuer", "test-audience").is_err());
        assert!(claims.validate("test-issuer", "wrong-audience").is_err());
    }

    #[test]
    fn test_access_claims_scope_checking() {
        let now = Utc::now();
        let standard = StandardClaims::new(
            "test".to_string(),
            UserId::new(),
            vec!["test".to_string()],
            TokenId::new(),
            now,
            now + chrono::Duration::seconds(900),
        );

        let mut scopes = HashSet::new();
        scopes.insert(ClaimScope::TradeRead);
        scopes.insert(ClaimScope::TradeWrite);

        let claims = AccessClaims {
            standard,
            domain: UserDomain::Retail,
            device_id: DeviceId::new().to_string(),
            session_id: SessionId::new().to_string(),
            scopes,
            nonce: "test-nonce".to_string(),
            kid: "key-1".to_string(),
            ip: "127.0.0.1".to_string(),
            risk_score: 0.2,
            mfa_verified: false,
            webauthn_verified: false,
            token_version: 1,
        };

        assert!(claims.has_scope(&ClaimScope::TradeRead));
        assert!(!claims.has_scope(&ClaimScope::AdminUsers));
        assert!(claims.has_all_scopes(&[ClaimScope::TradeRead, ClaimScope::TradeWrite]));
        assert!(!claims.has_all_scopes(&[ClaimScope::TradeRead, ClaimScope::AdminUsers]));
    }

    #[test]
    fn test_refresh_claims_rotation() {
        let now = Utc::now();
        let standard = StandardClaims::new(
            "test".to_string(),
            UserId::new(),
            vec!["test".to_string()],
            TokenId::new(),
            now,
            now + chrono::Duration::days(30),
        );

        let claims = RefreshClaims {
            standard,
            domain: UserDomain::Retail,
            device_id: DeviceId::new().to_string(),
            session_id: SessionId::new().to_string(),
            kid: "key-1".to_string(),
            parent_jti: None,
            generation: 0,
            rotation_count: 5,
            max_rotations: 10,
            token_version: 1,
        };

        assert!(claims.can_rotate());

        let mut exhausted = claims.clone();
        exhausted.rotation_count = 10;
        assert!(!exhausted.can_rotate());
    }
}
