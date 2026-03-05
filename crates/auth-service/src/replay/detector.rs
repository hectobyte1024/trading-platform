use chrono::Utc;
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, warn};

use super::nonce_store::{NonceStore, NonceStoreError};
use crate::domain::{AccessClaims, TokenId};

/// Replay detection errors
#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("Replay attack detected: {0}")]
    ReplayDetected(String),

    #[error("Timestamp too old: {0}")]
    TimestampTooOld(String),

    #[error("Timestamp in future: {0}")]
    TimestampInFuture(String),

    #[error("Nonce store error: {0}")]
    NonceStore(#[from] NonceStoreError),

    #[error("Invalid timestamp")]
    InvalidTimestamp,
}

/// Replay detector configuration
#[derive(Debug, Clone)]
pub struct ReplayDetectorConfig {
    /// Maximum age of a token (seconds)
    pub max_token_age: i64,
    /// Maximum clock skew tolerance (seconds)
    pub max_clock_skew: i64,
    /// Window for nonce validation (seconds)
    pub nonce_window: i64,
}

impl Default for ReplayDetectorConfig {
    fn default() -> Self {
        Self {
            max_token_age: 3600,    // 1 hour
            max_clock_skew: 300,    // 5 minutes
            nonce_window: 7200,     // 2 hours
        }
    }
}

/// Replay detector
pub struct ReplayDetector {
    nonce_store: Arc<dyn NonceStore>,
    config: ReplayDetectorConfig,
}

impl ReplayDetector {
    /// Create a new replay detector
    pub fn new(nonce_store: Arc<dyn NonceStore>, config: ReplayDetectorConfig) -> Self {
        Self {
            nonce_store,
            config,
        }
    }

    /// Check token for replay attacks
    pub async fn check_token(&self, claims: &AccessClaims) -> Result<(), ReplayError> {
        // Check timestamp validity
        self.check_timestamp(claims)?;

        // Check nonce uniqueness
        self.check_nonce(claims).await?;

        Ok(())
    }

    /// Check timestamp validity
    fn check_timestamp(&self, claims: &AccessClaims) -> Result<(), ReplayError> {
        let now = Utc::now().timestamp();
        let issued_at = claims.standard.iat;
        let expires_at = claims.standard.exp;

        // Check if token is too old
        let token_age = now - issued_at;
        if token_age > self.config.max_token_age {
            warn!(
                "Token too old: age={}, max={}, jti={}",
                token_age, self.config.max_token_age, claims.standard.jti
            );
            return Err(ReplayError::TimestampTooOld(format!(
                "Token issued {} seconds ago",
                token_age
            )));
        }

        // Check if timestamp is in the future (beyond clock skew)
        if issued_at > now + self.config.max_clock_skew {
            error!(
                "Token timestamp in future: iat={}, now={}, jti={}",
                issued_at, now, claims.standard.jti
            );
            return Err(ReplayError::TimestampInFuture(
                "Token issued in the future".to_string(),
            ));
        }

        // Check if token is expired (should be caught by validator, but double-check)
        if now > expires_at {
            warn!(
                "Expired token in replay check: exp={}, now={}, jti={}",
                expires_at, now, claims.standard.jti
            );
            return Err(ReplayError::TimestampTooOld("Token expired".to_string()));
        }

        Ok(())
    }

    /// Check nonce uniqueness
    async fn check_nonce(&self, claims: &AccessClaims) -> Result<(), ReplayError> {
        let nonce = &claims.nonce;
        let jti = TokenId::from_uuid(
            uuid::Uuid::parse_str(&claims.standard.jti)
                .map_err(|_| ReplayError::InvalidTimestamp)?,
        );

        // Check and store nonce atomically
        self.nonce_store
            .check_and_store(nonce, jti, self.config.nonce_window)
            .await
            .map_err(|e| match e {
                NonceStoreError::NonceAlreadyUsed => {
                    error!(
                        "Replay attack detected! Nonce={}, JTI={}",
                        nonce, claims.standard.jti
                    );
                    ReplayError::ReplayDetected(format!("Nonce {} already used", nonce))
                }
                _ => ReplayError::NonceStore(e),
            })?;

        Ok(())
    }

    /// Run cleanup of expired nonces (background task)
    pub async fn cleanup_expired_nonces(&self) -> usize {
        self.nonce_store.cleanup_expired().await
    }

