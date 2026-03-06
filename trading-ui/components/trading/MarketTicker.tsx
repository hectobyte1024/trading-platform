'use client';

import { useEffect, useState } from 'react';

interface MarketData {
  symbol: string;
  price: string;
  change_24h: string | null;
  volume_24h: string | null;
  timestamp: string;
}

export default function MarketTicker() {
  const [marketData, setMarketData] = useState<MarketData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchMarketData = async () => {
      try {
        const response = await fetch('http://localhost:8080/market-data/BTC-USD');
        if (!response.ok) {
          throw new Error('Failed to fetch market data');
        }
        const data = await response.json();
        setMarketData(data);
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Unknown error');
      } finally {
        setLoading(false);
      }
    };

    // Initial fetch
    fetchMarketData();

    // Poll every 2 seconds for updated price
    const interval = setInterval(fetchMarketData, 2000);

    return () => clearInterval(interval);
  }, []);

  if (loading) {
    return (
      <div className="flex items-center space-x-2 text-gray-400">
        <div className="animate-pulse h-4 w-24 bg-gray-700 rounded"></div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center space-x-2 text-red-400 text-sm">
        <span>Market data unavailable</span>
      </div>
    );
  }

  if (!marketData) {
    return null;
  }

  const price = parseFloat(marketData.price);
  const formattedPrice = new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(price);

  return (
    <div className="flex items-center space-x-4">
      <div className="flex items-center space-x-2">
        <span className="text-gray-400 text-sm">{marketData.symbol}</span>
        <span className="text-white font-semibold text-lg">{formattedPrice}</span>
      </div>
      
      {marketData.change_24h && (
        <div className={`text-sm ${parseFloat(marketData.change_24h) >= 0 ? 'text-green-400' : 'text-red-400'}`}>
          {parseFloat(marketData.change_24h) >= 0 ? '+' : ''}
          {parseFloat(marketData.change_24h).toFixed(2)}%
        </div>
      )}

      <div className="flex items-center space-x-1">
        <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse"></div>
        <span className="text-xs text-gray-500">LIVE</span>
      </div>
    </div>
  );
}
