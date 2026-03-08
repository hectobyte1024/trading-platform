use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use crate::rest::AppState;
use common::RiskCheck;
use event_journal::EventJournal;
use serde::{Deserialize, Serialize};
use market_data::Candlestick;

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

#[derive(Debug, Deserialize)]
pub struct CandlestickQuery {
    /// Timeframe: 1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w
    #[serde(default = "default_interval")]
    pub interval: String,
    /// Number of candles to return (max 1000)
    #[serde(default = "default_limit")]
    pub limit: u16,
}

fn default_interval() -> String {
    "1h".to_string()
}

fn default_limit() -> u16 {
    100
}

#[derive(Debug, Serialize)]
pub struct CandlestickResponse {
    pub symbol: String,
    pub interval: String,
    pub candles: Vec<CandlestickData>,
}

#[derive(Debug, Serialize)]
pub struct CandlestickData {
    pub timestamp: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
}

/// Get candlestick (OHLCV) data for TradingView-style charts
/// Query params: ?interval=1h&limit=100
/// Supported intervals: 1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w
pub async fn get_candlestick_data<J: EventJournal, R: RiskCheck>(
    Query(params): Query<CandlestickQuery>,
    State(state): State<AppState<J, R>>,
) -> std::result::Result<Json<CandlestickResponse>, (StatusCode, String)> {
    let aggregator = state.market_data
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Market data not available".to_string()))?;
    
    // Validate interval
    let valid_intervals = ["1m", "3m", "5m", "15m", "30m", "1h", "2h", "4h", "6h", "8h", "12h", "1d", "3d", "1w"];
    if !valid_intervals.contains(&params.interval.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Invalid interval. Supported: {}", valid_intervals.join(", "))
        ));
    }

    // Limit to max 1000 candles
    let limit = params.limit.min(1000);

    let candles = aggregator.get_candlesticks(&params.interval, limit).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to fetch candlesticks: {}", e)))?;

    let candle_data = candles
        .into_iter()
        .map(|c| CandlestickData {
            timestamp: c.timestamp,
            open: c.open.to_string(),
            high: c.high.to_string(),
            low: c.low.to_string(),
            close: c.close.to_string(),
            volume: c.volume.to_string(),
        })
        .collect();

    Ok(Json(CandlestickResponse {
        symbol: "BTC-USD".to_string(),
        interval: params.interval,
        candles: candle_data,
    }))
}
