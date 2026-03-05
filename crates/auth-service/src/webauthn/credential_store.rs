//! WebAuthn credential storage
//!
//! Stores registered authenticators and their public keys

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{AuthenticatorType, WebAuthnError};
use crate::domain::{DeviceId, UserId};

/// Stored credential metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredential {
    /// Credential ID (unique)
    pub credential_id: Vec<u8>,
    /// User ID
    pub user_id: UserId,
    /// Device ID (optional)
    pub device_id: Option<DeviceId>,
    /// Public key (COSE format)
    pub public_key: Vec<u8>,
    /// Signature counter (for clone detection)
    pub sign_count: u32,
    /// Metadata
    pub metadata: CredentialMetadata,
    /// Registration timestamp
    pub created_at: DateTime<Utc>,
    /// Last used timestamp
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Credential metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialMetadata {
    /// Authenticator type
    pub authenticator_type: AuthenticatorType,
    /// AAGUID (Authenticator Attestation GUID)
    pub aaguid: [u8; 16],
    /// Supported transports
    pub transports: Option<Vec<String>>,
    /// Friendly name (user-provided)
    pub name: Option<String>,
    /// Backup eligible flag
    pub backup_eligible: bool,
    /// Backup state flag
    pub backup_state: bool,
}

/// Credential storage trait
#[async_trait::async_trait]
pub trait CredentialStore: Send + Sync {
    /// Store a new credential
    async fn store(&self, credential: StoredCredential) -> Result<(), WebAuthnError>;

    /// Get credential by ID
    async fn get(&self, credential_id: &[u8]) -> Result<StoredCredential, WebAuthnError>;

    /// Get all credentials for a user
    async fn get_user_credentials(&self, user_id: &UserId) -> Result<Vec<StoredCredential>, WebAuthnError>;

    /// Update credential (signature counter, last used)
    async fn update(&self, credential: StoredCredential) -> Result<(), WebAuthnError>;

    /// Delete a credential
    async fn delete(&self, credential_id: &[u8]) -> Result<(), WebAuthnError>;

    /// Delete all credentials for a user
    async fn delete_user_credentials(&self, user_id: &UserId) -> Result<usize, WebAuthnError>;

    /// Check if credential exists
    async fn exists(&self, credential_id: &[u8]) -> Result<bool, WebAuthnError>;
}

/// In-memory credential store (for development/testing)
#[derive(Default)]
pub struct InMemoryCredentialStore {
    credentials: std::sync::Arc<dashmap::DashMap<Vec<u8>, StoredCredential>>,
}

impl InMemoryCredentialStore {
    /// Create a new in-memory credential store
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl CredentialStore for InMemoryCredentialStore {
    async fn store(&self, credential: StoredCredential) -> Result<(), WebAuthnError> {
        if self.credentials.contains_key(&credential.credential_id) {
            return Err(WebAuthnError::CredentialExists(
                hex::encode(&credential.credential_id),
            ));
        }

        self.credentials.insert(credential.credential_id.clone(), credential);
        Ok(())
    }

    async fn get(&self, credential_id: &[u8]) -> Result<StoredCredential, WebAuthnError> {
        self.credentials
            .get(credential_id)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| WebAuthnError::CredentialNotFound(hex::encode(credential_id)))
    }

    async fn get_user_credentials(&self, user_id: &UserId) -> Result<Vec<StoredCredential>, WebAuthnError> {
        let credentials: Vec<_> = self
            .credentials
            .iter()
            .filter(|entry| entry.value().user_id == *user_id)
            .map(|entry| entry.value().clone())
            .collect();

        Ok(credentials)
    }

    async fn update(&self, credential: StoredCredential) -> Result<(), WebAuthnError> {
        if !self.credentials.contains_key(&credential.credential_id) {
            return Err(WebAuthnError::CredentialNotFound(
                hex::encode(&credential.credential_id),
            ));
        }

        self.credentials.insert(credential.credential_id.clone(), credential);
        Ok(())
    }

    async fn delete(&self, credential_id: &[u8]) -> Result<(), WebAuthnError> {
        self.credentials
            .remove(credential_id)
            .ok_or_else(|| WebAuthnError::CredentialNotFound(hex::encode(credential_id)))?;

        Ok(())
    }

