use anyhow::Result;
use futures_util::StreamExt;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tokio::sync::broadcast;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

/// Binance WebSocket client for real-time BTC price updates
#[derive(Clone)]
pub struct BinanceWebSocket {
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinanceTrade {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub quantity: String,
    #[serde(rename = "T")]
    pub trade_time: i64,
}

#[derive(Debug, Clone)]
pub struct PriceUpdate {
    pub price: Decimal,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl BinanceWebSocket {
    pub fn new() -> Self {
        Self {
            url: "wss://stream.binance.com:9443/ws/btcusdt@trade".to_string(),
        }
    }

    /// Start streaming real-time BTC trade prices
    pub async fn stream_prices(
        &self,
        tx: broadcast::Sender<PriceUpdate>,
    ) -> Result<()> {
        info!("Connecting to Binance WebSocket: {}", self.url);

        loop {
            match self.connect_and_stream(&tx).await {
                Ok(_) => {
                    warn!("Binance WebSocket connection closed, reconnecting...");
                }
                Err(e) => {
                    error!("Binance WebSocket error: {}, reconnecting in 5s...", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn connect_and_stream(
        &self,
        tx: &broadcast::Sender<PriceUpdate>,
    ) -> Result<()> {
        let (ws_stream, _) = connect_async(&self.url).await?;
        info!("Connected to Binance WebSocket");

        let (_write, mut read) = ws_stream.split();

        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(trade) = serde_json::from_str::<BinanceTrade>(&text) {
                        if let Ok(price) = Decimal::from_str(&trade.price) {
                            let update = PriceUpdate {
                                price,
                                timestamp: chrono::Utc::now(),
                            };

                            // Broadcast to subscribers (ignore if no receivers)
                            let _ = tx.send(update);
                        }
                    }
                }
                Ok(Message::Ping(_)) => {
                    // Respond to ping with pong
                    info!("Received ping from Binance");
                }
                Ok(Message::Close(_)) => {
                    warn!("Binance WebSocket closed by server");
                    break;
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }
}

impl Default for BinanceWebSocket {
    fn default() -> Self {
        Self::new()
    }
}
