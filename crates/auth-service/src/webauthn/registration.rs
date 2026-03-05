//! WebAuthn registration ceremony
//!
//! Implements the registration flow for new authenticators

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

use super::{
    challenge::{ChallengeGenerator, ChallengeStore, ChallengeType},
    credential_store::{CredentialMetadata, CredentialStore, StoredCredential},
    types::*,
};
use crate::domain::{DeviceId, UserId};

/// Registration options (sent to client to start registration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationOptions {
    /// Challenge (base64url encoded)
    pub challenge: String,
    /// Challenge ID (for verification)
    #[serde(skip_serializing)]
    pub challenge_id: String,
    /// Relying party info
    pub rp: RelyingParty,
    /// User info
    pub user: UserInfo,
    /// Public key credential parameters
    #[serde(rename = "pubKeyCredParams")]
    pub pub_key_cred_params: Vec<PubKeyCredParam>,
    /// Timeout (milliseconds)
    pub timeout: Option<u64>,
    /// Excluded credentials (prevent re-registration)
    #[serde(rename = "excludeCredentials", skip_serializing_if = "Option::is_none")]
    pub exclude_credentials: Option<Vec<CredentialDescriptor>>,
    /// Authenticator selection criteria
    #[serde(rename = "authenticatorSelection", skip_serializing_if = "Option::is_none")]
    pub authenticator_selection: Option<AuthenticatorSelection>,
    /// Attestation conveyance
    pub attestation: AttestationType,
}

/// Relying party information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelyingParty {
    /// RP name
    pub name: String,
    /// RP ID (domain)
    pub id: String,
}

/// User information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// User ID (base64url encoded)
    pub id: String,
    /// Display name
    pub name: String,
    /// Display name (friendly)
    #[serde(rename = "displayName")]
    pub display_name: String,
}

/// Public key credential parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubKeyCredParam {
    /// Type (always "public-key")
    #[serde(rename = "type")]
    pub type_: String,
    /// Algorithm (COSE algorithm identifier)
    pub alg: i64,
}

/// Authenticator selection criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatorSelection {
    /// Authenticator attachment
    #[serde(rename = "authenticatorAttachment", skip_serializing_if = "Option::is_none")]
    pub authenticator_attachment: Option<String>,
    /// Resident key requirement
    #[serde(rename = "residentKey", skip_serializing_if = "Option::is_none")]
    pub resident_key: Option<String>,
    /// Require resident key (deprecated, use resident_key)
    #[serde(rename = "requireResidentKey", skip_serializing_if = "Option::is_none")]
    pub require_resident_key: Option<bool>,
    /// User verification requirement
    #[serde(rename = "userVerification")]
    pub user_verification: UserVerification,
}

/// Registration challenge (internal)
#[derive(Debug, Clone)]
pub struct RegistrationChallenge {
    /// Challenge ID
    pub challenge_id: String,
    /// Challenge bytes
    pub challenge: Vec<u8>,
    /// User ID
    pub user_id: UserId,
}

/// Registration response (from client)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationResponse {
    /// Challenge ID
    pub challenge_id: String,
    /// Credential
    pub credential: PublicKeyCredential,
    /// Device ID (optional)
    pub device_id: Option<String>,
    /// Credential name (optional)
    pub credential_name: Option<String>,
}

/// Registration verifier
pub struct RegistrationVerifier {
    /// RP ID
    rp_id: String,
    /// Expected origin
    origin: String,
    /// Challenge store
    challenge_store: Arc<dyn ChallengeStore>,
    /// Credential store
    credential_store: Arc<dyn CredentialStore>,
}

impl RegistrationVerifier {
    /// Create a new registration verifier
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

