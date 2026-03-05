'use client'

import { useState } from 'react'
import { Orderbook } from './Orderbook'
import { OrderForm } from './OrderForm'
import { TradeHistory } from './TradeHistory'
import { PriceChart } from './PriceChart'
import { MarketStats } from './MarketStats'

interface TradingDashboardProps {
  symbol: string
}

export function TradingDashboard({ symbol }: TradingDashboardProps) {
  const [view, setView] = useState<'chart' | 'trades'>('chart')

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
          <div className="bg-gray-900 rounded-t-lg border border-gray-800 px-4 py-2 flex gap-2">
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

          {/* Content */}
          <div className="flex-1 bg-gray-900 rounded-b-lg border border-t-0 border-gray-800">
            {view === 'chart' ? (
              <PriceChart symbol={symbol} />
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
