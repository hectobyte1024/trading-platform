# API Gateway

Production-ready HTTP/WebSocket API gateway for the institutional trading platform.

## Features

### REST API
- **Order Management**: Place and cancel orders with comprehensive validation
- **Market Data**: Query orderbook depth with configurable levels
- **Account Management**: Register accounts and query positions
- **Health Checks**: Service health monitoring

### WebSocket
- **Real-time Market Data**: Subscribe to orderbook updates
- **Trade Notifications**: Live trade execution events
- **Connection Management**: Automatic ping/pong for connection health
- **Error Handling**: Comprehensive error messages for invalid requests

### Middleware
- **Error Handling**: Automatic conversion of domain errors to HTTP responses
- **Request Logging**: Structured logging with tracing
- **Authentication**: Placeholder for JWT/OAuth2 integration
- **CORS**: Configurable cross-origin resource sharing

## Architecture

```
┌─────────────────────────────────────────────────┐
│              API Gateway                         │
│  ┌──────────────┐    ┌──────────────────────┐  │
│  │  REST API    │    │  WebSocket Handler    │  │
│  │              │    │                       │  │
│  │ - POST /orders    │ - Real-time updates   │  │
│  │ - DELETE /orders  │ - Market data stream  │  │
│  │ - GET /orderbook  │ - Trade notifications │  │
│  └──────┬───────┘    └───────────┬───────────┘  │
│         │                        │               │
│  ┌──────┴────────────────────────┴───────────┐  │
│  │         Middleware Stack                   │  │
│  │  - Error handling                          │  │
│  │  - Request logging                         │  │
│  │  - Authentication (placeholder)            │  │
│  └────────────────┬───────────────────────────┘  │
└───────────────────┼──────────────────────────────┘
                    │
         ┌──────────┴─────────┐
         │                    │
    ┌────┴─────┐       ┌──────┴────────┐
    │ Matching │       │ Risk Engine   │
    │  Engine  │       │               │
    └──────────┘       └───────────────┘
```

## REST API Endpoints

### Health Check
```http
GET /health
```

Response:
```json
{
  "status": "healthy",
  "service": "api-gateway",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

### Place Order
```http
POST /orders
Content-Type: application/json

{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "symbol": "BTC/USD",
  "side": "buy",
  "order_type": "limit",
  "price": "50000",
  "quantity": "1.5",
  "time_in_force": "gtc"
}
```

Response:
```json
{
  "order_id": "123e4567-e89b-12d3-a456-426614174000",
  "status": "submitted",
  "message": "Order placed successfully"
}
```

Error Response:
```json
{
  "error": "Insufficient balance",
  "code": "INSUFFICIENT_BALANCE"
}
```

### Cancel Order
```http
DELETE /orders/:order_id
Content-Type: application/json

{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "symbol": "BTC/USD"
}
```

Response:
```json
{
  "order_id": "123e4567-e89b-12d3-a456-426614174000",
  "status": "cancelled",
  "message": "Order cancelled successfully"
}
```

### Get Orderbook
```http
GET /orderbook/BTC%2FUSD?depth=10
```

Response:
```json
{
  "symbol": "BTC/USD",
  "bids": [
    { "price": "49999", "quantity": "2.5" },
    { "price": "49998", "quantity": "1.0" }
  ],
  "asks": [
    { "price": "50001", "quantity": "3.0" },
    { "price": "50002", "quantity": "0.5" }
  ],
  "best_bid": "49999",
  "best_ask": "50001"
}
```

### Register Account
```http
POST /accounts/register
Content-Type: application/json