    /// Create registration options for a user
    pub async fn create_options(
        &self,
        user_id: UserId,
        username: String,
        display_name: String,
        challenge_generator: &ChallengeGenerator,
        authenticator_type: Option<AuthenticatorType>,
    ) -> Result<RegistrationOptions, WebAuthnError> {
        // Generate challenge
        let stored_challenge =
            challenge_generator.generate(user_id, ChallengeType::Registration);

        let challenge_id = self
            .challenge_store
            .store(stored_challenge.clone())
            .await?;

        // Get existing credentials to exclude
        let existing_credentials = self
            .credential_store
            .get_user_credentials(&user_id)
            .await
            .unwrap_or_default();

        let exclude_credentials = if !existing_credentials.is_empty() {
            Some(
                existing_credentials
                    .into_iter()
                    .map(|cred| {
                        CredentialDescriptor::new(cred.credential_id, cred.metadata.transports)
                    })
                    .collect(),
            )
        } else {
            None
        };

        // Build authenticator selection
        let authenticator_selection = Some(AuthenticatorSelection {
            authenticator_attachment: authenticator_type.map(|t| match t {
                AuthenticatorType::Platform => "platform".to_string(),
                AuthenticatorType::CrossPlatform => "cross-platform".to_string(),
            }),
            resident_key: Some("preferred".to_string()),
            require_resident_key: None,
            user_verification: UserVerification::Preferred,
        });

        Ok(RegistrationOptions {
            challenge: URL_SAFE_NO_PAD.encode(&stored_challenge.challenge),
            challenge_id,
            rp: RelyingParty {
                name: "Trading Platform".to_string(),
                id: self.rp_id.clone(),
            },
            user: UserInfo {
                id: URL_SAFE_NO_PAD.encode(user_id.0.as_bytes()),
                name: username,
                display_name,
            },
            pub_key_cred_params: vec![
                PubKeyCredParam {
                    type_: "public-key".to_string(),
                    alg: -7, // ES256
                },
                PubKeyCredParam {
                    type_: "public-key".to_string(),
                    alg: -257, // RS256
                },
            ],
            timeout: Some(60000), // 60 seconds
            exclude_credentials,
            authenticator_selection,
            attestation: AttestationType::None,
        })
    }

