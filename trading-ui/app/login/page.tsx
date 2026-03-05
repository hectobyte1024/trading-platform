'use client'

import { useState } from 'react'
import { useRouter } from 'next/navigation'
import { useAuth } from '@/hooks/useAuth'

export default function LoginPage() {
  const router = useRouter()
  const { login, loginWithWebAuthn, isLoading } = useAuth()
  const [mode, setMode] = useState<'login' | 'webauthn'>('login')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')

  const handleEmailLogin = async (e: React.FormEvent) => {
    e.preventDefault()
    setError('')

    try {
      await login(email, password)
      router.push('/')
    } catch (err: any) {
      setError(err.response?.data?.message || 'Login failed. Please try again.')
    }
  }

  const handleWebAuthnLogin = async (e: React.FormEvent) => {
    e.preventDefault()
    setError('')

    try {
      await loginWithWebAuthn(email)
      router.push('/')
    } catch (err: any) {
      setError(err.message || 'WebAuthn login failed. Please try again.')
    }
  }

  return (
    <div className="min-h-screen bg-gray-950 flex items-center justify-center p-4">
      <div className="w-full max-w-md">
        <div className="bg-gray-900 rounded-lg border border-gray-800 p-8">
          <h1 className="text-3xl font-bold text-white mb-2 text-center">
            Trading Platform
          </h1>
          <p className="text-gray-400 text-center mb-8">
            Sign in to access your account
          </p>

          {/* Mode Selector */}
          <div className="grid grid-cols-2 gap-2 mb-6">
            <button
              onClick={() => setMode('login')}
              className={`py-2 rounded font-medium transition-colors ${
                mode === 'login'
                  ? 'bg-blue-600 text-white'
                  : 'bg-gray-800 text-gray-400 hover:bg-gray-700'
              }`}
            >
              Email/Password
            </button>
            <button
              onClick={() => setMode('webauthn')}
              className={`py-2 rounded font-medium transition-colors ${
                mode === 'webauthn'
                  ? 'bg-blue-600 text-white'
                  : 'bg-gray-800 text-gray-400 hover:bg-gray-700'
              }`}
            >
              WebAuthn
            </button>
          </div>

          {error && (
            <div className="mb-4 p-3 bg-red-900/20 border border-red-800 rounded text-red-400 text-sm">
              {error}
            </div>
          )}

          {mode === 'login' ? (
            <form onSubmit={handleEmailLogin} className="space-y-4">
              <div>
                <label htmlFor="email" className="block text-sm text-gray-400 mb-2">
                  Email
                </label>
                <input
                  id="email"
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  required
                  className="w-full bg-gray-800 text-white px-4 py-3 rounded border border-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                  placeholder="you@example.com"
                />
              </div>

              <div>
                <label htmlFor="password" className="block text-sm text-gray-400 mb-2">
                  Password
                </label>
                <input
                  id="password"
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  required
                  className="w-full bg-gray-800 text-white px-4 py-3 rounded border border-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                  placeholder="••••••••"
                />
              </div>

              <button
                type="submit"
                disabled={isLoading}
                className="w-full bg-blue-600 hover:bg-blue-700 text-white font-semibold py-3 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {isLoading ? 'Signing in...' : 'Sign In'}
              </button>
            </form>
          ) : (
            <form onSubmit={handleWebAuthnLogin} className="space-y-4">
              <div>
                <label htmlFor="webauthn-email" className="block text-sm text-gray-400 mb-2">
                  Email
                </label>
                <input
                  id="webauthn-email"
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  required
                  className="w-full bg-gray-800 text-white px-4 py-3 rounded border border-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                  placeholder="you@example.com"
                />
              </div>

              <div className="bg-gray-800 border border-gray-700 rounded p-4 text-sm text-gray-400">
                <p className="mb-2">🔐 Passwordless Authentication</p>
                <p>You'll be prompted to use your security key, fingerprint, or face recognition.</p>
              </div>

              <button
                type="submit"
                disabled={isLoading}
                className="w-full bg-green-600 hover:bg-green-700 text-white font-semibold py-3 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {isLoading ? 'Authenticating...' : 'Sign In with WebAuthn'}
              </button>
            </form>
          )}

          <div className="mt-6 text-center">
            <p className="text-sm text-gray-500">
              Demo account:{' '}
              <button
                onClick={() => {
                  setEmail('demo@trading.com')
                  setPassword('demo123')
                  setMode('login')
                }}
                className="text-blue-500 hover:text-blue-400"
              >
                Use demo credentials
              </button>
            </p>
          </div>
        </div>

        <p className="text-center text-gray-500 text-sm mt-4">
          Enterprise-grade security with KMS-backed JWT and WebAuthn/FIDO2
        </p>
      </div>
    </div>
  )
}
