import { useEffect, useState, useCallback } from 'react'
import { wsClient } from '@/lib/websocket'
import type { WebSocketMessage } from '@/types'

export function useWebSocket() {
  const [connected, setConnected] = useState(false)

  useEffect(() => {
    // Connect to WebSocket on mount
    wsClient.connect()
      .then(() => setConnected(true))
      .catch((error) => {
        console.error('WebSocket connection failed:', error)
        setConnected(false)
      })

    // Check connection status periodically
    const interval = setInterval(() => {
      setConnected(wsClient.isConnected())
    }, 1000)

    // Cleanup on unmount
    return () => {
      clearInterval(interval)
      // Don't disconnect - keep the singleton connection alive
    }
  }, [])

  const subscribe = useCallback((type: string, callback: (message: WebSocketMessage) => void) => {
    wsClient.subscribe(type, callback)
  }, [])

  const unsubscribe = useCallback((type: string, callback: (message: WebSocketMessage) => void) => {
    wsClient.unsubscribe(type, callback)
  }, [])

  const send = useCallback((data: any) => {
    wsClient.send(data)
  }, [])

  return {
    connected,
    subscribe,
    unsubscribe,
    send,
  }
}
