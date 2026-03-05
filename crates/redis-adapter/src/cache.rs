//! Redis Cache Implementation
//!
//! Provides high-performance caching for market data, sessions, and rate limiting.

use crate::connection::RedisPool;
use redis::{AsyncCommands, RedisError};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, warn};

/// Redis cache for market data and sessions
pub struct RedisCache {
    pool: RedisPool,
}

impl RedisCache {
    /// Create a new Redis cache
    pub fn new(pool: RedisPool) -> Self {
        Self { pool }
    }

    /// Get a value from cache
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>, RedisError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let mut conn = self.pool.get_connection().await?;
        let value: Option<String> = conn.get(key).await?;
        
        match value {
            Some(v) => {
                match serde_json::from_str(&v) {
                    Ok(data) => Ok(Some(data)),
                    Err(e) => {
                        warn!("Failed to deserialize cache value for key {}: {}", key, e);
                        Ok(None)
                    }
                }
            }
            None => Ok(None),
        }
    }

    /// Set a value in cache with optional TTL
    pub async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<(), RedisError>
    where
        T: Serialize,
    {
        let mut conn = self.pool.get_connection().await?;
        let serialized = serde_json::to_string(value)
            .map_err(|e| RedisError::from((redis::ErrorKind::TypeError, "Serialization failed", e.to_string())))?;
        
        if let Some(ttl) = ttl {
            conn.set_ex(key, serialized, ttl.as_secs()).await?;
        } else {
            conn.set(key, serialized).await?;
        }
        
        Ok(())
    }

    /// Delete a key from cache
    pub async fn delete(&self, key: &str) -> Result<bool, RedisError> {
        let mut conn = self.pool.get_connection().await?;
        let deleted: i32 = conn.del(key).await?;
        Ok(deleted > 0)
    }

    /// Check if a key exists
    pub async fn exists(&self, key: &str) -> Result<bool, RedisError> {
        let mut conn = self.pool.get_connection().await?;
        conn.exists(key).await
    }

    /// Set expiration on a key
    pub async fn expire(&self, key: &str, ttl: Duration) -> Result<bool, RedisError> {
        let mut conn = self.pool.get_connection().await?;
        conn.expire(key, ttl.as_secs() as i64).await
    }

    /// Get remaining TTL for a key
    pub async fn ttl(&self, key: &str) -> Result<Option<Duration>, RedisError> {
        let mut conn = self.pool.get_connection().await?;
        let ttl: i64 = conn.ttl(key).await?;
        
        match ttl {
            -2 => Ok(None), // Key doesn't exist
            -1 => Ok(None), // No expiration set
            seconds => Ok(Some(Duration::from_secs(seconds as u64))),
        }
    }

    /// Increment a counter
    pub async fn incr(&self, key: &str) -> Result<i64, RedisError> {
        let mut conn = self.pool.get_connection().await?;
        conn.incr(key, 1).await
    }

    /// Increment a counter with TTL (for rate limiting)
    pub async fn incr_with_ttl(&self, key: &str, ttl: Duration) -> Result<i64, RedisError> {
        let mut conn = self.pool.get_connection().await?;
        
        // Use pipeline for atomic operation
        let (count, _): (i64, ()) = redis::pipe()
            .atomic()
            .incr(key, 1)
            .expire(key, ttl.as_secs() as i64)
            .query_async(&mut conn)
            .await?;
        
        Ok(count)
    }

    /// Get multiple keys at once
    pub async fn mget<T>(&self, keys: &[&str]) -> Result<Vec<Option<T>>, RedisError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let mut conn = self.pool.get_connection().await?;
        let values: Vec<Option<String>> = conn.get(keys).await?;
        
        let results = values
            .into_iter()
            .map(|v| {
                v.and_then(|s| serde_json::from_str(&s).ok())
            })
            .collect();
        
        Ok(results)
    }

    /// Set multiple keys at once
    pub async fn mset<T>(&self, items: &[(&str, &T)]) -> Result<(), RedisError>
    where
        T: Serialize,
    {
        let mut conn = self.pool.get_connection().await?;
        let mut pipe = redis::pipe();
        
        for (key, value) in items {
            let serialized = serde_json::to_string(value)
                .map_err(|e| RedisError::from((redis::ErrorKind::TypeError, "Serialization failed", e.to_string())))?;
            pipe.set(*key, serialized);
        }
        
        pipe.query_async(&mut conn).await?;
        Ok(())
    }

    /// Flush all keys (use with caution!)
    pub async fn flush_all(&self) -> Result<(), RedisError> {
        let mut conn = self.pool.get_connection().await?;
        redis::cmd("FLUSHALL").query_async(&mut conn).await
    }
}

