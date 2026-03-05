use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use super::{KmsClientTrait, KmsError};

/// Key rotation policy
#[derive(Debug, Clone)]
pub struct KeyRotationPolicy {
    /// How often to rotate keys (in days)
    pub rotation_interval_days: i64,
    /// Grace period for old keys (in days)
    pub grace_period_days: i64,
    /// Auto-rotate enabled
    pub auto_rotate: bool,
}

impl Default for KeyRotationPolicy {
    fn default() -> Self {
        Self {
            rotation_interval_days: 90,  // Rotate every 90 days
            grace_period_days: 7,        // 7 day overlap for in-flight tokens
            auto_rotate: false,          // Manual rotation by default
        }
    }
}

/// Key metadata
#[derive(Debug, Clone)]
pub struct KeyMetadata {
    /// Key ID
    pub key_id: String,
    /// Key version
    pub version: u32,
    /// When the key was created
    pub created_at: DateTime<Utc>,
    /// When the key should be rotated
    pub rotate_at: DateTime<Utc>,
    /// When the key expires (no longer valid for verification)
    pub expires_at: DateTime<Utc>,
    /// Whether this is the active key
    pub is_active: bool,
    /// Algorithm (RS256, ES256, etc.)
    pub algorithm: String,
}

impl KeyMetadata {
    /// Check if key should be rotated
    pub fn should_rotate(&self) -> bool {
        Utc::now() >= self.rotate_at
    }

    /// Check if key is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    /// Check if key is still valid for verification
    pub fn is_valid_for_verification(&self) -> bool {
        !self.is_expired()
    }
}

/// Key manager for handling key rotation and lifecycle
pub struct KeyManager {
    kms_client: Arc<dyn KmsClientTrait>,
    keys: Arc<RwLock<HashMap<String, KeyMetadata>>>,
    active_key_id: Arc<RwLock<String>>,
    policy: KeyRotationPolicy,
}

impl KeyManager {
    /// Create a new key manager
    pub fn new(
        kms_client: Arc<dyn KmsClientTrait>,
        initial_key_id: String,
        policy: KeyRotationPolicy,
    ) -> Self {
        let mut keys = HashMap::new();
        
        let now = Utc::now();
        let metadata = KeyMetadata {
            key_id: initial_key_id.clone(),
            version: 1,
            created_at: now,
            rotate_at: now + Duration::days(policy.rotation_interval_days),
            expires_at: now
                + Duration::days(policy.rotation_interval_days + policy.grace_period_days),
            is_active: true,
            algorithm: "RS256".to_string(),
        };
        
        keys.insert(initial_key_id.clone(), metadata);

        Self {
            kms_client,
            keys: Arc::new(RwLock::new(keys)),
            active_key_id: Arc::new(RwLock::new(initial_key_id)),
            policy,
        }
    }

    /// Get the active key ID for signing
    pub async fn get_active_key_id(&self) -> String {
        self.active_key_id.read().await.clone()
    }

    /// Get key metadata
    pub async fn get_key_metadata(&self, key_id: &str) -> Option<KeyMetadata> {
        self.keys.read().await.get(key_id).cloned()
    }

    /// Get all valid keys (for verification)
    pub async fn get_valid_keys(&self) -> Vec<KeyMetadata> {
        self.keys
            .read()
            .await
            .values()
            .filter(|k| k.is_valid_for_verification())
            .cloned()
            .collect()
    }

    /// Check if a key is valid for verification
    pub async fn is_valid_key(&self, key_id: &str) -> bool {
        if let Some(meta) = self.get_key_metadata(key_id).await {
            meta.is_valid_for_verification()
        } else {
            false
        }
    }

    /// Rotate to a new key
    pub async fn rotate_key(&self, new_key_id: String) -> Result<(), KmsError> {
        info!("Starting key rotation to key: {}", new_key_id);

        // Verify the new key exists in KMS
        if !self.kms_client.key_exists(&new_key_id).await? {
            error!("New key {} not found in KMS", new_key_id);
            return Err(KmsError::KeyNotFound(new_key_id));
        }

        let now = Utc::now();
        let mut keys = self.keys.write().await;
        
        // Deactivate current active key
        let current_key_id = self.active_key_id.read().await.clone();
        if let Some(current) = keys.get_mut(&current_key_id) {
            current.is_active = false;
            info!("Deactivated old key: {}", current_key_id);
        }

        // Get next version number
        let next_version = keys.values().map(|k| k.version).max().unwrap_or(0) + 1;

        // Add new key metadata
        let new_metadata = KeyMetadata {
            key_id: new_key_id.clone(),
            version: next_version,
            created_at: now,
            rotate_at: now + Duration::days(self.policy.rotation_interval_days),
            expires_at: now
                + Duration::days(
                    self.policy.rotation_interval_days + self.policy.grace_period_days,
                ),
            is_active: true,
            algorithm: "RS256".to_string(),
        };

        keys.insert(new_key_id.clone(), new_metadata);

        // Update active key
        *self.active_key_id.write().await = new_key_id.clone();

        info!(
            "Key rotation complete. New active key: {} (version {})",
            new_key_id, next_version
        );

        Ok(())
    }

