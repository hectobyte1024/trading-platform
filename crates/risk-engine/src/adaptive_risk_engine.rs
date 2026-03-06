use crate::position_tracker::{Position, PositionTracker};
use async_trait::async_trait;
use common::{Order, Result, RiskCheck, Trade, TradingError, UserId};
use dashmap::DashMap;
use rust_decimal::Decimal;
use std::sync::Arc;
use tracing::info;

/// Configuration for risk limits
#[derive(Debug, Clone)]
pub struct RiskLimits {
    /// Maximum position size per symbol
    pub max_position_size: Decimal,
    /// Maximum total exposure across all symbols
    pub max_total_exposure: Decimal,
    /// Maximum number of open orders per user
    pub max_open_orders: usize,
    /// Maximum order size
    pub max_order_size: Decimal,
    /// Minimum order size
    pub min_order_size: Decimal,
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_position_size: Decimal::new(10000, 0),     // 10,000 units
            max_total_exposure: Decimal::new(50000, 0),     // 50,000 units
            max_open_orders: 100,
            max_order_size: Decimal::new(1000, 0),          // 1,000 units
            min_order_size: Decimal::new(1, 2),             // 0.01 units
        }
    }
}

/// User account balances
#[derive(Debug, Clone)]
pub struct UserAccount {
    pub user_id: UserId,
    pub available_balance: Decimal,
    pub reserved_balance: Decimal,
    pub currency: String,
}

impl UserAccount {
    pub fn new(user_id: UserId, balance: Decimal) -> Self {
        Self {
            user_id,
            available_balance: balance,
            reserved_balance: Decimal::ZERO,
            currency: "USD".to_string(),
        }
    }

    pub fn total_balance(&self) -> Decimal {
        self.available_balance + self.reserved_balance
    }
}

/// Adaptive risk engine with position limits, balance checks, and exposure management
/// This implements the RiskCheck trait and can be plugged into the matching engine
pub struct AdaptiveRiskEngine {
    position_tracker: Arc<PositionTracker>,
    limits: RiskLimits,
    /// User account balances
    accounts: Arc<DashMap<UserId, UserAccount>>,
    /// User-specific risk limits (overrides default limits)
    user_limits: Arc<DashMap<UserId, RiskLimits>>,
}

impl AdaptiveRiskEngine {
    pub fn new(limits: RiskLimits) -> Self {
        Self {
            position_tracker: Arc::new(PositionTracker::new()),
            limits,
            accounts: Arc::new(DashMap::new()),
            user_limits: Arc::new(DashMap::new()),
        }
    }

    /// Register a user account with initial balance
    pub fn register_account(&self, user_id: UserId, balance: Decimal) {
        let account = UserAccount::new(user_id, balance);
        self.accounts.insert(user_id, account);
        info!("Registered account for user {}, balance: {}", user_id, balance);
    }

    /// Set custom risk limits for a specific user
    pub fn set_user_limits(&self, user_id: UserId, limits: RiskLimits) {
        self.user_limits.insert(user_id, limits);
    }

    /// Get all positions for a user
    pub fn get_positions(&self, user_id: UserId) -> Vec<Position> {
        self.position_tracker.get_user_positions(user_id)
    }

    /// Get risk limits for a user (custom or default)
    fn get_limits(&self, user_id: &UserId) -> RiskLimits {
        self.user_limits
            .get(user_id)
            .map(|entry| entry.clone())
            .unwrap_or_else(|| self.limits.clone())
    }

    /// Check if user has sufficient balance for an order
    fn check_balance(&self, order: &Order) -> Result<()> {
        let account = self.accounts.get(&order.user_id).ok_or_else(|| {
            TradingError::RiskCheckFailed(format!("Account not found for user {}", order.user_id))
        })?;

        // For market orders, we can't calculate exact required balance upfront
        // Just check that user has some balance
        if order.order_type == common::OrderType::Market {
            if account.available_balance <= Decimal::ZERO {
                return Err(TradingError::InsufficientBalance {
                    required: Decimal::ONE,
                    available: account.available_balance,
                });
            }
            return Ok(());
        }

        // For limit orders, calculate required balance
        let required = order.price.0 * order.quantity.0;

        if account.available_balance < required {
            return Err(TradingError::InsufficientBalance {
                required,
                available: account.available_balance,
            });
        }

        Ok(())
    }

