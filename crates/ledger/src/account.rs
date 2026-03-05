use chrono::{DateTime, Utc};
use common::UserId;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for an account
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountId(pub Uuid);

impl AccountId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for AccountId {
    fn default() -> Self {
        Self::new()
    }
}

/// Type of account in the ledger
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountType {
    /// Asset account (e.g., cash, securities)
    Asset,
    /// Liability account (e.g., borrowed funds)
    Liability,
    /// Equity account (owner's capital)
    Equity,
    /// Revenue account
    Revenue,
    /// Expense account (e.g., fees, commissions)
    Expense,
}

/// Asset type for an account
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetType {
    /// Fiat currency
    Currency(String), // e.g., "USD", "EUR"
    /// Cryptocurrency or security
    Security(String), // e.g., "BTC", "ETH"
}

/// Account in the double-entry ledger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub user_id: UserId,
    pub account_type: AccountType,
    pub asset_type: AssetType,
    pub balance: Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Account {
    pub fn new(
        user_id: UserId,
        account_type: AccountType,
        asset_type: AssetType,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: AccountId::new(),
            user_id,
            account_type,
            asset_type,
            balance: Decimal::ZERO,
            created_at: now,
            updated_at: now,
        }
    }

    /// Debit the account (increase for assets/expenses, decrease for liabilities/equity/revenue)
    pub fn debit(&mut self, amount: Decimal) {
        match self.account_type {
            AccountType::Asset | AccountType::Expense => {
                self.balance += amount;
            }
            AccountType::Liability | AccountType::Equity | AccountType::Revenue => {
                self.balance -= amount;
            }
        }
        self.updated_at = Utc::now();
    }

    /// Credit the account (decrease for assets/expenses, increase for liabilities/equity/revenue)
    pub fn credit(&mut self, amount: Decimal) {
        match self.account_type {
            AccountType::Asset | AccountType::Expense => {
                self.balance -= amount;
            }
            AccountType::Liability | AccountType::Equity | AccountType::Revenue => {
                self.balance += amount;
            }
        }
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_asset_account_debit_credit() {
        let user_id = UserId::new();
        let mut account = Account::new(
            user_id,
            AccountType::Asset,
            AssetType::Currency("USD".to_string()),
        );

        // Debit increases asset
        account.debit(dec!(100));
        assert_eq!(account.balance, dec!(100));

        // Credit decreases asset
        account.credit(dec!(30));
        assert_eq!(account.balance, dec!(70));
    }

    #[test]
    fn test_liability_account_debit_credit() {
        let user_id = UserId::new();
        let mut account = Account::new(
            user_id,
            AccountType::Liability,
            AssetType::Currency("USD".to_string()),
        );

        // Credit increases liability
        account.credit(dec!(100));
        assert_eq!(account.balance, dec!(100));

        // Debit decreases liability
        account.debit(dec!(30));
        assert_eq!(account.balance, dec!(70));
    }
}
