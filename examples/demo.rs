use matching_engine::MatchingEngine;
use event_journal::FileJournal;
use risk_engine::{AdaptiveRiskEngine, RiskLimits};
use common::*;
use std::sync::Arc;
use rust_decimal_macros::dec;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing (logging)
    tracing_subscriber::fmt::init();

    println!("🚀 Institutional Trading Platform - Demo\n");

    // 1. Create event journal (persisted to disk)
    let journal_path = "/tmp/trading-events.jsonl";
    let journal = Arc::new(FileJournal::new(journal_path).await?);
    println!("✅ Event journal created: {}", journal_path);

    // 2. Create risk engine with limits
    let limits = RiskLimits {
        max_position_size: dec!(100),
        max_total_exposure: dec!(500),
        max_open_orders: 50,
        max_order_size: dec!(10),
        min_order_size: dec!(0.001),
    };
    let risk_engine = Arc::new(AdaptiveRiskEngine::new(limits.clone()));
    println!("✅ Risk engine initialized");

    // 3. Create matching engine
    let engine = MatchingEngine::new(journal.clone(), risk_engine.clone());
    println!("✅ Matching engine ready\n");

    // 4. Register user accounts
    let alice = UserId::new();
    let bob = UserId::new();
    
    risk_engine.register_account(alice, dec!(200000)); // $200,000
    risk_engine.register_account(bob, dec!(200000));   // $200,000
    
    println!("👤 Alice registered with $200,000");
    println!("👤 Bob registered with $200,000\n");

    // 5. Alice places a SELL order for 2 BTC @ $50,000
    println!("📤 Alice: SELL 2 BTC @ $50,000");
    let sell_order = Order {
        id: OrderId::new(),
        user_id: alice,
        symbol: Symbol::new("BTC/USD"),
        side: Side::Sell,
        order_type: OrderType::Limit,
        price: Price::new(dec!(50000)),
        quantity: Quantity::new(dec!(2.0)),
        filled_quantity: Quantity::zero(),
        time_in_force: TimeInForce::GTC,
        status: OrderStatus::Pending,
        timestamp: chrono::Utc::now(),
        sequence_number: 0,
    };
    
    engine.place_order(sell_order).await?;
    
    // 6. Bob places a BUY order for 1.5 BTC @ $50,000 (should match!)
    println!("📥 Bob: BUY 1.5 BTC @ $50,000");
    let buy_order = Order {
        id: OrderId::new(),
        user_id: bob,
        symbol: Symbol::new("BTC/USD"),
        side: Side::Buy,
        order_type: OrderType::Limit,
        price: Price::new(dec!(50000)),
        quantity: Quantity::new(dec!(1.5)),
        filled_quantity: Quantity::zero(),
        time_in_force: TimeInForce::GTC,
        status: OrderStatus::Pending,
        timestamp: chrono::Utc::now(),
        sequence_number: 0,
    };
    
    engine.place_order(buy_order).await?;
    
    println!("\n💱 Trade executed: 1.5 BTC @ $50,000");
    println!("   Buyer: Bob");
    println!("   Seller: Alice");
    println!("   Value: $75,000");
    
    // 7. Check orderbook state
    let symbol = Symbol::new("BTC/USD");
    if let Some(orderbook) = engine.get_orderbook(&symbol).await {
        let book = orderbook.read().await;
        
        println!("\n📊 Orderbook State:");
        println!("   Best Bid: {:?}", book.best_bid());
        println!("   Best Ask: {:?}", book.best_ask());
        println!("   Active Orders: {}", book.order_count());
        
        let ask_depth = book.ask_depth(5);
        if !ask_depth.is_empty() {
            println!("\n   Ask Side (remaining 0.5 BTC @ $50,000):");
            for (price, qty) in ask_depth {
                println!("      {} @ {}", qty, price);
            }
        }
    }
    
    // 8. Flush events to disk
    engine.flush().await?;
    println!("\n💾 All events persisted to journal");
    
    // 9. Demonstrate replay capability
    println!("\n🔄 Testing replay from journal...");
    let new_engine = MatchingEngine::new_with_replay(
        journal,
        Arc::new(AdaptiveRiskEngine::new(limits.clone()))
    ).await?;
    
    println!("✅ State successfully restored from events!");
    
    // Verify restored state
    if let Some(orderbook) = new_engine.get_orderbook(&symbol).await {
        let book = orderbook.read().await;
        println!("   Restored orderbook has {} active orders", book.order_count());
    }
    
    println!("\n✨ Demo complete! Check {} for event log", journal_path);
    
    Ok(())
}
