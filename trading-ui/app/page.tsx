'use client'

import { useState, useEffect } from 'react'
import { TradingDashboard } from '@/components/trading/TradingDashboard'
import { Header } from '@/components/ui/Header'
import { useWebSocket } from '@/hooks/useWebSocket'
import { ProtectedRoute } from '@/components/ProtectedRoute'

export default function Home() {
  const [selectedSymbol, setSelectedSymbol] = useState('BTC-USD')
  const { connected, subscribe, unsubscribe } = useWebSocket()

  useEffect(() => {
    if (!connected) return

    // Callback for market data updates
    const handleMarketData = (message: any) => {
      console.log('Market data update:', message)
    }

    // Subscribe to market data for this symbol
    subscribe(selectedSymbol, handleMarketData)
    
    return () => {
      // Unsubscribe when symbol changes or component unmounts
      unsubscribe(selectedSymbol, handleMarketData)
    }
  }, [selectedSymbol, connected, subscribe, unsubscribe])

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
