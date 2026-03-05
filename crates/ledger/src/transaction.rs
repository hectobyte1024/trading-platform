use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::account::AccountId;

/// Unique identifier for a ledger transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionId(pub Uuid);

impl TransactionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TransactionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Entry in a double-entry transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub id: Uuid,
    pub transaction_id: TransactionId,
    pub account_id: AccountId,
    pub entry_type: EntryType,
    pub amount: Decimal,
    pub created_at: DateTime<Utc>,
}

/// Type of ledger entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryType {
    Debit,
    Credit,
}

/// A complete double-entry transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: TransactionId,
    pub description: String,
    pub entries: Vec<LedgerEntry>,
    pub created_at: DateTime<Utc>,
}

impl Transaction {
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            id: TransactionId::new(),
            description: description.into(),
            entries: Vec::new(),
            created_at: Utc::now(),
        }
    }

    /// Add a debit entry
    pub fn debit(&mut self, account_id: AccountId, amount: Decimal) {
        self.entries.push(LedgerEntry {
            id: Uuid::new_v4(),
            transaction_id: self.id,
            account_id,
            entry_type: EntryType::Debit,
            amount,
            created_at: Utc::now(),
        });
    }

    /// Add a credit entry
    pub fn credit(&mut self, account_id: AccountId, amount: Decimal) {
        self.entries.push(LedgerEntry {
            id: Uuid::new_v4(),
            transaction_id: self.id,
            account_id,
            entry_type: EntryType::Credit,
            amount,
            created_at: Utc::now(),
        });
    }

    /// Validate that debits equal credits (fundamental accounting equation)
    pub fn is_balanced(&self) -> bool {
        let total_debits: Decimal = self
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Debit)
            .map(|e| e.amount)
            .sum();

        let total_credits: Decimal = self
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Credit)
            .map(|e| e.amount)
            .sum();

        total_debits == total_credits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AccountId;
    use rust_decimal_macros::dec;

    #[test]
    fn test_balanced_transaction() {
        let mut tx = Transaction::new("Test transfer");
        let account_a = AccountId::new();
        let account_b = AccountId::new();

        tx.debit(account_a, dec!(100));
        tx.credit(account_b, dec!(100));

        assert!(tx.is_balanced());
        assert_eq!(tx.entries.len(), 2);
    }

    #[test]
    fn test_unbalanced_transaction() {
        let mut tx = Transaction::new("Unbalanced");
        let account_a = AccountId::new();
        let account_b = AccountId::new();

        tx.debit(account_a, dec!(100));
        tx.credit(account_b, dec!(50));

        assert!(!tx.is_balanced());
    }

    #[test]
    fn test_complex_transaction() {
        let mut tx = Transaction::new("Trade with fees");
        let cash = AccountId::new();
        let securities = AccountId::new();
        let fees = AccountId::new();

        // Buy securities for $100 + $2 fee
        tx.debit(securities, dec!(100)); // Increase securities
        tx.debit(fees, dec!(2)); // Record fee expense
        tx.credit(cash, dec!(102)); // Decrease cash

        assert!(tx.is_balanced());
        assert_eq!(tx.entries.len(), 3);
    }
}
