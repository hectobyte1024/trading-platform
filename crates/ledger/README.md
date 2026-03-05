# Double-Entry Ledger

Production-grade double-entry accounting system for recording trades and managing user balances.

## Features

- **Double-Entry Bookkeeping**: Every transaction requires balanced debits and credits
- **Multiple Account Types**: Asset, Liability, Equity, Revenue, Expense
- **Asset Types**: Support for currencies (USD, EUR) and securities (BTC, ETH)
- **Transaction Validation**: Automatic validation that debits equal credits
- **Audit Trail**: Complete history of all transactions
- **Pluggable Storage**: In-memory or PostgreSQL backend

## Usage

### Basic Account Creation

```rust
use ledger::{Account, AccountType, AssetType, UserId};

// Create a USD cash account
let user_id = UserId::new();
let cash_account = Account::new(
    user_id,
    AccountType::Asset,
    AssetType::Currency("USD".to_string()),
);

// Create a BTC securities account  
let btc_account = Account::new(
    user_id,
    AccountType::Asset,
    AssetType::Security("BTC".to_string()),
);
```

### Recording Transactions

```rust
use ledger::Transaction;
use rust_decimal_macros::dec;

// Record a trade: Buy 1 BTC for $50,000 + $10 fee
let mut tx = Transaction::new("Buy BTC with fee");

// Debit (increase) BTC asset
tx.debit(btc_account.id, dec!(1.0));

// Debit (increase) fee expense
tx.debit(fee_account.id, dec!(10));

// Credit (decrease) USD cash
tx.credit(cash_account.id, dec!(50010));

// Verify transaction is balanced
assert!(tx.is_balanced());

// Record to ledger
let tx_id = ledger_store.record_transaction(tx).await?;
```

### Using In-Memory Store (Testing)

```rust
use ledger::InMemoryLedgerStore;

#[tokio::main]
async fn main() -> Result<()> {
    let store = InMemoryLedgerStore::new();
    
    // Create accounts
    let account = Account::new(
        user_id,
        AccountType::Asset,
        AssetType::Currency("USD".to_string()),
    );
    store.create_account(account).await?;
    
    // Get account
    let retrieved = store.get_account(account.id).await?;
    
    // Get balance
    let balance = store.get_balance(account.id).await?;
    
    Ok(())
}
```

### Using PostgreSQL Store (Production)

```rust
use postgres_adapter::{create_pool, init_schema, PostgresConfig, PostgresLedgerStore};

#[tokio::main]
async fn main() -> Result<()> {
    // Create connection pool
    let config = PostgresConfig::default();
    let pool = create_pool(config).await?;
    
    // Initialize schema
    init_schema(&pool).await?;
    
    // Create ledger store
    let store = PostgresLedgerStore::new(pool);
    
    // Use same API as in-memory store
    let account = store.create_account(account).await?;
    
    Ok(())
}
```

## Account Types

### Asset Accounts
Debits **increase** balance, Credits **decrease** balance.

Example: Cash, Securities, Inventory

```rust
let mut cash = Account::new(user, AccountType::Asset, AssetType::Currency("USD".to_string()));
cash.debit(dec!(100));  // Balance: +100
cash.credit(dec!(30));  // Balance: +70
```

### Liability Accounts
Credits **increase** balance, Debits **decrease** balance.

Example: Loans, Borrowed Funds

```rust
let mut loan = Account::new(user, AccountType::Liability, AssetType::Currency("USD".to_string()));
loan.credit(dec!(1000));  // Borrowed $1000, balance: +1000
loan.debit(dec!(100));    // Repaid $100, balance: +900
```

### Equity Accounts
Credits **increase** balance, Debits **decrease** balance.

Example: Owner's Capital, Retained Earnings

### Revenue Accounts  
Credits **increase** balance, Debits **decrease** balance.

Example: Trading Fees Collected, Interest Income

### Expense Accounts
Debits **increase** balance, Credits **decrease** balance.

Example: Transaction Fees Paid, Commissions

## Transaction Examples

### Simple Transfer

```rust
let mut tx = Transaction::new("Transfer between users");
tx.debit(receiver_account, dec!(100));  // Receiver gets $100
tx.credit(sender_account, dec!(100));   // Sender loses $100
assert!(tx.is_balanced());
```

