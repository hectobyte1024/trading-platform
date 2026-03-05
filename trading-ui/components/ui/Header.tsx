'use client'

import { useAuth } from '@/hooks/useAuth'
import { useRouter } from 'next/navigation'

interface HeaderProps {
  selectedSymbol: string
  onSymbolChange: (symbol: string) => void
  connected: boolean
}

const SYMBOLS = [
  { value: 'BTC-USD', label: 'BTC/USD' },
  { value: 'ETH-USD', label: 'ETH/USD' },
  { value: 'SOL-USD', label: 'SOL/USD' },
  { value: 'AVAX-USD', label: 'AVAX/USD' },
]

export function Header({ selectedSymbol, onSymbolChange, connected }: HeaderProps) {
  const { user, logout } = useAuth()
  const router = useRouter()

  const handleLogout = async () => {
    await logout()
    router.push('/login')
  }

  return (
    <header className="bg-gray-900 border-b border-gray-800 px-6 py-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-6">
          <h1 className="text-xl font-bold text-white">Trading Platform</h1>
          
          <select
            value={selectedSymbol}
            onChange={(e) => onSymbolChange(e.target.value)}
            className="bg-gray-800 text-white px-4 py-2 rounded-lg border border-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
          >
            {SYMBOLS.map((symbol) => (
              <option key={symbol.value} value={symbol.value}>
                {symbol.label}
              </option>
            ))}
          </select>
        </div>

        <div className="flex items-center gap-4">
          {/* Connection Status */}
          <div className="flex items-center gap-2">
            <div className={`w-2 h-2 rounded-full ${connected ? 'bg-green-500' : 'bg-red-500'}`} />
            <span className="text-sm text-gray-400">
              {connected ? 'Connected' : 'Disconnected'}
            </span>
          </div>

          {/* User Menu */}
          {user && (
            <div className="flex items-center gap-3">
              <button
                onClick={() => router.push('/account')}
                className="text-sm text-gray-400 hover:text-white transition-colors"
              >
                {user.name}
                <span className="ml-2 text-xs text-gray-500">({user.role})</span>
              </button>
              <button 
                onClick={handleLogout}
                className="text-sm text-gray-400 hover:text-white transition-colors"
              >
                Logout
              </button>
            </div>
          )}
        </div>
      </div>
    </header>
  )
}
