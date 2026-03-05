//! Redis Adapter
//!
//! High-performance Redis integration for caching and session storage.
//!
//! ## Features
//!
//! - Connection pooling with automatic reconnection
//! - Market data caching (orderbooks, trades, prices)
//! - Session storage for authentication
//! - Rate limiting support
//! - Generic key-value operations with TTL
//!
//! ## Usage
//!
//! ```rust,no_run
//! use redis_adapter::{RedisPool, RedisConfig, RedisCache};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create connection pool
//!     let config = RedisConfig::new("redis://127.0.0.1:6379");
//!     let pool = RedisPool::new(config).await?;
//!     
//!     // Create cache
//!     let cache = RedisCache::new(pool);
//!     
//!     // Cache some data
//!     cache.set("key", &"value", Some(Duration::from_secs(60))).await?;
//!     
//!     // Retrieve data
//!     let value: Option<String> = cache.get("key").await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod cache;
pub mod connection;

pub use cache::{RedisCache, RateLimitResult};
pub use connection::{RedisConfig, RedisPool};
