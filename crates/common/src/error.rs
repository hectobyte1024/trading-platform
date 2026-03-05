use thiserror::Error;

pub type Result<T> = std::result::Result<T, TradingError>;

#[derive(Debug, Error)]
pub enum TradingError {
    #[error("Order validation failed: {0}")]
    OrderValidation(String),

    #[error("Risk check failed: {0}")]
    RiskCheckFailed(String),

    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance {
        required: rust_decimal::Decimal,
        available: rust_decimal::Decimal,
    },

    #[error("Order not found: {0}")]
    OrderNotFound(String),

    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("Invalid price: {0}")]
    InvalidPrice(String),

    #[error("Invalid quantity: {0}")]
    InvalidQuantity(String),

    #[error("Position limit exceeded: {0}")]
    PositionLimitExceeded(String),

    #[error("Orderbook error: {0}")]
    OrderbookError(String),

    #[error("Event journal error: {0}")]
    EventJournalError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Kafka error: {0}")]
    KafkaError(String),

    #[error("Redis error: {0}")]
    RedisError(String),

    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    #[error("Authorization error: {0}")]
    AuthorizationError(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
