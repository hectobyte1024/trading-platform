//! Redis-backed token revocation store
//!
//! High-performance distributed revocation storage using Redis.
//! Ideal for production deployments with multiple auth service instances.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json;
use std::collections::HashMap;

use crate::domain::token::TokenId;
use crate::domain::user::UserId;
use crate::redis_adapter::RedisConnection;

use super::store::{
    RevocationError, RevocationReason, RevocationStats, RevocationStore, RevokedToken,
};

/// Redis-backed revocation store
pub struct RedisRevocationStore {
    /// Redis connection pool
    connection: RedisConnection,
    /// Key prefix for revocation entries
    key_prefix: String,
    /// Key prefix for user token sets
    user_key_prefix: String,
}

impl RedisRevocationStore {
    /// Create a new Redis revocation store
    pub fn new(connection: RedisConnection) -> Self {
        Self {
            connection,
            key_prefix: "auth:revoked:".to_string(),
            user_key_prefix: "auth:user_tokens:".to_string(),
        }
    }

    /// Create with custom key prefixes
    pub fn with_prefixes(
        connection: RedisConnection,
        key_prefix: String,
        user_key_prefix: String,
    ) -> Self {
        Self {
            connection,
            key_prefix,
            user_key_prefix,
        }
    }

    /// Get Redis key for a token
    fn token_key(&self, jti: &TokenId) -> String {
        format!("{}{}", self.key_prefix, jti)
    }

    /// Get Redis key for user's token set
    fn user_tokens_key(&self, user_id: &UserId) -> String {
        format!("{}{}", self.user_key_prefix, user_id)
    }

    /// Calculate TTL in seconds until expiration
    fn calculate_ttl(&self, expires_at: DateTime<Utc>) -> i64 {
        let now = Utc::now();
        let duration = expires_at.signed_duration_since(now);
        duration.num_seconds().max(0)
    }
}

#[async_trait]
impl RevocationStore for RedisRevocationStore {
    async fn is_revoked(&self, jti: &TokenId) -> Result<bool, RevocationError> {
        let key = self.token_key(jti);
        
        self.connection
            .exists(&key)
            .await
            .map_err(|e| RevocationError::Storage(e.to_string()))
    }