/// Market data cache operations
impl RedisCache {
    /// Cache an orderbook snapshot
    pub async fn cache_orderbook<T>(&self, symbol: &str, orderbook: &T, ttl: Duration) -> Result<(), RedisError>
    where
        T: Serialize,
    {
        let key = format!("orderbook:{}", symbol);
        self.set(&key, orderbook, Some(ttl)).await
    }

    /// Get cached orderbook
    pub async fn get_orderbook<T>(&self, symbol: &str) -> Result<Option<T>, RedisError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let key = format!("orderbook:{}", symbol);
        self.get(&key).await
    }

    /// Cache recent trades
    pub async fn cache_trades<T>(&self, symbol: &str, trades: &[T], ttl: Duration) -> Result<(), RedisError>
    where
        T: Serialize,
    {
        let key = format!("trades:{}", symbol);
        self.set(&key, &trades, Some(ttl)).await
    }

    /// Get cached trades
    pub async fn get_trades<T>(&self, symbol: &str) -> Result<Option<Vec<T>>, RedisError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let key = format!("trades:{}", symbol);
        self.get(&key).await
    }

    /// Cache market price
    pub async fn cache_price(&self, symbol: &str, price: &str, ttl: Duration) -> Result<(), RedisError> {
        let key = format!("price:{}", symbol);
        let mut conn = self.pool.get_connection().await?;
        conn.set_ex(key, price, ttl.as_secs()).await
    }

    /// Get cached price
    pub async fn get_price(&self, symbol: &str) -> Result<Option<String>, RedisError> {
        let key = format!("price:{}", symbol);
        let mut conn = self.pool.get_connection().await?;
        conn.get(key).await
    }
}

/// Session storage operations
impl RedisCache {
    /// Store a session
    pub async fn store_session<T>(&self, session_id: &str, session: &T, ttl: Duration) -> Result<(), RedisError>
    where
        T: Serialize,
    {
        let key = format!("session:{}", session_id);
        self.set(&key, session, Some(ttl)).await
    }

    /// Get a session
    pub async fn get_session<T>(&self, session_id: &str) -> Result<Option<T>, RedisError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let key = format!("session:{}", session_id);
        self.get(&key).await
    }

    /// Delete a session
    pub async fn delete_session(&self, session_id: &str) -> Result<bool, RedisError> {
        let key = format!("session:{}", session_id);
        self.delete(&key).await
    }

    /// Refresh session TTL
    pub async fn refresh_session(&self, session_id: &str, ttl: Duration) -> Result<bool, RedisError> {
        let key = format!("session:{}", session_id);
        self.expire(&key, ttl).await
    }
}

/// Rate limiting operations
impl RedisCache {
    /// Check rate limit (returns current count)
    pub async fn check_rate_limit(
        &self,
        key: &str,
        limit: u64,
        window: Duration,
    ) -> Result<RateLimitResult, RedisError> {
        let rate_key = format!("rate:{}", key);
        let count = self.incr_with_ttl(&rate_key, window).await?;
        
        Ok(RateLimitResult {
            allowed: count <= limit as i64,
            current: count as u64,
            limit,
            reset_in: window,
        })
    }

