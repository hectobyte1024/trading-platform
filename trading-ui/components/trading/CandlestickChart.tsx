'use client'

import { useEffect, useState } from 'react'
import { ComposedChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell } from 'recharts'
import { formatPrice, formatQuantity } from '@/lib/utils'

interface CandlestickChartProps {
  symbol: string
}

interface Candle {
  timestamp: number
  open: number
  high: number
  low: number
  close: number
  volume: number
}

type Timeframe = '15m' | '1h' | '4h' | '1d' | '1w'

const TIMEFRAME_CONFIG = {
  '15m': { label: '15m', limit: 96 },   // 24 hours
  '1h': { label: '1H', limit: 168 },    // 1 week
  '4h': { label: '4H', limit: 180 },    // 30 days
  '1d': { label: '1D', limit: 365 },    // 1 year
  '1w': { label: '1W', limit: 104 },    // 2 years
}

// Custom Candlestick Shape Component
const Candlestick = (props: any) => {
  const { x, y, width, height, open, close, high, low } = props
  const isGreen = close > open
  const color = isGreen ? '#10b981' : '#ef4444'
  const ratio = Math.abs(height / (open - close))
  
  // Make candles narrower - use max 80% of available width, capped at 8px
  const candleWidth = Math.min(width * 0.8, 8)
  const candleX = x + (width - candleWidth) / 2

  return (
    <g>
      {/* Wick (high-low line) */}
      <line
        x1={x + width / 2}
        y1={y - (high - Math.max(open, close)) * ratio}
        x2={x + width / 2}
        y2={y + height + (Math.min(open, close) - low) * ratio}
        stroke={color}
        strokeWidth={1}
      />
      {/* Body (open-close rectangle) */}
      <rect
        x={candleX}
        y={y}
        width={candleWidth}
        height={height || 1} // Min height of 1px for doji candles
        fill={color}
        stroke={color}
        strokeWidth={1}
        opacity={isGreen ? 0.8 : 1}
      />
    </g>
  )
}

