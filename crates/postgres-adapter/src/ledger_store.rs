use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::{Result, UserId};
use ledger::{
    Account, AccountId, AccountType, AssetType, EntryType, LedgerEntry, LedgerStore,
    Transaction, TransactionId,
};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use tracing::{debug, error};
use uuid::Uuid;

/// PostgreSQL-backed implementation of LedgerStore
pub struct PostgresLedgerStore {
    pool: PgPool,
}

impl PostgresLedgerStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Serialize account type to string
    fn account_type_to_str(account_type: &AccountType) -> &'static str {
        match account_type {
            AccountType::Asset => "Asset",
            AccountType::Liability => "Liability",
            AccountType::Equity => "Equity",
            AccountType::Revenue => "Revenue",
            AccountType::Expense => "Expense",
        }
    }

    /// Deserialize account type from string
    fn str_to_account_type(s: &str) -> AccountType {
        match s {
            "Asset" => AccountType::Asset,
            "Liability" => AccountType::Liability,
            "Equity" => AccountType::Equity,
            "Revenue" => AccountType::Revenue,
            "Expense" => AccountType::Expense,
            _ => AccountType::Asset, // Default
        }
    }

    /// Serialize asset type
    fn asset_type_parts(asset_type: &AssetType) -> (&'static str, &str) {
        match asset_type {
            AssetType::Currency(c) => ("Currency", c.as_str()),
            AssetType::Security(s) => ("Security", s.as_str()),
        }
    }

    /// Deserialize asset type
    fn parts_to_asset_type(kind: &str, value: &str) -> AssetType {
        match kind {
            "Currency" => AssetType::Currency(value.to_string()),
            "Security" => AssetType::Security(value.to_string()),
            _ => AssetType::Currency(value.to_string()), // Default
        }
    }

    /// Serialize entry type to string
    fn entry_type_to_str(entry_type: &EntryType) -> &'static str {
        match entry_type {
            EntryType::Debit => "Debit",
            EntryType::Credit => "Credit",
        }
    }

    /// Deserialize entry type from string
    fn str_to_entry_type(s: &str) -> EntryType {
        match s {
            "Debit" => EntryType::Debit,
            "Credit" => EntryType::Credit,
            _ => EntryType::Debit, // Default
        }
    }
}

#[async_trait]
impl LedgerStore for PostgresLedgerStore {
    async fn create_account(&self, account: Account) -> Result<Account> {
        let (asset_kind, asset_value) = Self::asset_type_parts(&account.asset_type);
        let account_type_str = Self::account_type_to_str(&account.account_type);

        sqlx::query(
            r#"
            INSERT INTO accounts (id, user_id, account_type, asset_type_kind, asset_type_value, balance, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(account.id.0)
        .bind(account.user_id.0)
        .bind(account_type_str)
        .bind(asset_kind)
        .bind(asset_value)
        .bind(account.balance)
        .bind(account.created_at)
        .bind(account.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to create account: {}", e);
            common::TradingError::DatabaseError(format!("Failed to create account: {}", e))
        })?;

        debug!("Created account: {}", account.id.0);
        Ok(account)
    }

    async fn get_account(&self, account_id: AccountId) -> Result<Option<Account>> {
        let row = sqlx::query(
            r#"
            SELECT id, user_id, account_type, asset_type_kind, asset_type_value, balance, created_at, updated_at
            FROM accounts
            WHERE id = $1
            "#,
        )
        .bind(account_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to get account: {}", e);
            common::TradingError::DatabaseError(format!("Failed to get account: {}", e))
        })?;

        Ok(row.map(|r| Account {
            id: AccountId(r.get("id")),
            user_id: UserId(r.get("user_id")),
            account_type: Self::str_to_account_type(r.get("account_type")),
            asset_type: Self::parts_to_asset_type(r.get("asset_type_kind"), r.get("asset_type_value")),
            balance: r.get("balance"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }

    async fn get_user_accounts(&self, user_id: UserId) -> Result<Vec<Account>> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, account_type, asset_type_kind, asset_type_value, balance, created_at, updated_at
            FROM accounts
            WHERE user_id = $1
            "#,
        )
        .bind(user_id.0)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to get user accounts: {}", e);
            common::TradingError::DatabaseError(format!("Failed to get user accounts: {}", e))
        })?;

        Ok(rows
            .into_iter()
            .map(|r| Account {
                id: AccountId(r.get("id")),
                user_id: UserId(r.get("user_id")),
                account_type: Self::str_to_account_type(r.get("account_type")),
                asset_type: Self::parts_to_asset_type(r.get("asset_type_kind"), r.get("asset_type_value")),
                balance: r.get("balance"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect())
    }

    async fn update_account_balance(&self, account_id: AccountId, balance: Decimal) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE accounts
            SET balance = $1, updated_at = $2
            WHERE id = $3
            "#,
        )
        .bind(balance)
        .bind(Utc::now())
        .bind(account_id.0)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to update account balance: {}", e);
            common::TradingError::DatabaseError(format!("Failed to update account balance: {}", e))
        })?;