    /// Reset rate limit for a key
    pub async fn reset_rate_limit(&self, key: &str) -> Result<bool, RedisError> {
        let rate_key = format!("rate:{}", key);
        self.delete(&rate_key).await
    }
}

/// Rate limit check result
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,
    /// Current count
    pub current: u64,
    /// Rate limit
    pub limit: u64,
    /// Time until reset
    pub reset_in: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::RedisConfig;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestData {
        value: String,
        count: u32,
    }

    async fn get_test_cache() -> RedisCache {
        let config = RedisConfig::default();
        let pool = RedisPool::new(config).await.unwrap();
        RedisCache::new(pool)
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_set_and_get() {
        let cache = get_test_cache().await;
        
        let data = TestData {
            value: "test".to_string(),
            count: 42,
        };
        
        cache.set("test_key", &data, None).await.unwrap();
        let retrieved: Option<TestData> = cache.get("test_key").await.unwrap();
        
        assert_eq!(retrieved, Some(data));
        
        cache.delete("test_key").await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_set_with_ttl() {
        let cache = get_test_cache().await;
        
        let data = TestData {
            value: "expires".to_string(),
            count: 1,
        };
        
        cache.set("ttl_key", &data, Some(Duration::from_secs(2))).await.unwrap();
        
        let ttl = cache.ttl("ttl_key").await.unwrap();
        assert!(ttl.is_some());
        assert!(ttl.unwrap().as_secs() <= 2);
        
        cache.delete("ttl_key").await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_orderbook_cache() {
        let cache = get_test_cache().await;
        
        let orderbook = vec![
            ("50000.00".to_string(), "1.5".to_string()),
            ("49999.00".to_string(), "2.0".to_string()),
        ];
        
        cache.cache_orderbook("BTC/USD", &orderbook, Duration::from_secs(60)).await.unwrap();
        
        let retrieved: Option<Vec<(String, String)>> = cache.get_orderbook("BTC/USD").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().len(), 2);
        
        cache.delete("orderbook:BTC/USD").await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_session_storage() {
        let cache = get_test_cache().await;
        
        let session = TestData {
            value: "user_session".to_string(),
            count: 123,
        };
        
        cache.store_session("session_123", &session, Duration::from_secs(3600)).await.unwrap();
        
        let retrieved: Option<TestData> = cache.get_session("session_123").await.unwrap();
        assert_eq!(retrieved, Some(session));
        
        cache.delete_session("session_123").await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_rate_limiting() {
        let cache = get_test_cache().await;
        
        let limit = 5;
        let window = Duration::from_secs(10);
        
        // First 5 requests should be allowed
        for i in 1..=5 {
            let result = cache.check_rate_limit("test_user", limit, window).await.unwrap();
            assert!(result.allowed, "Request {} should be allowed", i);
            assert_eq!(result.current, i);
        }
        
        // 6th request should be denied
        let result = cache.check_rate_limit("test_user", limit, window).await.unwrap();
        assert!(!result.allowed);
        assert_eq!(result.current, 6);
        
        cache.reset_rate_limit("test_user").await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_incr() {
        let cache = get_test_cache().await;
        
        let count1 = cache.incr("counter").await.unwrap();
        assert_eq!(count1, 1);
        
        let count2 = cache.incr("counter").await.unwrap();
        assert_eq!(count2, 2);
        
        cache.delete("counter").await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_mget_mset() {
        let cache = get_test_cache().await;
        
        let data1 = TestData { value: "first".to_string(), count: 1 };
        let data2 = TestData { value: "second".to_string(), count: 2 };
        
        cache.mset(&[("key1", &data1), ("key2", &data2)]).await.unwrap();
        
        let results: Vec<Option<TestData>> = cache.mget(&["key1", "key2"]).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], Some(data1));
        assert_eq!(results[1], Some(data2));
        
        cache.delete("key1").await.unwrap();
        cache.delete("key2").await.unwrap();
    }
}
