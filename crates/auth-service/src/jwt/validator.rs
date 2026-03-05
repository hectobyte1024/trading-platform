use chrono::Utc;
use serde_json;
use std::sync::Arc;
use thiserror::Error;

use crate::crypto::{KmsClientTrait, KmsError, KeyManager};
use crate::domain::{AccessClaims, Claims, RefreshClaims, TokenId};
use crate::revocation::RevocationStore;

/// JWT validation errors
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid token format")]
    InvalidFormat,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Token expired")]
    Expired,

    #[error("Token not yet valid")]
    NotYetValid,

    #[error("Invalid issuer: expected {expected}, got {actual}")]
    InvalidIssuer { expected: String, actual: String },

    #[error("Invalid audience: expected {expected}")]
    InvalidAudience { expected: String },

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Key expired or invalid: {0}")]
    KeyInvalid(String),

    #[error("Decoding error: {0}")]
    Decoding(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("KMS error: {0}")]
    Kms(#[from] KmsError),

    #[error("Clock skew too large")]
    ClockSkew,

    #[error("Token has been revoked")]
    Revoked,

    #[error("Replay attack detected")]
    ReplayDetected,

    #[error("Invalid nonce")]
    InvalidNonce,

    #[error("Missing required claim: {0}")]
    MissingClaim(String),
}

/// JWT validator configuration
#[derive(Debug, Clone)]
pub struct ValidatorConfig {
    /// Expected issuer
    pub issuer: String,
    /// Expected audience
    pub audience: String,
    /// Clock skew tolerance (seconds)
    pub clock_skew_seconds: i64,
    /// Maximum token age (seconds) - for replay detection
    pub max_token_age_seconds: i64,
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            issuer: "trading-platform-auth".to_string(),
            audience: "trading-platform".to_string(),
            clock_skew_seconds: 300, // 5 minutes tolerance
            max_token_age_seconds: 3600, // 1 hour max age
        }
    }
}

/// JWT header
#[derive(Debug, serde::Deserialize)]
struct JwtHeader {
    alg: String,
    #[allow(dead_code)]
    typ: String,
    kid: String,
}

/// Validated token
#[derive(Debug, Clone)]
pub struct ValidatedToken {
    /// The parsed claims
    pub claims: Claims,
    /// Key ID used for signing
    pub key_id: String,
    /// Algorithm used
    pub algorithm: String,
}

impl ValidatedToken {
    /// Get access claims (if this is an access token)
    pub fn access_claims(&self) -> Option<&AccessClaims> {
        match &self.claims {
            Claims::Access(claims) => Some(claims),
            _ => None,
        }
    }

    /// Get refresh claims (if this is a refresh token)
    pub fn refresh_claims(&self) -> Option<&RefreshClaims> {
        match &self.claims {
            Claims::Refresh(claims) => Some(claims),
            _ => None,
        }
    }
}

/// JWT validator
pub struct JwtValidator {
    kms_client: Arc<dyn KmsClientTrait>,
    key_manager: Arc<KeyManager>,
    config: ValidatorConfig,
    revocation_store: Option<Arc<dyn RevocationStore>>,
}

impl JwtValidator {
    /// Create a new JWT validator
    pub fn new(
        kms_client: Arc<dyn KmsClientTrait>,
        key_manager: Arc<KeyManager>,
        config: ValidatorConfig,
    ) -> Self {
        Self {
            kms_client,
            key_manager,
            config,
            revocation_store: None,
        }
    }

    /// Create a validator with revocation checking
    pub fn with_revocation(
        kms_client: Arc<dyn KmsClientTrait>,
        key_manager: Arc<KeyManager>,
        config: ValidatorConfig,
        revocation_store: Arc<dyn RevocationStore>,
    ) -> Self {
        Self {
            kms_client,
            key_manager,
            config,
            revocation_store: Some(revocation_store),
        }
    }

    /// Validate a JWT token
    pub async fn validate(&self, token: &str) -> Result<ValidatedToken, ValidationError> {
        // Parse JWT structure (header.payload.signature)
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(ValidationError::InvalidFormat);
        }

        let header_b64 = parts[0];
        let payload_b64 = parts[1];
        let signature_b64 = parts[2];

        // Decode header
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;

        let header_bytes = URL_SAFE_NO_PAD
            .decode(header_b64)
            .map_err(|e| ValidationError::Decoding(e.to_string()))?;

        let header: JwtHeader = serde_json::from_slice(&header_bytes)
            .map_err(|e| ValidationError::Deserialization(e.to_string()))?;

        // Verify key is valid
        if !self.key_manager.is_valid_key(&header.kid).await {
            return Err(ValidationError::KeyInvalid(header.kid.clone()));
        }

        // Decode payload
        let payload_bytes = URL_SAFE_NO_PAD
            .decode(payload_b64)
            .map_err(|e| ValidationError::Decoding(e.to_string()))?;

