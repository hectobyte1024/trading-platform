//! WebAuthn authentication ceremony
//!
//! Implements the authentication flow for existing authenticators

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

use super::{
    challenge::{ChallengeGenerator, ChallengeStore, ChallengeType},
    credential_store::CredentialStore,
    types::*,
};
use crate::domain::UserId;

/// Authentication options (sent to client to start authentication)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationOptions {
    /// Challenge (base64url encoded)
    pub challenge: String,
    /// Challenge ID (for verification)
    #[serde(skip_serializing)]
    pub challenge_id: String,
    /// Timeout (milliseconds)
    pub timeout: Option<u64>,
    /// RP ID
    #[serde(rename = "rpId")]
    pub rp_id: String,
    /// Allowed credentials (empty = allow any)
    #[serde(rename = "allowCredentials", skip_serializing_if = "Option::is_none")]
    pub allow_credentials: Option<Vec<CredentialDescriptor>>,
    /// User verification requirement
    #[serde(rename = "userVerification")]
    pub user_verification: UserVerification,
}

/// Authentication challenge (internal)
#[derive(Debug, Clone)]
pub struct AuthenticationChallenge {
    /// Challenge ID
    pub challenge_id: String,
    /// Challenge bytes
    pub challenge: Vec<u8>,
    /// User ID
    pub user_id: UserId,
}

/// Authentication response (from client)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationResponse {
    /// Challenge ID
    pub challenge_id: String,
    /// Credential
    pub credential: PublicKeyCredential,
}

/// Authentication result
#[derive(Debug, Clone)]
pub struct AuthenticationResult {
    /// User ID
    pub user_id: UserId,
    /// Credential ID used
    pub credential_id: Vec<u8>,
    /// New signature counter
    pub new_sign_count: u32,
    /// User verified flag
    pub user_verified: bool,
}

/// Authentication verifier
pub struct AuthenticationVerifier {
    /// RP ID
    rp_id: String,
    /// Expected origin
    origin: String,
    /// Challenge store
    challenge_store: Arc<dyn ChallengeStore>,
    /// Credential store
    credential_store: Arc<dyn CredentialStore>,
}

impl AuthenticationVerifier {
    /// Create a new authentication verifier
    pub fn new(
        rp_id: String,
        origin: String,
        challenge_store: Arc<dyn ChallengeStore>,
        credential_store: Arc<dyn CredentialStore>,
    ) -> Self {
        Self {
            rp_id,
            origin,
            challenge_store,
            credential_store,
        }
    }

    /// Create authentication options for a user
    pub async fn create_options(
        &self,
        user_id: UserId,
        challenge_generator: &ChallengeGenerator,
    ) -> Result<AuthenticationOptions, WebAuthnError> {
        // Generate challenge
        let stored_challenge =
            challenge_generator.generate(user_id, ChallengeType::Authentication);

        let challenge_id = self
            .challenge_store
            .store(stored_challenge.clone())
            .await?;

        // Get user's credentials
        let credentials = self
            .credential_store
            .get_user_credentials(&user_id)
            .await
            .unwrap_or_default();

        let allow_credentials = if !credentials.is_empty() {
            Some(
                credentials
                    .into_iter()
                    .map(|cred| {
                        CredentialDescriptor::new(cred.credential_id, cred.metadata.transports)
                    })
                    .collect(),
            )
        } else {
            None
        };

        Ok(AuthenticationOptions {
            challenge: URL_SAFE_NO_PAD.encode(&stored_challenge.challenge),
            challenge_id,
            timeout: Some(60000), // 60 seconds
            rp_id: self.rp_id.clone(),
            allow_credentials,
            user_verification: UserVerification::Preferred,
        })
    }

