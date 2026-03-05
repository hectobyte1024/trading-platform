'use client'

import { useState } from 'react'
import { useMutation, useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api'
import { useAuth } from '@/hooks/useAuth'
import type { CreateOrderRequest } from '@/types'
import { calculateTotal, formatPrice } from '@/lib/utils'

interface OrderFormProps {
  symbol: string
}

export function OrderForm({ symbol }: OrderFormProps) {
  const { user } = useAuth()
  const [side, setSide] = useState<'Buy' | 'Sell'>('Buy')
  const [orderType, setOrderType] = useState<'Limit' | 'Market'>('Limit')
  const [price, setPrice] = useState('')
  const [quantity, setQuantity] = useState('')

  // Fetch account data
  const { data: account } = useQuery({
    queryKey: ['account', user?.id],
    queryFn: () => user ? api.getAccount(user.id) : null,
    enabled: !!user,
    refetchInterval: 5000, // Refresh every 5 seconds
  })

  const placeOrderMutation = useMutation({
    mutationFn: (request: CreateOrderRequest) => api.placeOrder(request),
    onSuccess: () => {
      // Reset form
      setPrice('')
      setQuantity('')
      alert('Order placed successfully!')
    },
    onError: (error) => {
      alert(`Failed to place order: ${error.message}`)
    },
  })

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()

    const request: CreateOrderRequest = {
      symbol,
      side,
      order_type: orderType,
      quantity,
      ...(orderType === 'Limit' && { price }),
      time_in_force: 'GTC',
    }

    placeOrderMutation.mutate(request)
  }

  const total = price && quantity ? calculateTotal(price, quantity) : '0.00'
  const availableBalance = account?.available_balance || '0.00'
  const marginUsed = account?.margin_used || '0.00'

  return (
    <div className="bg-gray-900 rounded-lg border border-gray-800 h-full flex flex-col">
      <div className="px-4 py-3 border-b border-gray-800">
        <h2 className="text-lg font-semibold text-white">Place Order</h2>
      </div>

      <form onSubmit={handleSubmit} className="flex-1 flex flex-col p-4">
        {/* Side Selector */}
        <div className="grid grid-cols-2 gap-2 mb-4">
          <button
            type="button"
            onClick={() => setSide('Buy')}
            className={`py-2 rounded font-semibold transition-colors ${
              side === 'Buy'
                ? 'bg-green-600 text-white'
                : 'bg-gray-800 text-gray-400 hover:bg-gray-700'
            }`}
          >
            Buy
          </button>
          <button
            type="button"
            onClick={() => setSide('Sell')}
            className={`py-2 rounded font-semibold transition-colors ${
              side === 'Sell'
                ? 'bg-red-600 text-white'
                : 'bg-gray-800 text-gray-400 hover:bg-gray-700'
            }`}
          >
            Sell
          </button>
        </div>

        {/* Order Type */}
        <div className="mb-4">
          <label className="block text-sm text-gray-400 mb-2">Order Type</label>
          <select
            value={orderType}
            onChange={(e) => setOrderType(e.target.value as 'Limit' | 'Market')}
            className="w-full bg-gray-800 text-white px-3 py-2 rounded border border-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
          >
            <option value="Limit">Limit</option>
            <option value="Market">Market</option>
          </select>
        </div>

        {/* Price Input (only for Limit orders) */}
        {orderType === 'Limit' && (
          <div className="mb-4">
            <label className="block text-sm text-gray-400 mb-2">Price</label>
            <input
              type="number"
              step="0.01"
              value={price}
              onChange={(e) => setPrice(e.target.value)}
              placeholder="0.00"
              required={orderType === 'Limit'}
              className="w-full bg-gray-800 text-white px-3 py-2 rounded border border-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>
        )}

        {/* Quantity Input */}
        <div className="mb-4">
          <label className="block text-sm text-gray-400 mb-2">Quantity</label>
          <input
            type="number"
            step="0.0001"
            value={quantity}
            onChange={(e) => setQuantity(e.target.value)}
            placeholder="0.0000"
            required
            className="w-full bg-gray-800 text-white px-3 py-2 rounded border border-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        {/* Total */}
        <div className="mb-6 p-3 bg-gray-800 rounded">
          <div className="flex justify-between text-sm">
            <span className="text-gray-400">Total</span>
            <span className="text-white font-mono">{formatPrice(total)} USD</span>
          </div>
        </div>

        {/* Submit Button */}
        <button
          type="submit"
          disabled={placeOrderMutation.isPending}
          className={`w-full py-3 rounded font-semibold transition-colors ${
            side === 'Buy'
              ? 'bg-green-600 hover:bg-green-700 disabled:bg-green-800'
              : 'bg-red-600 hover:bg-red-700 disabled:bg-red-800'
          } text-white disabled:opacity-50 disabled:cursor-not-allowed`}
        >
          {placeOrderMutation.isPending
            ? 'Placing...'
            : `${side} ${symbol.split('-')[0]}`}
        </button>

        {/* Account Info */}
        <div className="mt-6 pt-6 border-t border-gray-800">
          <div className="text-xs text-gray-500 space-y-2">
            <div className="flex justify-between">
              <span>Available</span>
              <span className="text-white">${formatPrice(availableBalance)}</span>
            </div>
            <div className="flex justify-between">
              <span>Margin Used</span>
              <span className="text-white">${formatPrice(marginUsed)}</span>
            </div>
          </div>
        </div>
      </form>
    </div>
  )
}