        Ok(())
    }

    async fn record_transaction(&self, transaction: Transaction) -> Result<TransactionId> {
        if !transaction.is_balanced() {
            return Err(common::TradingError::OrderValidation(
                "Transaction is not balanced".to_string(),
            ));
        }

        // Start a database transaction
        let mut tx = self.pool.begin().await.map_err(|e| {
            error!("Failed to begin transaction: {}", e);
            common::TradingError::DatabaseError(format!("Failed to begin transaction: {}", e))
        })?;

        // Insert transaction
        sqlx::query(
            r#"
            INSERT INTO transactions (id, description, created_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(transaction.id.0)
        .bind(&transaction.description)
        .bind(transaction.created_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to insert transaction: {}", e);
            common::TradingError::DatabaseError(format!("Failed to insert transaction: {}", e))
        })?;

        // Insert ledger entries and update account balances
        for entry in &transaction.entries {
            // Insert entry
            sqlx::query(
                r#"
                INSERT INTO ledger_entries (id, transaction_id, account_id, entry_type, amount, created_at)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(entry.id)
            .bind(entry.transaction_id.0)
            .bind(entry.account_id.0)
            .bind(Self::entry_type_to_str(&entry.entry_type))
            .bind(entry.amount)
            .bind(entry.created_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to insert ledger entry: {}", e);
                common::TradingError::DatabaseError(format!("Failed to insert ledger entry: {}", e))
            })?;

            // Update account balance
            let account_row = sqlx::query(
                "SELECT account_type, balance FROM accounts WHERE id = $1"
            )
            .bind(entry.account_id.0)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to fetch account for balance update: {}", e);
                common::TradingError::DatabaseError(format!("Failed to fetch account: {}", e))
            })?;

            let account_type = Self::str_to_account_type(account_row.get("account_type"));
            let current_balance: Decimal = account_row.get("balance");

            let new_balance = match (&account_type, &entry.entry_type) {
                (AccountType::Asset, EntryType::Debit) | (AccountType::Expense, EntryType::Debit) => {
                    current_balance + entry.amount
                }
                (AccountType::Asset, EntryType::Credit) | (AccountType::Expense, EntryType::Credit) => {
                    current_balance - entry.amount
                }
                (AccountType::Liability, EntryType::Debit)
                | (AccountType::Equity, EntryType::Debit)
                | (AccountType::Revenue, EntryType::Debit) => current_balance - entry.amount,
                (AccountType::Liability, EntryType::Credit)
                | (AccountType::Equity, EntryType::Credit)
                | (AccountType::Revenue, EntryType::Credit) => current_balance + entry.amount,
            };

            sqlx::query(
                "UPDATE accounts SET balance = $1, updated_at = $2 WHERE id = $3"
            )
            .bind(new_balance)
            .bind(Utc::now())
            .bind(entry.account_id.0)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to update account balance: {}", e);
                common::TradingError::DatabaseError(format!("Failed to update account balance: {}", e))
            })?;
        }

        // Commit transaction
        tx.commit().await.map_err(|e| {
            error!("Failed to commit transaction: {}", e);
            common::TradingError::DatabaseError(format!("Failed to commit transaction: {}", e))
        })?;

        debug!("Recorded transaction: {}", transaction.id.0);
        Ok(transaction.id)
    }

    async fn get_transaction(&self, transaction_id: TransactionId) -> Result<Option<Transaction>> {
        // Fetch transaction
        let tx_row = sqlx::query(
            "SELECT id, description, created_at FROM transactions WHERE id = $1"
        )
        .bind(transaction_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to get transaction: {}", e);
            common::TradingError::DatabaseError(format!("Failed to get transaction: {}", e))
        })?;

        if tx_row.is_none() {
            return Ok(None);
        }

        let tx_row = tx_row.unwrap();

        // Fetch entries
        let entry_rows = sqlx::query(
            r#"
            SELECT id, transaction_id, account_id, entry_type, amount, created_at
            FROM ledger_entries
            WHERE transaction_id = $1
            "#,
        )
        .bind(transaction_id.0)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to get ledger entries: {}", e);
            common::TradingError::DatabaseError(format!("Failed to get ledger entries: {}", e))
        })?;

        let entries = entry_rows
            .into_iter()
            .map(|r| LedgerEntry {
                id: r.get("id"),
                transaction_id: TransactionId(r.get("transaction_id")),
                account_id: AccountId(r.get("account_id")),
                entry_type: Self::str_to_entry_type(r.get("entry_type")),
                amount: r.get("amount"),
                created_at: r.get("created_at"),
            })
            .collect();

        Ok(Some(Transaction {
            id: TransactionId(tx_row.get("id")),
            description: tx_row.get("description"),
            entries,
            created_at: tx_row.get("created_at"),
        }))
    }

    async fn get_transactions(&self, limit: usize, offset: usize) -> Result<Vec<Transaction>> {
        let tx_rows = sqlx::query(
            "SELECT id, description, created_at FROM transactions ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to get transactions: {}", e);
            common::TradingError::DatabaseError(format!("Failed to get transactions: {}", e))
        })?;

        let mut transactions = Vec::new();

        for tx_row in tx_rows {
            let tx_id: Uuid = tx_row.get("id");

            let entry_rows = sqlx::query(
                "SELECT id, transaction_id, account_id, entry_type, amount, created_at FROM ledger_entries WHERE transaction_id = $1"
            )
            .bind(tx_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to get ledger entries: {}", e);
                common::TradingError::DatabaseError(format!("Failed to get ledger entries: {}", e))
            })?;

            let entries = entry_rows
                .into_iter()
                .map(|r| LedgerEntry {
                    id: r.get("id"),
                    transaction_id: TransactionId(r.get("transaction_id")),
                    account_id: AccountId(r.get("account_id")),
                    entry_type: Self::str_to_entry_type(r.get("entry_type")),
                    amount: r.get("amount"),
                    created_at: r.get("created_at"),
                })
                .collect();

            transactions.push(Transaction {
                id: TransactionId(tx_id),
                description: tx_row.get("description"),
                entries,
                created_at: tx_row.get("created_at"),
            });
        }

        Ok(transactions)
    }

    async fn get_balance(&self, account_id: AccountId) -> Result<Decimal> {
        let row = sqlx::query("SELECT balance FROM accounts WHERE id = $1")
            .bind(account_id.0)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to get balance: {}", e);
                common::TradingError::DatabaseError(format!("Failed to get balance: {}", e))
            })?;

        Ok(row.map(|r| r.get("balance")).unwrap_or(Decimal::ZERO))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires PostgreSQL instance
    async fn test_postgres_ledger_store() {
        // Test would require a running PostgreSQL instance
        // This is a placeholder to verify compilation
    }
}
