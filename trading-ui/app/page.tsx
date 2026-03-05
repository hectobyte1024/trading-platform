'use client'

import { useState, useEffect } from 'react'
import { TradingDashboard } from '@/components/trading/TradingDashboard'
import { Header } from '@/components/ui/Header'
import { useWebSocket } from '@/hooks/useWebSocket'
import { ProtectedRoute } from '@/components/ProtectedRoute'

export default function Home() {
  const [selectedSymbol, setSelectedSymbol] = useState('BTC-USD')
  const { connected, send } = useWebSocket()

  useEffect(() => {
    if (connected) {
      // Subscribe to market data for this symbol
      send({ action: 'subscribe', symbol: selectedSymbol })
    }
    
    return () => {
      if (connected) {
        // Unsubscribe when symbol changes
        send({ action: 'unsubscribe', symbol: selectedSymbol })
      }
    }
  }, [selectedSymbol, connected, send])

  return (
    <ProtectedRoute>
      <main className="min-h-screen bg-gray-950 flex flex-col">
        <Header 
          selectedSymbol={selectedSymbol}
          onSymbolChange={setSelectedSymbol}
          connected={connected}
        />
        
        <TradingDashboard symbol={selectedSymbol} />
      </main>
    </ProtectedRoute>
  )
}