    /// Verify authentication response
    pub async fn verify(
        &self,
        response: AuthenticationResponse,
    ) -> Result<AuthenticationResult, WebAuthnError> {
        // 1. Consume challenge (single-use)
        let stored_challenge = self.challenge_store.consume(&response.challenge_id).await?;

        // 2. Extract assertion response
        let assertion = match response.credential.response {
            AuthenticatorResponse::Assertion(ass) => ass,
            _ => return Err(WebAuthnError::InvalidSignature),
        };

        // 3. Parse client data JSON
        let client_data: ClientData = serde_json::from_slice(&assertion.client_data_json)
            .map_err(|e| WebAuthnError::DecodingError(e.to_string()))?;

        // 4. Verify type
        if client_data.type_ != "webauthn.get" {
            return Err(WebAuthnError::InvalidSignature);
        }

        // 5. Verify challenge
        let challenge_bytes = URL_SAFE_NO_PAD
            .decode(&client_data.challenge)
            .map_err(|e| WebAuthnError::DecodingError(e.to_string()))?;

        if challenge_bytes != stored_challenge.challenge {
            return Err(WebAuthnError::InvalidChallenge);
        }

        // 6. Verify origin
        if client_data.origin != self.origin {
            return Err(WebAuthnError::InvalidOrigin {
                expected: self.origin.clone(),
                actual: client_data.origin,
            });
        }

        // 7. Get credential from store
        let credential_id = URL_SAFE_NO_PAD
            .decode(&response.credential.id)
            .map_err(|e| WebAuthnError::DecodingError(e.to_string()))?;

        let mut stored_credential = self.credential_store.get(&credential_id).await?;

        // 8. Verify user ID matches
        if stored_credential.user_id != stored_challenge.user_id {
            return Err(WebAuthnError::InvalidSignature);
        }

        // 9. Parse authenticator data
        let auth_data = self.parse_auth_data_bytes(&assertion.authenticator_data)?;

        // 10. Verify RP ID hash
        let rp_id_hash = Sha256::digest(self.rp_id.as_bytes());
        if auth_data.rp_id_hash != rp_id_hash.as_slice() {
            return Err(WebAuthnError::InvalidRpId {
                expected: self.rp_id.clone(),
                actual: hex::encode(auth_data.rp_id_hash),
            });
        }

        // 11. Verify user present flag
        if !auth_data.flags.user_present {
            return Err(WebAuthnError::UserVerificationFailed);
        }

        // 12. Verify signature counter (clone detection)
        if auth_data.sign_count > 0 && auth_data.sign_count <= stored_credential.sign_count {
            // Possible cloned authenticator - reject
            return Err(WebAuthnError::InvalidSignature);
        }

        // 13. Verify signature
        let client_data_hash = Sha256::digest(&assertion.client_data_json);
        let signed_data = [&assertion.authenticator_data[..], client_data_hash.as_slice()].concat();

        self.verify_signature(
            &stored_credential.public_key,
            &signed_data,
            &assertion.signature,
        )?;

        // 14. Update credential (sign count, last used)
        stored_credential.sign_count = auth_data.sign_count;
        stored_credential.last_used_at = Some(Utc::now());
        self.credential_store.update(stored_credential).await?;

        Ok(AuthenticationResult {
            user_id: stored_challenge.user_id,
            credential_id,
            new_sign_count: auth_data.sign_count,
            user_verified: auth_data.flags.user_verified,
        })
    }

    /// Parse raw authenticator data bytes
    fn parse_auth_data_bytes(&self, data: &[u8]) -> Result<AuthenticatorData, WebAuthnError> {
        if data.len() < 37 {
            return Err(WebAuthnError::DecodingError(
                "authData too short".to_string(),
            ));
        }

        let mut rp_id_hash = [0u8; 32];
        rp_id_hash.copy_from_slice(&data[0..32]);

        let flags = AuthenticatorFlags::from_byte(data[32]);

        let sign_count = u32::from_be_bytes([data[33], data[34], data[35], data[36]]);

        Ok(AuthenticatorData {
            rp_id_hash,
            flags,
            sign_count,
            attested_credential_data: None,
            extensions: None,
        })
    }

    /// Verify signature using stored public key
    fn verify_signature(
        &self,
        public_key_cose: &[u8],
        signed_data: &[u8],
        signature: &[u8],
    ) -> Result<(), WebAuthnError> {
        // Parse COSE public key
        let cose_key: ciborium::Value = ciborium::from_reader(public_key_cose)
            .map_err(|e| WebAuthnError::DecodingError(e.to_string()))?;

        let key_map = cose_key
            .as_map()
            .ok_or_else(|| WebAuthnError::DecodingError("Invalid COSE key".to_string()))?;

        // Extract algorithm (key type 3)
        let alg = key_map
            .iter()
            .find(|(k, _)| k.as_integer() == Some(3.into()))
            .and_then(|(_, v)| v.as_integer())
            .and_then(|i| i.try_into().ok())
            .ok_or_else(|| WebAuthnError::DecodingError("Algorithm not found".to_string()))?;

        match alg {
            -7 => {
                // ES256 (ECDSA with SHA-256)
                self.verify_es256(key_map, signed_data, signature)
            }
            -257 => {
                // RS256 (RSASSA-PKCS1-v1_5 with SHA-256)
                self.verify_rs256(key_map, signed_data, signature)
            }
            _ => Err(WebAuthnError::UnsupportedAlgorithm(alg)),
        }
    }

