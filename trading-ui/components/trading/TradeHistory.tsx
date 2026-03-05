'use client'

import { useEffect, useState } from 'react'
import { useWebSocket } from '@/hooks/useWebSocket'
import type { Trade, WebSocketMessage } from '@/types'
import { formatPrice, formatQuantity, formatTimestamp } from '@/lib/utils'

interface TradeHistoryProps {
  symbol: string
}

export function TradeHistory({ symbol }: TradeHistoryProps) {
  const { subscribe, unsubscribe } = useWebSocket()
  const [trades, setTrades] = useState<Trade[]>([])

  useEffect(() => {
    const handleTradeUpdate = (message: WebSocketMessage) => {
      if (message.type === 'trade' && message.data.symbol === symbol) {
        setTrades((prevTrades) => [message.data, ...prevTrades.slice(0, 99)])
      }
    }

    subscribe('trade', handleTradeUpdate)

    return () => {
      unsubscribe('trade', handleTradeUpdate)
    }
  }, [symbol, subscribe, unsubscribe])

  return (
    <div className="h-full flex flex-col">
      <div className="px-4 py-3 border-b border-gray-800">
        <h3 className="text-sm font-semibold text-white">Recent Trades</h3>
      </div>

      {/* Header */}
      <div className="px-4 py-2 grid grid-cols-4 text-xs text-gray-500 border-b border-gray-800">
        <div className="text-left">Time</div>
        <div className="text-right">Price</div>
        <div className="text-right">Size</div>
        <div className="text-right">Side</div>
      </div>

      {/* Trades List */}
      <div className="flex-1 overflow-y-auto">
        {trades.length === 0 ? (
          <div className="flex items-center justify-center h-full text-gray-500 text-sm">
            No trades yet
          </div>
        ) : (
          trades.map((trade, index) => (
            <div
              key={`${trade.id}-${index}`}
              className="px-4 py-2 grid grid-cols-4 text-sm hover:bg-gray-800 transition-colors"
            >
              <div className="text-gray-400 text-xs">
                {formatTimestamp(trade.timestamp)}
              </div>
              <div
                className={`text-right font-mono ${
                  trade.side === 'Buy' ? 'text-green-400' : 'text-red-400'
                }`}
              >
                {formatPrice(trade.price)}
              </div>
              <div className="text-right text-gray-300">
                {formatQuantity(trade.quantity)}
              </div>
              <div
                className={`text-right text-xs font-semibold ${
                  trade.side === 'Buy' ? 'text-green-400' : 'text-red-400'
                }`}
              >
                {trade.side}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  )
}
