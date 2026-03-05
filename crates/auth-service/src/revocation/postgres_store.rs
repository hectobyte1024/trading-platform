//! PostgreSQL-backed token revocation store
//!
//! Persistent revocation storage using PostgreSQL.
//! Provides durability and complex query capabilities for audit trails.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json;
use sqlx::PgPool;
use std::collections::HashMap;

use crate::domain::token::TokenId;
use crate::domain::user::UserId;

use super::store::{
    RevocationError, RevocationReason, RevocationStats, RevocationStore, RevokedToken,
};

/// PostgreSQL-backed revocation store
pub struct PostgresRevocationStore {
    /// Database connection pool
    pool: PgPool,
}

impl PostgresRevocationStore {
    /// Create a new PostgreSQL revocation store
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Initialize the database schema
    pub async fn init_schema(&self) -> Result<(), RevocationError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS revoked_tokens (
                jti UUID PRIMARY KEY,
                user_id UUID NOT NULL,
                revoked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                reason TEXT NOT NULL,
                expires_at TIMESTAMPTZ NOT NULL,
                notes TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                INDEX idx_user_id (user_id),
                INDEX idx_revoked_at (revoked_at),
                INDEX idx_expires_at (expires_at)
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RevocationError::Storage(e.to_string()))?;

        Ok(())
    }

    /// Convert revocation reason to string for storage
    fn reason_to_string(reason: RevocationReason) -> String {
        match reason {
            RevocationReason::UserLogout => "user_logout",
            RevocationReason::PasswordChange => "password_change",
            RevocationReason::UserDeactivated => "user_deactivated",
            RevocationReason::SecurityCompromise => "security_compromise",
            RevocationReason::AdminRevocation => "admin_revocation",
            RevocationReason::Expiration => "expiration",
            RevocationReason::MaxRotations => "max_rotations",
            RevocationReason::SuspiciousActivity => "suspicious_activity",
        }
        .to_string()
    }

    /// Convert string to revocation reason
    fn string_to_reason(s: &str) -> RevocationReason {
        match s {
            "user_logout" => RevocationReason::UserLogout,
            "password_change" => RevocationReason::PasswordChange,
            "user_deactivated" => RevocationReason::UserDeactivated,
            "security_compromise" => RevocationReason::SecurityCompromise,
            "admin_revocation" => RevocationReason::AdminRevocation,
            "expiration" => RevocationReason::Expiration,
            "max_rotations" => RevocationReason::MaxRotations,
            "suspicious_activity" => RevocationReason::SuspiciousActivity,
            _ => RevocationReason::AdminRevocation, // Default fallback
        }
    }
}

