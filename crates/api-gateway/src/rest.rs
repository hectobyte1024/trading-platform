use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use common::*;
use matching_engine::MatchingEngine;
use risk_engine::AdaptiveRiskEngine;
use event_journal::EventJournal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Shared application state
pub struct AppState<J: EventJournal, R: RiskCheck> {
    pub engine: Arc<MatchingEngine<J, R>>,
    pub risk_engine: Arc<AdaptiveRiskEngine>,
}

impl<J: EventJournal, R: RiskCheck> Clone for AppState<J, R> {
    fn clone(&self) -> Self {
        Self {
            engine: self.engine.clone(),
            risk_engine: self.risk_engine.clone(),
        }
    }
}

/// Request to place a new order
#[derive(Debug, Deserialize)]
pub struct PlaceOrderRequest {
    pub user_id: String,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub price: String,
    pub quantity: String,
    pub time_in_force: Option<String>,
}

/// Response after placing an order
#[derive(Debug, Serialize)]
pub struct PlaceOrderResponse {
    pub order_id: String,
    pub status: String,
    pub message: String,
}

/// Request to cancel an order
#[derive(Debug, Deserialize)]
pub struct CancelOrderRequest {
    pub user_id: String,
    pub symbol: String,
}

/// Response for orderbook depth
#[derive(Debug, Serialize)]
pub struct OrderbookResponse {
    pub symbol: String,
    pub bids: Vec<PriceLevel>,
    pub asks: Vec<PriceLevel>,
    pub best_bid: Option<String>,
    pub best_ask: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PriceLevel {
    pub price: String,
    pub quantity: String,
}

/// Query parameters for orderbook depth
#[derive(Debug, Deserialize)]
pub struct DepthQuery {
    #[serde(default = "default_depth")]
    pub depth: usize,
}

fn default_depth() -> usize {
    10
}

/// Health check endpoint
pub async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "api-gateway",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Place a new order
pub async fn place_order<J: EventJournal, R: RiskCheck>(
    State(state): State<AppState<J, R>>,
    Json(req): Json<PlaceOrderRequest>,
) -> std::result::Result<Json<PlaceOrderResponse>, (StatusCode, String)> {
    // Parse request
    let user_id = UserId(uuid::Uuid::parse_str(&req.user_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID".to_string()))?);

    let symbol = Symbol::new(&req.symbol);

    let side = match req.side.to_lowercase().as_str() {
        "buy" => Side::Buy,
        "sell" => Side::Sell,
        _ => return Err((StatusCode::BAD_REQUEST, "Invalid side (use 'buy' or 'sell')".to_string())),
    };

    let order_type = match req.order_type.to_lowercase().as_str() {
        "limit" => OrderType::Limit,
        "market" => OrderType::Market,
        _ => return Err((StatusCode::BAD_REQUEST, "Invalid order type (use 'limit' or 'market')".to_string())),
    };

    let price = req.price.parse::<rust_decimal::Decimal>()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid price".to_string()))?;
    let price = Price::new(price);

    let quantity = req.quantity.parse::<rust_decimal::Decimal>()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid quantity".to_string()))?;
    let quantity = Quantity::new(quantity);

    let time_in_force = match req.time_in_force.as_deref() {
        Some("gtc") | None => TimeInForce::GTC,
        Some("ioc") => TimeInForce::IOC,
        Some("fok") => TimeInForce::FOK,
        _ => return Err((StatusCode::BAD_REQUEST, "Invalid time_in_force".to_string())),
    };

    // Create order
    let order = Order {
        id: OrderId::new(),
        user_id,
        symbol,
        side,
        order_type,
        price,
        quantity,
        filled_quantity: Quantity::zero(),
        time_in_force,
        status: OrderStatus::Pending,
        timestamp: chrono::Utc::now(),
        sequence_number: 0,
    };

    let order_id = order.id;

    // Place order in matching engine
    state.engine.place_order(order).await
        .map_err(|e| match e {
            TradingError::RiskCheckFailed(msg) => (StatusCode::FORBIDDEN, msg),
            TradingError::InsufficientBalance { .. } => (StatusCode::FORBIDDEN, "Insufficient balance".to_string()),
            TradingError::OrderValidation(msg) => (StatusCode::BAD_REQUEST, msg),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".to_string()),
        })?;

    Ok(Json(PlaceOrderResponse {
        order_id: order_id.to_string(),
        status: "submitted".to_string(),
        message: "Order placed successfully".to_string(),
    }))
}

/// Cancel an existing order
pub async fn cancel_order<J: EventJournal, R: RiskCheck>(
    State(state): State<AppState<J, R>>,
    Path(order_id): Path<String>,
    Json(req): Json<CancelOrderRequest>,
) -> std::result::Result<Json<serde_json::Value>, (StatusCode, String)> {
    let order_id = OrderId(uuid::Uuid::parse_str(&order_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid order ID".to_string()))?);    

    let user_id = UserId(uuid::Uuid::parse_str(&req.user_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID".to_string()))?);

    let symbol = Symbol::new(&req.symbol);

    state.engine.cancel_order(order_id, user_id, symbol).await
        .map_err(|e| match e {
            TradingError::OrderNotFound(msg) => (StatusCode::NOT_FOUND, msg),
            TradingError::AuthenticationError(msg) => (StatusCode::UNAUTHORIZED, msg),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".to_string()),
        })?;

    Ok(Json(serde_json::json!({
        "order_id": order_id.to_string(),
        "status": "cancelled",
        "message": "Order cancelled successfully",
    })))
}

/// Get orderbook depth for a symbol
pub async fn get_orderbook<J: EventJournal, R: RiskCheck>(
    State(state): State<AppState<J, R>>,
    Path(symbol): Path<String>,
    Query(query): Query<DepthQuery>,
) -> std::result::Result<Json<OrderbookResponse>, (StatusCode, String)> {
    let symbol = Symbol::new(&symbol);

    let orderbook = state.engine.get_orderbook(&symbol).await
        .ok_or((StatusCode::NOT_FOUND, "Orderbook not found".to_string()))?;

    let book = orderbook.read().await;

    let bids: Vec<PriceLevel> = book
        .bid_depth(query.depth)
        .into_iter()
        .map(|(price, qty)| PriceLevel {
            price: price.to_string(),
            quantity: qty.to_string(),
        })
        .collect();

    let asks: Vec<PriceLevel> = book
        .ask_depth(query.depth)
        .into_iter()
        .map(|(price, qty)| PriceLevel {
            price: price.to_string(),
            quantity: qty.to_string(),
        })
        .collect();

    let best_bid = book.best_bid().map(|p| p.to_string());
    let best_ask = book.best_ask().map(|p| p.to_string());

    Ok(Json(OrderbookResponse {
        symbol: symbol.0,
        bids,
        asks,
        best_bid,
        best_ask,
    }))
}

/// Register a new user account with initial balance
#[derive(Debug, Deserialize)]
pub struct RegisterAccountRequest {
    pub user_id: String,
    pub initial_balance: String,
}

pub async fn register_account<J: EventJournal, R: RiskCheck>(
    State(state): State<AppState<J, R>>,
    Json(req): Json<RegisterAccountRequest>,
) -> std::result::Result<Json<serde_json::Value>, (StatusCode, String)> {
    let user_id = UserId(uuid::Uuid::parse_str(&req.user_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID".to_string()))?);

    let balance = req.initial_balance.parse::<rust_decimal::Decimal>()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid balance".to_string()))?;

    state.risk_engine.register_account(user_id, balance);

    Ok(Json(serde_json::json!({
        "user_id": user_id.to_string(),
        "balance": balance.to_string(),
        "message": "Account registered successfully",
    })))
}

/// Get account positions
pub async fn get_positions<J: EventJournal, R: RiskCheck>(
    State(state): State<AppState<J, R>>,
    Path(user_id): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, (StatusCode, String)> {
    let user_id = UserId(uuid::Uuid::parse_str(&user_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID".to_string()))?);        

    let positions = state.risk_engine.get_positions(user_id);

    let positions_data: Vec<_> = positions
        .iter()
        .map(|position| {
            serde_json::json!({
                "symbol": position.symbol.0,
                "net_quantity": position.net_quantity.to_string(),
                "total_buy": position.total_buy.to_string(),
                "total_sell": position.total_sell.to_string(),
                "open_orders": position.open_orders,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "user_id": user_id.to_string(),
        "positions": positions_data,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_place_order_request_deserialize() {
        let json = r#"{
            "user_id": "550e8400-e29b-41d4-a716-446655440000",
            "symbol": "BTC/USD",
            "side": "buy",
            "order_type": "limit",
            "price": "50000",
            "quantity": "1.5"
        }"#;

        let req: PlaceOrderRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.symbol, "BTC/USD");
        assert_eq!(req.side, "buy");
    }
}
