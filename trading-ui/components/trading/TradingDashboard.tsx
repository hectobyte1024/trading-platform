'use client'

import { useState } from 'react'
import { Orderbook } from './Orderbook'
import { OrderForm } from './OrderForm'
import { TradeHistory } from './TradeHistory'
import { PriceChart } from './PriceChart'
import { CandlestickChart } from './CandlestickChart'
import { MarketStats } from './MarketStats'

interface TradingDashboardProps {
  symbol: string
}

export function TradingDashboard({ symbol }: TradingDashboardProps) {
  const [view, setView] = useState<'chart' | 'trades'>('chart')
  const [chartType, setChartType] = useState<'line' | 'candlestick'>('candlestick')

  return (
    <div className="flex-1 flex flex-col gap-4 p-4">
      {/* Market Stats */}
      <MarketStats symbol={symbol} />

      {/* Main Trading Grid */}
      <div className="grid grid-cols-12 gap-4 flex-1">
        {/* Left: Orderbook */}
        <div className="col-span-3">
          <Orderbook symbol={symbol} />
        </div>

        {/* Center: Chart or Trades */}
        <div className="col-span-6 flex flex-col">
          {/* View Selector */}
          <div className="bg-gray-900 rounded-t-lg border border-gray-800 px-4 py-2 flex gap-2 justify-between">
            <div className="flex gap-2">
              <button
                onClick={() => setView('chart')}
                className={`px-4 py-1 rounded ${
                  view === 'chart'
                    ? 'bg-blue-600 text-white'
                    : 'text-gray-400 hover:text-white'
                }`}
              >
                Chart
              </button>
              <button
                onClick={() => setView('trades')}
                className={`px-4 py-1 rounded ${
                  view === 'trades'
                    ? 'bg-blue-600 text-white'
                    : 'text-gray-400 hover:text-white'
                }`}
              >
                Trades
              </button>
            </div>

            {/* Chart Type Toggle (only show when chart view is active) */}
            {view === 'chart' && (
              <div className="flex gap-1 bg-gray-800 rounded px-1 py-1">
                <button
                  onClick={() => setChartType('candlestick')}
                  className={`px-3 py-1 text-xs font-medium rounded transition-colors ${
                    chartType === 'candlestick'
                      ? 'bg-gray-700 text-white'
                      : 'text-gray-400 hover:text-white'
                  }`}
                  title="Candlestick Chart"
                >
                  📊 Candles
                </button>
                <button
                  onClick={() => setChartType('line')}
                  className={`px-3 py-1 text-xs font-medium rounded transition-colors ${
                    chartType === 'line'
                      ? 'bg-gray-700 text-white'
                      : 'text-gray-400 hover:text-white'
                  }`}
                  title="Line Chart"
                >
                  📈 Line
                </button>
              </div>
            )}
          </div>

          {/* Content */}
          <div className="flex-1 bg-gray-900 rounded-b-lg border border-t-0 border-gray-800">
            {view === 'chart' ? (
              chartType === 'candlestick' ? (
                <CandlestickChart symbol={symbol} />
              ) : (
                <PriceChart symbol={symbol} />
              )
            ) : (
              <TradeHistory symbol={symbol} />
            )}
          </div>
        </div>

        {/* Right: Order Form */}
        <div className="col-span-3">
          <OrderForm symbol={symbol} />
        </div>
      </div>
    </div>
  )
}
