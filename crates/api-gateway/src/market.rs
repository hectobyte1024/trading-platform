use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use crate::rest::AppState;
use common::RiskCheck;
use event_journal::EventJournal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MarketDataResponse {
    pub symbol: String,
    pub price: String,
    pub change_24h: Option<String>,
    pub volume_24h: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoricalDataResponse {
    pub symbol: String,
    pub interval: String,
    pub data: Vec<PricePoint>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PricePoint {
    pub timestamp: i64,
    pub price: String,
}

/// Get current market data for BTC-USD
pub async fn get_market_data<J: EventJournal, R: RiskCheck>(
    State(state): State<AppState<J, R>>,
) -> std::result::Result<Json<MarketDataResponse>, (StatusCode, String)> {
    let aggregator = state.market_data
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Market data not available".to_string()))?;
    
    let price = aggregator.get_current_price().await;
    
    Ok(Json(MarketDataResponse {
        symbol: "BTC-USD".to_string(),
        price: price.to_string(),
        change_24h: None,
        volume_24h: None,
        timestamp: chrono::Utc::now(),
    }))
}

/// Get historical price data for charts (24 hours)
pub async fn get_historical_data<J: EventJournal, R: RiskCheck>(
    State(state): State<AppState<J, R>>,
) -> std::result::Result<Json<HistoricalDataResponse>, (StatusCode, String)> {
    let aggregator = state.market_data
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Market data not available".to_string()))?;
    
    let historical = aggregator.get_historical_24h().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to fetch historical data: {}", e)))?;

    let data = historical
        .into_iter()
        .map(|(timestamp, price)| PricePoint {
            timestamp,
            price: price.to_string(),
        })
        .collect();

    Ok(Json(HistoricalDataResponse {
        symbol: "BTC-USD".to_string(),
        interval: "1m".to_string(),
        data,
    }))
}
