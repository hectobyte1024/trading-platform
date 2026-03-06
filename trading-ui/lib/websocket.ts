import type { WebSocketMessage, Orderbook, Trade } from '@/types'

type MessageCallback = (message: WebSocketMessage) => void

class WebSocketClient {
  private ws: WebSocket | null = null
  private url: string
  private reconnectTimeout: number = 3000
  private reconnectAttempts: number = 0
  private maxReconnectAttempts: number = 10
  private callbacks: Map<string, Set<MessageCallback>> = new Map()
  private connected: boolean = false
  private activeSubscriptions: Set<string> = new Set()

  constructor(url?: string) {
    this.url = url || process.env.NEXT_PUBLIC_WS_URL || 'ws://localhost:8080/ws'
  }

  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        this.ws = new WebSocket(this.url)

        this.ws.onopen = () => {
          console.log('WebSocket connected')
          this.connected = true
          this.reconnectAttempts = 0
          
          // Re-send all active subscriptions
          this.activeSubscriptions.forEach(symbol => {
            this.send({
              type: 'subscribe',
              symbol,
            })
          })
          
          resolve()
        }

        this.ws.onmessage = (event) => {
          try {
            const message: WebSocketMessage = JSON.parse(event.data)
            this.handleMessage(message)
          } catch (error) {
            console.error('Failed to parse WebSocket message:', error)
          }
        }

        this.ws.onerror = (error) => {
          console.error('WebSocket error:', error)
          reject(error)
        }

        this.ws.onclose = () => {
          console.log('WebSocket disconnected')
          this.connected = false
          this.attemptReconnect()
        }
      } catch (error) {
        reject(error)
      }
    })
  }

  private attemptReconnect() {
    if (this.reconnectAttempts < this.maxReconnectAttempts) {
      this.reconnectAttempts++
      console.log(`Reconnecting... Attempt ${this.reconnectAttempts}`)
      setTimeout(() => {
        this.connect().catch(console.error)
      }, this.reconnectTimeout)
    } else {
      console.error('Max reconnection attempts reached')
    }
  }

  private handleMessage(message: WebSocketMessage) {
    const callbacks = this.callbacks.get(message.type)
    if (callbacks) {
      callbacks.forEach(callback => callback(message))
    }

    // Also call global listeners
    const globalCallbacks = this.callbacks.get('*')
    if (globalCallbacks) {
      globalCallbacks.forEach(callback => callback(message))
    }
  }

  subscribe(type: string, callback: MessageCallback) {
    if (!this.callbacks.has(type)) {
      this.callbacks.set(type, new Set())
    }
    this.callbacks.get(type)!.add(callback)

    // Track active subscription
    this.activeSubscriptions.add(type)

    // Send subscription message if connected
    // Backend expects: { type: 'subscribe', symbol: 'BTC/USD' }
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.send({
        type: 'subscribe',
        symbol: type,
      })
    }
  }

  unsubscribe(type: string, callback: MessageCallback) {
    const callbacks = this.callbacks.get(type)
    if (callbacks) {
      callbacks.delete(callback)
      if (callbacks.size === 0) {
        this.callbacks.delete(type)
        this.activeSubscriptions.delete(type)
        
        // Send unsubscription message
        // Backend expects: { type: 'unsubscribe', symbol: 'BTC/USD' }
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
          this.send({
            type: 'unsubscribe',
            symbol: type,
          })
        }
      }
    }
  }

  send(data: any) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(data))
    } else {
      console.warn('WebSocket not connected, message not sent:', data)
    }
  }

  disconnect() {
    if (this.ws) {
      this.ws.close()
      this.ws = null
      this.connected = false
    }
  }

  isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === WebSocket.OPEN
  }
}

// Export singleton instance
export const wsClient = new WebSocketClient()
