use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use matching_engine::{MatchingEngine, OrderBook};
use common::{Order, OrderId, OrderStatus, OrderType, Price, Quantity, Side, Symbol, TimeInForce, UserId};
use event_journal::InMemoryJournal;
use chrono::Utc;
use rust_decimal_macros::dec;
use std::sync::Arc;
use async_trait::async_trait;
use common::{RiskCheck, Result, Trade};

// Mock risk check for benchmarking
struct NoOpRiskCheck;

#[async_trait]
impl RiskCheck for NoOpRiskCheck {
    async fn check_order(&self, _: &Order) -> Result<()> { Ok(()) }
    async fn check_trade(&self, _: &Trade) -> Result<()> { Ok(()) }
    async fn on_trade_executed(&self, _: &Trade) -> Result<()> { Ok(()) }
    async fn on_order_cancelled(&self, _: &Order) -> Result<()> { Ok(()) }
}

fn create_order(side: Side, price: rust_decimal::Decimal, qty: rust_decimal::Decimal, seq: u64) -> Order {
    Order {
        id: OrderId::new(),
        user_id: UserId::new(),
        symbol: Symbol::new("BTC/USD"),
        side,
        order_type: OrderType::Limit,
        price: Price::new(price),
        quantity: Quantity::new(qty),
        filled_quantity: Quantity::zero(),
        time_in_force: TimeInForce::GTC,
        status: OrderStatus::Open,
        timestamp: Utc::now(),
        sequence_number: seq,
    }
}

fn benchmark_orderbook_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("orderbook_add");
    
    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let mut book = OrderBook::new(Symbol::new("BTC/USD"));
                for i in 0..size {
                    let order = create_order(
                        Side::Buy,
                        dec!(50000) + rust_decimal::Decimal::from(i),
                        dec!(1.0),
                        i as u64,
                    );
                    book.add_order(order);
                }
                black_box(book);
            });
        });
    }
    group.finish();
}

fn benchmark_orderbook_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("orderbook_matching");
    
    // Benchmark matching against different orderbook depths
    for depth in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(depth), depth, |b, &depth| {
            b.iter(|| {
                let mut book = OrderBook::new(Symbol::new("BTC/USD"));
                
                // Build orderbook with sell orders
                for i in 0..depth {
                    let order = create_order(
                        Side::Sell,
                        dec!(50000) + rust_decimal::Decimal::from(i),
                        dec!(1.0),
                        i as u64,
                    );
                    book.add_order(order);
                }
                
                // Match with a buy order
                let buy_order = create_order(Side::Buy, dec!(51000), dec!(1.0), depth as u64);
                let result = book.match_order(buy_order, depth as u64, Utc::now());
                black_box(result);
            });
        });
    }
    group.finish();
}

fn benchmark_price_time_priority(c: &mut Criterion) {
    let mut group = c.benchmark_group("price_time_priority");
    
    // Benchmark matching when there are many orders at the same price
    for orders_at_price in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::from_parameter(orders_at_price),
            orders_at_price,
            |b, &orders_at_price| {
                b.iter(|| {
                    let mut book = OrderBook::new(Symbol::new("BTC/USD"));
                    
                    // Add many orders at the same price
                    for i in 0..orders_at_price {
                        let order = create_order(Side::Sell, dec!(50000), dec!(0.1), i as u64);
                        book.add_order(order);
                    }
                    
                    // Match with buy order that will consume multiple orders
                    let buy_order = create_order(Side::Buy, dec!(50000), dec!(5.0), orders_at_price as u64);
                    let result = book.match_order(buy_order, orders_at_price as u64, Utc::now());
                    black_box(result);
                });
            },
        );
    }
    group.finish();
}

fn benchmark_concurrent_orderbooks(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("concurrent_place_orders", |b| {
        b.to_async(&rt).iter(|| async {
            let journal = Arc::new(InMemoryJournal::new());
            let risk_check = Arc::new(NoOpRiskCheck);
            let engine = MatchingEngine::new(journal, risk_check);
            
            // Simulate concurrent order placement from multiple users
            let mut handles = vec![];
            
            for i in 0..10 {
                let engine = engine.clone();
                let handle = tokio::spawn(async move {
                    for j in 0..10 {
                        let order = create_order(
                            if j % 2 == 0 { Side::Buy } else { Side::Sell },
                            dec!(50000) + rust_decimal::Decimal::from(i * 10 + j),
                            dec!(1.0),
                            (i * 10 + j) as u64,
                        );
                        engine.place_order(order).await.unwrap();
                    }
                });
                handles.push(handle);
            }
            
            for handle in handles {
                handle.await.unwrap();
            }
            
            black_box(engine);
        });
    });
}

fn benchmark_replay(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("replay");
    
    for event_count in [100, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*event_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(event_count),
            event_count,
            |b, &event_count| {
                // Pre-populate journal
                let journal = rt.block_on(async {
                    let journal = Arc::new(InMemoryJournal::new());
                    let risk_check = Arc::new(NoOpRiskCheck);
                    let engine = MatchingEngine::new(journal.clone(), risk_check);
                    
                    for i in 0..event_count {
                        let order = create_order(
                            if i % 2 == 0 { Side::Buy } else { Side::Sell },
                            dec!(50000) + rust_decimal::Decimal::from(i),
                            dec!(1.0),
                            i as u64,
                        );
                        engine.place_order(order).await.unwrap();
                    }
                    
                    journal
                });
                
                b.to_async(&rt).iter(|| async {
                    let risk_check = Arc::new(NoOpRiskCheck);
                    let engine = MatchingEngine::new_with_replay(journal.clone(), risk_check)
                        .await
                        .unwrap();
                    black_box(engine);
                });
            },
        );
    }
    group.finish();
}

fn benchmark_market_depth(c: &mut Criterion) {
    c.bench_function("market_depth_query", |b| {
        let mut book = OrderBook::new(Symbol::new("BTC/USD"));
        
        // Build a deep orderbook
        for i in 0..1000 {
            let buy_order = create_order(
                Side::Buy,
                dec!(50000) - rust_decimal::Decimal::from(i),
                dec!(1.0),
                i as u64,
            );
            let sell_order = create_order(
                Side::Sell,
                dec!(50000) + rust_decimal::Decimal::from(i),
                dec!(1.0),
                (i + 1000) as u64,
            );
            book.add_order(buy_order);
            book.add_order(sell_order);
        }
        
        b.iter(|| {
            let bid_depth = book.bid_depth(10);
            let ask_depth = book.ask_depth(10);
            black_box((bid_depth, ask_depth));
        });
    });
}

criterion_group!(
    benches,
    benchmark_orderbook_add,
    benchmark_orderbook_matching,
    benchmark_price_time_priority,
    benchmark_concurrent_orderbooks,
    benchmark_replay,
    benchmark_market_depth,
);

criterion_main!(benches);
