import axios, { AxiosInstance } from 'axios'
import type { Order, Orderbook, Account, CreateOrderRequest, CancelOrderRequest } from '@/types'

class TradingAPI {
  public client: AxiosInstance

  constructor(baseURL?: string) {
    this.client = axios.create({
      baseURL: baseURL || process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080',
      timeout: 10000,
      headers: {
        'Content-Type': 'application/json',
      },
    })

    // Add request interceptor for auth
    this.client.interceptors.request.use((config) => {
      const token = typeof window !== 'undefined' ? localStorage.getItem('auth_token') : null
      if (token) {
        config.headers.Authorization = `Bearer ${token}`
      }
      return config
    })
  }

  // Orders
  async placeOrder(request: CreateOrderRequest): Promise<Order> {
    const response = await this.client.post('/orders', request)
    return response.data
  }

  async cancelOrder(request: CancelOrderRequest): Promise<void> {
    await this.client.delete(`/orders/${request.order_id}`)
  }

  async getOrders(userId: string): Promise<Order[]> {
    const response = await this.client.get(`/users/${userId}/orders`)
    return response.data
  }

  // Orderbook
  async getOrderbook(symbol: string): Promise<Orderbook> {
    const response = await this.client.get(`/orderbook/${symbol}`)
    return response.data
  }

  // Account
  async getAccount(userId: string): Promise<Account> {
    const response = await this.client.get(`/accounts/${userId}`)
    return response.data
  }

  async getPositions(userId: string): Promise<Account> {
    const response = await this.client.get(`/accounts/${userId}/positions`)
    return response.data
  }

  // Health check
  async healthCheck(): Promise<{ status: string }> {
    const response = await this.client.get('/health')
    return response.data
  }
}

// Export singleton instance
export const api = new TradingAPI()
