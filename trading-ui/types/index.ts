// Type definitions for the trading platform

export interface Order {
  id: string
  user_id: string
  symbol: string
  side: 'Buy' | 'Sell'
  order_type: 'Limit' | 'Market'
  price: string
  quantity: string
  filled_quantity: string
  time_in_force: 'GTC' | 'IOC' | 'FOK' | 'GTD'
  status: 'Pending' | 'Open' | 'PartiallyFilled' | 'Filled' | 'Cancelled' | 'Rejected' | 'Expired'
  timestamp: string
  sequence_number: number
}

export interface Trade {
  id: string
  order_id: string
  counter_order_id: string
  symbol: string
  price: string
  quantity: string
  side: 'Buy' | 'Sell'
  timestamp: string
  sequence_number: number
}

export interface OrderbookLevel {
  price: string
  quantity: string
  orders: number
}

export interface Orderbook {
  symbol: string
  bids: OrderbookLevel[]
  asks: OrderbookLevel[]
  timestamp: string
}

export interface Position {
  symbol: string
  quantity: string
  average_price: string
  unrealized_pnl: string
  realized_pnl: string
  margin_used: string
}

export interface Account {
  user_id: string
  balance: string
  available_balance: string
  margin_used: string
  positions: Position[]
}

export interface MarketData {
  symbol: string
  last_price: string
  bid: string
  ask: string
  volume_24h: string
  high_24h: string
  low_24h: string
  change_24h: string
  timestamp: string
}

export interface WebSocketMessage {
  type: 'orderbook' | 'trade' | 'order_update' | 'position_update' | 'market_data'
  data: any
}

export interface CreateOrderRequest {
  symbol: string
  side: 'Buy' | 'Sell'
  order_type: 'Limit' | 'Market'
  price?: string
  quantity: string
  time_in_force?: 'GTC' | 'IOC' | 'FOK'
}

export interface CancelOrderRequest {
  order_id: string
}
