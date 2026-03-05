//! Challenge generation and storage
//!
//! WebAuthn challenges must be cryptographically random and single-use

use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::WebAuthnError;
use crate::domain::UserId;

/// Challenge data stored during registration/authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredChallenge {
    /// Challenge bytes (32 bytes of random data)
    pub challenge: Vec<u8>,
    /// User ID associated with this challenge
    pub user_id: UserId,
    /// Challenge creation time
    pub created_at: DateTime<Utc>,
    /// Challenge expiration time
    pub expires_at: DateTime<Utc>,
    /// Challenge type (registration or authentication)
    pub challenge_type: ChallengeType,
}

/// Type of challenge
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChallengeType {
    /// Registration ceremony
    Registration,
    /// Authentication ceremony
    Authentication,
}

/// Challenge generator
pub struct ChallengeGenerator {
    /// Challenge TTL (default: 5 minutes)
    ttl: Duration,
}

impl ChallengeGenerator {
    /// Create a new challenge generator
    pub fn new(ttl_seconds: i64) -> Self {
        Self {
            ttl: Duration::seconds(ttl_seconds),
        }
    }

    /// Generate a new challenge
    pub fn generate(
        &self,
        user_id: UserId,
        challenge_type: ChallengeType,
    ) -> StoredChallenge {
        let mut challenge = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut challenge);

        let now = Utc::now();
        StoredChallenge {
            challenge,
            user_id,
            created_at: now,
            expires_at: now + self.ttl,
            challenge_type,
        }
    }
}

impl Default for ChallengeGenerator {
    fn default() -> Self {
        Self::new(300) // 5 minutes
    }
}

/// Challenge storage trait
#[async_trait::async_trait]
pub trait ChallengeStore: Send + Sync {
    /// Store a challenge
    async fn store(&self, challenge: StoredChallenge) -> Result<String, WebAuthnError>;

    /// Retrieve and remove a challenge (single-use)
    async fn consume(&self, challenge_id: &str) -> Result<StoredChallenge, WebAuthnError>;

    /// Check if a challenge exists (without consuming)
    async fn exists(&self, challenge_id: &str) -> Result<bool, WebAuthnError>;

    /// Cleanup expired challenges
    async fn cleanup_expired(&self) -> Result<usize, WebAuthnError>;
}

/// In-memory challenge store (for development/testing)
pub struct InMemoryChallengeStore {
    challenges: Arc<DashMap<String, StoredChallenge>>,
}

impl InMemoryChallengeStore {
    /// Create a new in-memory challenge store
    pub fn new() -> Self {
        Self {
            challenges: Arc::new(DashMap::new()),
        }
    }

    /// Generate a challenge ID
    fn generate_id() -> String {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;

        let mut id = vec![0u8; 16];
        rand::thread_rng().fill_bytes(&mut id);
        URL_SAFE_NO_PAD.encode(&id)
    }
}

impl Default for InMemoryChallengeStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ChallengeStore for InMemoryChallengeStore {
    async fn store(&self, challenge: StoredChallenge) -> Result<String, WebAuthnError> {
        let id = Self::generate_id();
        self.challenges.insert(id.clone(), challenge);
        Ok(id)
    }

    async fn consume(&self, challenge_id: &str) -> Result<StoredChallenge, WebAuthnError> {
        let challenge = self
            .challenges
            .remove(challenge_id)
            .ok_or(WebAuthnError::ChallengeNotFound)?
            .1;

        // Check expiration
        if Utc::now() > challenge.expires_at {
            return Err(WebAuthnError::ChallengeExpired);
        }

        Ok(challenge)
    }

    async fn exists(&self, challenge_id: &str) -> Result<bool, WebAuthnError> {
        Ok(self.challenges.contains_key(challenge_id))
    }

    async fn cleanup_expired(&self) -> Result<usize, WebAuthnError> {
        let now = Utc::now();
        let mut removed = 0;

        self.challenges.retain(|_, challenge| {
            if challenge.expires_at <= now {
                removed += 1;
                false
            } else {
                true
            }
        });

        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_challenge_generation() {
        let generator = ChallengeGenerator::default();
        let user_id = UserId::new();

        let challenge = generator.generate(user_id, ChallengeType::Registration);

        assert_eq!(challenge.challenge.len(), 32);
        assert_eq!(challenge.user_id, user_id);
        assert_eq!(challenge.challenge_type, ChallengeType::Registration);
        assert!(challenge.expires_at > challenge.created_at);
    }

    #[test]
    fn test_challenge_uniqueness() {
        let generator = ChallengeGenerator::default();
        let user_id = UserId::new();

        let challenge1 = generator.generate(user_id, ChallengeType::Registration);
        let challenge2 = generator.generate(user_id, ChallengeType::Registration);

        assert_ne!(challenge1.challenge, challenge2.challenge);
    }

    #[tokio::test]
    async fn test_in_memory_store() {
        let store = InMemoryChallengeStore::new();
        let generator = ChallengeGenerator::default();
        let user_id = UserId::new();

        let challenge = generator.generate(user_id, ChallengeType::Registration);
        let id = store.store(challenge.clone()).await.unwrap();

        assert!(store.exists(&id).await.unwrap());

        let retrieved = store.consume(&id).await.unwrap();
        assert_eq!(retrieved.challenge, challenge.challenge);
        assert_eq!(retrieved.user_id, challenge.user_id);

        // Should be consumed (removed)
        assert!(!store.exists(&id).await.unwrap());
    }

    #[tokio::test]
    async fn test_expired_challenge() {
        let store = InMemoryChallengeStore::new();
        let generator = ChallengeGenerator::new(-10); // Expired 10 seconds ago
        let user_id = UserId::new();

        let challenge = generator.generate(user_id, ChallengeType::Registration);
        let id = store.store(challenge).await.unwrap();

        let result = store.consume(&id).await;
        assert!(matches!(result, Err(WebAuthnError::ChallengeExpired)));
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let store = InMemoryChallengeStore::new();
        let user_id = UserId::new();

        // Create expired challenge
        let expired_gen = ChallengeGenerator::new(-10);
        let expired = expired_gen.generate(user_id, ChallengeType::Registration);
        store.store(expired).await.unwrap();

        // Create valid challenge
        let valid_gen = ChallengeGenerator::new(300);
        let valid = valid_gen.generate(user_id, ChallengeType::Authentication);
        let valid_id = store.store(valid).await.unwrap();

        // Cleanup should remove 1 expired
        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 1);

        // Valid challenge should still exist
        assert!(store.exists(&valid_id).await.unwrap());
    }
}
