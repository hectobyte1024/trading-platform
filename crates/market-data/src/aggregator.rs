use crate::{BinanceRestClient, BinanceWebSocket, CoinGeckoClient};
use anyhow::Result;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{info, error};

/// Market data update event
#[derive(Debug, Clone)]
pub struct MarketUpdate {
    pub price: Decimal,
    pub change_24h: Option<Decimal>,
    pub volume_24h: Option<Decimal>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Aggregates market data from multiple sources
pub struct MarketDataAggregator {
    coingecko: CoinGeckoClient,
    binance_rest: BinanceRestClient,
    binance: BinanceWebSocket,
    current_price: Arc<RwLock<Decimal>>,
    update_tx: broadcast::Sender<MarketUpdate>,
}

impl MarketDataAggregator {
    pub fn new() -> Self {
        let (update_tx, _) = broadcast::channel(100);
        
        Self {
            coingecko: CoinGeckoClient::new(),
            binance_rest: BinanceRestClient::new(),
            binance: BinanceWebSocket::new(),
            current_price: Arc::new(RwLock::new(Decimal::ZERO)),
            update_tx,
        }
    }

    /// Subscribe to market data updates
    pub fn subscribe(&self) -> broadcast::Receiver<MarketUpdate> {
        self.update_tx.subscribe()
    }

    /// Get current BTC price
    pub async fn get_current_price(&self) -> Decimal {
        *self.current_price.read().await
    }

    /// Start the market data aggregator
    /// This will:
    /// 1. Fetch initial price from CoinGecko
    /// 2. Start Binance WebSocket for real-time updates
    /// 3. Periodically refresh 24h stats from CoinGecko
    pub async fn start(self: Arc<Self>) -> Result<()> {
        info!("Starting market data aggregator");

        // Fetch initial price from CoinGecko
        if let Ok(market_data) = self.coingecko.get_btc_price().await {
            *self.current_price.write().await = market_data.price;
            info!("Initial BTC price: ${}", market_data.price);
            
            let _ = self.update_tx.send(MarketUpdate {
                price: market_data.price,
                change_24h: market_data.change_24h,
                volume_24h: market_data.volume_24h,
                timestamp: market_data.last_updated,
            });
        }

        // Start Binance WebSocket in background
        let self_clone = self.clone();
        tokio::spawn(async move {
            let (binance_tx, mut binance_rx) = broadcast::channel(100);
            
            // Spawn Binance stream
            let ws = self_clone.binance.clone();
            tokio::spawn(async move {
                if let Err(e) = ws.stream_prices(binance_tx).await {
                    error!("Binance stream error: {}", e);
                }
            });

            // Process Binance updates
            while let Ok(price_update) = binance_rx.recv().await {
                *self_clone.current_price.write().await = price_update.price;
                
                // Broadcast aggregated update
                let _ = self_clone.update_tx.send(MarketUpdate {
                    price: price_update.price,
                    change_24h: None,
                    volume_24h: None,
                    timestamp: price_update.timestamp,
                });
            }
        });

        // Periodically refresh 24h stats from CoinGecko (every 5 minutes)
        let self_clone = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));
            loop {
                interval.tick().await;
                
                if let Ok(market_data) = self_clone.coingecko.get_btc_price().await {
                    info!("Updated 24h stats from CoinGecko");
                    
                    let current_price = *self_clone.current_price.read().await;
                    let _ = self_clone.update_tx.send(MarketUpdate {
                        price: current_price,
                        change_24h: market_data.change_24h,
                        volume_24h: market_data.volume_24h,
                        timestamp: chrono::Utc::now(),
                    });
                }
            }
        });

        Ok(())
    }

    /// Get historical price data for charts
    pub async fn get_historical_24h(&self) -> Result<Vec<(i64, Decimal)>> {
        // Use Binance REST API for historical data (more reliable than CoinGecko free tier)
        self.binance_rest.get_historical_24h().await
    }
}

impl Default for MarketDataAggregator {
    fn default() -> Self {
        Self::new()
    }
}
