use crate::types::{Order, Trade};
use crate::error::Result;
use async_trait::async_trait;

/// RiskCheck trait for pre-trade risk validation
/// Implementations can check balances, position limits, credit limits, etc.
#[async_trait]
pub trait RiskCheck: Send + Sync {
    /// Validate an order before it enters the matching engine
    /// Returns Ok(()) if the order passes risk checks
    /// Returns Err if the order should be rejected
    async fn check_order(&self, order: &Order) -> Result<()>;

    /// Validate a potential trade execution
    /// This is called before the trade is executed to ensure both sides are still compliant
    async fn check_trade(&self, trade: &Trade) -> Result<()>;

    /// Update risk state after a trade is executed
    /// This allows the risk engine to update positions, exposures, etc.
    async fn on_trade_executed(&self, trade: &Trade) -> Result<()>;

    /// Update risk state after an order is cancelled
    async fn on_order_cancelled(&self, order: &Order) -> Result<()>;
}

/// Event publisher trait for emitting events to Kafka, etc.
#[async_trait]
pub trait EventPublisher: Send + Sync {
    /// Publish an event
    async fn publish(&self, topic: &str, key: &[u8], payload: &[u8]) -> Result<()>;
}

/// Event subscriber trait for consuming events
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Subscribe to a topic and process events
    async fn subscribe<F>(&self, topic: &str, handler: F) -> Result<()>
    where
        F: Fn(&[u8], &[u8]) -> Result<()> + Send + Sync + 'static;
}

/// Trait for persisting ledger entries
#[async_trait]
pub trait LedgerStore: Send + Sync {
    /// Record a double-entry transaction
    async fn record_transaction(&self, transaction: LedgerTransaction) -> Result<()>;

    /// Get account balance
    async fn get_balance(&self, account_id: uuid::Uuid) -> Result<rust_decimal::Decimal>;
}

/// Represents a double-entry ledger transaction
#[derive(Debug, Clone)]
pub struct LedgerTransaction {
    pub id: uuid::Uuid,
    pub debit_account: uuid::Uuid,
    pub credit_account: uuid::Uuid,
    pub amount: rust_decimal::Decimal,
    pub currency: String,
    pub reference: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
