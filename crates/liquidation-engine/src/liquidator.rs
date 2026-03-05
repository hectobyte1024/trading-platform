use crate::monitor::{LiquidationCandidate, PositionMonitor};
use common::{
    Order, OrderId, OrderStatus, OrderType, Price, Quantity, Result, RiskCheck, Side, Symbol,
    TimeInForce, UserId,
};
use event_journal::EventJournal;
use matching_engine::MatchingEngine;
use risk_engine::AdaptiveRiskEngine;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

/// Liquidation strategy
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiquidationStrategy {
    /// Liquidate entire position immediately (market order)
    FullMarket,
    /// Liquidate in chunks to minimize market impact
    Gradual,
    /// Attempt limit orders first, fallback to market
    LimitThenMarket,
}

/// Liquidator configuration
#[derive(Debug, Clone)]
pub struct LiquidatorConfig {
    /// Default liquidation strategy
    pub strategy: LiquidationStrategy,
    /// Liquidation check interval in seconds
    pub check_interval_secs: u64,
    /// Maximum slippage tolerance for limit orders (percentage)
    pub max_slippage: Decimal,
    /// Chunk size for gradual liquidation (percentage of position)
    pub chunk_size: Decimal,
}

impl Default for LiquidatorConfig {
    fn default() -> Self {
        Self {
            strategy: LiquidationStrategy::FullMarket,
            check_interval_secs: 1,
            max_slippage: Decimal::new(5, 2), // 5%
            chunk_size: Decimal::new(50, 2),  // 50%
        }
    }
}

/// Result of a liquidation attempt
#[derive(Debug)]
pub struct LiquidationResult {
    pub user_id: UserId,
    pub symbol: Symbol,
    pub order_id: OrderId,
    pub quantity_liquidated: Quantity,
    pub success: bool,
    pub error: Option<String>,
}

/// Automated liquidation engine
pub struct Liquidator<J: EventJournal, R: RiskCheck> {
    monitor: Arc<PositionMonitor>,
    matching_engine: Arc<MatchingEngine<J, R>>,
    risk_engine: Arc<AdaptiveRiskEngine>,
    config: LiquidatorConfig,
    /// System user ID for liquidation orders
    system_user_id: UserId,
}

impl<J: EventJournal, R: RiskCheck> Liquidator<J, R> {
    pub fn new(
        monitor: Arc<PositionMonitor>,
        matching_engine: Arc<MatchingEngine<J, R>>,
        risk_engine: Arc<AdaptiveRiskEngine>,
        config: LiquidatorConfig,
    ) -> Self {
        Self {
            monitor,
            matching_engine,
            risk_engine,
            config,
            system_user_id: UserId::new(), // System liquidation account
        }
    }

    /// Execute liquidation for a candidate
    pub async fn liquidate(&self, candidate: &LiquidationCandidate) -> Result<LiquidationResult> {
        info!(
            "Liquidating position: user={}, symbol={}, net_qty={}, urgency={:?}",
            candidate.user_id,
            candidate.symbol.0,
            candidate.position.net_quantity,
            candidate.urgency
        );

        match self.config.strategy {
            LiquidationStrategy::FullMarket => self.liquidate_market(candidate).await,
            LiquidationStrategy::Gradual => self.liquidate_gradual(candidate).await,
            LiquidationStrategy::LimitThenMarket => self.liquidate_limit_then_market(candidate).await,
        }
    }

