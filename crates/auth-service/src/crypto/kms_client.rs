use async_trait::async_trait;
use rusoto_core::Region;
use rusoto_kms::{Kms, KmsClient as RusotoKmsClient, SignRequest};
use thiserror::Error;

/// KMS errors
#[derive(Debug, Error)]
pub enum KmsError {
    #[error("AWS KMS error: {0}")]
    AwsKms(String),

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Invalid key ID format")]
    InvalidKeyId,

    #[error("Operation not supported: {0}")]
    Unsupported(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Cryptographic error: {0}")]
    Crypto(String),
}

/// KMS configuration
#[derive(Debug, Clone)]
pub struct KmsConfig {
    /// AWS region
    pub region: Region,
    /// KMS key ID for JWT signing (primary)
    pub jwt_signing_key_id: String,
    /// Previous KMS key ID (for rotation overlap)
    pub jwt_signing_key_id_previous: Option<String>,
    /// Signature algorithm (RS256, ES256, etc.)
    pub signing_algorithm: String,
}

impl Default for KmsConfig {
    fn default() -> Self {
        Self {
            region: Region::UsEast1,
            jwt_signing_key_id: String::new(),
            jwt_signing_key_id_previous: None,
            signing_algorithm: "RSASSA_PKCS1_V1_5_SHA_256".to_string(), // RS256
        }
    }
}

/// KMS client trait for testability
#[async_trait]
pub trait KmsClientTrait: Send + Sync {
    /// Sign data using KMS
    async fn sign(&self, key_id: &str, message: &[u8]) -> Result<Vec<u8>, KmsError>;

    /// Get public key for verification
    async fn get_public_key(&self, key_id: &str) -> Result<Vec<u8>, KmsError>;

    /// List available keys
    async fn list_keys(&self) -> Result<Vec<String>, KmsError>;

    /// Check if key exists
    async fn key_exists(&self, key_id: &str) -> Result<bool, KmsError>;
}

/// AWS KMS client
pub struct KmsClient {
    client: RusotoKmsClient,
    config: KmsConfig,
}

impl KmsClient {
    /// Create a new KMS client
    pub fn new(config: KmsConfig) -> Self {
        let client = RusotoKmsClient::new(config.region.clone());
        Self { client, config }
    }

    /// Get the current signing key ID
    pub fn current_key_id(&self) -> &str {
        &self.config.jwt_signing_key_id
    }

    /// Get the previous signing key ID (for rotation)
    pub fn previous_key_id(&self) -> Option<&str> {
        self.config.jwt_signing_key_id_previous.as_deref()
    }

    /// Get all valid key IDs (current + previous during rotation)
    pub fn valid_key_ids(&self) -> Vec<&str> {
        let mut keys = vec![self.current_key_id()];
        if let Some(prev) = self.previous_key_id() {
            keys.push(prev);
        }
        keys
    }
}

#[async_trait]
impl KmsClientTrait for KmsClient {
    async fn sign(&self, key_id: &str, message: &[u8]) -> Result<Vec<u8>, KmsError> {
        let request = SignRequest {
            key_id: key_id.to_string(),
            message: message.to_vec().into(),
            message_type: Some("RAW".to_string()),
            signing_algorithm: self.config.signing_algorithm.clone(),
            ..Default::default()
        };

        match self.client.sign(request).await {
            Ok(response) => {
                if let Some(signature) = response.signature {
                    Ok(signature.to_vec())
                } else {
                    Err(KmsError::InvalidSignature)
                }
            }
            Err(e) => Err(KmsError::AwsKms(format!("{:?}", e))),
        }
    }

    async fn get_public_key(&self, key_id: &str) -> Result<Vec<u8>, KmsError> {
        use rusoto_kms::GetPublicKeyRequest;

        let request = GetPublicKeyRequest {
            key_id: key_id.to_string(),
            ..Default::default()
        };

        match self.client.get_public_key(request).await {
            Ok(response) => {
                if let Some(public_key) = response.public_key {
                    Ok(public_key.to_vec())
                } else {
                    Err(KmsError::KeyNotFound(key_id.to_string()))
                }
            }
            Err(e) => Err(KmsError::AwsKms(format!("{:?}", e))),
        }
    }

