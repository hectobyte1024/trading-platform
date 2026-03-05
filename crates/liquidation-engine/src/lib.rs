use common::RiskCheck;
use event_journal::EventJournal;
use liquidator::Liquidator;
use matching_engine::MatchingEngine;
use monitor::PositionMonitor;
use risk_engine::AdaptiveRiskEngine;
use std::sync::Arc;

pub mod monitor;
pub mod liquidator;

pub use monitor::{LiquidationCandidate, LiquidationUrgency, MarginLevel, MarginConfig};
pub use liquidator::{LiquidationResult, LiquidationStrategy, LiquidatorConfig};

/// Complete liquidation engine system
pub struct LiquidationEngine<J: EventJournal, R: RiskCheck> {
    monitor: Arc<PositionMonitor>,
    liquidator: Arc<Liquidator<J, R>>,
}

impl<J: EventJournal + 'static, R: RiskCheck + 'static> LiquidationEngine<J, R> {
    /// Create a new liquidation engine
    pub fn new(
        matching_engine: Arc<MatchingEngine<J, R>>,
        risk_engine: Arc<AdaptiveRiskEngine>,
        margin_config: MarginConfig,
        liquidator_config: LiquidatorConfig,
    ) -> Self {
        let monitor = Arc::new(PositionMonitor::new(
            risk_engine.clone(),
            margin_config,
        ));

        let liquidator = Arc::new(Liquidator::new(
            monitor.clone(),
            matching_engine,
            risk_engine,
            liquidator_config,
        ));

        Self {
            monitor,
            liquidator,
        }
    }

    /// Get the position monitor
    pub fn monitor(&self) -> Arc<PositionMonitor> {
        self.monitor.clone()
    }

    /// Get the liquidator
    pub fn liquidator(&self) -> Arc<Liquidator<J, R>> {
        self.liquidator.clone()
    }

    /// Start both monitoring and liquidation tasks
    pub async fn start(self: Arc<Self>) {
        let monitor = self.monitor.clone();
        let liquidator = self.liquidator.clone();

        // Spawn monitor task
        tokio::spawn(async move {
            monitor.start_monitoring().await;
        });

        // Spawn liquidator task
        tokio::spawn(async move {
            liquidator.start_liquidation_task().await;
        });

        tracing::info!("Liquidation engine started");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use event_journal::FileJournal;
    use risk_engine::RiskLimits;
    use rust_decimal_macros::dec;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_liquidation_engine_creation() {
        let temp_dir = TempDir::new().unwrap();
        let journal_path = temp_dir.path().join("test_liquidation.journal");
        let journal = Arc::new(FileJournal::new(journal_path).await.unwrap());

        let limits = RiskLimits::default();
        let risk_engine = Arc::new(AdaptiveRiskEngine::new(limits));

        let matching_engine = Arc::new(MatchingEngine::new(journal, risk_engine.clone()));

        let margin_config = MarginConfig::default();
        let liquidator_config = LiquidatorConfig::default();

        let _engine = LiquidationEngine::new(
            matching_engine,
            risk_engine,
            margin_config,
            liquidator_config,
        );

        assert!(true); // Engine created successfully
    }

    #[test]
    fn test_margin_config() {
        let config = MarginConfig::default();
        assert_eq!(config.initial_margin, dec!(0.10));
        assert_eq!(config.maintenance_margin, dec!(0.05));
        assert_eq!(config.liquidation_margin, dec!(0.03));
    }

    #[test]
    fn test_liquidator_config() {
        let config = LiquidatorConfig::default();
        assert_eq!(config.strategy, LiquidationStrategy::FullMarket);
        assert_eq!(config.check_interval_secs, 1);
    }
}