export function CandlestickChart({ symbol }: CandlestickChartProps) {
  const [candles, setCandles] = useState<Candle[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [timeframe, setTimeframe] = useState<Timeframe>('1h')
  const [hoveredCandle, setHoveredCandle] = useState<Candle | null>(null)

  useEffect(() => {
    const fetchCandlesticks = async () => {
      try {
        setLoading(true)
        const config = TIMEFRAME_CONFIG[timeframe]
        const response = await fetch(
          `http://localhost:8080/market-data/BTC-USD/candlesticks?interval=${timeframe}&limit=${config.limit}`
        )

        if (!response.ok) {
          throw new Error('Failed to fetch candlestick data')
        }

        const data = await response.json()

        // Convert to Candle format
        const candleData: Candle[] = data.candles.map((c: any) => ({
          timestamp: c.timestamp,
          open: parseFloat(c.open),
          high: parseFloat(c.high),
          low: parseFloat(c.low),
          close: parseFloat(c.close),
          volume: parseFloat(c.volume),
        }))

        setCandles(candleData)
        setError(null)
      } catch (err) {
        console.error('Error fetching candlesticks:', err)
        setError(err instanceof Error ? err.message : 'Failed to load chart')
      } finally {
        setLoading(false)
      }
    }

    fetchCandlesticks()
  }, [symbol, timeframe])

  // Calculate price range for better Y-axis scaling
  const priceRange = candles.length > 0 ? {
    min: Math.min(...candles.map(c => c.low)),
    max: Math.max(...candles.map(c => c.high)),
  } : { min: 0, max: 0 }

  const maxVolume = candles.length > 0 ? Math.max(...candles.map(c => c.volume)) : 0

  // Format timestamp based on timeframe
  const formatTimestamp = (timestamp: number) => {
    const date = new Date(timestamp)
    switch (timeframe) {
      case '15m':
      case '1h':
        return `${date.getHours()}:${date.getMinutes().toString().padStart(2, '0')}`
      case '4h':
        return `${date.getMonth() + 1}/${date.getDate()} ${date.getHours()}:00`
      case '1d':
        return `${date.getMonth() + 1}/${date.getDate()}`
      case '1w':
        return `${date.getMonth() + 1}/${date.getDate()}`
      default:
        return date.toLocaleString()
    }
  }

  const CustomTooltip = ({ active, payload }: any) => {
    if (!active || !payload || !payload[0]) return null

    const data = payload[0].payload
    const isGreen = data.close > data.open
    const change = data.close - data.open
    const changePercent = (change / data.open) * 100

    return (
      <div className="bg-gray-900 border border-gray-700 rounded-lg p-3 shadow-lg">
        <div className="text-xs text-gray-400 mb-2">
          {new Date(data.timestamp).toLocaleString()}
        </div>
        <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
          <div className="text-gray-400">O:</div>
          <div className="text-white">{formatPrice(data.open)}</div>
          <div className="text-gray-400">H:</div>
          <div className={data.high === data.low ? "text-gray-400" : "text-green-400"}>
            {formatPrice(data.high)}
          </div>
          <div className="text-gray-400">L:</div>
          <div className={data.high === data.low ? "text-gray-400" : "text-red-400"}>
            {formatPrice(data.low)}
          </div>
          <div className="text-gray-400">C:</div>
          <div className={isGreen ? "text-green-400" : "text-red-400"}>
            {formatPrice(data.close)}
          </div>
          <div className="text-gray-400">Vol:</div>
          <div className="text-white">{formatQuantity(data.volume, 2)}</div>
          <div className="text-gray-400">Δ:</div>
          <div className={isGreen ? "text-green-400" : "text-red-400"}>
            {formatPrice(Math.abs(change))} ({changePercent.toFixed(2)}%)
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="h-full flex flex-col p-4">
      {/* Header with Timeframe Selector */}
      <div className="mb-4 flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold text-white">
            {symbol} Candlestick Chart
          </h3>
          <p className="text-xs text-gray-500">
            {loading ? 'Loading...' : `${candles.length} candles`}
          </p>
        </div>

        {/* Timeframe Buttons */}
        <div className="flex gap-1 bg-gray-800 rounded-lg p-1">
          {(Object.keys(TIMEFRAME_CONFIG) as Timeframe[]).map((tf) => (
            <button
              key={tf}
              onClick={() => setTimeframe(tf)}
              className={`px-3 py-1 text-xs font-medium rounded transition-colors ${
                timeframe === tf
                  ? 'bg-blue-600 text-white'
                  : 'text-gray-400 hover:text-white hover:bg-gray-700'
              }`}
            >
              {TIMEFRAME_CONFIG[tf].label}
            </button>
          ))}
        </div>
      </div>

      <div className="flex-1 flex flex-col gap-2">
        {loading ? (
          <div className="flex items-center justify-center h-full">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
          </div>
        ) : error ? (
          <div className="flex items-center justify-center h-full text-red-400 text-sm">
            {error}
          </div>
        ) : candles.length === 0 ? (
          <div className="flex items-center justify-center h-full text-gray-500 text-sm">
            No candlestick data available
          </div>
        ) : (
          <>
            {/* Price Chart (70% height) */}
            <div className="flex-[7]">
              <ResponsiveContainer width="100%" height="100%">
                <ComposedChart 
                  data={candles} 
                  margin={{ top: 10, right: 10, bottom: 0, left: 0 }}
                  barCategoryGap="10%"
                >
                  <XAxis
                    dataKey="timestamp"
                    tickFormatter={formatTimestamp}
                    stroke="#6b7280"
                    style={{ fontSize: '11px' }}
                    minTickGap={30}
                    height={30}
                  />
                  <YAxis
                    domain={[priceRange.min * 0.999, priceRange.max * 1.001]}
                    tickFormatter={(value) => formatPrice(value)}
                    stroke="#6b7280"
                    style={{ fontSize: '11px' }}
                    width={80}
                    orientation="right"
                  />
                  <Tooltip content={<CustomTooltip />} />
                  <Bar
                    dataKey="open"
                    shape={<Candlestick />}
                    isAnimationActive={false}
                    maxBarSize={12}
                  />
                </ComposedChart>
              </ResponsiveContainer>
            </div>

            {/* Volume Chart (30% height) */}
            <div className="flex-[3]">
              <ResponsiveContainer width="100%" height="100%">
                <ComposedChart 
                  data={candles} 
                  margin={{ top: 0, right: 10, bottom: 10, left: 0 }}
                  barCategoryGap="10%"
                >
                  <XAxis
                    dataKey="timestamp"
                    tickFormatter={formatTimestamp}
                    stroke="#6b7280"
                    style={{ fontSize: '11px' }}
                    minTickGap={30}
                    height={30}
                  />
                  <YAxis
                    tickFormatter={(value) => formatQuantity(value, 0)}
                    stroke="#6b7280"
                    style={{ fontSize: '11px' }}
                    width={80}
                    orientation="right"
                  />
                  <Tooltip
                    contentStyle={{
                      backgroundColor: '#1f2937',
                      border: '1px solid #374151',
                      borderRadius: '6px',
                    }}
                    formatter={(value: number) => [formatQuantity(value, 2), 'Volume']}
                  />
                  <Bar 
                    dataKey="volume" 
                    isAnimationActive={false}
                    maxBarSize={12}
                  >
                    {candles.map((candle, index) => (
                      <Cell
                        key={`cell-${index}`}
                        fill={candle.close > candle.open ? '#10b98180' : '#ef444480'}
                      />
                    ))}
                  </Bar>
                </ComposedChart>
              </ResponsiveContainer>
            </div>
          </>
        )}
      </div>
    </div>
  )
}