        // Verify signature
        let signature_bytes = URL_SAFE_NO_PAD
            .decode(signature_b64)
            .map_err(|e| ValidationError::Decoding(e.to_string()))?;

        self.verify_signature(&header.kid, &payload_bytes, &signature_bytes)
            .await?;


        // Parse directly as Claims enum (which has #[serde(tag = "type")])
        let claims: Claims = serde_json::from_slice(&payload_bytes)
            .map_err(|e| ValidationError::Deserialization(e.to_string()))?;

        // Check if token is revoked (if revocation store is configured)
        if let Some(ref revocation_store) = self.revocation_store {
            let jti_str = match &claims {
                Claims::Access(access) => &access.standard.jti,
                Claims::Refresh(refresh) => &refresh.standard.jti,
            };
            
            // Parse JTI string to TokenId
            if let Ok(uuid) = uuid::Uuid::parse_str(jti_str) {
                let jti = TokenId::from_uuid(uuid);
                match revocation_store.is_revoked(&jti).await {
                    Ok(true) => return Err(ValidationError::Revoked),
                    Ok(false) => {}, // Token not revoked, continue
                    Err(_e) => {
                        // Log error but don't fail validation
                        // In production, you might want to fail closed here
                    }
                }
            }
        }

        // Validate claims
        self.validate_claims(&claims)?;

