use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use thiserror::Error;

use crate::crypto::{KmsError, SignerTrait};
use crate::domain::{
    AccessClaims, AccessToken, Claims, DeviceId, RefreshClaims, RefreshToken, SessionId,
    StandardClaims, TokenId, TokenPair, UserDomain, UserId,
};

/// JWT generation errors
#[derive(Debug, Error)]
pub enum GenerationError {
    #[error("KMS error: {0}")]
    Kms(#[from] KmsError),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Encoding error: {0}")]
    Encoding(String),

    #[error("Invalid configuration: {0}")]
    Config(String),
}

/// JWT generator configuration
#[derive(Debug, Clone)]
pub struct GeneratorConfig {
    /// Issuer (iss claim)
    pub issuer: String,
    /// Audience (aud claim)
    pub audience: Vec<String>,
    /// Token version (for rolling updates)
    pub token_version: u32,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            issuer: "trading-platform-auth".to_string(),
            audience: vec!["trading-platform".to_string()],
            token_version: 1,
        }
    }
}

/// JWT generator
pub struct JwtGenerator {
    signer: Arc<dyn SignerTrait>,
    config: GeneratorConfig,
}

impl JwtGenerator {
    /// Create a new JWT generator
    pub fn new(signer: Arc<dyn SignerTrait>, config: GeneratorConfig) -> Self {
        Self { signer, config }
    }

    /// Generate an access token
    pub async fn generate_access_token(
        &self,
        user_id: UserId,
        device_id: DeviceId,
        session_id: SessionId,
        domain: UserDomain,
        scopes: std::collections::HashSet<crate::domain::ClaimScope>,
        ip: String,
        risk_score: f32,
        mfa_verified: bool,
        webauthn_verified: bool,
        nonce: String,
        ttl: i64,
    ) -> Result<(String, AccessToken), GenerationError> {
        let now = Utc::now();
        let expires_at = now + chrono::Duration::seconds(ttl);
        let jti = TokenId::new();
        let kid = self.signer.get_key_id().await;

        // Create standard claims
        let standard = StandardClaims::new(
            self.config.issuer.clone(),
            user_id,
            self.config.audience.clone(),
            jti,
            now,
            expires_at,
        );

        // Create access claims
        let access_claims = AccessClaims {
            standard,
            domain,
            device_id: device_id.to_string(),
            session_id: session_id.to_string(),
            scopes,
            nonce: nonce.clone(),
            kid: kid.clone(),
            ip,
            risk_score,
            mfa_verified,
            webauthn_verified,
            token_version: self.config.token_version,
        };

        // Wrap in Claims enum for proper serialization
        let claims = Claims::Access(access_claims);

        // Serialize claims to JSON
        let claims_json = serde_json::to_vec(&claims)
            .map_err(|e| GenerationError::Serialization(e.to_string()))?;

        // Sign the claims
        let signature = self.signer.sign(&claims_json).await?;

        // Encode as JWT (header.payload.signature)
        let header = json!({
            "alg": self.signer.algorithm().jwt_algorithm(),
            "typ": "JWT",
            "kid": kid,
        });

        let header_json = serde_json::to_vec(&header)
            .map_err(|e| GenerationError::Serialization(e.to_string()))?;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;

        let header_b64 = URL_SAFE_NO_PAD.encode(&header_json);
        let payload_b64 = URL_SAFE_NO_PAD.encode(&claims_json);
        let signature_b64 = URL_SAFE_NO_PAD.encode(&signature.bytes);

        let jwt = format!("{}.{}.{}", header_b64, payload_b64, signature_b64);

        // Create access token metadata
        let token_metadata = AccessToken {
            jti,
            user_id,
            device_id,
            issued_at: now,
            expires_at,
            nonce,
            algorithm: self.signer.algorithm().jwt_algorithm().to_string(),
            key_id: kid,
        };

        Ok((jwt, token_metadata))
    }

    /// Generate a refresh token
    pub async fn generate_refresh_token(
        &self,
        user_id: UserId,
        device_id: DeviceId,
        session_id: SessionId,
        domain: UserDomain,
        parent_jti: Option<TokenId>,
        generation: u32,
        rotation_count: u32,
        max_rotations: u32,
        ttl: i64,
    ) -> Result<(String, RefreshToken), GenerationError> {
        let now = Utc::now();
        let expires_at = now + chrono::Duration::seconds(ttl);
        let jti = TokenId::new();
        let kid = self.signer.get_key_id().await;

        // Create standard claims
        let standard = StandardClaims::new(
            self.config.issuer.clone(),
            user_id,
            self.config.audience.clone(),
            jti,
            now,
            expires_at,
        );

        // Create refresh claims
        let claims = RefreshClaims {
            standard,
            domain,
            device_id: device_id.to_string(),
            session_id: session_id.to_string(),
            kid: kid.clone(),
            parent_jti: parent_jti.map(|t| t.to_string()),
            generation,
            rotation_count,
            max_rotations,
            token_version: self.config.token_version,
        };

        // Serialize claims to JSON
        let claims_json = serde_json::to_vec(&claims)
            .map_err(|e| GenerationError::Serialization(e.to_string()))?;

        // Sign the claims
        let signature = self.signer.sign(&claims_json).await?;

        // Encode as JWT
        let header = json!({
            "alg": self.signer.algorithm().jwt_algorithm(),
            "typ": "JWT",
            "kid": kid,
        });

        let header_json = serde_json::to_vec(&header)
            .map_err(|e| GenerationError::Serialization(e.to_string()))?;

        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;

        let header_b64 = URL_SAFE_NO_PAD.encode(&header_json);
        let payload_b64 = URL_SAFE_NO_PAD.encode(&claims_json);
        let signature_b64 = URL_SAFE_NO_PAD.encode(&signature.bytes);

        let jwt = format!("{}.{}.{}", header_b64, payload_b64, signature_b64);

        // Create refresh token metadata
        let token_metadata = RefreshToken {
            jti,
            user_id,
            device_id,
            issued_at: now,
            expires_at,
            parent_jti,
            generation,
            max_rotations,
            rotation_count,
            algorithm: self.signer.algorithm().jwt_algorithm().to_string(),
            key_id: kid,
        };

        Ok((jwt, token_metadata))
    }