    /// Check position limits
    fn check_position_limits(&self, order: &Order) -> Result<()> {
        let limits = self.get_limits(&order.user_id);
        
        // Check order size
        if order.quantity.0 > limits.max_order_size {
            return Err(TradingError::OrderValidation(format!(
                "Order size {} exceeds maximum {}",
                order.quantity.0, limits.max_order_size
            )));
        }

        if order.quantity.0 < limits.min_order_size {
            return Err(TradingError::OrderValidation(format!(
                "Order size {} below minimum {}",
                order.quantity.0, limits.min_order_size
            )));
        }

        // Check if this order would exceed position limits
        if let Some(position) = self.position_tracker.get_position(order.user_id, &order.symbol) {
            let new_position = match order.side {
                common::Side::Buy => position.net_quantity + order.quantity.0,
                common::Side::Sell => position.net_quantity - order.quantity.0,
            };

            if new_position.abs() > limits.max_position_size {
                return Err(TradingError::PositionLimitExceeded(format!(
                    "New position {} would exceed limit {}",
                    new_position.abs(),
                    limits.max_position_size
                )));
            }
        }

        // Check total exposure
        let total_exposure = self.position_tracker.get_total_exposure(order.user_id);
        if total_exposure > limits.max_total_exposure {
            return Err(TradingError::PositionLimitExceeded(format!(
                "Total exposure {} exceeds limit {}",
                total_exposure, limits.max_total_exposure
            )));
        }

        // Check open orders count
        if let Some(position) = self.position_tracker.get_position(order.user_id, &order.symbol) {
            if position.open_orders >= limits.max_open_orders {
                return Err(TradingError::RiskCheckFailed(format!(
                    "Maximum open orders {} reached",
                    limits.max_open_orders
                )));
            }
        }

        Ok(())
    }
}

#[async_trait]
impl RiskCheck for AdaptiveRiskEngine {
    async fn check_order(&self, order: &Order) -> Result<()> {
        // Run all checks
        self.check_balance(order)?;
        self.check_position_limits(order)?;

        // Record the order placement
        self.position_tracker.on_order_placed(order);

        info!("Risk check passed for order {}", order.id);
        Ok(())
    }

    async fn check_trade(&self, _trade: &Trade) -> Result<()> {
        // In production, you might want to do additional checks here
        // For now, we assume trades are already validated
        Ok(())
    }

    async fn on_trade_executed(&self, trade: &Trade) -> Result<()> {
        // Update position tracker
        self.position_tracker.on_trade(trade);

        // Update account balances (simplified)
        // In production, this would integrate with the ledger
        info!(
            "Trade executed: {} {} @ {} (buyer: {}, seller: {})",
            trade.quantity, trade.symbol, trade.price, trade.buyer_user_id, trade.seller_user_id
        );

        Ok(())
    }

    async fn on_order_cancelled(&self, order: &Order) -> Result<()> {
        // Update position tracker
        self.position_tracker.on_order_cancelled(order);

        info!("Order cancelled: {}", order.id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{OrderId, OrderStatus, OrderType, Price, Quantity, Side, Symbol, TimeInForce};
    use chrono::Utc;
    use rust_decimal_macros::dec;

    fn create_test_order(user_id: UserId, side: Side, quantity: Decimal) -> Order {
        Order {
            id: OrderId::new(),
            user_id,
            symbol: Symbol::new("BTC/USD"),
            side,
            order_type: OrderType::Limit,
            price: Price::new(dec!(50000)),
            quantity: Quantity::new(quantity),
            filled_quantity: Quantity::zero(),
            time_in_force: TimeInForce::GTC,
            status: OrderStatus::Pending,
            timestamp: Utc::now(),
            sequence_number: 0,
        }
    }

    #[tokio::test]
    async fn test_balance_check() {
        let engine = AdaptiveRiskEngine::new(RiskLimits::default());
        let user_id = UserId::new();

        // Register account with insufficient balance
        engine.register_account(user_id, dec!(1000));

        // Try to place order that requires more balance
        let order = create_test_order(user_id, Side::Buy, dec!(1.0)); // Requires 50,000
        let result = engine.check_order(&order).await;

        assert!(result.is_err());
        match result {
            Err(TradingError::InsufficientBalance { .. }) => {}
            _ => panic!("Expected InsufficientBalance error"),
        }
    }

    #[tokio::test]
    async fn test_position_limits() {
        let mut limits = RiskLimits::default();
        limits.max_order_size = dec!(0.5);
        
        let engine = AdaptiveRiskEngine::new(limits);
        let user_id = UserId::new();
        engine.register_account(user_id, dec!(100000));

        // Try to place order that exceeds size limit
        let order = create_test_order(user_id, Side::Buy, dec!(1.0));
        let result = engine.check_order(&order).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_successful_check() {
        let engine = AdaptiveRiskEngine::new(RiskLimits::default());
        let user_id = UserId::new();
        engine.register_account(user_id, dec!(100000));

        let order = create_test_order(user_id, Side::Buy, dec!(0.5));
        let result = engine.check_order(&order).await;

        assert!(result.is_ok());
    }
}
