import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function formatPrice(price: string | number, decimals: number = 2): string {
  const num = typeof price === 'string' ? parseFloat(price) : price
  return num.toLocaleString('en-US', {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  })
}

export function formatQuantity(quantity: string | number, decimals: number = 4): string {
  const num = typeof quantity === 'string' ? parseFloat(quantity) : quantity
  return num.toLocaleString('en-US', {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  })
}

export function formatPercent(value: string | number, decimals: number = 2): string {
  const num = typeof value === 'string' ? parseFloat(value) : value
  const sign = num > 0 ? '+' : ''
  return `${sign}${num.toFixed(decimals)}%`
}

export function formatTimestamp(timestamp: string): string {
  return new Date(timestamp).toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

export function calculateTotal(price: string, quantity: string): string {
  const p = parseFloat(price)
  const q = parseFloat(quantity)
  return (p * q).toFixed(2)
}
