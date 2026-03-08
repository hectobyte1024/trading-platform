use axum::{
    Router,
    routing::{get, post, delete},
    middleware as axum_middleware,
};
use common::*;
use matching_engine::MatchingEngine;
use risk_engine::AdaptiveRiskEngine;
use event_journal::EventJournal;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::TraceLayer;

pub mod rest;
pub mod websocket;
pub mod middleware;
pub mod auth;
pub mod market;

/// API Gateway Configuration
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub enable_cors: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            enable_cors: true,
        }
    }
}

/// Create API router with all endpoints
pub fn create_router<J: EventJournal + 'static, R: RiskCheck + 'static>(
    engine: Arc<MatchingEngine<J, R>>,
    risk_engine: Arc<AdaptiveRiskEngine>,
    market_data: Option<Arc<market_data::MarketDataAggregator>>,
    config: ApiConfig,
) -> Router {
    let state = rest::AppState {
        engine,
        risk_engine,
        market_data,
    };

    let mut router = Router::new()
        // Health check
        .route("/health", get(rest::health_check))
        // Authentication endpoints
        .route("/auth/login", post(auth::login))
        .route("/auth/register", post(auth::register))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/refresh", post(auth::refresh))
        .route("/auth/webauthn/register/init", post(auth::webauthn_register_init))
        .route("/auth/webauthn/register/complete", post(auth::webauthn_register_complete))
        .route("/auth/webauthn/auth/init", post(auth::webauthn_auth_init))
        .route("/auth/webauthn/auth/complete", post(auth::webauthn_auth_complete))
        // Trading endpoints
        .route("/orders", post(rest::place_order))
        .route("/orders/:order_id", delete(rest::cancel_order))
        .route("/orderbook/:symbol", get(rest::get_orderbook))
        .route("/accounts/register", post(rest::register_account))
        .route("/accounts/:user_id/positions", get(rest::get_positions))
        .route("/accounts/:user_id", get(rest::get_account))
        // Market data endpoints 
        .route("/market-data/BTC-USD", get(market::get_market_data))
        .route("/market-data/BTC-USD/historical", get(market::get_historical_data))
        .route("/market-data/BTC-USD/candlesticks", get(market::get_candlestick_data))
        // WebSocket endpoint
        .route("/ws", get(websocket::ws_handler))
        .with_state(state);

    // Add middleware
    let middleware_stack = ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(axum_middleware::from_fn(middleware::log_request));

    router = router.layer(middleware_stack);

    // Add CORS if enabled
    if config.enable_cors {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
        router = router.layer(cors);
    }

    router
}

/// Start the API gateway server
pub async fn start_server<J: EventJournal + 'static, R: RiskCheck + 'static>(
    engine: Arc<MatchingEngine<J, R>>,
    risk_engine: Arc<AdaptiveRiskEngine>,
    market_data: Option<Arc<market_data::MarketDataAggregator>>,
    config: ApiConfig,
) -> std::result::Result<(), TradingError> {
    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Starting API Gateway on {}", addr);

    let router = create_router(engine, risk_engine, market_data, config);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| TradingError::Other(anyhow::anyhow!("Failed to bind to {}: {}", addr, e)))?;

    tracing::info!("API Gateway listening on {}", addr);

    axum::serve(listener, router)
        .await
        .map_err(|e| TradingError::Other(anyhow::anyhow!("Server error: {}", e)))?;

    Ok(())
}

/// Graceful shutdown handler
pub async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C signal");
        }
        _ = terminate => {
            tracing::info!("Received terminate signal");
        }
    }

    tracing::info!("Shutting down API Gateway gracefully");
}

#[cfg(test)]
mod tests {
    use super::*;
    use event_journal::FileJournal;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_router() {
        let temp_dir = TempDir::new().unwrap();
        let journal_path = temp_dir.path().join("test_api.journal");
        let journal = Arc::new(FileJournal::new(journal_path).await.unwrap());

        let limits = risk_engine::RiskLimits::default();
        let risk_engine = Arc::new(AdaptiveRiskEngine::new(limits));

        let engine = Arc::new(MatchingEngine::new(journal, risk_engine.clone()));

        let config = ApiConfig::default();
        let _router = create_router(engine, risk_engine, config);

        // Just verify the router can be created
        assert!(true);
    }

    #[test]
    fn test_api_config_default() {
        let config = ApiConfig::default();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert!(config.enable_cors);
    }
}
