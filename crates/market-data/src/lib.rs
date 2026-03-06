pub mod coingecko;
pub mod binance;
pub mod aggregator;

pub use coingecko::CoinGeckoClient;
pub use binance::BinanceWebSocket;
pub use aggregator::{MarketDataAggregator, MarketUpdate};
