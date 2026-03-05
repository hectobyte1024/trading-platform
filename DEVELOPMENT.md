# Development Guide

## Quick Start

### Prerequisites

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install system dependencies (optional, for Kafka support)
sudo apt install libsasl2-dev  # Debian/Ubuntu
```

### Build the Project

```bash
cd /home/hectobyte1024/Documents/trading-platform
cargo build --release
```

### Run Tests

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p matching-engine
cargo test -p risk-engine
cargo test -p event-journal

# Run with output
cargo test -- --nocapture
```

### Run Benchmarks

```bash
# Run matching engine benchmarks
cargo bench -p matching-engine

# View benchmark results
firefox target/criterion/report/index.html
```

## Development Workflow

### Adding a New Feature

1. Create a new module in the appropriate crate
2. Add tests in a `#[cfg(test)] mod tests` block
3. Run `cargo test` to verify
4. Add benchmarks if performance-critical
5. Update documentation

### Code Quality Checks

```bash
# Check for compilation errors
cargo check

# Run clippy (linter)
cargo clippy --all-targets --all-features

# Format code
cargo fmt --all

# Check for unused dependencies
cargo machete
```

## Testing Strategy

### Unit Tests

Each module contains comprehensive unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        // Test implementation
    }

    #[tokio::test]
    async fn test_async_something() {
        // Async test implementation
    }
}
```

### Integration Tests

Integration tests go in `tests/` directory:

```bash
crates/matching-engine/tests/integration_test.rs
```

### Benchmarks

Performance-critical code should have benchmarks:

```bash
crates/matching-engine/benches/orderbook_bench.rs
```

Run with:
```bash
cargo bench -p matching-engine
```

## Architecture Principles

### Event Sourcing

All state changes flow through events:

1. Command arrives (e.g., PlaceOrder)
2. Validation and risk checks
3. Event emitted (e.g., OrderPlaced)
4. Event persisted to journal
5. State updated from event
6. Event published to Kafka (in production)

This ensures:
- Complete audit trail
- Deterministic replay
- Easy debugging
- Regulatory compliance

### Determinism

The matching engine is fully deterministic:

- Same events → same state
- No random numbers
- No timestamps in matching logic (only in events)
- No threading within matching logic

This enables:
- Replay from event log
- Time-travel debugging
- Exact state recovery

### Concurrency

- DashMap for lock-free concurrent access to orderbooks by symbol
- RwLock for reader/writer access to individual orderbooks
- Arc for shared ownership
- No unsafe code in application logic

## Performance Tips

### Matching Engine

- Orders are matched in O(log n) time for price level lookup
- Time priority within a price level is O(1) with IndexMap
- Market depth queries are O(k) where k is depth

### Event Journal

- File journal uses newline-delimited JSON for simplicity
- For production, use Kafka-backed journal for:
  - Distributed replication
  - Higher throughput
  - Better durability

### Risk Engine

- Position tracking uses DashMap for concurrent access
- Balance checks are in-memory
- For production, integrate with ledger database

## Troubleshooting

### Kafka Compilation Errors

If you see `sasl2-sys` errors:

```bash
# Install system dependency
sudo apt install libsasl2-dev

# Then uncomment kafka-adapter in Cargo.toml
```

### Test Failures

```bash
# Run single test with output
cargo test test_name -- --nocapture --test-threads=1

# Run tests in specific module
cargo test orderbook::tests
```

### Performance Issues

```bash
# Profile with criterion
cargo bench -p matching-engine

# Profile with perf
cargo build --release
perf record --call-graph=dwarf ./target/release/your-binary
perf report
```

## Production Deployment

### Database Setup

```sql
-- PostgreSQL schema for ledger
CREATE TABLE accounts (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    currency VARCHAR(10) NOT NULL,
    balance DECIMAL(20, 8) NOT NULL
);

CREATE TABLE transactions (
    id UUID PRIMARY KEY,
    debit_account UUID NOT NULL,
    credit_account UUID NOT NULL,
    amount DECIMAL(20, 8) NOT NULL,
    currency VARCHAR(10) NOT NULL,
    reference VARCHAR(255),
    timestamp TIMESTAMPTZ NOT NULL
);
```

### Kafka Setup

```bash
# Create topics
kafka-topics.sh --create --topic matching-events \
    --partitions 10 --replication-factor 3

kafka-topics.sh --create --topic trade-events \
    --partitions 10 --replication-factor 3
```

### Kubernetes Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: matching-engine
spec:
  replicas: 3
  selector:
    matchLabels:
      app: matching-engine
  template:
    metadata:
      labels:
        app: matching-engine
    spec:
      containers:
      - name: matching-engine
        image: trading-platform/matching-engine:latest
        resources:
          requests:
            memory: "2Gi"
            cpu: "1000m"
          limits:
            memory: "4Gi"
            cpu: "2000m"
```

## Next Implementation Steps

1. **Kafka Adapter** (crates/kafka-adapter)
   - Implement KafkaJournal trait
   - Event publisher/subscriber
   - Integration with matching engine

2. **PostgreSQL Ledger** (crates/ledger)
   - Double-entry transaction recording
   - Balance management
   - Account creation/management

3. **API Gateway** (crates/api-gateway)
   - REST endpoints for order placement
   - WebSocket for real-time market data
   - Authentication middleware

4. **Liquidation Engine** (crates/liquidation-engine)
   - Position monitoring
   - Margin call detection
   - Automated liquidation

5. **Auth Service** (crates/auth-service)
   - JWT generation and validation
   - Hardware-backed signing
   - User session management

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Event Sourcing](https://martinfowler.com/eaaDev/EventSourcing.html)
- [Trading System Architecture](https://www.youtube.com/watch?v=b1e4t2k2KJY)

## Getting Help

- Check existing tests for examples
- Read module documentation
- Review the architecture diagram in README.md
- Open an issue for bugs or questions
