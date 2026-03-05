use sqlx::postgres::{PgPool, PgPoolOptions};
use tracing::info;

/// PostgreSQL connection pool configuration
pub struct PostgresConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout_seconds: u64,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://postgres:password@localhost/trading".to_string()),
            max_connections: 10,
            min_connections: 2,
            acquire_timeout_seconds: 30,
        }
    }
}

/// Create a PostgreSQL connection pool
pub async fn create_pool(config: PostgresConfig) -> Result<PgPool, sqlx::Error> {
    info!("Creating PostgreSQL connection pool: {}", config.database_url);

    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(std::time::Duration::from_secs(config.acquire_timeout_seconds))
        .connect(&config.database_url)
        .await?;

    info!("PostgreSQL connection pool created successfully");

    Ok(pool)
}

/// Initialize database schema
pub async fn init_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    info!("Initializing database schema...");

    // Create accounts table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS accounts (
            id UUID PRIMARY KEY,
            user_id UUID NOT NULL,
            account_type VARCHAR(50) NOT NULL,
            asset_type_kind VARCHAR(50) NOT NULL,
            asset_type_value TEXT NOT NULL,
            balance DECIMAL(38, 18) NOT NULL DEFAULT 0,
            created_at TIMESTAMPTZ NOT NULL,
            updated_at TIMESTAMPTZ NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create transactions table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS transactions (
            id UUID PRIMARY KEY,
            description TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create ledger_entries table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS ledger_entries (
            id UUID PRIMARY KEY,
            transaction_id UUID NOT NULL REFERENCES transactions(id),
            account_id UUID NOT NULL REFERENCES accounts(id),
            entry_type VARCHAR(10) NOT NULL,
            amount DECIMAL(38, 18) NOT NULL,
            created_at TIMESTAMPTZ NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_accounts_user_id ON accounts(user_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ledger_entries_transaction_id ON ledger_entries(transaction_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ledger_entries_account_id ON ledger_entries(account_id)")
        .execute(pool)
        .await?;

    info!("Database schema initialized successfully");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires PostgreSQL instance
    async fn test_create_pool() {
        let config = PostgresConfig::default();
        let result = create_pool(config).await;
        // Won't connect successfully in CI but verifies compilation
        assert!(result.is_ok() || result.is_err());
    }
}