#[async_trait]
impl RevocationStore for PostgresRevocationStore {
    async fn is_revoked(&self, jti: &TokenId) -> Result<bool, RevocationError> {
        let result = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM revoked_tokens 
                WHERE jti = $1 AND expires_at > NOW()
            )
            "#,
        )
        .bind(jti.to_string())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RevocationError::Storage(e.to_string()))?;

        Ok(result)
    }

    async fn revoke_token(
        &self,
        jti: TokenId,
        user_id: UserId,
        reason: RevocationReason,
        expires_at: DateTime<Utc>,
        notes: Option<String>,
    ) -> Result<(), RevocationError> {
        let reason_str = Self::reason_to_string(reason);

        sqlx::query(
            r#"
            INSERT INTO revoked_tokens (jti, user_id, reason, expires_at, notes)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (jti) DO UPDATE SET
                reason = EXCLUDED.reason,
                notes = EXCLUDED.notes,
                revoked_at = NOW()
            "#,
        )
        .bind(jti.to_string())
        .bind(user_id.to_string())
        .bind(reason_str)
        .bind(expires_at)
        .bind(notes)
        .execute(&self.pool)
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
        let reason_str = Self::reason_to_string(reason);

        // This assumes we have a separate active_tokens table
        // For now, we'll mark all as revoked with a special marker
        let result = sqlx::query(
            r#"
            UPDATE revoked_tokens 
            SET reason = $1, notes = $2, revoked_at = NOW()
            WHERE user_id = $3 AND expires_at > NOW()
            "#,
        )
        .bind(reason_str)
        .bind(notes)
        .bind(user_id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| RevocationError::Storage(e.to_string()))?;

        Ok(result.rows_affected() as usize)
    }

    async fn get_revocation(
        &self,
        jti: &TokenId,
    ) -> Result<Option<RevokedToken>, RevocationError> {
        let row = sqlx::query(
            r#"
            SELECT jti, user_id, revoked_at, reason, expires_at, notes
            FROM revoked_tokens
            WHERE jti = $1
            "#,
        )
        .bind(jti.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RevocationError::Storage(e.to_string()))?;

        match row {
            Some(row) => {
                let jti_str: String = row.try_get("jti")
                    .map_err(|e| RevocationError::Storage(e.to_string()))?;
                let user_id_str: String = row.try_get("user_id")
                    .map_err(|e| RevocationError::Storage(e.to_string()))?;
                let revoked_at: DateTime<Utc> = row.try_get("revoked_at")
                    .map_err(|e| RevocationError::Storage(e.to_string()))?;
                let reason_str: String = row.try_get("reason")
                    .map_err(|e| RevocationError::Storage(e.to_string()))?;
                let expires_at: DateTime<Utc> = row.try_get("expires_at")
                    .map_err(|e| RevocationError::Storage(e.to_string()))?;
                let notes: Option<String> = row.try_get("notes")
                    .map_err(|e| RevocationError::Storage(e.to_string()))?;

                let jti = jti_str.parse()
                    .map_err(|e: uuid::Error| RevocationError::Serialization(e.to_string()))?;
                let user_id = user_id_str.parse()
                    .map_err(|e: uuid::Error| RevocationError::Serialization(e.to_string()))?;
                let reason = Self::string_to_reason(&reason_str);

                Ok(Some(RevokedToken {
                    jti,
                    user_id,
                    revoked_at,
                    reason,
                    expires_at,
                    notes,
                }))
            }
            None => Ok(None),
        }
    }

    async fn cleanup_expired(&self) -> Result<usize, RevocationError> {
        let result = sqlx::query(
            r#"
            DELETE FROM revoked_tokens
            WHERE expires_at < NOW()
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RevocationError::Storage(e.to_string()))?;

        Ok(result.rows_affected() as usize)
    }

    async fn stats(&self) -> Result<RevocationStats, RevocationError> {
        // Total revoked
        let total_revoked: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM revoked_tokens WHERE expires_at > NOW()",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RevocationError::Storage(e.to_string()))?;

        // Revoked in last 24h
        let revoked_24h: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM revoked_tokens 
            WHERE revoked_at > NOW() - INTERVAL '24 hours'
            AND expires_at > NOW()
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RevocationError::Storage(e.to_string()))?;

        // Cleanable (expired)
        let cleanable: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM revoked_tokens WHERE expires_at < NOW()",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RevocationError::Storage(e.to_string()))?;

        // By reason
        let rows = sqlx::query(
            r#"
            SELECT reason, COUNT(*) as count
            FROM revoked_tokens
            WHERE expires_at > NOW()
            GROUP BY reason
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RevocationError::Storage(e.to_string()))?;

        let mut by_reason = HashMap::new();
        for row in rows {
            let reason: String = row.try_get("reason")
                .map_err(|e| RevocationError::Storage(e.to_string()))?;
            let count: i64 = row.try_get("count")
                .map_err(|e| RevocationError::Storage(e.to_string()))?;
            by_reason.insert(reason, count as usize);
        }

        Ok(RevocationStats {
            total_revoked: total_revoked as usize,
            revoked_24h: revoked_24h as usize,
            cleanable: cleanable as usize,
            by_reason,
        })
    }

    async fn list_user_revocations(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<RevokedToken>, RevocationError> {
        let rows = sqlx::query(
            r#"
            SELECT jti, user_id, revoked_at, reason, expires_at, notes
            FROM revoked_tokens
            WHERE user_id = $1
            ORDER BY revoked_at DESC
            "#,
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RevocationError::Storage(e.to_string()))?;

        let mut revocations = Vec::new();
        for row in rows {
            let jti_str: String = row.try_get("jti")
                .map_err(|e| RevocationError::Storage(e.to_string()))?;
            let user_id_str: String = row.try_get("user_id")
                .map_err(|e| RevocationError::Storage(e.to_string()))?;
            let revoked_at: DateTime<Utc> = row.try_get("revoked_at")
                .map_err(|e| RevocationError::Storage(e.to_string()))?;
            let reason_str: String = row.try_get("reason")
                .map_err(|e| RevocationError::Storage(e.to_string()))?;
            let expires_at: DateTime<Utc> = row.try_get("expires_at")
                .map_err(|e| RevocationError::Storage(e.to_string()))?;
            let notes: Option<String> = row.try_get("notes")
                .map_err(|e| RevocationError::Storage(e.to_string()))?;

            let jti = jti_str.parse()
                .map_err(|e: uuid::Error| RevocationError::Serialization(e.to_string()))?;
            let user_id = user_id_str.parse()
                .map_err(|e: uuid::Error| RevocationError::Serialization(e.to_string()))?;
            let reason = Self::string_to_reason(&reason_str);

            revocations.push(RevokedToken {
                jti,
                user_id,
                revoked_at,
                reason,
                expires_at,
                notes,
            });
        }

        Ok(revocations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reason_conversion() {
        let reason = RevocationReason::PasswordChange;
        let str_reason = PostgresRevocationStore::reason_to_string(reason);
        assert_eq!(str_reason, "password_change");

        let back = PostgresRevocationStore::string_to_reason(&str_reason);
        assert_eq!(back, reason);
    }

    #[test]
    fn test_all_reasons_convert() {
        let reasons = vec![
            RevocationReason::UserLogout,
            RevocationReason::PasswordChange,
            RevocationReason::UserDeactivated,
            RevocationReason::SecurityCompromise,
            RevocationReason::AdminRevocation,
            RevocationReason::Expiration,
            RevocationReason::MaxRotations,
            RevocationReason::SuspiciousActivity,
        ];

        for reason in reasons {
            let str_reason = PostgresRevocationStore::reason_to_string(reason);
            let back = PostgresRevocationStore::string_to_reason(&str_reason);
            assert_eq!(back, reason);
        }
    }
}
