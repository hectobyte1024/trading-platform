/// Token revocation management
/// 
/// Provides persistent storage for revoked tokens (JTI blacklist) to prevent
/// reuse of compromised or invalidated tokens across distributed systems.

mod store;

pub use store::{
    RevocationStore, RevocationError, RevocationReason, 
    RevokedToken, RevocationStats,
};

#[cfg(feature = "redis")]
mod redis_store;

#[cfg(feature = "redis")]
pub use redis_store::RedisRevocationStore;

#[cfg(feature = "postgres")]
mod postgres_store;

#[cfg(feature = "postgres")]
pub use postgres_store::PostgresRevocationStore;