    /// background task for auto-rotation
    pub async fn check_rotation(&self) -> Result<bool, KmsError> {
        let active_key_id = self.get_active_key_id().await;
        
        if let Some(meta) = self.get_key_metadata(&active_key_id).await {
            if meta.should_rotate() {
                warn!(
                    "Active key {} should be rotated (created at {})",
                    active_key_id, meta.created_at
                );
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Cleanup expired keys
    pub async fn cleanup_expired_keys(&self) -> usize {
        let mut keys = self.keys.write().await;
        let initial_count = keys.len();
        
        keys.retain(|key_id, meta| {
            if meta.is_expired() {
                info!("Removing expired key: {} (created at {})", key_id, meta.created_at);
                false
            } else {
                true
            }
        });

        initial_count - keys.len()
    }

    /// Get key rotation statistics
    pub async fn get_stats(&self) -> KeyRotationStats {
        let keys = self.keys.read().await;
        let active_key_id = self.active_key_id.read().await.clone();
        
        let total_keys = keys.len();
        let valid_keys = keys.values().filter(|k| k.is_valid_for_verification()).count();
        let expired_keys = keys.values().filter(|k| k.is_expired()).count();
        
        let active_key_age = keys.get(&active_key_id).map(|k| {
            (Utc::now() - k.created_at).num_days()
        });

        KeyRotationStats {
            total_keys,
            valid_keys,
            expired_keys,
            active_key_id,
            active_key_age_days: active_key_age,
        }
    }
}

/// Key rotation statistics
#[derive(Debug, Clone)]
pub struct KeyRotationStats {
    pub total_keys: usize,
    pub valid_keys: usize,
    pub expired_keys: usize,
    pub active_key_id: String,
    pub active_key_age_days: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::kms_client::MockKmsClient;

    #[tokio::test]
    async fn test_key_manager_creation() {
        let client = Arc::new(MockKmsClient::new());
        let policy = KeyRotationPolicy::default();
        let manager = KeyManager::new(client, "test-key-1".to_string(), policy);

        assert_eq!(manager.get_active_key_id().await, "test-key-1");
    }

    #[tokio::test]
    async fn test_key_rotation() {
        let client = Arc::new(MockKmsClient::new());
        let policy = KeyRotationPolicy::default();
        let manager = KeyManager::new(client, "test-key-1".to_string(), policy);

        assert_eq!(manager.get_active_key_id().await, "test-key-1");

        // Rotate to new key
        manager.rotate_key("test-key-2".to_string()).await.unwrap();
        assert_eq!(manager.get_active_key_id().await, "test-key-2");

        // Old key should still be valid for verification
        assert!(manager.is_valid_key("test-key-1").await);
        assert!(manager.is_valid_key("test-key-2").await);
    }

    #[tokio::test]
    async fn test_key_metadata() {
        let client = Arc::new(MockKmsClient::new());
        let policy = KeyRotationPolicy::default();
        let manager = KeyManager::new(client, "test-key-1".to_string(), policy);

        let meta = manager.get_key_metadata("test-key-1").await.unwrap();
        assert_eq!(meta.key_id, "test-key-1");
        assert_eq!(meta.version, 1);
        assert!(meta.is_active);
        assert!(!meta.should_rotate());
    }

    #[tokio::test]
    async fn test_get_valid_keys() {
        let client = Arc::new(MockKmsClient::new());
        let policy = KeyRotationPolicy::default();
        let manager = KeyManager::new(client, "test-key-1".to_string(), policy);

        manager.rotate_key("test-key-2".to_string()).await.unwrap();

        let valid_keys = manager.get_valid_keys().await;
        assert_eq!(valid_keys.len(), 2);
    }

    #[tokio::test]
    async fn test_key_rotation_stats() {
        let client = Arc::new(MockKmsClient::new());
        let policy = KeyRotationPolicy::default();
        let manager = KeyManager::new(client, "test-key-1".to_string(), policy);

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_keys, 1);
        assert_eq!(stats.valid_keys, 1);
        assert_eq!(stats.expired_keys, 0);
        assert_eq!(stats.active_key_id, "test-key-1");
    }
}
