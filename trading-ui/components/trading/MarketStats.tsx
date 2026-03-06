'use client'

import { useEffect, useState } from 'react'
import { useWebSocket } from '@/hooks/useWebSocket'
import type { MarketData, WebSocketMessage } from '@/types'
import { formatPrice, formatQuantity, formatPercent } from '@/lib/utils'

interface MarketStatsProps {
  symbol: string
}

interface ExternalMarketData {
  symbol: string
  price: string
  change_24h: string | null
  volume_24h: string | null
  timestamp: string
}

export function MarketStats({ symbol }: MarketStatsProps) {
  const { subscribe, unsubscribe } = useWebSocket()
  const [marketData, setMarketData] = useState<MarketData | null>(null)
  const [externalData, setExternalData] = useState<ExternalMarketData | null>(null)

  // Fetch real market data from CoinGecko/Binance
  useEffect(() => {
    const fetchMarketData = async () => {
      try {
        const response = await fetch('http://localhost:8080/market-data/BTC-USD')
        if (response.ok) {
          const data = await response.json()
          setExternalData(data)
        }
      } catch (err) {
        console.error('Error fetching external market data:', err)
      }
    }

    fetchMarketData()
    const interval = setInterval(fetchMarketData, 3000) // Update every 3 seconds

    return () => clearInterval(interval)
  }, [symbol])

  useEffect(() => {
    const handleMarketUpdate = (message: WebSocketMessage) => {
      if (message.type === 'market_data' && message.data.symbol === symbol) {
        setMarketData(message.data)
      }
    }

    subscribe('market_data', handleMarketUpdate)

    return () => {
      unsubscribe('market_data', handleMarketUpdate)
    }
  }, [symbol, subscribe, unsubscribe])

  // Use external data if available, otherwise use WebSocket data or mock
  const displayData: MarketData = externalData ? {
    symbol: externalData.symbol,
    last_price: externalData.price,
    bid: marketData?.bid || '0.00',
    ask: marketData?.ask || '0.00',
    volume_24h: externalData.volume_24h || '0',
    high_24h: marketData?.high_24h || externalData.price,
    low_24h: marketData?.low_24h || externalData.price,
    change_24h: externalData.change_24h || '0',
    timestamp: externalData.timestamp,
  } : marketData || {
    symbol,
    last_price: '0.00',
    bid: '0.00',
    ask: '0.00',
    volume_24h: '0',
    high_24h: '0.00',
    low_24h: '0.00',
    change_24h: '0',
    timestamp: new Date().toISOString(),
  }

  const change = parseFloat(displayData.change_24h)
  const isPositive = change >= 0

  return (
    <div className="bg-gray-900 rounded-lg border border-gray-800 p-4">
      <div className="grid grid-cols-7 gap-6">
        {/* Last Price */}
        <div>
          <div className="text-xs text-gray-500 mb-1">Last Price</div>
          <div className={`text-2xl font-bold ${isPositive ? 'text-green-400' : 'text-red-400'}`}>
            {formatPrice(displayData.last_price)}
          </div>
        </div>

        {/* 24h Change */}
        <div>
          <div className="text-xs text-gray-500 mb-1">24h Change</div>
          <div className={`text-lg font-semibold ${isPositive ? 'text-green-400' : 'text-red-400'}`}>
            {formatPercent(displayData.change_24h)}
          </div>
        </div>

        {/* 24h High */}
        <div>
          <div className="text-xs text-gray-500 mb-1">24h High</div>
          <div className="text-lg font-semibold text-white">
            {formatPrice(displayData.high_24h)}
          </div>
        </div>

        {/* 24h Low */}
        <div>
          <div className="text-xs text-gray-500 mb-1">24h Low</div>
          <div className="text-lg font-semibold text-white">
            {formatPrice(displayData.low_24h)}
          </div>
        </div>

        {/* 24h Volume */}
        <div>
          <div className="text-xs text-gray-500 mb-1">24h Volume</div>
          <div className="text-lg font-semibold text-white">
            {formatQuantity(displayData.volume_24h, 0)}
          </div>
        </div>

        {/* Bid */}
        <div>
          <div className="text-xs text-gray-500 mb-1">Bid</div>
          <div className="text-lg font-semibold text-green-400">
            {formatPrice(displayData.bid)}
          </div>
        </div>

        {/* Ask */}
        <div>
          <div className="text-xs text-gray-500 mb-1">Ask</div>
          <div className="text-lg font-semibold text-red-400">
            {formatPrice(displayData.ask)}
          </div>
        </div>
      </div>
    </div>
  )
}
