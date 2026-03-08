pub mod coingecko;
pub mod binance;
pub mod aggregator;

pub use coingecko::CoinGeckoClient;
pub use binance::{BinanceRestClient, BinanceWebSocket, Candlestick};
pub use aggregator::{MarketDataAggregator, MarketUpdate};
