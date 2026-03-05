'use client'

import { useEffect, useState } from 'react'
import { useWebSocket } from '@/hooks/useWebSocket'
import type { Orderbook as OrderbookType, WebSocketMessage } from '@/types'
import { formatPrice, formatQuantity } from '@/lib/utils'

interface OrderbookProps {
  symbol: string
}

export function Orderbook({ symbol }: OrderbookProps) {
  const { subscribe, unsubscribe } = useWebSocket()
  const [orderbook, setOrderbook] = useState<OrderbookType | null>(null)

  useEffect(() => {
    const handleOrderbookUpdate = (message: WebSocketMessage) => {
      if (message.type === 'orderbook' && message.data.symbol === symbol) {
        setOrderbook(message.data)
      }
    }

    subscribe('orderbook', handleOrderbookUpdate)

    return () => {
      unsubscribe('orderbook', handleOrderbookUpdate)
    }
  }, [symbol, subscribe, unsubscribe])

  const asks = orderbook?.asks.slice(0, 15).reverse() || []
  const bids = orderbook?.bids.slice(0, 15) || []

  return (
    <div className="bg-gray-900 rounded-lg border border-gray-800 h-full flex flex-col">
      <div className="px-4 py-3 border-b border-gray-800">
        <h2 className="text-lg font-semibold text-white">Order Book</h2>
      </div>

      <div className="flex-1 overflow-hidden flex flex-col">
        {/* Header */}
        <div className="px-4 py-2 grid grid-cols-3 text-xs text-gray-500 border-b border-gray-800">
          <div className="text-left">Price</div>
          <div className="text-right">Size</div>
          <div className="text-right">Total</div>
        </div>

        {/* Asks (Sell orders) */}
        <div className="flex-1 overflow-y-auto">
          {asks.map((level, index) => {
            const total = (parseFloat(level.price) * parseFloat(level.quantity)).toFixed(2)
            return (
              <div key={`ask-${index}`} className="orderbook-row orderbook-row-sell group">
                <div className="text-red-400">{formatPrice(level.price)}</div>
                <div className="text-gray-300">{formatQuantity(level.quantity)}</div>
                <div className="text-gray-500 text-xs">{total}</div>
              </div>
            )
          })}
        </div>

        {/* Spread */}
        <div className="px-4 py-2 bg-gray-800 text-center">
          {orderbook && bids.length > 0 && asks.length > 0 && (
            <div className="text-sm">
              <span className="text-gray-400">Spread: </span>
              <span className="text-white font-mono">
                {formatPrice(
                  parseFloat(asks[asks.length - 1].price) - parseFloat(bids[0].price)
                )}
              </span>
            </div>
          )}
        </div>

        {/* Bids (Buy orders) */}
        <div className="flex-1 overflow-y-auto">
          {bids.map((level, index) => {
            const total = (parseFloat(level.price) * parseFloat(level.quantity)).toFixed(2)
            return (
              <div key={`bid-${index}`} className="orderbook-row orderbook-row-buy group">
                <div className="text-green-400">{formatPrice(level.price)}</div>
                <div className="text-gray-300">{formatQuantity(level.quantity)}</div>
                <div className="text-gray-500 text-xs">{total}</div>
              </div>
            )
          })}
        </div>
      </div>
    </div>
  )
}
