//! WebAuthn core types
//!
//! Types for WebAuthn registration and authentication flows

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{DeviceId, UserId};

/// WebAuthn operation errors
#[derive(Debug, Error)]
pub enum WebAuthnError {
    #[error("Invalid challenge")]
    InvalidChallenge,

    #[error("Challenge expired")]
    ChallengeExpired,

    #[error("Challenge not found")]
    ChallengeNotFound,

    #[error("Invalid origin: expected {expected}, got {actual}")]
    InvalidOrigin { expected: String, actual: String },

    #[error("Invalid RP ID: expected {expected}, got {actual}")]
    InvalidRpId { expected: String, actual: String },

    #[error("User verification failed")]
    UserVerificationFailed,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Credential not found: {0}")]
    CredentialNotFound(String),

    #[error("Credential already exists: {0}")]
    CredentialExists(String),

    #[error("Invalid attestation")]
    InvalidAttestation,

    #[error("Unsupported algorithm: {0}")]
    UnsupportedAlgorithm(i64),

    #[error("Decoding error: {0}")]
    DecodingError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Type of authenticator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthenticatorType {
    /// Platform authenticator (TouchID, FaceID, Windows Hello)
    Platform,
    /// Cross-platform authenticator (YubiKey, security key)
    CrossPlatform,
}

/// User verification requirement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserVerification {
    /// User verification required
    Required,
    /// User verification preferred but not required
    Preferred,
    /// User verification discouraged
    Discouraged,
}

impl Default for UserVerification {
    fn default() -> Self {
        Self::Preferred
    }
}

/// Attestation conveyance preference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttestationType {
    /// No attestation
    None,
    /// Indirect attestation (anonymized)
    Indirect,
    /// Direct attestation (full chain)
    Direct,
    /// Enterprise attestation
    Enterprise,
}

impl Default for AttestationType {
    fn default() -> Self {
        Self::None
    }
}

/// Public key credential (from authenticator)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKeyCredential {
    /// Credential ID
    pub id: String,
    /// Raw credential ID (base64url encoded)
    pub raw_id: Vec<u8>,
    /// Response data
    pub response: AuthenticatorResponse,
    /// Authenticator attachment
    pub authenticator_attachment: Option<String>,
    /// Client extension results
    pub client_extension_results: Option<serde_json::Value>,
}

/// Authenticator response (registration or authentication)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AuthenticatorResponse {
    /// Registration response
    Attestation(AttestationResponse),
    /// Authentication response
    Assertion(AssertionResponse),
}

/// Attestation response (registration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationResponse {
    /// Client data JSON
    pub client_data_json: Vec<u8>,
    /// Attestation object
    pub attestation_object: Vec<u8>,
    /// Transports supported by authenticator
    pub transports: Option<Vec<String>>,
}

/// Assertion response (authentication)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResponse {
    /// Client data JSON
    pub client_data_json: Vec<u8>,
    /// Authenticator data
    pub authenticator_data: Vec<u8>,
    /// Signature
    pub signature: Vec<u8>,
    /// User handle (optional)
    pub user_handle: Option<Vec<u8>>,
}

/// Parsed authenticator data
#[derive(Debug, Clone)]
pub struct AuthenticatorData {
    /// SHA-256 hash of RP ID
    pub rp_id_hash: [u8; 32],
    /// Flags byte
    pub flags: AuthenticatorFlags,
    /// Signature counter
    pub sign_count: u32,
    /// Attested credential data (only present during registration)
    pub attested_credential_data: Option<AttestedCredentialData>,
    /// Extensions (optional)
    pub extensions: Option<Vec<u8>>,
}

/// Authenticator flags
#[derive(Debug, Clone, Copy)]
pub struct AuthenticatorFlags {
    /// User present (UP)
    pub user_present: bool,
    /// User verified (UV)
    pub user_verified: bool,
    /// Attested credential data included (AT)
    pub attested_credential_data_included: bool,
    /// Extension data included (ED)
    pub extension_data_included: bool,
}

impl AuthenticatorFlags {
    /// Parse from flags byte
    pub fn from_byte(byte: u8) -> Self {
        Self {
            user_present: (byte & 0x01) != 0,
            user_verified: (byte & 0x04) != 0,
            attested_credential_data_included: (byte & 0x40) != 0,
            extension_data_included: (byte & 0x80) != 0,
        }
    }

    /// Convert to byte
    pub fn to_byte(&self) -> u8 {
        let mut byte = 0u8;
        if self.user_present {
            byte |= 0x01;
        }
        if self.user_verified {
            byte |= 0x04;
        }
        if self.attested_credential_data_included {
            byte |= 0x40;
        }
        if self.extension_data_included {
            byte |= 0x80;
        }
        byte
    }
}

/// Attested credential data (from registration)
#[derive(Debug, Clone)]
pub struct AttestedCredentialData {
    /// AAGUID (Authenticator Attestation GUID)
    pub aaguid: [u8; 16],
    /// Credential ID
    pub credential_id: Vec<u8>,
    /// Credential public key (COSE format)
    pub credential_public_key: Vec<u8>,
}

/// Parsed client data JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientData {
    /// Type ("webauthn.create" or "webauthn.get")
    #[serde(rename = "type")]
    pub type_: String,
    /// Challenge (base64url)
    pub challenge: String,
    /// Origin
    pub origin: String,
    /// Cross-origin (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cross_origin: Option<bool>,
}

/// Credential descriptor (for authentication)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialDescriptor {
    /// Type (always "public-key")
    #[serde(rename = "type")]
    pub type_: String,
    /// Credential ID (base64url)
    pub id: String,
    /// Transports (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transports: Option<Vec<String>>,
}

impl CredentialDescriptor {
    /// Create a new credential descriptor
    pub fn new(credential_id: Vec<u8>, transports: Option<Vec<String>>) -> Self {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;

        Self {
            type_: "public-key".to_string(),
            id: URL_SAFE_NO_PAD.encode(&credential_id),
            transports,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authenticator_flags() {
        let flags = AuthenticatorFlags {
            user_present: true,
            user_verified: true,
            attested_credential_data_included: false,
            extension_data_included: false,
        };

        let byte = flags.to_byte();
        assert_eq!(byte, 0x05); // 0x01 | 0x04

        let parsed = AuthenticatorFlags::from_byte(byte);
        assert!(parsed.user_present);
        assert!(parsed.user_verified);
        assert!(!parsed.attested_credential_data_included);
        assert!(!parsed.extension_data_included);
    }

    #[test]
    fn test_user_verification_default() {
        let uv: UserVerification = Default::default();
        assert_eq!(uv, UserVerification::Preferred);
    }

    #[test]
    fn test_attestation_type_default() {
        let att: AttestationType = Default::default();
        assert_eq!(att, AttestationType::None);
    }

    #[test]
    fn test_credential_descriptor() {
        let cred_id = vec![1, 2, 3, 4, 5];
        let transports = Some(vec!["usb".to_string(), "nfc".to_string()]);

        let descriptor = CredentialDescriptor::new(cred_id.clone(), transports.clone());

        assert_eq!(descriptor.type_, "public-key");
        assert_eq!(descriptor.transports, transports);
    }
}
