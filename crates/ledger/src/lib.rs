//! Double-Entry Ledger
//!
//! Production-grade accounting ledger with double-entry bookkeeping.
//!
//! Features:
//! - Double-entry accounting (debits = credits)
//! - Multiple account types (Asset, Liability, Equity, Revenue, Expense)
//! - Atomic transaction recording
//! - Balance validation
//! - Full audit trail

pub mod account;
pub mod transaction;

pub use account::*;
pub use transaction::*;

use async_trait::async_trait;
use common::Result;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Trait for ledger storage backends
#[async_trait]
pub trait LedgerStore: Send + Sync {
    /// Create a new account
    async fn create_account(&self, account: Account) -> Result<Account>;

    /// Get an account by ID
    async fn get_account(&self, account_id: AccountId) -> Result<Option<Account>>;

    /// Get all accounts for a user
    async fn get_user_accounts(&self, user_id: common::UserId) -> Result<Vec<Account>>;

    /// Update account balance
    async fn update_account_balance(&self, account_id: AccountId, balance: rust_decimal::Decimal) -> Result<()>;

    /// Record a transaction (must be balanced)
    async fn record_transaction(&self, transaction: Transaction) -> Result<TransactionId>;

    /// Get a transaction by ID
    async fn get_transaction(&self, transaction_id: TransactionId) -> Result<Option<Transaction>>;

    /// Get all transactions
    async fn get_transactions(&self, limit: usize, offset: usize) -> Result<Vec<Transaction>>;

    /// Get account balance
    async fn get_balance(&self, account_id: AccountId) -> Result<rust_decimal::Decimal>;
}

/// In-memory ledger store for testing
pub struct InMemoryLedgerStore {
    accounts: RwLock<HashMap<AccountId, Account>>,
    transactions: RwLock<HashMap<TransactionId, Transaction>>,
}

impl InMemoryLedgerStore {
    pub fn new() -> Self {
        Self {
            accounts: RwLock::new(HashMap::new()),
            transactions: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryLedgerStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LedgerStore for InMemoryLedgerStore {
    async fn create_account(&self, account: Account) -> Result<Account> {
        let mut accounts = self.accounts.write().await;
        accounts.insert(account.id, account.clone());
        Ok(account)
    }

    async fn get_account(&self, account_id: AccountId) -> Result<Option<Account>> {
        let accounts = self.accounts.read().await;
        Ok(accounts.get(&account_id).cloned())
    }

    async fn get_user_accounts(&self, user_id: common::UserId) -> Result<Vec<Account>> {
        let accounts = self.accounts.read().await;
        Ok(accounts
            .values()
            .filter(|a| a.user_id == user_id)
            .cloned()
            .collect())
    }

    async fn update_account_balance(&self, account_id: AccountId, balance: rust_decimal::Decimal) -> Result<()> {
        let mut accounts = self.accounts.write().await;
        if let Some(account) = accounts.get_mut(&account_id) {
            account.balance = balance;
            account.updated_at = chrono::Utc::now();
        }
        Ok(())
    }

    async fn record_transaction(&self, transaction: Transaction) -> Result<TransactionId> {
        if !transaction.is_balanced() {
            return Err(common::TradingError::OrderValidation("Transaction is not balanced".to_string()));
        }

        // Apply entries to accounts
        let mut accounts = self.accounts.write().await;
        for entry in &transaction.entries {
            if let Some(account) = accounts.get_mut(&entry.account_id) {
                match entry.entry_type {
                    EntryType::Debit => account.debit(entry.amount),
                    EntryType::Credit => account.credit(entry.amount),
                }
            }
        }

        // Store transaction
        let tx_id = transaction.id;
        let mut transactions = self.transactions.write().await;
        transactions.insert(tx_id, transaction);

        Ok(tx_id)
    }

    async fn get_transaction(&self, transaction_id: TransactionId) -> Result<Option<Transaction>> {
        let transactions = self.transactions.read().await;
        Ok(transactions.get(&transaction_id).cloned())
    }

    async fn get_transactions(&self, limit: usize, offset: usize) -> Result<Vec<Transaction>> {
        let transactions = self.transactions.read().await;
        Ok(transactions
            .values()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect())
    }

    async fn get_balance(&self, account_id: AccountId) -> Result<rust_decimal::Decimal> {
        let accounts = self.accounts.read().await;
        Ok(accounts
            .get(&account_id)
            .map(|a| a.balance)
            .unwrap_or(rust_decimal::Decimal::ZERO))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::UserId;
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn test_create_and_get_account() {
        let store = InMemoryLedgerStore::new();
        let user_id = UserId::new();

        let account = Account::new(
            user_id,
            AccountType::Asset,
            AssetType::Currency("USD".to_string()),
        );

        let created = store.create_account(account.clone()).await.unwrap();
        assert_eq!(created.id, account.id);

        let retrieved = store.get_account(account.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, account.id);
    }

    #[tokio::test]
    async fn test_record_balanced_transaction() {
        let store = InMemoryLedgerStore::new();
        let user_id = UserId::new();

        let mut cash_account = Account::new(
            user_id,
            AccountType::Asset,
            AssetType::Currency("USD".to_string()),
        );
        cash_account.balance = dec!(1000);

        let securities_account = Account::new(
            user_id,
            AccountType::Asset,
            AssetType::Security("BTC".to_string()),
        );

        store.create_account(cash_account.clone()).await.unwrap();
        store.create_account(securities_account.clone()).await.unwrap();

        let mut tx = Transaction::new("Buy BTC");
        tx.debit(securities_account.id, dec!(100));
        tx.credit(cash_account.id, dec!(100));

        let tx_id = store.record_transaction(tx).await.unwrap();

        // Verify accounts updated
        let cash_balance = store.get_balance(cash_account.id).await.unwrap();
        assert_eq!(cash_balance, dec!(900));

        let btc_balance = store.get_balance(securities_account.id).await.unwrap();
        assert_eq!(btc_balance, dec!(100));

        // Verify transaction stored
        let retrieved_tx = store.get_transaction(tx_id).await.unwrap();
        assert!(retrieved_tx.is_some());
    }

    #[tokio::test]
    async fn test_reject_unbalanced_transaction() {
        let store = InMemoryLedgerStore::new();
        let user_id = UserId::new();

        let account_a = Account::new(
            user_id,
            AccountType::Asset,
            AssetType::Currency("USD".to_string()),
        );
        let account_b = Account::new(
            user_id,
            AccountType::Asset,
            AssetType::Currency("USD".to_string()),
        );

        store.create_account(account_a.clone()).await.unwrap();
        store.create_account(account_b.clone()).await.unwrap();

        let mut tx = Transaction::new("Unbalanced");
        tx.debit(account_a.id, dec!(100));
        tx.credit(account_b.id, dec!(50)); // Not balanced!

        let result = store.record_transaction(tx).await;
        assert!(result.is_err());
    }
}