    /// Verify registration response
    pub async fn verify(
        &self,
        response: RegistrationResponse,
    ) -> Result<StoredCredential, WebAuthnError> {
        // 1. Consume challenge (single-use)
        let stored_challenge = self.challenge_store.consume(&response.challenge_id).await?;

        // 2. Extract attestation response
        let attestation = match response.credential.response {
            AuthenticatorResponse::Attestation(att) => att,
            _ => return Err(WebAuthnError::InvalidAttestation),
        };

        // 3. Parse client data JSON
        let client_data: ClientData = serde_json::from_slice(&attestation.client_data_json)
            .map_err(|e| WebAuthnError::DecodingError(e.to_string()))?;

        // 4. Verify type
        if client_data.type_ != "webauthn.create" {
            return Err(WebAuthnError::InvalidAttestation);
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

        // 7. Parse authenticator data
        let auth_data = self.parse_authenticator_data(&attestation.attestation_object)?;

        // 8. Verify RP ID hash
        let rp_id_hash = Sha256::digest(self.rp_id.as_bytes());
        if auth_data.rp_id_hash != rp_id_hash.as_slice() {
            return Err(WebAuthnError::InvalidRpId {
                expected: self.rp_id.clone(),
                actual: hex::encode(auth_data.rp_id_hash),
            });
        }

        // 9. Verify user present flag
        if !auth_data.flags.user_present {
            return Err(WebAuthnError::UserVerificationFailed);
        }

        // 10. Extract credential data
        let attested_cred = auth_data
            .attested_credential_data
            .ok_or(WebAuthnError::InvalidAttestation)?;

        // 11. Build stored credential
        let device_id = response
            .device_id
            .and_then(|id| uuid::Uuid::parse_str(&id).ok())
            .map(DeviceId);

        let credential = StoredCredential {
            credential_id: attested_cred.credential_id,
            user_id: stored_challenge.user_id,
            device_id,
            public_key: attested_cred.credential_public_key,
            sign_count: auth_data.sign_count,
            metadata: CredentialMetadata {
                authenticator_type: if response
                    .credential
                    .authenticator_attachment
                    .as_deref()
                    == Some("platform")
                {
                    AuthenticatorType::Platform
                } else {
                    AuthenticatorType::CrossPlatform
                },
                aaguid: attested_cred.aaguid,
                transports: attestation.transports,
                name: response.credential_name,
                backup_eligible: false,
                backup_state: false,
            },
            created_at: Utc::now(),
            last_used_at: None,
        };

        // 12. Store credential
        self.credential_store.store(credential.clone()).await?;

        Ok(credential)
    }

    /// Parse authenticator data from attestation object
    fn parse_authenticator_data(
        &self,
        attestation_object: &[u8],
    ) -> Result<AuthenticatorData, WebAuthnError> {
        // Decode CBOR attestation object
        let value: ciborium::Value = ciborium::from_reader(attestation_object)
            .map_err(|e| WebAuthnError::DecodingError(e.to_string()))?;

        // Extract authData
        let auth_data_bytes = value
            .as_map()
            .and_then(|map| {
                map.iter()
                    .find(|(k, _)| k.as_text() == Some("authData"))
                    .and_then(|(_, v)| v.as_bytes())
            })
            .ok_or_else(|| WebAuthnError::DecodingError("authData not found".to_string()))?;

        self.parse_auth_data_bytes(auth_data_bytes)
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

        let mut attested_credential_data = None;
        if flags.attested_credential_data_included {
            if data.len() < 55 {
                return Err(WebAuthnError::DecodingError(
                    "attested credential data missing".to_string(),
                ));
            }

            let mut aaguid = [0u8; 16];
            aaguid.copy_from_slice(&data[37..53]);

            let cred_id_len = u16::from_be_bytes([data[53], data[54]]) as usize;

            if data.len() < 55 + cred_id_len {
                return Err(WebAuthnError::DecodingError(
                    "credential ID incomplete".to_string(),
                ));
            }

            let credential_id = data[55..55 + cred_id_len].to_vec();

            // Public key follows credential ID (CBOR encoded)
            let public_key_start = 55 + cred_id_len;
            let credential_public_key = data[public_key_start..].to_vec();

            attested_credential_data = Some(AttestedCredentialData {
                aaguid,
                credential_id,
                credential_public_key,
            });
        }

        Ok(AuthenticatorData {
            rp_id_hash,
            flags,
            sign_count,
            attested_credential_data,
            extensions: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webauthn::challenge::InMemoryChallengeStore;
    use crate::webauthn::credential_store::InMemoryCredentialStore;

    #[tokio::test]
    async fn test_create_registration_options() {
        let challenge_store = Arc::new(InMemoryChallengeStore::new());
        let credential_store = Arc::new(InMemoryCredentialStore::new());

        let verifier = RegistrationVerifier::new(
            "trading-platform.com".to_string(),
            "https://trading-platform.com".to_string(),
            challenge_store,
            credential_store,
        );

        let generator = ChallengeGenerator::default();
        let user_id = UserId::new();

        let options = verifier
            .create_options(
                user_id,
                "testuser".to_string(),
                "Test User".to_string(),
                &generator,
                Some(AuthenticatorType::Platform),
            )
            .await
            .unwrap();

        assert_eq!(options.rp.id, "trading-platform.com");
        assert_eq!(options.user.name, "testuser");
        assert!(!options.challenge.is_empty());
        assert_eq!(options.pub_key_cred_params.len(), 2);
    }

    #[test]
    fn test_authenticator_flags_parsing() {
        let flags = AuthenticatorFlags::from_byte(0x45); // UP=1, UV=1, AT=1

        assert!(flags.user_present);
        assert!(flags.user_verified);
        assert!(flags.attested_credential_data_included);
        assert!(!flags.extension_data_included);
    }
}
