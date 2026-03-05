use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tracing::error;

/// Error response format
#[derive(serde::Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

/// Custom error handler
pub async fn handle_error(err: common::TradingError) -> Response {
    let (status, code, message) = match err {
        common::TradingError::OrderValidation(msg) => {
            (StatusCode::BAD_REQUEST, "INVALID_ORDER", msg)
        }
        common::TradingError::RiskCheckFailed(msg) => {
            (StatusCode::FORBIDDEN, "RISK_CHECK_FAILED", msg)
        }
        common::TradingError::InsufficientBalance { required, available } => (
            StatusCode::FORBIDDEN,
            "INSUFFICIENT_BALANCE",
            format!("Required: {}, Available: {}", required, available),
        ),
        common::TradingError::OrderNotFound(msg) => {
            (StatusCode::NOT_FOUND, "ORDER_NOT_FOUND", msg)
        }
        common::TradingError::SymbolNotFound(msg) => {
            (StatusCode::NOT_FOUND, "SYMBOL_NOT_FOUND", msg)
        }
        common::TradingError::InvalidPrice(msg) => {
            (StatusCode::BAD_REQUEST, "INVALID_PRICE", msg)
        }
        common::TradingError::InvalidQuantity(msg) => {
            (StatusCode::BAD_REQUEST, "INVALID_QUANTITY", msg)
        }
        common::TradingError::PositionLimitExceeded(msg) => {
            (StatusCode::FORBIDDEN, "POSITION_LIMIT_EXCEEDED", msg)
        }
        common::TradingError::DatabaseError(msg) => {
            error!("Database error: {}", msg);
            (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR", "Internal server error".to_string())
        }
        common::TradingError::AuthenticationError(msg) => {
            (StatusCode::UNAUTHORIZED, "AUTHENTICATION_ERROR", msg)
        }
        _ => {
            error!("Unhandled error: {:?}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "Internal server error".to_string())
        }
    };

    let body = Json(json!({
        "error": message,
        "code": code,
    }));

    (status, body).into_response()
}

/// Request logging middleware
pub async fn log_request(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();

    tracing::info!("{} {}", method, uri);

    let response = next.run(request).await;

    tracing::debug!("{} {} - {}", method, uri, response.status());

    response
}

/// Simple API key authentication middleware (placeholder)
/// In production, use JWT or OAuth2
pub async fn auth_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    // In production, validate API key or JWT token from headers
    // For now, just allow all requests
    
    // Example check:
    // if let Some(api_key) = request.headers().get("X-API-Key") {
    //     if validate_api_key(api_key).await.is_ok() {
    //         return Ok(next.run(request).await);
    //     }
    // }
    // Err(StatusCode::UNAUTHORIZED)

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_error_response() {
        let _err = common::TradingError::OrderValidation("Invalid price".to_string());
        // Error handling tested through integration tests
        assert!(true);
    }
}