    /// Get statistics
    pub async fn stats(&self) -> super::nonce_store::NonceStoreStats {
        self.nonce_store.stats().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DeviceId, SessionId, StandardClaims, UserDomain, UserId};
    use crate::replay::InMemoryNonceStore;
    use std::collections::HashSet;

    fn create_test_claims(iat_offset: i64, exp_offset: i64, nonce: &str) -> AccessClaims {
        let now = Utc::now();
        let user_id = UserId::new();
        let jti = TokenId::new();

        let standard = StandardClaims {
            iss: "test-issuer".to_string(),
            sub: user_id.to_string(),
            aud: vec!["test-audience".to_string()],
            exp: (now + chrono::Duration::seconds(exp_offset)).timestamp(),
            nbf: now.timestamp(),
            iat: (now + chrono::Duration::seconds(iat_offset)).timestamp(),
            jti: jti.to_string(),
        };

        AccessClaims {
            standard,
            domain: UserDomain::Retail,
            device_id: DeviceId::new().to_string(),
            session_id: SessionId::new().to_string(),
            scopes: HashSet::new(),
            nonce: nonce.to_string(),
            kid: "test-key".to_string(),
            ip: "127.0.0.1".to_string(),
            risk_score: 0.2,
            mfa_verified: false,
            webauthn_verified: false,
            token_version: 1,
        }
    }

    #[tokio::test]
    async fn test_valid_token() {
        let store = Arc::new(InMemoryNonceStore::new());
        let detector = ReplayDetector::new(store, ReplayDetectorConfig::default());

        let claims = create_test_claims(0, 900, "unique-nonce-1");

        assert!(detector.check_token(&claims).await.is_ok());
    }

    #[tokio::test]
    async fn test_replay_attack_detection() {
        let store = Arc::new(InMemoryNonceStore::new());
        let detector = ReplayDetector::new(store, ReplayDetectorConfig::default());

        let claims = create_test_claims(0, 900, "duplicate-nonce");

        // First request should succeed
        assert!(detector.check_token(&claims).await.is_ok());

        // Second request with same nonce should fail
        let result = detector.check_token(&claims).await;
        assert!(matches!(result, Err(ReplayError::ReplayDetected(_))));
    }

    #[tokio::test]
    async fn test_token_too_old() {
        let store = Arc::new(InMemoryNonceStore::new());
        let mut config = ReplayDetectorConfig::default();
        config.max_token_age = 60; // 1 minute max

        let detector = ReplayDetector::new(store, config);

        // Create token issued 2 minutes ago
        let claims = create_test_claims(-120, 900, "old-nonce");

        let result = detector.check_token(&claims).await;
        assert!(matches!(result, Err(ReplayError::TimestampTooOld(_))));
    }

    #[tokio::test]
    async fn test_future_timestamp() {
        let store = Arc::new(InMemoryNonceStore::new());
        let detector = ReplayDetector::new(store, ReplayDetectorConfig::default());

        // Create token issued 10 minutes in the future
        let claims = create_test_claims(600, 1500, "future-nonce");

        let result = detector.check_token(&claims).await;
        assert!(matches!(result, Err(ReplayError::TimestampInFuture(_))));
    }

    #[tokio::test]
    async fn test_cleanup_expired_nonces() {
        let store = Arc::new(InMemoryNonceStore::new());
        let detector = ReplayDetector::new(store.clone(), ReplayDetectorConfig::default());

        // Add some nonces
        let claims1 = create_test_claims(0, 900, "nonce-1");
        let claims2 = create_test_claims(0, 900, "nonce-2");

        detector.check_token(&claims1).await.unwrap();
        detector.check_token(&claims2).await.unwrap();

        // Manually add an expired nonce
        store
            .store_nonce("expired-nonce", TokenId::new(), -100)
            .await
            .unwrap();

        let removed = detector.cleanup_expired_nonces().await;
        assert_eq!(removed, 1);
    }

    #[tokio::test]
    async fn test_stats() {
        let store = Arc::new(InMemoryNonceStore::new());
        let detector = ReplayDetector::new(store, ReplayDetectorConfig::default());

        let claims = create_test_claims(0, 900, "stats-nonce");
        detector.check_token(&claims).await.unwrap();

        // Trigger replay
        let _ = detector.check_token(&claims).await;

        let stats = detector.stats().await;
        assert!(stats.total_nonces > 0);
        assert_eq!(stats.replay_attempts, 1);
    }
}