{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "initial_balance": "100000"
}
```

Response:
```json
{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "balance": "100000",
  "message": "Account registered successfully"
}
```

### Get Positions
```http
GET /accounts/550e8400-e29b-41d4-a716-446655440000/positions
```

Response:
```json
{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "positions": [
    {
      "symbol": "BTC/USD",
      "net_quantity": "1.5",
      "total_buy": "2.0",
      "total_sell": "0.5",
      "open_orders": 3
    }
  ]
}
```

## WebSocket Protocol

### Connection
```javascript
const ws = new WebSocket('ws://localhost:8080/ws');
```

### Subscribe to Market Data
```json
{
  "type": "subscribe",
  "symbol": "BTC/USD"
}
```

### Unsubscribe
```json
{
  "type": "unsubscribe",
  "symbol": "BTC/USD"
}
```

### Ping/Pong
```json
{
  "type": "ping"
}
```

Response:
```json
{
  "type": "pong"
}
```

### Trade Notification
```json
{
  "type": "trade",
  "symbol": "BTC/USD",
  "price": "50000",
  "quantity": "1.5",
  "side": "buy",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

### Orderbook Update
```json
{
  "type": "orderbook_update",
  "symbol": "BTC/USD",
  "bids": [
    { "price": "49999", "quantity": "2.5" }
  ],
  "asks": [
    { "price": "50001", "quantity": "3.0" }
  ]
}
```

### Error Message
```json
{
  "type": "error",
  "message": "Invalid message format: missing required field"
}
```

## Usage Example

```rust
use api_gateway::{create_router, start_server, ApiConfig};
use matching_engine::MatchingEngine;
use risk_engine::{AdaptiveRiskEngine, RiskLimits};
use event_journal::FileJournal;
use std::sync::Arc;
use rust_decimal_macros::dec;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize components
    let journal = Arc::new(FileJournal::new("/tmp/events.journal").await?);
    
    let limits = RiskLimits {
        max_position_size: dec!(100),
        max_total_exposure: dec!(500),
        max_open_orders: 50,
        max_order_size: dec!(10),
        min_order_size: dec!(0.001),
    };
    let risk_engine = Arc::new(AdaptiveRiskEngine::new(limits));
    
    let engine = Arc::new(MatchingEngine::new(journal, risk_engine.clone()));

    // Configure API gateway
    let config = ApiConfig {
        host: "0.0.0.0".to_string(),
        port: 8080,
        enable_cors: true,
    };

    // Start server
    start_server(engine, risk_engine, config).await?;
    
    Ok(())
}
```

## Configuration

### ApiConfig
- **host**: Bind address (default: "0.0.0.0")
- **port**: Listen port (default: 8080)
- **enable_cors**: Enable CORS middleware (default: true)

## Error Handling

All errors are automatically converted to appropriate HTTP status codes:

| Error Type | Status Code | Description |
|------------|-------------|-------------|
| OrderValidation | 400 Bad Request | Invalid order parameters |
| InvalidPrice/InvalidQuantity | 400 Bad Request | Invalid numeric values |
| AuthenticationError | 401 Unauthorized | Authentication failed |
| RiskCheckFailed | 403 Forbidden | Risk limits exceeded |
| InsufficientBalance | 403 Forbidden | Insufficient funds |
| OrderNotFound | 404 Not Found | Order ID not found |
| SymbolNotFound | 404 Not Found | Symbol not found |
| PositionLimitExceeded | 403 Forbidden | Position limits exceeded |
| DatabaseError | 500 Internal Server Error | Database operation failed |

## Testing

```bash
# Run tests
cargo test -p api-gateway

# Test specific modules
cargo test -p api-gateway rest::tests
cargo test -p api-gateway websocket::tests
cargo test -p api-gateway middleware::tests
```

## Dependencies

- **axum**: Fast, ergonomic HTTP framework
- **tower**: Middleware and service composition
- **tower-http**: HTTP-specific middleware (CORS, tracing)
- **tokio**: Async runtime
- **serde/serde_json**: Serialization
- **futures**: Stream and sink utilities for WebSocket

## Production Considerations

### Authentication
Replace the placeholder `auth_middleware` with production authentication:
- JWT token validation
- OAuth2 integration
- API key management
- Session management with Redis

### Rate Limiting
Add rate limiting middleware:
```rust
use tower::limit::RateLimitLayer;

let rate_limit = RateLimitLayer::new(100, Duration::from_secs(1));
router.layer(rate_limit);
```

### TLS/HTTPS
Use `axum-server` with TLS configuration for production:
```rust
use axum_server::tls_rustls::RustlsConfig;

let tls_config = RustlsConfig::from_pem_file(
    "cert.pem",
    "key.pem"
).await?;

axum_server::bind_rustls(addr, tls_config)
    .serve(router.into_make_service())
    .await?;
```

### Monitoring
- Add Prometheus metrics
- Structured logging with `tracing-subscriber`
- Distributed tracing with OpenTelemetry
- Health check integration with orchestration

### Load Balancing
Deploy multiple instances behind:
- Nginx
- HAProxy
- Cloud load balancer (AWS ALB, GCP Load Balancer)

## Performance

The API gateway is designed for high throughput:
- **Async I/O**: Non-blocking request handling
- **Zero-copy**: Efficient data passing with Arc
- **Connection pooling**: Reusable WebSocket connections
- **Minimal allocations**: Optimized request/response handling

Expected performance (single instance):
- **REST API**: 10K-50K requests/sec
- **WebSocket**: 10K+ concurrent connections
- **Latency**: p99 < 5ms (internal network)

## License

MIT OR Apache-2.0
