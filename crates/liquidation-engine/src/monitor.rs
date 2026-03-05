use common::{Symbol, UserId};
use dashmap::DashMap;
use risk_engine::{AdaptiveRiskEngine, Position};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};

/// Configuration for margin and liquidation thresholds
#[derive(Debug, Clone)]
pub struct MarginConfig {
    /// Initial margin requirement (e.g., 0.10 = 10%)
    pub initial_margin: Decimal,
    /// Maintenance margin requirement (e.g., 0.05 = 5%)
    pub maintenance_margin: Decimal,
    /// Liquidation margin threshold (e.g., 0.03 = 3%)
    pub liquidation_margin: Decimal,
    /// Monitoring interval in seconds
    pub monitor_interval_secs: u64,
}

impl Default for MarginConfig {
    fn default() -> Self {
        Self {
            initial_margin: Decimal::new(10, 2),      // 10%
            maintenance_margin: Decimal::new(5, 2),   // 5%
            liquidation_margin: Decimal::new(3, 2),   // 3%
            monitor_interval_secs: 1,                  // Check every second
        }
    }
}

/// Margin level for a user's position
#[derive(Debug, Clone)]
pub struct MarginLevel {
    pub user_id: UserId,
    pub equity: Decimal,
    pub used_margin: Decimal,
    pub free_margin: Decimal,
    pub margin_level: Decimal, // equity / used_margin (percentage)
    pub at_risk: bool,
}

impl MarginLevel {
    /// Calculate margin level from equity and used margin
    pub fn calculate(user_id: UserId, equity: Decimal, used_margin: Decimal) -> Self {
        let free_margin = equity - used_margin;
        let margin_level = if used_margin > Decimal::ZERO {
            (equity / used_margin) * Decimal::new(100, 0)
        } else {
            Decimal::new(999999, 0) // Effectively infinite if no positions
        };

        let at_risk = margin_level < Decimal::new(100, 0); // Below 100% is risky

        Self {
            user_id,
            equity,
            used_margin,
            free_margin,
            margin_level,
            at_risk,
        }
    }
}

/// Liquidation candidate
#[derive(Debug, Clone)]
pub struct LiquidationCandidate {
    pub user_id: UserId,
    pub symbol: Symbol,
    pub position: Position,
    pub margin_level: MarginLevel,
    pub urgency: LiquidationUrgency,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiquidationUrgency {
    /// Margin level below liquidation threshold - immediate liquidation
    Critical,
    /// Margin level below maintenance but above liquidation - margin call
    Warning,
    /// Margin level OK
    Normal,
}

/// Monitors positions and detects liquidation candidates
pub struct PositionMonitor {
    risk_engine: Arc<AdaptiveRiskEngine>,
    config: MarginConfig,
    /// Latest prices for symbols (simplified - in production use market data feed)
    prices: Arc<DashMap<Symbol, Decimal>>,
    /// Liquidation candidates detected
    candidates: Arc<DashMap<(UserId, Symbol), LiquidationCandidate>>,
}

impl PositionMonitor {
    pub fn new(risk_engine: Arc<AdaptiveRiskEngine>, config: MarginConfig) -> Self {
        Self {
            risk_engine,
            config,
            prices: Arc::new(DashMap::new()),
            candidates: Arc::new(DashMap::new()),
        }
    }

    /// Update market price for a symbol
    pub fn update_price(&self, symbol: Symbol, price: Decimal) {
        debug!("Updated price for {}: {}", symbol.0, price);
        self.prices.insert(symbol, price);
    }

    /// Get current price for a symbol
    pub fn get_price(&self, symbol: &Symbol) -> Option<Decimal> {
        self.prices.get(symbol).map(|p| *p)
    }

