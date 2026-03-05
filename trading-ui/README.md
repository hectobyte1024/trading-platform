# Trading Platform UI

Modern, real-time trading interface built with Next.js 14, TypeScript, and TailwindCSS.

## Features

- **Real-time Market Data**: WebSocket integration for live orderbook and trade updates
- **Advanced Order Entry**: Support for Market and Limit orders with time-in-force options
- **Interactive Charts**: Real-time price charts with Recharts
- **Orderbook Visualization**: Live bid/ask ladder with depth visualization
- **Trade History**: Streaming trade feed with millisecond precision
- **Market Statistics**: 24h price, volume, and change metrics
- **Dark Mode**: Optimized for extended trading sessions

## Tech Stack

- **Framework**: Next.js 14 (App Router)
- **Language**: TypeScript 5.3
- **Styling**: TailwindCSS 3.4
- **State Management**: 
  - React Query (TanStack Query) for server state
  - Zustand for client state
- **Charts**: Recharts 2.12
- **Real-time**: Native WebSocket
- **HTTP Client**: Axios

## Getting Started

### Prerequisites

- Node.js >= 18.0.0
- npm >= 9.0.0
- Backend API running on `http://localhost:8080`
- WebSocket server on `ws://localhost:8080/ws`

### Installation

```bash
cd trading-ui
npm install
```

### Environment Variables

Create a `.env.local` file:

```bash
NEXT_PUBLIC_API_URL=http://localhost:8080
NEXT_PUBLIC_WS_URL=ws://localhost:8080/ws
```

### Development

```bash
npm run dev
```

Open [http://localhost:3000](http://localhost:3000) in your browser.

### Build

```bash
npm run build
npm start
```

## Project Structure

```
trading-ui/
├── app/                    # Next.js App Router
│   ├── layout.tsx         # Root layout with providers
│   ├── page.tsx           # Main trading dashboard
│   ├── globals.css        # Global styles
│   └── providers.tsx      # React Query provider
├── components/
│   ├── ui/                # UI primitives
│   │   └── Header.tsx     # Top navigation and symbol selector
│   └── trading/           # Trading-specific components
│       ├── TradingDashboard.tsx   # Main trading layout
│       ├── Orderbook.tsx          # Order book display
│       ├── OrderForm.tsx          # Order entry form
│       ├── TradeHistory.tsx       # Recent trades
│       ├── PriceChart.tsx         # Real-time price chart
│       └── MarketStats.tsx        # Market statistics
├── hooks/
│   └── useWebSocket.ts    # WebSocket hook
├── lib/
│   ├── api.ts            # REST API client
│   ├── websocket.ts      # WebSocket client
│   └── utils.ts          # Utility functions
├── types/
│   └── index.ts          # TypeScript type definitions
└── store/                # Zustand stores (future)
```

## Key Components

### TradingDashboard
Main trading interface with grid layout containing:
- Orderbook (left)
- Price chart / Trade history (center)
- Order entry form (right)
- Market statistics (top)

### Orderbook
Real-time bid/ask ladder showing:
- Price levels with sizes and totals
- Spread indicator
- Visual depth bars

### OrderForm
Order entry with:
- Buy/Sell side selector
- Market/Limit order types
- Price and quantity inputs
- Account balance display

### WebSocket Integration
Real-time data streaming for:
- Orderbook updates
- Trade executions
- Order status updates
- Position updates

## API Integration

The UI connects to the Rust backend via:

1. **REST API** (`/api/*`):
   - Place orders: `POST /orders`
   - Cancel orders: `DELETE /orders/:id`
   - Get account: `GET /accounts/:userId`
   - Get positions: `GET /accounts/:userId/positions`

2. **WebSocket** (`ws://localhost:8080/ws`):
   - Subscribe to market data
   - Receive real-time updates
   - Order and position notifications

## Styling

Custom TailwindCSS theme with:
- Buy/Sell color palette (green/red)
- Dark mode optimized
- Custom orderbook row styles
- Button variants for trading actions

## Type Safety

Full TypeScript coverage with type definitions matching the Rust backend:
- Order, Trade, Orderbook types
- WebSocket message types
- API request/response types

## Performance

- Server-side rendering with Next.js
- Optimized WebSocket reconnection
- React Query caching and deduplication
- Minimal re-renders with proper memoization

## License

Proprietary - Internal Use Only