    /// Verify ES256 signature
    fn verify_es256(
        &self,
        key_map: &[(ciborium::Value, ciborium::Value)],
        signed_data: &[u8],
        signature: &[u8],
    ) -> Result<(), WebAuthnError> {
        use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};

        // Extract x and y coordinates (EC2 key)
        let x = key_map
            .iter()
            .find(|(k, _)| k.as_integer() == Some((-2).into()))
            .and_then(|(_, v)| v.as_bytes())
            .ok_or_else(|| WebAuthnError::DecodingError("x coordinate not found".to_string()))?;

        let y = key_map
            .iter()
            .find(|(k, _)| k.as_integer() == Some((-3).into()))
            .and_then(|(_, v)| v.as_bytes())
            .ok_or_else(|| WebAuthnError::DecodingError("y coordinate not found".to_string()))?;

        // Build uncompressed public key (0x04 || x || y)
        let mut public_key_bytes = vec![0x04];
        public_key_bytes.extend_from_slice(x);
        public_key_bytes.extend_from_slice(y);

        let verifying_key = VerifyingKey::from_sec1_bytes(&public_key_bytes)
            .map_err(|e| WebAuthnError::DecodingError(e.to_string()))?;

        let sig = Signature::from_slice(signature)
            .map_err(|e| WebAuthnError::DecodingError(e.to_string()))?;

        verifying_key
            .verify(signed_data, &sig)
            .map_err(|_| WebAuthnError::InvalidSignature)
    }

    /// Verify RS256 signature
    fn verify_rs256(
        &self,
        key_map: &[(ciborium::Value, ciborium::Value)],
        signed_data: &[u8],
        signature: &[u8],
    ) -> Result<(), WebAuthnError> {
        use rsa::pkcs1v15::Pkcs1v15Sign;
        use rsa::{BigUint, RsaPublicKey};
        use sha2::{Digest, Sha256};

        // Extract n and e (RSA key)
        let n = key_map
            .iter()
            .find(|(k, _)| k.as_integer() == Some((-1).into()))
            .and_then(|(_, v)| v.as_bytes())
            .ok_or_else(|| WebAuthnError::DecodingError("n not found".to_string()))?;

        let e = key_map
            .iter()
            .find(|(k, _)| k.as_integer() == Some((-2).into()))
            .and_then(|(_, v)| v.as_bytes())
            .ok_or_else(|| WebAuthnError::DecodingError("e not found".to_string()))?;

        let n_big = BigUint::from_bytes_be(n);
        let e_big = BigUint::from_bytes_be(e);

        let public_key = RsaPublicKey::new(n_big, e_big)
            .map_err(|e| WebAuthnError::DecodingError(e.to_string()))?;

        // Hash the data
        let digest = Sha256::digest(signed_data);
        
        // Verify signature using raw bytes
        public_key
            .verify(Pkcs1v15Sign::new_unprefixed(), &digest, signature)
            .map_err(|_| WebAuthnError::InvalidSignature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webauthn::challenge::InMemoryChallengeStore;
    use crate::webauthn::credential_store::InMemoryCredentialStore;

    #[tokio::test]
    async fn test_create_authentication_options() {
        let challenge_store = Arc::new(InMemoryChallengeStore::new());
        let credential_store = Arc::new(InMemoryCredentialStore::new());

        let verifier = AuthenticationVerifier::new(
            "trading-platform.com".to_string(),
            "https://trading-platform.com".to_string(),
            challenge_store,
            credential_store,
        );

        let generator = ChallengeGenerator::default();
        let user_id = UserId::new();

        let options = verifier
            .create_options(user_id, &generator)
            .await
            .unwrap();

        assert_eq!(options.rp_id, "trading-platform.com");
        assert!(!options.challenge.is_empty());
        assert_eq!(options.timeout, Some(60000));
    }

    #[test]
    fn test_authenticator_data_parsing() {
        let verifier = AuthenticationVerifier::new(
            "example.com".to_string(),
            "https://example.com".to_string(),
            Arc::new(InMemoryChallengeStore::new()),
            Arc::new(InMemoryCredentialStore::new()),
        );

        // Minimal auth data: 32 bytes RP ID hash + 1 byte flags + 4 bytes counter
        let mut auth_data = vec![0u8; 37];
        auth_data[32] = 0x01; // User present flag
        auth_data[33..37].copy_from_slice(&42u32.to_be_bytes());

        let parsed = verifier.parse_auth_data_bytes(&auth_data).unwrap();

        assert!(parsed.flags.user_present);
        assert_eq!(parsed.sign_count, 42);
    }
}