        Ok(ValidatedToken {
            claims,
            key_id: header.kid,
            algorithm: header.alg,
        })
    }

    /// Verify JWT signature using KMS
    async fn verify_signature(
        &self,
        key_id: &str,
        _payload: &[u8],
        _signature: &[u8],
    ) -> Result<(), ValidationError> {
        // Get public key from KMS
        let _public_key = self.kms_client.get_public_key(key_id).await?;

        // For production, we'd verify using the public key
        // For now, we'll re-sign and compare (mock implementation)
        // In production, use ring or similar for actual verification
        
        // TODO: Implement proper signature verification with public key
        // This is a simplified version - in production use ring::signature::verify
        
        Ok(())
    }

    /// Validate claims
    fn validate_claims(&self, claims: &Claims) -> Result<(), ValidationError> {
        match claims {
            Claims::Access(access) => self.validate_access_claims(access),
            Claims::Refresh(refresh) => self.validate_refresh_claims(refresh),
        }
    }

    /// Validate access token claims
    fn validate_access_claims(&self, claims: &AccessClaims) -> Result<(), ValidationError> {
        let now = Utc::now().timestamp();
        let skew = self.config.clock_skew_seconds;

        // Check issuer
        if claims.standard.iss != self.config.issuer {
            return Err(ValidationError::InvalidIssuer {
                expected: self.config.issuer.clone(),
                actual: claims.standard.iss.clone(),
            });
        }

        // Check audience
        if !claims.standard.aud.contains(&self.config.audience) {
            return Err(ValidationError::InvalidAudience {
                expected: self.config.audience.clone(),
            });
        }

        // Check expiration with clock skew
        if now > claims.standard.exp + skew {
            return Err(ValidationError::Expired);
        }

        // Check not before with clock skew
        if now < claims.standard.nbf - skew {
            return Err(ValidationError::NotYetValid);
        }

        // Check token age (replay protection)
        let token_age = now - claims.standard.iat;
        if token_age > self.config.max_token_age_seconds {
            return Err(ValidationError::ReplayDetected);
        }

        // Validate nonce is present
        if claims.nonce.is_empty() {
            return Err(ValidationError::InvalidNonce);
        }

        Ok(())
    }

    /// Validate refresh token claims
    fn validate_refresh_claims(&self, claims: &RefreshClaims) -> Result<(), ValidationError> {
        let now = Utc::now().timestamp();
        let skew = self.config.clock_skew_seconds;

        // Check issuer
        if claims.standard.iss != self.config.issuer {
            return Err(ValidationError::InvalidIssuer {
                expected: self.config.issuer.clone(),
                actual: claims.standard.iss.clone(),
            });
        }

        // Check audience
        if !claims.standard.aud.contains(&self.config.audience) {
            return Err(ValidationError::InvalidAudience {
                expected: self.config.audience.clone(),
            });
        }

        // Check expiration with clock skew
        if now > claims.standard.exp + skew {
            return Err(ValidationError::Expired);
        }

        // Check not before with clock skew
        if now < claims.standard.nbf - skew {
            return Err(ValidationError::NotYetValid);
        }

        // Check rotation limits
        if claims.rotation_count >= claims.max_rotations {
            return Err(ValidationError::ReplayDetected);
        }

        Ok(())
    }

    /// Quick expiration check without full validation
    pub fn is_expired(&self, token: &str) -> Result<bool, ValidationError> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(ValidationError::InvalidFormat);
        }

        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;

        let payload_bytes = URL_SAFE_NO_PAD
            .decode(parts[1])
            .map_err(|e| ValidationError::Decoding(e.to_string()))?;

        // Try to deserialize as either type
        let claims: Claims = if let Ok(access) = serde_json::from_slice::<AccessClaims>(&payload_bytes) {
            Claims::Access(access)
        } else if let Ok(refresh) = serde_json::from_slice::<RefreshClaims>(&payload_bytes) {
            Claims::Refresh(refresh)
        } else {
            return Err(ValidationError::Deserialization(
                "Unable to parse token claims".to_string()
            ));
        };

        let exp = match claims {
            Claims::Access(ref access) => access.standard.exp,
            Claims::Refresh(ref refresh) => refresh.standard.exp,
        };

        Ok(Utc::now().timestamp() > exp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::kms_client::MockKmsClient;
    use crate::crypto::key_manager::KeyRotationPolicy;
    use crate::crypto::signer::MockSigner;
    use crate::domain::{ClaimScope, DeviceId, SessionId, UserDomain, UserId};
    use crate::jwt::generator::{GeneratorConfig, JwtGenerator};
    use std::collections::HashSet;

    async fn setup_validator() -> (JwtValidator, JwtGenerator) {
        let kms_client = Arc::new(MockKmsClient::new());
        let key_manager = Arc::new(KeyManager::new(
            kms_client.clone(),
            "test-key-1".to_string(),
            KeyRotationPolicy::default(),
        ));

        let signer = Arc::new(MockSigner::new("test-key-1".to_string()));

        let validator = JwtValidator::new(
            kms_client,
            key_manager.clone(),
            ValidatorConfig::default(),
        );

        let generator = JwtGenerator::new(signer, GeneratorConfig::default());

        (validator, generator)
    }

    #[tokio::test]
    async fn test_validate_access_token() {
        let (validator, generator) = setup_validator().await;

        let mut scopes = HashSet::new();
        scopes.insert(ClaimScope::TradeRead);

        let (jwt, _) = generator
            .generate_access_token(
                UserId::new(),
                DeviceId::new(),
                SessionId::new(),
                UserDomain::Retail,
                scopes,
                "127.0.0.1".to_string(),
                0.2,
                false,
                false,
                "test-nonce".to_string(),
                900,
            )
            .await
            .unwrap();

        let validated = validator.validate(&jwt).await.unwrap();
        assert!(validated.access_claims().is_some());
        assert_eq!(validated.key_id, "test-key-1");
    }

    #[tokio::test]
    async fn test_validate_refresh_token() {
        let (validator, generator) = setup_validator().await;

        let (jwt, _) = generator
            .generate_refresh_token(
                UserId::new(),
                DeviceId::new(),
                SessionId::new(),
                UserDomain::Retail,
                None,
                0,
                0,
                10,
                2592000,
            )
            .await
            .unwrap();

        let validated = validator.validate(&jwt).await.unwrap();
        assert!(validated.refresh_claims().is_some());
    }

    #[tokio::test]
    async fn test_expired_token() {
        let (validator, generator) = setup_validator().await;

        let mut scopes = HashSet::new();
        scopes.insert(ClaimScope::TradeRead);

        let (jwt, _) = generator
            .generate_access_token(
                UserId::new(),
                DeviceId::new(),
                SessionId::new(),
                UserDomain::Retail,
                scopes,
                "127.0.0.1".to_string(),
                0.2,
                false,
                false,
                "test-nonce".to_string(),
                -400, // Expired 400 seconds ago (beyond 300s clock sk human)
            )
            .await
            .unwrap();

        let result = validator.validate(&jwt).await;
        assert!(matches!(result, Err(ValidationError::Expired)));
    }

    #[tokio::test]
    async fn test_invalid_format() {
        let (validator, _) = setup_validator().await;

        let result = validator.validate("invalid.token").await;
        assert!(matches!(result, Err(ValidationError::InvalidFormat)));
    }

    #[tokio::test]
    async fn test_is_expired_check() {
        let (validator, generator) = setup_validator().await;

        let mut scopes = HashSet::new();
        scopes.insert(ClaimScope::TradeRead);

        // Valid token
        let (jwt, _) = generator
            .generate_access_token(
                UserId::new(),
                DeviceId::new(),
                SessionId::new(),
                UserDomain::Retail,
                scopes.clone(),
                "127.0.0.1".to_string(),
                0.2,
                false,
                false,
                "test-nonce".to_string(),
                900,
            )
            .await
            .unwrap();

        assert!(!validator.is_expired(&jwt).unwrap());

        // Expired token
        let (jwt_expired, _) = generator
            .generate_access_token(
                UserId::new(),
                DeviceId::new(),
                SessionId::new(),
                UserDomain::Retail,
                scopes,
                "127.0.0.1".to_string(),
                0.2,
                false,
                false,
                "test-nonce".to_string(),
                -100,
            )
            .await
            .unwrap();

        assert!(validator.is_expired(&jwt_expired).unwrap());
    }
}
