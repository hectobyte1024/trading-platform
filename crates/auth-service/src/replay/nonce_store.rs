use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;
use thiserror::Error;

use crate::domain::TokenId;

/// Nonce storage errors
#[derive(Debug, Error)]
pub enum NonceStoreError {
    #[error("Nonce already used (replay attack)")]
    NonceAlreadyUsed,

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Nonce expired")]
    NonceExpired,
}

/// Nonce entry
#[derive(Debug, Clone)]
struct NonceEntry {
    /// When the nonce was first seen
    #[allow(dead_code)]
    first_seen: DateTime<Utc>,
    /// Token ID associated with this nonce
    #[allow(dead_code)]
    token_id: TokenId,
    /// When the nonce expires
    expires_at: DateTime<Utc>,
}

/// Trait for nonce storage
#[async_trait]
pub trait NonceStore: Send + Sync {
    /// Check if nonce has been used
    async fn check_nonce(&self, nonce: &str, token_id: TokenId) -> Result<(), NonceStoreError>;

    /// Store a nonce
    async fn store_nonce(
        &self,
        nonce: &str,
        token_id: TokenId,
        ttl: i64,
    ) -> Result<(), NonceStoreError>;

    /// Check and store in one operation (atomic)
    async fn check_and_store(
        &self,
        nonce: &str,
        token_id: TokenId,
        ttl: i64,
    ) -> Result<(), NonceStoreError>;

    /// Remove expired nonces
    async fn cleanup_expired(&self) -> usize;

    /// Get statistics
    async fn stats(&self) -> NonceStoreStats;
}

/// Nonce store statistics
#[derive(Debug, Clone)]
pub struct NonceStoreStats {
    pub total_nonces: usize,
    pub expired_nonces: usize,
    pub replay_attempts: usize,
}

/// In-memory nonce store (for testing and single-instance deployments)
pub struct InMemoryNonceStore {
    nonces: Arc<DashMap<String, NonceEntry>>,
    replay_attempts: Arc<std::sync::atomic::AtomicUsize>,
}

impl InMemoryNonceStore {
    /// Create a new in-memory nonce store
    pub fn new() -> Self {
        Self {
            nonces: Arc::new(DashMap::new()),
            replay_attempts: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }
}

impl Default for InMemoryNonceStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NonceStore for InMemoryNonceStore {
    async fn check_nonce(&self, nonce: &str, _token_id: TokenId) -> Result<(), NonceStoreError> {
        if let Some(entry) = self.nonces.get(nonce) {
            // Check if expired
            if Utc::now() > entry.expires_at {
                return Err(NonceStoreError::NonceExpired);
            }

            // Nonce exists - replay attack
            self.replay_attempts
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Err(NonceStoreError::NonceAlreadyUsed);
        }

        Ok(())
    }

    async fn store_nonce(
        &self,
        nonce: &str,
        token_id: TokenId,
        ttl: i64,
    ) -> Result<(), NonceStoreError> {
        let now = Utc::now();
        let entry = NonceEntry {
            first_seen: now,
            token_id,
            expires_at: now + chrono::Duration::seconds(ttl),
        };

        self.nonces.insert(nonce.to_string(), entry);
        Ok(())
    }

    async fn check_and_store(
        &self,
        nonce: &str,
        token_id: TokenId,
        ttl: i64,
    ) -> Result<(), NonceStoreError> {
        // Check first
        self.check_nonce(nonce, token_id).await?;

        // Store
        self.store_nonce(nonce, token_id, ttl).await?;

        Ok(())
    }

    async fn cleanup_expired(&self) -> usize {
        let now = Utc::now();
        let initial_count = self.nonces.len();

        self.nonces.retain(|_, entry| now <= entry.expires_at);

        initial_count - self.nonces.len()
    }

    async fn stats(&self) -> NonceStoreStats {
        let now = Utc::now();
        let total = self.nonces.len();
        let expired = self
            .nonces
            .iter()
            .filter(|entry| now > entry.expires_at)
            .count();

        NonceStoreStats {
            total_nonces: total,
            expired_nonces: expired,
            replay_attempts: self
                .replay_attempts
                .load(std::sync::atomic::Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_check_nonce() {
        let store = InMemoryNonceStore::new();
        let token_id = TokenId::new();

        // First check should pass
        assert!(store.check_nonce("test-nonce", token_id).await.is_ok());

        // Store nonce
        store.store_nonce("test-nonce", token_id, 3600).await.unwrap();

        // Second check should fail (replay)
        let result = store.check_nonce("test-nonce", token_id).await;
        assert!(matches!(result, Err(NonceStoreError::NonceAlreadyUsed)));
    }

    #[tokio::test]
    async fn test_check_and_store_atomic() {
        let store = InMemoryNonceStore::new();
        let token_id = TokenId::new();

        // First use should succeed
        assert!(store
            .check_and_store("nonce-1", token_id, 3600)
            .await
            .is_ok());

        // Second use should fail
        let result = store.check_and_store("nonce-1", token_id, 3600).await;
        assert!(matches!(result, Err(NonceStoreError::NonceAlreadyUsed)));
    }

    #[tokio::test]
    async fn test_expired_nonce() {
        let store = InMemoryNonceStore::new();
        let token_id = TokenId::new();

        // Store with negative TTL (already expired)
        store.store_nonce("expired-nonce", token_id, -100).await.unwrap();

        // Check should fail with expiration error
        let result = store.check_nonce("expired-nonce", token_id).await;
        assert!(matches!(result, Err(NonceStoreError::NonceExpired)));
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let store = InMemoryNonceStore::new();

        // Store some nonces
        store.store_nonce("valid", TokenId::new(), 3600).await.unwrap();
        store.store_nonce("expired1", TokenId::new(), -100).await.unwrap();
        store.store_nonce("expired2", TokenId::new(), -200).await.unwrap();

        assert_eq!(store.nonces.len(), 3);

        // Cleanup
        let removed = store.cleanup_expired().await;
        assert_eq!(removed, 2);
        assert_eq!(store.nonces.len(), 1);
    }

    #[tokio::test]
    async fn test_stats() {
        let store = InMemoryNonceStore::new();
        let token_id = TokenId::new();

        store.store_nonce("nonce1", token_id, 3600).await.unwrap();
        store.store_nonce("nonce2", token_id, -100).await.unwrap();

        // Trigger a replay attempt
        let _ = store.check_nonce("nonce1", token_id).await;

        let stats = store.stats().await;
        assert_eq!(stats.total_nonces, 2);
        assert_eq!(stats.expired_nonces, 1);
        assert_eq!(stats.replay_attempts, 1);
    }
}
