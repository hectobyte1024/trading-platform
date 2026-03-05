//! Redis Connection Pool Management
//!
//! Provides connection pooling for high-performance Redis access.

use redis::{aio::ConnectionManager, Client, RedisError};
use std::time::Duration;
use tracing::{debug, info};

/// Redis connection configuration
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Redis URL (redis://host:port or rediss://host:port for TLS)
    pub url: String,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Response timeout
    pub response_timeout: Duration,
    /// Maximum number of retries
    pub max_retries: u32,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://127.0.0.1:6379".to_string(),
            connection_timeout: Duration::from_secs(5),
            response_timeout: Duration::from_secs(3),
            max_retries: 3,
        }
    }
}

impl RedisConfig {
    /// Create a new Redis configuration
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..Default::default()
        }
    }

    /// Set connection timeout
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }

    /// Set response timeout
    pub fn with_response_timeout(mut self, timeout: Duration) -> Self {
        self.response_timeout = timeout;
        self
    }

    /// Set maximum retries
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }
}

/// Redis connection pool
pub struct RedisPool {
    client: Client,
    config: RedisConfig,
}

impl RedisPool {
    /// Create a new Redis connection pool
    pub async fn new(config: RedisConfig) -> Result<Self, RedisError> {
        info!("Connecting to Redis at {}", config.url);
        
        let client = Client::open(config.url.clone())?;
        
        // Test the connection
        let mut conn = client.get_connection_manager().await?;
        redis::cmd("PING").query_async::<_, String>(&mut conn).await?;
        
        debug!("Redis connection established successfully");
        
        Ok(Self { client, config })
    }

    /// Get a connection manager
    pub async fn get_connection(&self) -> Result<ConnectionManager, RedisError> {
        self.client.get_connection_manager().await
    }

    /// Get the configuration
    pub fn config(&self) -> &RedisConfig {
        &self.config
    }

    /// Test the connection
    pub async fn ping(&self) -> Result<(), RedisError> {
        let mut conn = self.get_connection().await?;
        redis::cmd("PING").query_async::<_, String>(&mut conn).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires running Redis instance
    async fn test_redis_connection() {
        let config = RedisConfig::default();
        let pool = RedisPool::new(config).await.unwrap();
        
        pool.ping().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_multiple_connections() {
        let config = RedisConfig::default();
        let pool = RedisPool::new(config).await.unwrap();
        
        let mut conn1 = pool.get_connection().await.unwrap();
        let mut conn2 = pool.get_connection().await.unwrap();
        
        redis::cmd("PING").query_async::<_, String>(&mut conn1).await.unwrap();
        redis::cmd("PING").query_async::<_, String>(&mut conn2).await.unwrap();
    }
}
