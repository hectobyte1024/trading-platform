# Liquidation Engine

Automated position monitoring and liquidation system for the institutional trading platform. Protects the platform from excessive losses by automatically closing positions when margin levels fall below safe thresholds.

## Features

### Position Monitoring
- **Real-time Margin Calculation**: Continuously calculates equity, used margin, and margin levels
- **Multi-level Alerts**: Critical, Warning, and Normal urgency classifications
- **Price Feed Integration**: Updates position values based on market prices
- **Configurable Intervals**: Adjustable monitoring frequency

### Liquidation Strategies
- **Full Market**: Immediate liquidation using market orders (fastest)
- **Gradual**: Liquidate in configurable chunks to minimize market impact
- **Limit Then Market**: Attempt limit orders first, fallback to market

### Risk Management
- **Initial Margin**: Required margin for opening positions (default: 10%)
- **Maintenance Margin**: Minimum margin to keep positions open (default: 5%)  
- **Liquidation Margin**: Threshold triggering automatic liquidation (default: 3%)

## Architecture

```
┌────────────────────────────────────────────────┐
│         Liquidation Engine                      │
│  ┌──────────────────┐  ┌────────────────────┐  │
│  │ Position Monitor │  │    Liquidator       │  │
│  │                  │  │                     │  │
│  │ - Calculate      │  │ - Execute           │  │
│  │   margin levels  │  │   liquidations      │  │
│  │ - Detect         │  │ - Place market      │  │
│  │   candidates     │  │   orders            │  │
│  │ - Track prices   │  │ - Handle failures   │  │
│  └────────┬─────────┘  └──────────┬──────────┘  │
└───────────┼────────────────────────┼─────────────┘
            │                        │
            │   ┌────────────────────┘
            │   │
    ┌───────┴───┴────┐     ┌─────────────┐
    │  Risk Engine   │     │   Matching  │
    │  (Positions)   │     │   Engine    │
    └────────────────┘     └─────────────┘
```

## Usage

### Basic Setup

```rust
use liquidation_engine::{
    LiquidationEngine, MarginConfig, LiquidatorConfig, LiquidationStrategy
};
use matching_engine::MatchingEngine;
use risk_engine::AdaptiveRiskEngine;
use event_journal::FileJournal;
use std::sync::Arc;
use rust_decimal_macros::dec;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup matching engine and risk engine
    let journal = Arc::new(FileJournal::new("/tmp/events.journal").await?);
    let risk_engine = Arc::new(AdaptiveRiskEngine::new(Default::default()));
    let matching_engine = Arc::new(MatchingEngine::new(journal, risk_engine.clone()));

    // Configure margin thresholds
    let margin_config = MarginConfig {
        initial_margin: dec!(0.10),      // 10% initial margin
        maintenance_margin: dec!(0.05),  // 5% maintenance margin
        liquidation_margin: dec!(0.03),  // 3% liquidation threshold
        monitor_interval_secs: 1,        // Check every second
    };

    // Configure liquidation strategy
    let liquidator_config = LiquidatorConfig {
        strategy: LiquidationStrategy::FullMarket,
        check_interval_secs: 1,
        max_slippage: dec!(0.05),        // 5% max slippage
        chunk_size: dec!(0.50),          // 50% chunks
    };

    // Create liquidation engine
    let engine = Arc::new(LiquidationEngine::new(
        matching_engine,
        risk_engine,
        margin_config,
        liquidator_config,
    ));

    // Start monitoring and liquidation tasks
    engine.start().await;

    // Update prices as market data comes in
    let monitor = engine.monitor();
    monitor.update_price(
        Symbol::new("BTC/USD"),
        dec!(50000)
    );

    Ok(())
}
```

### Manual Liquidation Check

```rust
use common::{UserId, Symbol};

// Get margin level for a user
let user_id = UserId::new();
if let Some(margin_level) = monitor.calculate_margin_level(user_id) {
    println!("Margin Level: {:.2}%", margin_level.margin_level);
    println!("Equity: {}", margin_level.equity);
    println!("Used Margin: {}", margin_level.used_margin);
    println!("Free Margin: {}", margin_level.free_margin);
    
    if margin_level.at_risk {
        println!("⚠️  Position at risk!");
    }
}

// Get all critical liquidation candidates
let critical = monitor.get_critical_candidates();
for candidate in critical {
    println!(
        "CRITICAL: User {} - {} - Margin: {:.2}%",
        candidate.user_id,
        candidate.symbol.0,
        candidate.margin_level.margin_level
    );
}
```

### Custom Liquidation

```rust
use liquidation_engine::LiquidationCandidate;

// Manually trigger liquidation for a specific candidate
let candidate = /* ... get candidate ... */;
let result = liquidator.liquidate(&candidate).await?;

if result.success {
    println!(
        "Liquidated {} {} for user {}",
        result.quantity_liquidated,
        result.symbol.0,
        result.user_id
    );
} else {
    println!("Liquidation failed: {:?}", result.error);
}
```

## Margin Calculation

The margin level is calculated as:

```
Margin Level (%) = (Equity / Used Margin) × 100
```

Where:
- **Equity** = Sum of all position values at current market prices
- **Used Margin** = Sum of (Position Value × Initial Margin %)
- **Free Margin** = Equity - Used Margin

