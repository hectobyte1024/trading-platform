'use client'

import { useEffect, useState } from 'react'
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts'
import { useWebSocket } from '@/hooks/useWebSocket'
import type { WebSocketMessage } from '@/types'
import { formatPrice } from '@/lib/utils'

interface PriceChartProps {
  symbol: string
}

interface PricePoint {
  timestamp: number
  price: number
}

export function PriceChart({ symbol }: PriceChartProps) {
  const { subscribe, unsubscribe } = useWebSocket()
  const [priceData, setPriceData] = useState<PricePoint[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  // Fetch historical data on mount
  useEffect(() => {
    const fetchHistoricalData = async () => {
      try {
        setLoading(true)
        const response = await fetch('http://localhost:8080/market-data/BTC-USD/historical')
        
        if (!response.ok) {
          throw new Error('Failed to fetch historical data')
        }

        const data = await response.json()
        
        // Convert to PricePoint format
        const historical: PricePoint[] = data.data.map((point: { timestamp: number; price: string }) => ({
          timestamp: point.timestamp,
          price: parseFloat(point.price),
        }))

        setPriceData(historical)
        setError(null)
      } catch (err) {
        console.error('Error fetching historical data:', err)
        setError(err instanceof Error ? err.message : 'Failed to load chart data')
      } finally {
        setLoading(false)
      }
    }

    fetchHistoricalData()
  }, [symbol])

  // Subscribe to real-time trade updates
  useEffect(() => {
    const handleTradeUpdate = (message: WebSocketMessage) => {
      if (message.type === 'trade' && message.data.symbol === symbol) {
        const newPoint: PricePoint = {
          timestamp: new Date(message.data.timestamp).getTime(),
          price: parseFloat(message.data.price),
        }

        setPriceData((prevData) => {
          const updatedData = [...prevData, newPoint]
          // Keep last 500 points for better historical view
          return updatedData.slice(-500)
        })
      }
    }

    subscribe('trade', handleTradeUpdate)

    return () => {
      unsubscribe('trade', handleTradeUpdate)
    }
  }, [symbol, subscribe, unsubscribe])

  return (
    <div className="h-full flex flex-col p-4">
      <div className="mb-4">
        <h3 className="text-sm font-semibold text-white">Price Chart (24h)</h3>
        <p className="text-xs text-gray-500">
          {loading ? 'Loading...' : `${priceData.length} data points`}
        </p>
      </div>

      <div className="flex-1">
        {loading ? (
          <div className="flex items-center justify-center h-full">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
          </div>
        ) : error ? (
          <div className="flex items-center justify-center h-full text-red-400 text-sm">
            {error}
          </div>
        ) : priceData.length === 0 ? (
          <div className="flex items-center justify-center h-full text-gray-500 text-sm">
            No chart data available
          </div>
        ) : (
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={priceData}>
              <XAxis
                dataKey="timestamp"
                type="number"
                domain={['dataMin', 'dataMax']}
                tickFormatter={(timestamp) => {
                  const date = new Date(timestamp)
                  return `${date.getHours()}:${date.getMinutes().toString().padStart(2, '0')}`
                }}
                stroke="#6b7280"
                style={{ fontSize: '12px' }}
              />
              <YAxis
                domain={['auto', 'auto']}
                tickFormatter={(value) => formatPrice(value)}
                stroke="#6b7280"
                style={{ fontSize: '12px' }}
                width={80}
              />
              <Tooltip
                contentStyle={{
                  backgroundColor: '#1f2937',
                  border: '1px solid #374151',
                  borderRadius: '6px',
                }}
                labelStyle={{ color: '#9ca3af' }}
                itemStyle={{ color: '#10b981' }}
                formatter={(value: number) => [formatPrice(value), 'Price']}
                labelFormatter={(timestamp) => {
                  const date = new Date(timestamp)
                  return date.toLocaleString()
                }}
              />
              <Line
                type="monotone"
                dataKey="price"
                stroke="#10b981"
                strokeWidth={2}
                dot={false}
                isAnimationActive={false}
              />
            </LineChart>
          </ResponsiveContainer>
        )}
      </div>
    </div>
  )
}
