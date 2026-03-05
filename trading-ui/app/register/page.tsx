'use client'

import { useState } from 'react'
import { useRouter } from 'next/navigation'
import { useAuth } from '@/hooks/useAuth'
import { authService } from '@/lib/auth'

export default function RegisterPage() {
  const router = useRouter()
  const { register, isLoading } = useAuth()
  const [name, setName] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [setupWebAuthn, setSetupWebAuthn] = useState(false)
  const [error, setError] = useState('')
  const [step, setStep] = useState<'register' | 'webauthn'>('register')

  const handleRegister = async (e: React.FormEvent) => {
    e.preventDefault()
    setError('')

    if (password !== confirmPassword) {
      setError('Passwords do not match')
      return
    }

    if (password.length < 8) {
      setError('Password must be at least 8 characters')
      return
    }

    try {
      await register(email, password, name)
      
      if (setupWebAuthn) {
        setStep('webauthn')
      } else {
        router.push('/')
      }
    } catch (err: any) {
      setError(err.response?.data?.message || 'Registration failed. Please try again.')
    }
  }

  const handleWebAuthnSetup = async () => {
    setError('')
    try {
      const user = authService.getCurrentUser()
      if (!user) {
        throw new Error('No user found')
      }
      await authService.registerWebAuthn(user.id)
      router.push('/')
    } catch (err: any) {
      setError(err.message || 'WebAuthn setup failed. You can set it up later.')
      // Even if WebAuthn setup fails, user is registered, so redirect after a delay
      setTimeout(() => router.push('/'), 2000)
    }
  }

  if (step === 'webauthn') {
    return (
      <div className="min-h-screen bg-gray-950 flex items-center justify-center p-4">
        <div className="w-full max-w-md">
          <div className="bg-gray-900 rounded-lg border border-gray-800 p-8">
            <h1 className="text-2xl font-bold text-white mb-2 text-center">
              Set Up WebAuthn
            </h1>
            <p className="text-gray-400 text-center mb-8">
              Add passwordless authentication for enhanced security
            </p>

            {error && (
              <div className="mb-4 p-3 bg-red-900/20 border border-red-800 rounded text-red-400 text-sm">
                {error}
              </div>
            )}

            <div className="bg-gray-800 border border-gray-700 rounded p-4 mb-6">
              <h3 className="text-white font-semibold mb-3">Benefits:</h3>
              <ul className="space-y-2 text-sm text-gray-400">
                <li>✓ No passwords to remember</li>
                <li>✓ Phishing-resistant authentication</li>
                <li>✓ Faster login with biometrics</li>
                <li>✓ Industry-standard FIDO2 security</li>
              </ul>
            </div>

            <button
              onClick={handleWebAuthnSetup}
              disabled={isLoading}
              className="w-full bg-green-600 hover:bg-green-700 text-white font-semibold py-3 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed mb-3"
            >
              {isLoading ? 'Setting up...' : 'Set Up WebAuthn'}
            </button>

            <button
              onClick={() => router.push('/')}
              className="w-full bg-gray-700 hover:bg-gray-600 text-white font-medium py-3 rounded transition-colors"
            >
              Skip for Now
            </button>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-gray-950 flex items-center justify-center p-4">
      <div className="w-full max-w-md">
        <div className="bg-gray-900 rounded-lg border border-gray-800 p-8">
          <h1 className="text-3xl font-bold text-white mb-2 text-center">
            Create Account
          </h1>
          <p className="text-gray-400 text-center mb-8">
            Join the trading platform
          </p>

          {error && (
            <div className="mb-4 p-3 bg-red-900/20 border border-red-800 rounded text-red-400 text-sm">
              {error}
            </div>
          )}

          <form onSubmit={handleRegister} className="space-y-4">
            <div>
              <label htmlFor="name" className="block text-sm text-gray-400 mb-2">
                Full Name
              </label>
              <input
                id="name"
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                required
                className="w-full bg-gray-800 text-white px-4 py-3 rounded border border-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                placeholder="John Doe"
              />
            </div>

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
              <p className="text-xs text-gray-500 mt-1">Minimum 8 characters</p>
            </div>

            <div>
              <label htmlFor="confirm-password" className="block text-sm text-gray-400 mb-2">
                Confirm Password
              </label>
              <input
                id="confirm-password"
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                required
                className="w-full bg-gray-800 text-white px-4 py-3 rounded border border-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                placeholder="••••••••"
              />
            </div>

            <div className="flex items-start">
              <input
                id="setup-webauthn"
                type="checkbox"
                checked={setupWebAuthn}
                onChange={(e) => setSetupWebAuthn(e.target.checked)}
                className="mt-1 mr-3"
              />
              <label htmlFor="setup-webauthn" className="text-sm text-gray-400">
                Set up passwordless login (WebAuthn) after registration
              </label>
            </div>

            <button
              type="submit"
              disabled={isLoading}
              className="w-full bg-blue-600 hover:bg-blue-700 text-white font-semibold py-3 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {isLoading ? 'Creating Account...' : 'Create Account'}
            </button>
          </form>

          <div className="mt-6 text-center">
            <p className="text-sm text-gray-500">
              Already have an account?{' '}
              <a href="/login" className="text-blue-500 hover:text-blue-400">
                Sign in
              </a>
            </p>
          </div>
        </div>
      </div>
    </div>
  )
}