    /// Liquidate using market order (fastest, but potentially worst price)
    async fn liquidate_market(&self, candidate: &LiquidationCandidate) -> Result<LiquidationResult> {
        let net_qty = candidate.position.net_quantity;
        
        // Determine liquidation side (opposite of position)
        let side = if net_qty > Decimal::ZERO {
            Side::Sell // Close long position
        } else {
            Side::Buy // Close short position
        };

        let quantity = Quantity::new(net_qty.abs());

        // Get current market price for logging
        let current_price = self.monitor.get_price(&candidate.symbol)
            .unwrap_or(Decimal::ZERO);

        let order = Order {
            id: OrderId::new(),
            user_id: candidate.user_id, // Liquidate from user's account
            symbol: candidate.symbol.clone(),
            side,
            order_type: OrderType::Market,
            price: Price::new(current_price), // Market orders use current price as reference
            quantity,
            filled_quantity: Quantity::zero(),
            time_in_force: TimeInForce::IOC, // Immediate or cancel
            status: OrderStatus::Pending,
            timestamp: chrono::Utc::now(),
            sequence_number: 0,
        };

        let order_id = order.id;

        match self.matching_engine.place_order(order).await {
            Ok(_) => {
                info!(
                    "Liquidation order placed: order_id={}, symbol={}, side={:?}, qty={}",
                    order_id,
                    candidate.symbol.0,
                    side,
                    quantity
                );

                Ok(LiquidationResult {
                    user_id: candidate.user_id,
                    symbol: candidate.symbol.clone(),
                    order_id,
                    quantity_liquidated: quantity,
                    success: true,
                    error: None,
                })
            }
            Err(e) => {
                error!("Liquidation order failed: {:?}", e);
                
                Ok(LiquidationResult {
                    user_id: candidate.user_id,
                    symbol: candidate.symbol.clone(),
                    order_id,
                    quantity_liquidated: Quantity::zero(),
                    success: false,
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// Liquidate gradually in smaller chunks
    async fn liquidate_gradual(&self, candidate: &LiquidationCandidate) -> Result<LiquidationResult> {
        let net_qty = candidate.position.net_quantity.abs();
        let chunk_qty = net_qty * self.config.chunk_size;

        // For now, just liquidate the first chunk
        // In production, this would be a multi-step process
        let mut modified_candidate = candidate.clone();
        modified_candidate.position.net_quantity = if candidate.position.net_quantity > Decimal::ZERO {
            chunk_qty
        } else {
            -chunk_qty
        };

        self.liquidate_market(&modified_candidate).await
    }

    /// Try limit order first, fallback to market
    async fn liquidate_limit_then_market(&self, candidate: &LiquidationCandidate) -> Result<LiquidationResult> {
        // For simplicity, just use market order
        // In production, would place limit order and wait, then use market as fallback
        self.liquidate_market(candidate).await
    }

    /// Start automated liquidation task
    pub async fn start_liquidation_task(self: Arc<Self>) {
        let mut ticker = interval(Duration::from_secs(self.config.check_interval_secs));

        info!("Liquidation engine started (interval: {}s)", self.config.check_interval_secs);

        loop {
            ticker.tick().await;

            // Get critical candidates that need immediate liquidation
            let candidates = self.monitor.get_critical_candidates();

            if !candidates.is_empty() {
                warn!("Processing {} critical liquidation candidates", candidates.len());

                for candidate in candidates {
                    match self.liquidate(&candidate).await {
                        Ok(result) => {
                            if result.success {
                                info!(
                                    "Liquidation successful: user={}, symbol={}, qty={}",
                                    result.user_id,
                                    result.symbol.0,
                                    result.quantity_liquidated
                                );
                            } else {
                                error!(
                                    "Liquidation failed: user={}, symbol={}, error={:?}",
                                    result.user_id,
                                    result.symbol.0,
                                    result.error
                                );
                            }
                        }
                        Err(e) => {
                            error!("Liquidation error: {:?}", e);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_liquidator_config_default() {
        let config = LiquidatorConfig::default();
        assert_eq!(config.strategy, LiquidationStrategy::FullMarket);
        assert_eq!(config.check_interval_secs, 1);
    }

    #[test]
    fn test_liquidation_strategy() {
        assert_eq!(LiquidationStrategy::FullMarket, LiquidationStrategy::FullMarket);
        assert_ne!(LiquidationStrategy::FullMarket, LiquidationStrategy::Gradual);
    }
}
