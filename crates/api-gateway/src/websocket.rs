use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use common::*;
use event_journal::EventJournal;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use futures::{sink::SinkExt, stream::StreamExt};

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    /// Subscribe to market data for a symbol
    Subscribe {
        symbol: String,
    },
    /// Unsubscribe from market data
    Unsubscribe {
        symbol: String,
    },
    /// Trade notification
    Trade {
        symbol: String,
        price: String,
        quantity: String,
        side: String,
        timestamp: String,
    },
    /// Orderbook update
    OrderbookUpdate {
        symbol: String,
        bids: Vec<PriceLevel>,
        asks: Vec<PriceLevel>,
    },
    /// Order status update
    OrderUpdate {
        order_id: String,
        status: String,
        filled_quantity: String,
    },
    /// Error message
    Error {
        message: String,
    },
    /// Ping/Pong for connection health
    Ping,
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: String,
    pub quantity: String,
}

/// WebSocket handler
pub async fn ws_handler<J: EventJournal + 'static, R: RiskCheck + 'static>(
    ws: WebSocketUpgrade,
    State(state): State<crate::rest::AppState<J, R>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle individual WebSocket connection
async fn handle_socket<J: EventJournal + 'static, R: RiskCheck + 'static>(
    socket: WebSocket,
    state: crate::rest::AppState<J, R>,
) {
    tracing::info!("New WebSocket connection established");

    let (mut sender, mut receiver) = socket.split();

    // Create a channel for broadcasting market data
    let (tx, mut rx) = broadcast::channel::<WsMessage>(100);

    // Spawn a task to send messages to the client
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let json = match serde_json::to_string(&msg) {
                Ok(json) => json,
                Err(e) => {
                    tracing::error!("Failed to serialize message: {}", e);
                    continue;
                }
            };

            if sender.send(Message::Text(json)).await.is_err() {
                tracing::info!("Client disconnected");
                break;
            }
        }
    });

    // Process incoming messages from client
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    tracing::debug!("Received WebSocket message: {}", text);

                    let ws_msg: WsMessage = match serde_json::from_str(&text) {
                        Ok(msg) => msg,
                        Err(e) => {
                            tracing::error!("Failed to parse WebSocket message: {}", e);
                            let error_msg = WsMessage::Error {
                                message: format!("Invalid message format: {}", e),
                            };
                            let _ = tx.send(error_msg);
                            continue;
                        }
                    };

                    match ws_msg {
                        WsMessage::Subscribe { symbol } => {
                            tracing::info!("Client subscribed to {}", symbol);
                            
                            // Send initial orderbook snapshot
                            let symbol_obj = Symbol::new(&symbol);
                            if let Some(orderbook) = state.engine.get_orderbook(&symbol_obj).await {
                                let book = orderbook.read().await;
                                
                                let bids: Vec<PriceLevel> = book
                                    .bid_depth(10)
                                    .into_iter()
                                    .map(|(price, qty)| PriceLevel {
                                        price: price.to_string(),
                                        quantity: qty.to_string(),
                                    })
                                    .collect();

                                let asks: Vec<PriceLevel> = book
                                    .ask_depth(10)
                                    .into_iter()
                                    .map(|(price, qty)| PriceLevel {
                                        price: price.to_string(),
                                        quantity: qty.to_string(),
                                    })
                                    .collect();

                                let update = WsMessage::OrderbookUpdate {
                                    symbol: symbol.clone(),
                                    bids,
                                    asks,
                                };
                                
                                let _ = tx.send(update);
                            }
                        }
                        WsMessage::Unsubscribe { symbol } => {
                            tracing::info!("Client unsubscribed from {}", symbol);
                        }
                        WsMessage::Ping => {
                            let _ = tx.send(WsMessage::Pong);
                        }
                        _ => {
                            tracing::warn!("Unexpected message type from client");
                        }
                    }
                }
                Message::Binary(_) => {
                    tracing::warn!("Received binary message (not supported)");
                }
                Message::Ping(_) => {
                    // Axum handles ping/pong automatically
                }
                Message::Pong(_) => {
                    // Pong received
                }
                Message::Close(_) => {
                    tracing::info!("Client closed connection");
                    break;
                }
            }
        }
    });

    // Wait for either task to finish (which means the connection is closed)
    tokio::select! {
        _ = &mut send_task => {
            recv_task.abort();
        }
        _ = &mut recv_task => {
            send_task.abort();
        }
    }

    tracing::info!("WebSocket connection closed");
}

/// Broadcast a trade to all connected WebSocket clients
/// This would be called from the matching engine when a trade occurs
pub async fn broadcast_trade(
    symbol: &Symbol,
    price: Price,
    quantity: Quantity,
    side: Side,
) -> WsMessage {
    WsMessage::Trade {
        symbol: symbol.0.clone(),
        price: price.to_string(),
        quantity: quantity.to_string(),
        side: match side {
            Side::Buy => "buy".to_string(),
            Side::Sell => "sell".to_string(),
        },
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}

/// Broadcast an orderbook update
pub async fn broadcast_orderbook_update(
    symbol: &Symbol,
    bids: Vec<(Price, Quantity)>,
    asks: Vec<(Price, Quantity)>,
) -> WsMessage {
    let bids: Vec<PriceLevel> = bids
        .into_iter()
        .map(|(price, qty)| PriceLevel {
            price: price.to_string(),
            quantity: qty.to_string(),
        })
        .collect();

    let asks: Vec<PriceLevel> = asks
        .into_iter()
        .map(|(price, qty)| PriceLevel {
            price: price.to_string(),
            quantity: qty.to_string(),
        })
        .collect();

    WsMessage::OrderbookUpdate {
        symbol: symbol.0.clone(),
        bids,
        asks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_message_serialize() {
        let msg = WsMessage::Trade {
            symbol: "BTC/USD".to_string(),
            price: "50000".to_string(),
            quantity: "1.5".to_string(),
            side: "buy".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("trade"));
        assert!(json.contains("BTC/USD"));
    }

    #[test]
    fn test_ws_message_deserialize() {
        let json = r#"{"type":"subscribe","symbol":"BTC/USD"}"#;
        let msg: WsMessage = serde_json::from_str(json).unwrap();
        
        match msg {
            WsMessage::Subscribe { symbol } => {
                assert_eq!(symbol, "BTC/USD");
            }
            _ => panic!("Expected Subscribe message"),
        }
    }
}
