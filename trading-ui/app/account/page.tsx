'use client'

import { useAuth } from '@/hooks/useAuth'
import { ProtectedRoute } from '@/components/ProtectedRoute'
import { useState } from 'react'
import { authService } from '@/lib/auth'
import { useRouter } from 'next/navigation'

export default function AccountPage() {
  const { user } = useAuth()
  const router = useRouter()
  const [setupWebAuthn, setSetupWebAuthn] = useState(false)
  const [error, setError] = useState('')
  const [success, setSuccess] = useState('')

  const handleWebAuthnSetup = async () => {
    if (!user) return
    
    setError('')
    setSuccess('')
    setSetupWebAuthn(true)
    
    try {
      await authService.registerWebAuthn(user.id)
      setSuccess('WebAuthn successfully configured! You can now use passwordless login.')
    } catch (err: any) {
      setError(err.message || 'Failed to set up WebAuthn. Please try again.')
    } finally {
      setSetupWebAuthn(false)
    }
  }

  return (
    <ProtectedRoute>
      <div className="min-h-screen bg-gray-950">
        <header className="bg-gray-900 border-b border-gray-800 px-6 py-4">
          <div className="flex items-center justify-between">
            <h1 className="text-xl font-bold text-white">Account Settings</h1>
            <button
              onClick={() => router.push('/')}
              className="text-sm text-gray-400 hover:text-white transition-colors"
            >
              ← Back to Trading
            </button>
          </div>
        </header>

        <div className="max-w-4xl mx-auto p-6">
          {/* Profile Section */}
          <div className="bg-gray-900 rounded-lg border border-gray-800 p-6 mb-6">
            <h2 className="text-lg font-semibold text-white mb-4">Profile Information</h2>
            
            <div className="space-y-4">
              <div>
                <label className="block text-sm text-gray-400 mb-1">Name</label>
                <div className="text-white">{user?.name}</div>
              </div>
              
              <div>
                <label className="block text-sm text-gray-400 mb-1">Email</label>
                <div className="text-white">{user?.email}</div>
              </div>
              
              <div>
                <label className="block text-sm text-gray-400 mb-1">Role</label>
                <div className="text-white capitalize">{user?.role}</div>
              </div>
              
              <div>
                <label className="block text-sm text-gray-400 mb-1">User ID</label>
                <div className="text-gray-500 font-mono text-sm">{user?.id}</div>
              </div>
            </div>
          </div>

          {/* Security Section */}
          <div className="bg-gray-900 rounded-lg border border-gray-800 p-6">
            <h2 className="text-lg font-semibold text-white mb-4">Security</h2>
            
            {error && (
              <div className="mb-4 p-3 bg-red-900/20 border border-red-800 rounded text-red-400 text-sm">
                {error}
              </div>
            )}
            
            {success && (
              <div className="mb-4 p-3 bg-green-900/20 border border-green-800 rounded text-green-400 text-sm">
                {success}
              </div>
            )}

            <div className="bg-gray-800 border border-gray-700 rounded p-4 mb-4">
              <h3 className="text-white font-semibold mb-2">WebAuthn / FIDO2</h3>
              <p className="text-sm text-gray-400 mb-4">
                Set up passwordless authentication using security keys, fingerprint, or face recognition.
              </p>
              
              <ul className="space-y-2 text-sm text-gray-400 mb-4">
                <li>✓ More secure than passwords</li>
                <li>✓ Phishing-resistant</li>
                <li>✓ Faster login experience</li>
                <li>✓ Industry-standard FIDO2 protocol</li>
              </ul>
              
              <button
                onClick={handleWebAuthnSetup}
                disabled={setupWebAuthn}
                className="bg-green-600 hover:bg-green-700 text-white px-4 py-2 rounded font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {setupWebAuthn ? 'Setting up...' : 'Set Up WebAuthn'}
              </button>
            </div>

            <div className="bg-gray-800 border border-gray-700 rounded p-4">
              <h3 className="text-white font-semibold mb-2">Change Password</h3>
              <p className="text-sm text-gray-400 mb-4">
                Update your password to keep your account secure.
              </p>
              <button className="bg-blue-600 hover:bg-blue-700 text-white px-4 py-2 rounded font-medium transition-colors">
                Change Password
              </button>
            </div>
          </div>

          {/* Trading Stats (Placeholder) */}
          <div className="bg-gray-900 rounded-lg border border-gray-800 p-6 mt-6">
            <h2 className="text-lg font-semibold text-white mb-4">Trading Statistics</h2>
            
            <div className="grid grid-cols-3 gap-4">
              <div>
                <div className="text-sm text-gray-400 mb-1">Total Trades</div>
                <div className="text-2xl font-bold text-white">0</div>
              </div>
              
              <div>
                <div className="text-sm text-gray-400 mb-1">Win Rate</div>
                <div className="text-2xl font-bold text-white">0%</div>
              </div>
              
              <div>
                <div className="text-sm text-gray-400 mb-1">Total P&L</div>
                <div className="text-2xl font-bold text-white">$0.00</div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </ProtectedRoute>
  )
}