    async fn revoke_token(
        &self,
        jti: TokenId,
        user_id: UserId,
        reason: RevocationReason,
        expires_at: DateTime<Utc>,
        notes: Option<String>,
    ) -> Result<(), RevocationError> {
        let revoked = RevokedToken::new(jti, user_id, reason, expires_at, notes);
        
        let key = self.token_key(&jti);
        let user_key = self.user_tokens_key(&user_id);
        let ttl = self.calculate_ttl(expires_at);

        // Serialize token data
        let value = serde_json::to_string(&revoked)
            .map_err(|e| RevocationError::Serialization(e.to_string()))?;

        // Store revocation with TTL
        self.connection
            .set_with_expiry(&key, &value, ttl)
            .await
            .map_err(|e| RevocationError::Storage(e.to_string()))?;

        // Add to user's token set
        self.connection
            .sadd(&user_key, &jti.to_string())
            .await
            .map_err(|e| RevocationError::Storage(e.to_string()))?;

        // Set TTL on user's token set (cleanup after all tokens expire)
        self.connection
            .expire(&user_key, ttl + 86400) // +24h buffer
            .await
            .map_err(|e| RevocationError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn revoke_all_user_tokens(
        &self,
        user_id: UserId,
        reason: RevocationReason,
        notes: Option<String>,
    ) -> Result<usize, RevocationError> {
        let user_key = self.user_tokens_key(&user_id);

        // Get all token IDs for this user
        let token_ids: Vec<String> = self
            .connection
            .smembers(&user_key)
            .await
            .map_err(|e| RevocationError::Storage(e.to_string()))?;

        let mut revoked_count = 0;

        // Revoke each token
        // Note: In production, this should use a pipeline for efficiency
        for token_id_str in token_ids {
            if let Ok(jti) = token_id_str.parse::<TokenId>() {
                // Get existing revocation to preserve expires_at
                if let Ok(Some(existing)) = self.get_revocation(&jti).await {
                    self.revoke_token(
                        jti,
                        user_id,
                        reason,
                        existing.expires_at,
                        notes.clone(),
                    )
                    .await?;
                    revoked_count += 1;
                }
            }
        }

        Ok(revoked_count)
    }

    async fn get_revocation(
        &self,
        jti: &TokenId,
    ) -> Result<Option<RevokedToken>, RevocationError> {
        let key = self.token_key(jti);

        let value: Option<String> = self
            .connection
            .get(&key)
            .await
            .map_err(|e| RevocationError::Storage(e.to_string()))?;

        match value {
            Some(json) => {
                let revoked: RevokedToken = serde_json::from_str(&json)
                    .map_err(|e| RevocationError::Serialization(e.to_string()))?;
                Ok(Some(revoked))
            }
            None => Ok(None),
        }
    }

    async fn cleanup_expired(&self) -> Result<usize, RevocationError> {
        // Redis automatically cleans up expired keys via TTL
        // Return 0 as cleanup is handled automatically
        Ok(0)
    }

    async fn stats(&self) -> Result<RevocationStats, RevocationError> {
        // Scan for all revocation keys
        let pattern = format!("{}*", self.key_prefix);
        let keys: Vec<String> = self
            .connection
            .scan(&pattern)
            .await
            .map_err(|e| RevocationError::Storage(e.to_string()))?;

        let total_revoked = keys.len();
        let mut revoked_24h = 0;
        let mut by_reason: HashMap<String, usize> = HashMap::new();

        // Sample tokens to get statistics
        // In production, use Redis analytics or maintain separate counters
        for key in keys.iter().take(100) {
            if let Ok(Some(value)) = self.connection.get::<String>(key).await {
                if let Ok(revoked) = serde_json::from_str::<RevokedToken>(&value) {
                    // Count recent revocations
                    let age = Utc::now().signed_duration_since(revoked.revoked_at);
                    if age.num_hours() < 24 {
                        revoked_24h += 1;
                    }

                    // Count by reason
                    let reason_key = revoked.reason.description().to_string();
                    *by_reason.entry(reason_key).or_insert(0) += 1;
                }
            }
        }

        Ok(RevocationStats {
            total_revoked,
            revoked_24h,
            cleanable: 0, // Redis handles cleanup automatically
            by_reason,
        })
    }

    async fn list_user_revocations(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<RevokedToken>, RevocationError> {
        let user_key = self.user_tokens_key(user_id);

        // Get all token IDs for this user
        let token_ids: Vec<String> = self
            .connection
            .smembers(&user_key)
            .await
            .map_err(|e| RevocationError::Storage(e.to_string()))?;

        let mut revocations = Vec::new();

        for token_id_str in token_ids {
            if let Ok(jti) = token_id_str.parse::<TokenId>() {
                if let Ok(Some(revoked)) = self.get_revocation(&jti).await {
                    revocations.push(revoked);
                }
            }
        }

        Ok(revocations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() {
        let connection = RedisConnection::new("redis://localhost:6379");
        let store = RedisRevocationStore::new(connection);
        
        let jti = TokenId::new();
        let user_id = UserId::new();

        let token_key = store.token_key(&jti);
        assert!(token_key.starts_with("auth:revoked:"));
        assert!(token_key.contains(&jti.to_string()));

        let user_key = store.user_tokens_key(&user_id);
        assert!(user_key.starts_with("auth:user_tokens:"));
        assert!(user_key.contains(&user_id.to_string()));
    }

    #[test]
    fn test_custom_prefixes() {
        let connection = RedisConnection::new("redis://localhost:6379");
        let store = RedisRevocationStore::with_prefixes(
            connection,
            "custom:rev:".to_string(),
            "custom:user:".to_string(),
        );

        let jti = TokenId::new();
        let token_key = store.token_key(&jti);
        assert!(token_key.starts_with("custom:rev:"));
    }

    #[test]
    fn test_ttl_calculation() {
        let connection = RedisConnection::new("redis://localhost:6379");
        let store = RedisRevocationStore::new(connection);

        let now = Utc::now();
        let future = now + chrono::Duration::hours(2);
        
        let ttl = store.calculate_ttl(future);
        assert!(ttl > 7000 && ttl < 7300); // ~2 hours in seconds

        // Past expiration should return 0
        let past = now - chrono::Duration::hours(1);
        let ttl_past = store.calculate_ttl(past);
        assert_eq!(ttl_past, 0);
    }
}