    async fn list_keys(&self) -> Result<Vec<String>, KmsError> {
        use rusoto_kms::ListKeysRequest;

        let request = ListKeysRequest {
            limit: Some(100),
            ..Default::default()
        };

        match self.client.list_keys(request).await {
            Ok(response) => {
                if let Some(keys) = response.keys {
                    Ok(keys.into_iter().filter_map(|k| k.key_id).collect())
                } else {
                    Ok(vec![])
                }
            }
            Err(e) => Err(KmsError::AwsKms(format!("{:?}", e))),
        }
    }

    async fn key_exists(&self, key_id: &str) -> Result<bool, KmsError> {
        use rusoto_kms::DescribeKeyRequest;

        let request = DescribeKeyRequest {
            key_id: key_id.to_string(),
            ..Default::default()
        };

        match self.client.describe_key(request).await {
            Ok(_) => Ok(true),
            Err(e) => {
                let error_msg = format!("{:?}", e);
                if error_msg.contains("NotFoundException") {
                    Ok(false)
                } else {
                    Err(KmsError::AwsKms(error_msg))
                }
            }
        }
    }
}

/// Mock KMS client for testing
#[cfg(test)]
pub struct MockKmsClient {
    pub sign_responses: std::sync::Arc<std::sync::Mutex<Vec<Result<Vec<u8>, KmsError>>>>,
}

#[cfg(test)]
impl MockKmsClient {
    pub fn new() -> Self {
        Self {
            sign_responses: std::sync::Arc::new(std::sync::Mutex::new(vec![
                Ok(vec![1, 2, 3, 4]), // Default mock signature
            ])),
        }
    }

    pub fn with_response(self, response: Result<Vec<u8>, KmsError>) -> Self {
        self.sign_responses.lock().unwrap().clear();
        self.sign_responses.lock().unwrap().push(response);
        self
    }
}

#[cfg(test)]
#[async_trait]
impl KmsClientTrait for MockKmsClient {
    async fn sign(&self, _key_id: &str, _message: &[u8]) -> Result<Vec<u8>, KmsError> {
        let mut responses = self.sign_responses.lock().unwrap();
        if responses.is_empty() {
            Ok(vec![1, 2, 3, 4])
        } else {
            responses.remove(0)
        }
    }

    async fn get_public_key(&self, _key_id: &str) -> Result<Vec<u8>, KmsError> {
        Ok(vec![0; 32]) // Mock public key
    }

    async fn list_keys(&self) -> Result<Vec<String>, KmsError> {
        Ok(vec!["test-key-1".to_string(), "test-key-2".to_string()])
    }

    async fn key_exists(&self, key_id: &str) -> Result<bool, KmsError> {
        Ok(key_id.starts_with("test-key"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_kms_config_default() {
        let config = KmsConfig::default();
        assert_eq!(config.signing_algorithm, "RSASSA_PKCS1_V1_5_SHA_256");
    }

    #[tokio::test]
    async fn test_kms_client_key_ids() {
        let config = KmsConfig {
            region: Region::UsEast1,
            jwt_signing_key_id: "key-1".to_string(),
            jwt_signing_key_id_previous: Some("key-0".to_string()),
            signing_algorithm: "RSASSA_PKCS1_V1_5_SHA_256".to_string(),
        };

        let client = KmsClient::new(config);
        assert_eq!(client.current_key_id(), "key-1");
        assert_eq!(client.previous_key_id(), Some("key-0"));
        assert_eq!(client.valid_key_ids(), vec!["key-1", "key-0"]);
    }

    #[tokio::test]
    async fn test_mock_kms_client_sign() {
        let client = MockKmsClient::new();
        let signature = client.sign("test-key", b"message").await.unwrap();
        assert!(!signature.is_empty());
    }

    #[tokio::test]
    async fn test_mock_kms_client_get_public_key() {
        let client = MockKmsClient::new();
        let public_key = client.get_public_key("test-key").await.unwrap();
        assert_eq!(public_key.len(), 32);
    }

    #[tokio::test]
    async fn test_mock_kms_client_list_keys() {
        let client = MockKmsClient::new();
        let keys = client.list_keys().await.unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[tokio::test]
    async fn test_mock_kms_client_key_exists() {
        let client = MockKmsClient::new();
        assert!(client.key_exists("test-key-1").await.unwrap());
        assert!(!client.key_exists("invalid-key").await.unwrap());
    }
}
