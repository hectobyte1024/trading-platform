use api_gateway::{ApiConfig, start_server};
use matching_engine::MatchingEngine;
use event_journal::FileJournal;
use risk_engine::{AdaptiveRiskEngine, RiskLimits};
use std::sync::Arc;
use rust_decimal_macros::dec;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing (logging)
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    println!("🚀 Starting Trading Platform API Gateway...\n");

    // Create event journal (persisted to disk)
    let journal_path = "/tmp/trading-events.jsonl";
    let journal = Arc::new(FileJournal::new(journal_path).await?);
    tracing::info!("Event journal created: {}", journal_path);

    // Create risk engine with limits
    let limits = RiskLimits {
        max_position_size: dec!(1000),
        max_total_exposure: dec!(10000),
        max_open_orders: 100,
        max_order_size: dec!(100),
        min_order_size: dec!(0.00001),
    };
    let risk_engine = Arc::new(AdaptiveRiskEngine::new(limits));
    tracing::info!("Risk engine initialized");

    // Create matching engine
    let engine = Arc::new(MatchingEngine::new(journal, risk_engine.clone()));
    tracing::info!("Matching engine ready");

    // Configure API Gateway
    let config = ApiConfig {
        host: "0.0.0.0".to_string(),
        port: 8080,
        enable_cors: true,
    };

    println!("✅ All components initialized");
    println!("📡 API Gateway starting on http://{}:{}", config.host, config.port);
    println!("🔌 WebSocket endpoint: ws://{}:{}/ws", config.host, config.port);
    println!("🏥 Health check: http://{}:{}/health\n", config.host, config.port);

    // Start the server
    start_server(engine, risk_engine, config).await?;

    Ok(())
}
