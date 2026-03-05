use async_trait::async_trait;
use std::sync::Arc;

use super::{KmsClientTrait, KmsError, KeyManager};

/// Signature produced by a signer
#[derive(Debug, Clone)]
pub struct Signature {
    /// The signature bytes
    pub bytes: Vec<u8>,
    /// The key ID used for signing
    pub key_id: String,
    /// The algorithm used
    pub algorithm: SigningAlgorithm,
}

/// Signing algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigningAlgorithm {
    /// RSA with SHA-256 (RS256)
    Rs256,
    /// ECDSA with SHA-256 (ES256)
    Es256,
    /// ECDSA with SHA-384 (ES384)
    Es384,
}

impl SigningAlgorithm {
    /// Get the KMS algorithm string
    pub fn kms_algorithm(&self) -> &'static str {
        match self {
            Self::Rs256 => "RSASSA_PKCS1_V1_5_SHA_256",
            Self::Es256 => "ECDSA_SHA_256",
            Self::Es384 => "ECDSA_SHA_384",
        }
    }

    /// Get the JWT algorithm string
    pub fn jwt_algorithm(&self) -> &'static str {
        match self {
            Self::Rs256 => "RS256",
            Self::Es256 => "ES256",
            Self::Es384 => "ES384",
        }
    }

    /// Parse from JWT algorithm string
    pub fn from_jwt_algorithm(alg: &str) -> Option<Self> {
        match alg {
            "RS256" => Some(Self::Rs256),
            "ES256" => Some(Self::Es256),
            "ES384" => Some(Self::Es384),
            _ => None,
        }
    }
}

/// Signer trait for signing messages
#[async_trait]
pub trait SignerTrait: Send + Sync {
    /// Sign a message
    async fn sign(&self, message: &[u8]) -> Result<Signature, KmsError>;

    /// Get the current signing key ID
    async fn get_key_id(&self) -> String;

    /// Get the signing algorithm
    fn algorithm(&self) -> SigningAlgorithm;
}

/// KMS-backed signer
pub struct Signer {
    kms_client: Arc<dyn KmsClientTrait>,
    key_manager: Arc<KeyManager>,
    algorithm: SigningAlgorithm,
}

impl Signer {
    /// Create a new signer
    pub fn new(
        kms_client: Arc<dyn KmsClientTrait>,
        key_manager: Arc<KeyManager>,
        algorithm: SigningAlgorithm,
    ) -> Self {
        Self {
            kms_client,
            key_manager,
            algorithm,
        }
    }

    /// Create signer with RS256 algorithm
    pub fn rs256(kms_client: Arc<dyn KmsClientTrait>, key_manager: Arc<KeyManager>) -> Self {
        Self::new(kms_client, key_manager, SigningAlgorithm::Rs256)
    }

    /// Create signer with ES256 algorithm
    pub fn es256(kms_client: Arc<dyn KmsClientTrait>, key_manager: Arc<KeyManager>) -> Self {
        Self::new(kms_client, key_manager, SigningAlgorithm::Es256)
    }
}

#[async_trait]
impl SignerTrait for Signer {
    async fn sign(&self, message: &[u8]) -> Result<Signature, KmsError> {
        let key_id = self.key_manager.get_active_key_id().await;
        
        // Sign using KMS
        let signature_bytes = self.kms_client.sign(&key_id, message).await?;

        Ok(Signature {
            bytes: signature_bytes,
            key_id,
            algorithm: self.algorithm,
        })
    }

    async fn get_key_id(&self) -> String {
        self.key_manager.get_active_key_id().await
    }

    fn algorithm(&self) -> SigningAlgorithm {
        self.algorithm
    }
}

/// In-memory signer for testing (NEVER use in production)
#[cfg(test)]
pub struct MockSigner {
    key_id: String,
    algorithm: SigningAlgorithm,
}

#[cfg(test)]
impl MockSigner {
    pub fn new(key_id: String) -> Self {
        Self {
            key_id,
            algorithm: SigningAlgorithm::Rs256,
        }
    }
}

#[cfg(test)]
#[async_trait]
impl SignerTrait for MockSigner {
    async fn sign(&self, message: &[u8]) -> Result<Signature, KmsError> {
        use ring::digest;
        
        // Create a fake signature by hashing the message
        let digest = digest::digest(&digest::SHA256, message);
        
        Ok(Signature {
            bytes: digest.as_ref().to_vec(),
            key_id: self.key_id.clone(),
            algorithm: self.algorithm,
        })
    }

    async fn get_key_id(&self) -> String {
        self.key_id.clone()
    }

    fn algorithm(&self) -> SigningAlgorithm {
        self.algorithm
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::kms_client::MockKmsClient;
    use crate::crypto::key_manager::KeyRotationPolicy;

    #[test]
    fn test_signing_algorithm_conversions() {
        assert_eq!(SigningAlgorithm::Rs256.kms_algorithm(), "RSASSA_PKCS1_V1_5_SHA_256");
        assert_eq!(SigningAlgorithm::Rs256.jwt_algorithm(), "RS256");
        
        assert_eq!(SigningAlgorithm::from_jwt_algorithm("RS256"), Some(SigningAlgorithm::Rs256));
        assert_eq!(SigningAlgorithm::from_jwt_algorithm("ES256"), Some(SigningAlgorithm::Es256));
        assert_eq!(SigningAlgorithm::from_jwt_algorithm("INVALID"), None);
    }

    #[tokio::test]
    async fn test_signer() {
        let kms_client = Arc::new(MockKmsClient::new());
        let key_manager = Arc::new(KeyManager::new(
            kms_client.clone(),
            "test-key-1".to_string(),
            KeyRotationPolicy::default(),
        ));

        let signer = Signer::rs256(kms_client, key_manager);

        assert_eq!(signer.algorithm(), SigningAlgorithm::Rs256);
        assert_eq!(signer.get_key_id().await, "test-key-1");

        let signature = signer.sign(b"test message").await.unwrap();
        assert!(!signature.bytes.is_empty());
        assert_eq!(signature.key_id, "test-key-1");
        assert_eq!(signature.algorithm, SigningAlgorithm::Rs256);
    }

    #[tokio::test]
    async fn test_mock_signer() {
        let signer = MockSigner::new("mock-key".to_string());
        
        let signature = signer.sign(b"test message").await.unwrap();
        assert!(!signature.bytes.is_empty());
        assert_eq!(signature.key_id, "mock-key");
    }

    #[tokio::test]
    async fn test_signer_after_key_rotation() {
        let kms_client = Arc::new(MockKmsClient::new());
        let key_manager = Arc::new(KeyManager::new(
            kms_client.clone(),
            "test-key-1".to_string(),
            KeyRotationPolicy::default(),
        ));

        let signer = Signer::rs256(kms_client, key_manager.clone());

        // Initial key
        let sig1 = signer.sign(b"message1").await.unwrap();
        assert_eq!(sig1.key_id, "test-key-1");

        // Rotate key
        key_manager.rotate_key("test-key-2".to_string()).await.unwrap();

        // New signatures should use new key
        let sig2 = signer.sign(b"message2").await.unwrap();
        assert_eq!(sig2.key_id, "test-key-2");
    }
}