    async fn delete_user_credentials(&self, user_id: &UserId) -> Result<usize, WebAuthnError> {
        let to_delete: Vec<_> = self
            .credentials
            .iter()
            .filter(|entry| entry.value().user_id == *user_id)
            .map(|entry| entry.key().clone())
            .collect();

        let count = to_delete.len();
        for credential_id in to_delete {
            self.credentials.remove(&credential_id);
        }

        Ok(count)
    }

    async fn exists(&self, credential_id: &[u8]) -> Result<bool, WebAuthnError> {
        Ok(self.credentials.contains_key(credential_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_credential(user_id: UserId) -> StoredCredential {
        StoredCredential {
            credential_id: vec![1, 2, 3, 4, 5],
            user_id,
            device_id: None,
            public_key: vec![10, 20, 30, 40],
            sign_count: 0,
            metadata: CredentialMetadata {
                authenticator_type: AuthenticatorType::Platform,
                aaguid: [0u8; 16],
                transports: Some(vec!["internal".to_string()]),
                name: Some("TouchID".to_string()),
                backup_eligible: false,
                backup_state: false,
            },
            created_at: Utc::now(),
            last_used_at: None,
        }
    }

    #[tokio::test]
    async fn test_store_and_get() {
        let store = InMemoryCredentialStore::new();
        let user_id = UserId::new();
        let credential = create_test_credential(user_id);

        store.store(credential.clone()).await.unwrap();

        let retrieved = store.get(&credential.credential_id).await.unwrap();
        assert_eq!(retrieved.credential_id, credential.credential_id);
        assert_eq!(retrieved.user_id, credential.user_id);
    }

    #[tokio::test]
    async fn test_duplicate_credential() {
        let store = InMemoryCredentialStore::new();
        let user_id = UserId::new();
        let credential = create_test_credential(user_id);

        store.store(credential.clone()).await.unwrap();

        let result = store.store(credential).await;
        assert!(matches!(result, Err(WebAuthnError::CredentialExists(_))));
    }

    #[tokio::test]
    async fn test_get_user_credentials() {
        let store = InMemoryCredentialStore::new();
        let user_id = UserId::new();

        let mut cred1 = create_test_credential(user_id);
        cred1.credential_id = vec![1];
        let mut cred2 = create_test_credential(user_id);
        cred2.credential_id = vec![2];

        store.store(cred1).await.unwrap();
        store.store(cred2).await.unwrap();

        let credentials = store.get_user_credentials(&user_id).await.unwrap();
        assert_eq!(credentials.len(), 2);
    }

    #[tokio::test]
    async fn test_update_credential() {
        let store = InMemoryCredentialStore::new();
        let user_id = UserId::new();
        let mut credential = create_test_credential(user_id);

        store.store(credential.clone()).await.unwrap();

        credential.sign_count = 5;
        credential.last_used_at = Some(Utc::now());

        store.update(credential.clone()).await.unwrap();

        let retrieved = store.get(&credential.credential_id).await.unwrap();
        assert_eq!(retrieved.sign_count, 5);
        assert!(retrieved.last_used_at.is_some());
    }

    #[tokio::test]
    async fn test_delete_credential() {
        let store = InMemoryCredentialStore::new();
        let user_id = UserId::new();
        let credential = create_test_credential(user_id);

        store.store(credential.clone()).await.unwrap();
        store.delete(&credential.credential_id).await.unwrap();

        let result = store.get(&credential.credential_id).await;
        assert!(matches!(result, Err(WebAuthnError::CredentialNotFound(_))));
    }

    #[tokio::test]
    async fn test_delete_user_credentials() {
        let store = InMemoryCredentialStore::new();
        let user_id = UserId::new();

        let mut cred1 = create_test_credential(user_id);
        cred1.credential_id = vec![1];
        let mut cred2 = create_test_credential(user_id);
        cred2.credential_id = vec![2];

        store.store(cred1).await.unwrap();
        store.store(cred2).await.unwrap();

        let deleted = store.delete_user_credentials(&user_id).await.unwrap();
        assert_eq!(deleted, 2);

        let credentials = store.get_user_credentials(&user_id).await.unwrap();
        assert_eq!(credentials.len(), 0);
    }
}