    /// Generate a complete token pair
    pub async fn generate_token_pair(
        &self,
        user_id: UserId,
        device_id: DeviceId,
        session_id: SessionId,
        domain: UserDomain,
        scopes: std::collections::HashSet<crate::domain::ClaimScope>,
        ip: String,
        risk_score: f32,
        mfa_verified: bool,
        webauthn_verified: bool,
    ) -> Result<TokenPair, GenerationError> {
        // Generate nonce for access token
        let nonce = uuid::Uuid::new_v4().to_string();

        // Get TTLs from domain
        let access_ttl = domain.access_token_ttl();
        let refresh_ttl = domain.refresh_token_ttl();

        // Generate access token
        let (access_jwt, access_metadata) = self
            .generate_access_token(
                user_id,
                device_id,
                session_id,
                domain,
                scopes,
                ip,
                risk_score,
                mfa_verified,
                webauthn_verified,
                nonce,
                access_ttl,
            )
            .await?;

        // Generate refresh token (if applicable for this domain)
        let (refresh_jwt, refresh_metadata) = if refresh_ttl > 0 {
            let (jwt, meta) = self
                .generate_refresh_token(
                    user_id,
                    device_id,
                    session_id,
                    domain,
                    None,
                    0,
                    0,
                    10, // Max 10 rotations
                    refresh_ttl,
                )
                .await?;
            (Some(jwt), Some(meta))
        } else {
            (None, None)
        };

        Ok(TokenPair::new(
            access_jwt,
            refresh_jwt,
            access_metadata,
            refresh_metadata,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::signer::MockSigner;
    use crate::domain::ClaimScope;
    use std::collections::HashSet;

    #[tokio::test]
    async fn test_generate_access_token() {
        let signer = Arc::new(MockSigner::new("test-key".to_string()));
        let config = GeneratorConfig::default();
        let generator = JwtGenerator::new(signer, config);

        let mut scopes = HashSet::new();
        scopes.insert(ClaimScope::TradeRead);
        scopes.insert(ClaimScope::TradeWrite);

        let (jwt, metadata) = generator
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

        assert!(!jwt.is_empty());
        assert!(jwt.contains('.'));
        assert_eq!(metadata.algorithm, "RS256");
    }

    #[tokio::test]
    async fn test_generate_refresh_token() {
        let signer = Arc::new(MockSigner::new("test-key".to_string()));
        let config = GeneratorConfig::default();
        let generator = JwtGenerator::new(signer, config);

        let (jwt, metadata) = generator
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

        assert!(!jwt.is_empty());
        assert!(jwt.contains('.'));
        assert_eq!(metadata.generation, 0);
        assert_eq!(metadata.max_rotations, 10);
    }

    #[tokio::test]
    async fn test_generate_token_pair() {
        let signer = Arc::new(MockSigner::new("test-key".to_string()));
        let config = GeneratorConfig::default();
        let generator = JwtGenerator::new(signer, config);

        let mut scopes = HashSet::new();
        scopes.insert(ClaimScope::TradeRead);

        let token_pair = generator
            .generate_token_pair(
                UserId::new(),
                DeviceId::new(),
                SessionId::new(),
                UserDomain::Retail,
                scopes,
                "127.0.0.1".to_string(),
                0.2,
                false,
                false,
            )
            .await
            .unwrap();

        assert!(!token_pair.access_token.is_empty());
        assert!(token_pair.refresh_token.is_some());
        assert_eq!(token_pair.token_type, "Bearer");
    }

    #[tokio::test]
    async fn test_service_domain_no_refresh() {
        let signer = Arc::new(MockSigner::new("test-key".to_string()));
        let config = GeneratorConfig::default();
        let generator = JwtGenerator::new(signer, config);

        let mut scopes = HashSet::new();
        scopes.insert(ClaimScope::ApiRead);

        let token_pair = generator
            .generate_token_pair(
                UserId::new(),
                DeviceId::new(),
                SessionId::new(),
                UserDomain::Service, // Service domain has no refresh tokens
                scopes,
                "127.0.0.1".to_string(),
                0.0,
                false,
                false,
            )
            .await
            .unwrap();

        assert!(!token_pair.access_token.is_empty());
        assert!(token_pair.refresh_token.is_none());
    }
}
