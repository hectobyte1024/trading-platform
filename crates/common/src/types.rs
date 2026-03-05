use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for an order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub Uuid);

impl OrderId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for OrderId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a trade
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TradeId(pub Uuid);

impl TradeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TradeId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TradeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a user/account
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub Uuid);

impl UserId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Trading instrument/symbol (e.g., "BTC/USD", "ETH/USDT")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol(pub String);

impl Symbol {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Order side - Buy or Sell
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub fn opposite(&self) -> Self {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Side::Buy => write!(f, "BUY"),
            Side::Sell => write!(f, "SELL"),
        }
    }
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    /// Limit order with specified price
    Limit,
    /// Market order - execute at best available price
    Market,
    /// Stop-loss order
    StopLoss { trigger_price: Price },
    /// Stop-limit order
    StopLimit {
        trigger_price: Price,
        limit_price: Price,
    },
}

/// Time in force for orders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    /// Good Till Cancelled - remains active until filled or cancelled
    GTC,
    /// Immediate or Cancel - fill immediately (partial allowed), cancel remainder
    IOC,
    /// Fill or Kill - fill entire order immediately or cancel
    FOK,
    /// Good Till Date - remains active until specified time
    GTD(DateTime<Utc>),
}

/// Price with deterministic decimal precision
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Price(pub Decimal);

impl Price {
    pub fn new(value: Decimal) -> Self {
        Self(value)
    }

    pub fn from_cents(cents: i64) -> Self {
        Self(Decimal::new(cents, 2))
    }

    pub fn zero() -> Self {
        Self(Decimal::ZERO)
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::ops::Add for Price {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Price {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::Mul<Decimal> for Price {
    type Output = Self;
    fn mul(self, rhs: Decimal) -> Self::Output {
        Self(self.0 * rhs)
    }
}

/// Quantity with deterministic decimal precision
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Quantity(pub Decimal);

impl Quantity {
    pub fn new(value: Decimal) -> Self {
        Self(value)
    }

    pub fn zero() -> Self {
        Self(Decimal::ZERO)
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::ops::Add for Quantity {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Quantity {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::Mul<Decimal> for Quantity {
    type Output = Self;
    fn mul(self, rhs: Decimal) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl PartialEq<Decimal> for Quantity {
    fn eq(&self, other: &Decimal) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<Decimal> for Quantity {
    fn partial_cmp(&self, other: &Decimal) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    /// Order received but not yet processed
    Pending,
    /// Order is active in the orderbook
    Open,
    /// Order is partially filled
    PartiallyFilled,
    /// Order is completely filled
    Filled,
    /// Order is cancelled
    Cancelled,
    /// Order is rejected (risk check failed, etc.)
    Rejected,
    /// Order has expired (GTD)
    Expired,
}

/// Complete order structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    pub user_id: UserId,
    pub symbol: Symbol,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Price,
    pub quantity: Quantity,
    pub filled_quantity: Quantity,
    pub time_in_force: TimeInForce,
    pub status: OrderStatus,
    pub timestamp: DateTime<Utc>,
    pub sequence_number: u64,
}

impl Order {
    pub fn remaining_quantity(&self) -> Quantity {
        Quantity(self.quantity.0 - self.filled_quantity.0)
    }

    pub fn is_fully_filled(&self) -> bool {
        self.filled_quantity >= self.quantity
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Open | OrderStatus::PartiallyFilled
        )
    }
}

/// Trade execution record
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trade {
    pub id: TradeId,
    pub symbol: Symbol,
    pub price: Price,
    pub quantity: Quantity,
    pub buy_order_id: OrderId,
    pub sell_order_id: OrderId,
    pub buyer_user_id: UserId,
    pub seller_user_id: UserId,
    pub timestamp: DateTime<Utc>,
    pub sequence_number: u64,
}

/// Sequence number for deterministic ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SequenceNumber(pub u64);

impl SequenceNumber {
    pub fn new(n: u64) -> Self {
        Self(n)
    }

    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

impl fmt::Display for SequenceNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_price_ordering() {
        let p1 = Price::new(dec!(100.50));
        let p2 = Price::new(dec!(100.51));
        assert!(p1 < p2);
    }

    #[test]
    fn test_quantity_operations() {
        let q1 = Quantity::new(dec!(10.5));
        let q2 = Quantity::new(dec!(5.2));
        let result = q1 - q2;
        assert_eq!(result.0, dec!(5.3));
    }

    #[test]
    fn test_side_opposite() {
        assert_eq!(Side::Buy.opposite(), Side::Sell);
        assert_eq!(Side::Sell.opposite(), Side::Buy);
    }
}
