use anyhow::Result;
use reqwest::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{info, warn};

/// CoinGecko API client for fetching BTC market data
pub struct CoinGeckoClient {
    client: Client,
    base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinGeckoPrice {
    pub bitcoin: BitcoinData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinData {
    pub usd: f64,
    pub usd_24h_change: Option<f64>,
    pub usd_24h_vol: Option<f64>,
    pub usd_market_cap: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub price: Decimal,
    pub change_24h: Option<Decimal>,
    pub volume_24h: Option<Decimal>,
    pub market_cap: Option<Decimal>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl CoinGeckoClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://api.coingecko.com/api/v3".to_string(),
        }
    }

    /// Fetch current BTC price and 24h statistics
    pub async fn get_btc_price(&self) -> Result<MarketData> {
        let url = format!(
            "{}/simple/price?ids=bitcoin&vs_currencies=usd&include_24hr_change=true&include_24hr_vol=true&include_market_cap=true",
            self.base_url
        );

        info!("Fetching BTC price from CoinGecko");
        
        let response = self.client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            warn!("CoinGecko API error: {}", response.status());
            anyhow::bail!("CoinGecko API returned error: {}", response.status());
        }

        let data: CoinGeckoPrice = response.json().await?;
        
        let market_data = MarketData {
            price: Decimal::from_str(&data.bitcoin.usd.to_string())?,
            change_24h: data.bitcoin.usd_24h_change
                .and_then(|v| Decimal::from_str(&v.to_string()).ok()),
            volume_24h: data.bitcoin.usd_24h_vol
                .and_then(|v| Decimal::from_str(&v.to_string()).ok()),
            market_cap: data.bitcoin.usd_market_cap
                .and_then(|v| Decimal::from_str(&v.to_string()).ok()),
            last_updated: chrono::Utc::now(),
        };

        info!(
            "BTC Price: ${}, 24h Change: {:?}%",
            market_data.price,
            market_data.change_24h
        );

        Ok(market_data)
    }

    /// Fetch historical price data for charts (last 24 hours)
    pub async fn get_historical_24h(&self) -> Result<Vec<(i64, Decimal)>> {
        let url = format!(
            "{}/coins/bitcoin/market_chart?vs_currency=usd&days=1",
            self.base_url
        );

        info!("Fetching 24h historical BTC data from CoinGecko");

        let response = self.client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            warn!("CoinGecko API error: {}", response.status());
            anyhow::bail!("CoinGecko API returned error: {}", response.status());
        }

        let data: serde_json::Value = response.json().await?;
        
        let mut prices = Vec::new();
        if let Some(price_array) = data["prices"].as_array() {
            for item in price_array {
                if let Some(arr) = item.as_array() {
                    if arr.len() >= 2 {
                        if let (Some(timestamp), Some(price)) = (arr[0].as_i64(), arr[1].as_f64()) {
                            if let Ok(decimal_price) = Decimal::from_str(&price.to_string()) {
                                prices.push((timestamp, decimal_price));
                            }
                        }
                    }
                }
            }
        }

        info!("Fetched {} historical price points", prices.len());
        Ok(prices)
    }
}

impl Default for CoinGeckoClient {
    fn default() -> Self {
        Self::new()
    }
}
