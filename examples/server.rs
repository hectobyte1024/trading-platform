use api_gateway::{ApiConfig, start_server};
use matching_engine::MatchingEngine;
use event_journal::FileJournal;
use risk_engine::{AdaptiveRiskEngine, RiskLimits};
use common::{Order, OrderId, UserId, Symbol, Side, OrderType, Price, Quantity, TimeInForce, OrderStatus};
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

    // Seed orderbook with liquidity
    println!("\n💧 Seeding orderbook with initial liquidity...");
    seed_orderbook(engine.clone(), risk_engine.clone()).await?;
    println!("✅ Orderbook seeded\n");

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

/// Seed the orderbook with initial liquidity
async fn seed_orderbook(
    engine: Arc<MatchingEngine<impl event_journal::EventJournal, impl common::RiskCheck>>,
    risk_engine: Arc<AdaptiveRiskEngine>,
) -> anyhow::Result<()> {
    // Create market maker accounts
    let mm1 = UserId::new();
    let mm2 = UserId::new();
    let mm3 = UserId::new();
    
    // Register with large balances
    risk_engine.register_account(mm1, dec!(1000000));
    risk_engine.register_account(mm2, dec!(1000000));
    risk_engine.register_account(mm3, dec!(1000000));
    
    let symbol = Symbol::new("BTC-USD");
    
    // Place BUY orders (bids) - creating support levels
    let buy_orders = vec![
        (mm1, dec!(49000), dec!(0.5)),  // Best bid
        (mm2, dec!(48900), dec!(1.0)),
        (mm1, dec!(48800), dec!(0.75)),
        (mm3, dec!(48700), dec!(1.5)),
        (mm2, dec!(48600), dec!(2.0)),
        (mm1, dec!(48500), dec!(1.0)),
        (mm3, dec!(48400), dec!(0.5)),
        (mm2, dec!(48300), dec!(1.25)),
    ];
    
    // Place SELL orders (asks) - creating resistance levels
    let sell_orders = vec![
        (mm1, dec!(50000), dec!(0.5)),  // Best ask
        (mm2, dec!(50100), dec!(1.0)),
        (mm3, dec!(50200), dec!(0.75)),
        (mm1, dec!(50300), dec!(1.5)),
        (mm2, dec!(50400), dec!(2.0)),
        (mm3, dec!(50500), dec!(1.0)),
        (mm1, dec!(50600), dec!(0.5)),
        (mm2, dec!(50700), dec!(1.25)),
    ];
    
    println!("   Placing buy orders (support)...");
    for (user_id, price, quantity) in buy_orders {
        let order = Order {
            id: OrderId::new(),
            user_id,
            symbol: symbol.clone(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            price: Price::new(price),
            quantity: Quantity::new(quantity),
            filled_quantity: Quantity::zero(),
            time_in_force: TimeInForce::GTC,
            status: OrderStatus::Pending,
            timestamp: chrono::Utc::now(),
            sequence_number: 0,
        };
        
        engine.place_order(order).await?;
    }
    
    println!("   Placing sell orders (resistance)...");
    for (user_id, price, quantity) in sell_orders {
        let order = Order {
            id: OrderId::new(),
            user_id,
            symbol: symbol.clone(),
            side: Side::Sell,
            order_type: OrderType::Limit,
            price: Price::new(price),
            quantity: Quantity::new(quantity),
            filled_quantity: Quantity::zero(),
            time_in_force: TimeInForce::GTC,
            status: OrderStatus::Pending,
            timestamp: chrono::Utc::now(),
            sequence_number: 0,
        };
        
        engine.place_order(order).await?;
    }
    
    println!("   📈 Spread: $49,000 (bid) - $50,000 (ask) = $1,000");
    println!("   📊 Total: {} buy orders, {} sell orders", 8, 8);
    
    Ok(())
}