    /// Calculate margin level for a user
    pub fn calculate_margin_level(&self, user_id: UserId) -> Option<MarginLevel> {
        // Get all positions for the user
        let positions = self.risk_engine.get_positions(user_id);
        
        if positions.is_empty() {
            return None;
        }

        // Calculate equity and used margin
        let mut total_value = Decimal::ZERO;
        let mut used_margin = Decimal::ZERO;

        for position in &positions {
            if let Some(price) = self.get_price(&position.symbol) {
                let position_value = position.net_quantity.abs() * price;
                total_value += position_value;
                used_margin += position_value * self.config.initial_margin;
            }
        }

        // Get account balance (simplified - assuming balance is tracked separately)
        // In production, this would come from the ledger
        let equity = total_value; // Simplified for now

        Some(MarginLevel::calculate(user_id, equity, used_margin))
    }

    /// Check a single position for liquidation
    pub fn check_position(&self, user_id: UserId, position: &Position) -> Option<LiquidationCandidate> {
        let margin_level = self.calculate_margin_level(user_id)?;
        
        let urgency = if margin_level.margin_level < self.config.liquidation_margin * Decimal::new(100, 0) {
            LiquidationUrgency::Critical
        } else if margin_level.margin_level < self.config.maintenance_margin * Decimal::new(100, 0) {
            LiquidationUrgency::Warning
        } else {
            LiquidationUrgency::Normal
        };

        if urgency != LiquidationUrgency::Normal {
            Some(LiquidationCandidate {
                user_id,
                symbol: position.symbol.clone(),
                position: position.clone(),
                margin_level,
                urgency,
            })
        } else {
            None
        }
    }

    /// Scan all positions for liquidation candidates
    pub fn scan_positions(&self) -> Vec<LiquidationCandidate> {
        let candidates = Vec::new();

        // Get all users with positions (simplified - in production, track this separately)
        // For now, we'll scan through known positions
        // This is a placeholder - real implementation would track all active users

        candidates
    }

    /// Get all liquidation candidates
    pub fn get_candidates(&self) -> Vec<LiquidationCandidate> {
        self.candidates.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Get critical liquidation candidates only
    pub fn get_critical_candidates(&self) -> Vec<LiquidationCandidate> {
        self.get_candidates()
            .into_iter()
            .filter(|c| c.urgency == LiquidationUrgency::Critical)
            .collect()
    }

    /// Start monitoring task (runs in background)
    pub async fn start_monitoring(self: Arc<Self>) {
        let mut ticker = interval(Duration::from_secs(self.config.monitor_interval_secs));

        info!("Position monitor started (interval: {}s)", self.config.monitor_interval_secs);

        loop {
            ticker.tick().await;
            
            // Scan for liquidation candidates
            let candidates = self.scan_positions();
            
            if !candidates.is_empty() {
                warn!("Found {} liquidation candidates", candidates.len());
                
                for candidate in candidates {
                    let key = (candidate.user_id, candidate.symbol.clone());
                    
                    if candidate.urgency == LiquidationUrgency::Critical {
                        warn!(
                            "CRITICAL: User {} position in {} at margin level {:.2}%",
                            candidate.user_id,
                            candidate.symbol.0,
                            candidate.margin_level.margin_level
                        );
                    }
                    
                    self.candidates.insert(key, candidate);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_margin_level_calculation() {
        let user_id = UserId::new();
        let equity = dec!(10000);
        let used_margin = dec!(5000);

        let margin_level = MarginLevel::calculate(user_id, equity, used_margin);

        assert_eq!(margin_level.equity, dec!(10000));
        assert_eq!(margin_level.used_margin, dec!(5000));
        assert_eq!(margin_level.free_margin, dec!(5000));
        assert_eq!(margin_level.margin_level, dec!(200)); // 200%
        assert!(!margin_level.at_risk);
    }

    #[test]
    fn test_margin_level_at_risk() {
        let user_id = UserId::new();
        let equity = dec!(4000);
        let used_margin = dec!(5000);

        let margin_level = MarginLevel::calculate(user_id, equity, used_margin);

        assert_eq!(margin_level.margin_level, dec!(80)); // 80%
        assert!(margin_level.at_risk);
    }

    #[test]
    fn test_margin_config_default() {
        let config = MarginConfig::default();
        assert_eq!(config.initial_margin, dec!(0.10));
        assert_eq!(config.maintenance_margin, dec!(0.05));
        assert_eq!(config.liquidation_margin, dec!(0.03));
    }
}