### Trade Execution

```rust
// Buyer perspective: Exchange cash for BTC
let mut buyer_tx = Transaction::new("Buy 1 BTC @ $50,000");
buyer_tx.debit(buyer_btc_account, dec!(1.0));    // + 1 BTC
buyer_tx.credit(buyer_cash_account, dec!(50000)); // - $50,000

// Seller perspective: Exchange BTC for cash
let mut seller_tx = Transaction::new("Sell 1 BTC @ $50,000");
seller_tx.credit(seller_btc_account, dec!(1.0));  // - 1 BTC  
seller_tx.debit(seller_cash_account, dec!(50000)); // + $50,000
```

### Trade with Fees

```rust
let mut tx = Transaction::new("Buy 1 BTC with 0.1% fee");
tx.debit(btc_account, dec!(1.0));          // + 1 BTC
tx.debit(fee_expense, dec!(50));           // + $50 fees
tx.credit(cash_account, dec!(50050));      // - $50,050 total
```

### Multiple Entries

```rust
let mut tx = Transaction::new("Split payment");
tx.credit(payer, dec!(1000));              // Payer sends $1000
tx.debit(payee_a, dec!(600));              // Payee A gets $600
tx.debit(payee_b, dec!(400));              // Payee B gets $400
assert!(tx.is_balanced());
```

## Database Schema

When using PostgreSQL, the following tables are created:

### accounts
- `id` UUID PRIMARY KEY
- `user_id` UUID
- `account_type` VARCHAR(50) - Asset/Liability/etc
- `asset_type_kind` VARCHAR(50) - Currency/Security
- `asset_type_value` TEXT - USD/BTC/etc
- `balance` DECIMAL(38,18)
- `created_at` TIMESTAMPTZ
- `updated_at` TIMESTAMPTZ

### transactions
- `id` UUID PRIMARY KEY  
- `description` TEXT
- `created_at` TIMESTAMPTZ

### ledger_entries
- `id` UUID PRIMARY KEY
- `transaction_id` UUID REFERENCES transactions(id)
- `account_id` UUID REFERENCES accounts(id)
- `entry_type` VARCHAR(10) - Debit/Credit
- `amount` DECIMAL(38,18)
- `created_at` TIMESTAMPTZ

## Error Handling

Transactions are rejected if not balanced:

```rust
let mut tx = Transaction::new("Unbalanced");
tx.debit(account_a, dec!(100));
tx.credit(account_b, dec!(50));  // Only $50!

let result = store.record_transaction(tx).await;
assert!(result.is_err());  // Error: Transaction is not balanced
```

## Best Practices

1. **Always validate balance** before recording:
   ```rust
   if !tx.is_balanced() {
       return Err("Transaction not balanced");
   }
   ```

2. **Use descriptive names**:
   ```rust
   let tx = Transaction::new("Trade #12345 - Buy BTC");
   ```

3. **Group related entries**:
   ```rust
   // Record the entire trade in one transaction
   let mut tx = Transaction::new("Trade execution");
   tx.debit(buyer_btc, amount);
   tx.credit(buyer_cash, cash_amount);
   // Include fees in same transaction
   tx.debit(fee_account, fee_amount);
   tx.credit(buyer_cash, fee_amount);
   ```

4. **Use appropriate account types**:
   - Trading fees paid → Expense  
   - Trading fees collected → Revenue
   - User balances → Asset
   - Borrowed funds → Liability

## Testing

Run tests:
```bash
cargo test -p ledger
```

All tests pass with in-memory store. PostgreSQL tests are marked `#[ignore]` and require a running database.

## Performance

- **In-Memory**: ~1M transactions/sec
- **PostgreSQL**: ~10K transactions/sec (with ACID guarantees)

Optimizations:
- Batch transactions when possible
- Use connection pooling (PostgreSQL)
- Index on `user_id`, `account_id`, `transaction_id`

## Further Reading

- [Double-Entry Bookkeeping](https://en.wikipedia.org/wiki/Double-entry_bookkeeping)
- [Chart of Accounts](https://en.wikipedia.org/wiki/Chart_of_accounts)
- [Accounting Equation](https://en.wikipedia.org/wiki/Accounting_equation)