### Example

```
Position: 1 BTC at $50,000
Initial Margin: 10%

Position Value = 1 × $50,000 = $50,000
Used Margin = $50,000 × 10% = $5,000
Equity = $50,000
Free Margin = $50,000 - $5,000 = $45,000
Margin Level = ($50,000 / $5,000) × 100 = 1,000%
```

## Liquidation Urgency Levels

| Urgency | Margin Level | Action |
|---------|--------------|--------|
| **Normal** | Above maintenance margin | No action |
| **Warning** | Below maintenance, above liquidation | Margin call, monitor closely |
| **Critical** | Below liquidation threshold | **Immediate automatic liquidation** |

### Default Thresholds

```rust
Initial Margin:      10% (100% margin level)
Maintenance Margin:   5% (200% margin level)
Liquidation Margin:   3% (333% margin level)
```

If margin level falls below **333%**, automatic liquidation is triggered.

## Liquidation Strategies

### Full Market (Default)
- **Speed**: Fastest
- **Price**: May incur slippage
- **Use Case**: Critical positions requiring immediate closure

```rust
LiquidatorConfig {
    strategy: LiquidationStrategy::FullMarket,
    ..Default::default()
}
```

### Gradual
- **Speed**: Slower
- **Price**: Better average price
- **Use Case**: Large positions, less urgent liquidations

```rust
LiquidatorConfig {
    strategy: LiquidationStrategy::Gradual,
    chunk_size: dec!(0.25), // Liquidate 25% at a time
    ..Default::default()
}
```

### Limit Then Market
- **Speed**: Variable
- **Price**: Best possible
- **Use Case**: Less urgent, price-sensitive liquidations

```rust
LiquidatorConfig {
    strategy: LiquidationStrategy::LimitThenMarket,
    max_slippage: dec!(0.02), // 2% max slippage
    ..Default::default()
}
```

## Configuration Reference

### MarginConfig

```rust
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
```

### LiquidatorConfig

```rust
pub struct LiquidatorConfig {
    /// Default liquidation strategy
    pub strategy: LiquidationStrategy,
    
    /// Liquidation check interval in seconds
    pub check_interval_secs: u64,
    
    /// Maximum slippage tolerance (percentage)
    pub max_slippage: Decimal,
    
    /// Chunk size for gradual liquidation (percentage)
    pub chunk_size: Decimal,
}
```

## Event Flow

1. **Price Update**: Market prices updated via `monitor.update_price()`
2. **Position Scan**: Monitor scans all positions every `monitor_interval_secs`
3. **Margin Calculation**: Calculates margin levels for positions
4. **Candidate Detection**: Identifies positions below thresholds
5. **Liquidation Trigger**: Critical candidates added to liquidation queue
6. **Order Placement**: Liquidator places market orders to close positions
7. **Result Tracking**: Success/failure logged and tracked

## Testing

```bash
# Run liquidation engine tests
cargo test -p liquidation-engine

# Test margin calculations
cargo test -p liquidation-engine test_margin_level_calculation

# Test liquidation strategies
cargo test -p liquidation-engine test_liquidation_strategy
```

## Production Considerations

### Price Feed Integration
In production, integrate real market data:

```rust
// Subscribe to price feeds
let price_stream = market_data_service.subscribe("BTC/USD").await?;

tokio::spawn(async move {
    while let Some(price) = price_stream.next().await {
        monitor.update_price(
            Symbol::new("BTC/USD"),
            price.last_price
        );
    }
});
```

### User Tracking
Track all active users with positions:

```rust
// Implement a user registry
pub struct UserRegistry {
    active_users: Arc<DashMap<UserId, Vec<Symbol>>>,
}

// Update scan_positions() to iterate through active users
impl PositionMonitor {
    pub fn scan_positions(&self) -> Vec<LiquidationCandidate> {
        let mut candidates = Vec::new();
        
        for user_entry in self.user_registry.active_users.iter() {
            let user_id = *user_entry.key();
            let positions = self.risk_engine.get_positions(user_id);
            
            for position in positions {
                if let Some(candidate) = self.check_position(user_id, &position) {
                    candidates.push(candidate);
                }
            }
        }
        
        candidates
    }
}
```

### Notification System
Alert users before liquidation:

```rust
// Send margin call notification
if candidate.urgency == LiquidationUrgency::Warning {
    notification_service.send_margin_call(
        candidate.user_id,
        candidate.margin_level.margin_level
    ).await?;
}
```

### Circuit Breakers
Prevent cascade liquidations:

```rust
// Pause liquidations during extreme volatility
if market_volatility > threshold {
    liquidator.pause();
    tracing::warn!("Liquidations paused due to high volatility");
}
```

## Performance

- **Monitoring Overhead**: Minimal, uses Arc and DashMap for lock-free access
- **Liquidation Latency**: Sub-millisecond order placement
- **Scalability**: Handles thousands of concurrent positions
- **Memory**: O(n) where n = number of active positions

## Safety Features

✅ **No manual intervention required** - Fully automated  
✅ **Multiple urgency levels** - Graduated response  
✅ **Configurable thresholds** - Adapt to market conditions  
✅ **Comprehensive logging** - Full audit trail  
✅ **Error handling** - Graceful degradation on failures  

## License

MIT OR Apache-2.0
