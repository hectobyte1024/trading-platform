import { api } from './api'

export interface User {
  id: string
  email: string
  name: string
  role: string
}

export interface AuthTokens {
  access_token: string
  refresh_token: string
  expires_in: number
}

export interface LoginCredentials {
  email: string
  password: string
}

export interface RegisterRequest {
  email: string
  password: string
  name: string
}

class AuthService {
  private readonly TOKEN_KEY = 'auth_token'
  private readonly REFRESH_TOKEN_KEY = 'refresh_token'
  private readonly USER_KEY = 'user'

  // Get current user from localStorage
  getCurrentUser(): User | null {
    if (typeof window === 'undefined') return null
    const userJson = localStorage.getItem(this.USER_KEY)
    return userJson ? JSON.parse(userJson) : null
  }

  // Get access token
  getAccessToken(): string | null {
    if (typeof window === 'undefined') return null
    return localStorage.getItem(this.TOKEN_KEY)
  }

  // Get refresh token
  getRefreshToken(): string | null {
    if (typeof window === 'undefined') return null
    return localStorage.getItem(this.REFRESH_TOKEN_KEY)
  }

  // Save tokens
  saveTokens(tokens: AuthTokens, user: User) {
    localStorage.setItem(this.TOKEN_KEY, tokens.access_token)
    localStorage.setItem(this.REFRESH_TOKEN_KEY, tokens.refresh_token)
    localStorage.setItem(this.USER_KEY, JSON.stringify(user))
  }

  // Clear tokens
  clearTokens() {
    localStorage.removeItem(this.TOKEN_KEY)
    localStorage.removeItem(this.REFRESH_TOKEN_KEY)
    localStorage.removeItem(this.USER_KEY)
  }

  // Login with email/password
  async login(credentials: LoginCredentials): Promise<{ user: User; tokens: AuthTokens }> {
    const response = await api.client.post('/auth/login', credentials)
    const { user, tokens } = response.data
    this.saveTokens(tokens, user)
    return { user, tokens }
  }

  // Register new user
  async register(request: RegisterRequest): Promise<{ user: User; tokens: AuthTokens }> {
    const response = await api.client.post('/auth/register', request)
    const { user, tokens } = response.data
    this.saveTokens(tokens, user)
    return { user, tokens }
  }

  // Logout
  async logout() {
    try {
      await api.client.post('/auth/logout')
    } catch (error) {
      console.error('Logout error:', error)
    } finally {
      this.clearTokens()
    }
  }

  // Refresh access token
  async refreshAccessToken(): Promise<AuthTokens> {
    const refreshToken = this.getRefreshToken()
    if (!refreshToken) {
      throw new Error('No refresh token available')
    }

    const response = await api.client.post('/auth/refresh', {
      refresh_token: refreshToken,
    })

    const tokens = response.data
    const user = this.getCurrentUser()
    if (user) {
      this.saveTokens(tokens, user)
    }
    return tokens
  }

  // Check if user is authenticated
  isAuthenticated(): boolean {
    return !!this.getAccessToken()
  }

  // WebAuthn registration
  async registerWebAuthn(userId: string): Promise<void> {
    // Get registration options from backend
    const optionsResponse = await api.client.post('/auth/webauthn/register/start', {
      user_id: userId,
    })
    const options = optionsResponse.data

    // Create credential
    const credential = await navigator.credentials.create({
      publicKey: {
        challenge: this.base64ToArrayBuffer(options.challenge),
        rp: options.rp,
        user: {
          id: this.base64ToArrayBuffer(options.user.id),
          name: options.user.name,
          displayName: options.user.displayName,
        },
        pubKeyCredParams: options.pubKeyCredParams,
        authenticatorSelection: options.authenticatorSelection,
        timeout: options.timeout,
        attestation: options.attestation,
      },
    })

    if (!credential) {
      throw new Error('Failed to create credential')
    }

    // Send credential to backend for verification
    await api.client.post('/auth/webauthn/register/finish', {
      credential: this.credentialToJSON(credential),
    })
  }

  // WebAuthn authentication
  async authenticateWebAuthn(email: string): Promise<{ user: User; tokens: AuthTokens }> {
    // Get authentication options from backend
    const optionsResponse = await api.client.post('/auth/webauthn/authenticate/start', {
      email,
    })
    const options = optionsResponse.data

    // Get credential
    const credential = await navigator.credentials.get({
      publicKey: {
        challenge: this.base64ToArrayBuffer(options.challenge),
        allowCredentials: options.allowCredentials.map((c: any) => ({
          ...c,
          id: this.base64ToArrayBuffer(c.id),
        })),
        timeout: options.timeout,
        userVerification: options.userVerification,
      },
    })

    if (!credential) {
      throw new Error('Failed to get credential')
    }

    // Send credential to backend for verification
    const response = await api.client.post('/auth/webauthn/authenticate/finish', {
      credential: this.credentialToJSON(credential),
    })

    const { user, tokens } = response.data
    this.saveTokens(tokens, user)
    return { user, tokens }
  }

  // Helper: Convert base64 to ArrayBuffer
  private base64ToArrayBuffer(base64: string): ArrayBuffer {
    const binary = atob(base64)
    const bytes = new Uint8Array(binary.length)
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i)
    }
    return bytes.buffer
  }

  // Helper: Convert credential to JSON
  private credentialToJSON(credential: Credential): any {
    const cred = credential as PublicKeyCredential
    const response = cred.response as AuthenticatorAttestationResponse | AuthenticatorAssertionResponse

    return {
      id: cred.id,
      rawId: this.arrayBufferToBase64(cred.rawId),
      type: cred.type,
      response: {
        clientDataJSON: this.arrayBufferToBase64(response.clientDataJSON),
        attestationObject: 'attestationObject' in response 
          ? this.arrayBufferToBase64(response.attestationObject)
          : undefined,
        authenticatorData: 'authenticatorData' in response
          ? this.arrayBufferToBase64(response.authenticatorData)
          : undefined,
        signature: 'signature' in response
          ? this.arrayBufferToBase64(response.signature)
          : undefined,
        userHandle: 'userHandle' in response && response.userHandle
          ? this.arrayBufferToBase64(response.userHandle)
          : undefined,
      },
    }
  }

  // Helper: Convert ArrayBuffer to base64
  private arrayBufferToBase64(buffer: ArrayBuffer): string {
    const bytes = new Uint8Array(buffer)
    let binary = ''
    for (let i = 0; i < bytes.length; i++) {
      binary += String.fromCharCode(bytes[i])
    }
    return btoa(binary)
  }
}

export const authService = new AuthService()
