//! WebSocket transport for LSP communication.
//!
//! Connects to a remote LSP server over WebSocket (ws:// or wss://).
//! Each LSP message is sent/received as a single WebSocket binary frame.
//!
//! Feature: `transport-websocket`

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;

use crate::error::LspzError;

use super::Transport;

/// Transport over a WebSocket connection.
///
/// Connects to a remote LSP server that exposes a WebSocket endpoint.
/// Supports both `ws://` and `wss://` URLs.
pub struct WsTransport {
    ws_stream: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
}

impl WsTransport {
    /// Connect to a WebSocket URL.
    ///
    /// `url` should be a `ws://` or `wss://` URL (e.g. `"ws://localhost:8080/lsp"`).
    pub async fn connect(url: &str) -> Result<Self, LspzError> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(|e| LspzError::Config(format!("WebSocket connect failed: {e}")))?;
        Ok(Self { ws_stream })
    }
}

#[async_trait::async_trait]
impl Transport for WsTransport {
    async fn receive(&mut self) -> Result<Vec<u8>, LspzError> {
        loop {
            match self.ws_stream.next().await {
                Some(Ok(Message::Binary(data))) => return Ok(data.to_vec()),
                Some(Ok(Message::Close(_))) => return Err(LspzError::ServerExited),
                Some(Ok(Message::Ping(_))) => continue, // auto-pong handled by tungstenite
                Some(Ok(Message::Pong(_))) => continue,
                // Some servers send LSP frames as Text; treat UTF-8 bytes as the payload.
                Some(Ok(Message::Text(text))) => return Ok(text.as_bytes().to_vec()),
                Some(Ok(Message::Frame(_))) => continue,
                Some(Err(e)) => {
                    return Err(LspzError::Io(std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset,
                        e.to_string(),
                    )));
                }
                None => return Err(LspzError::ServerExited),
            }
        }
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), LspzError> {
        self.ws_stream
            .send(Message::Binary(data.to_vec().into()))
            .await
            .map_err(|e| {
                LspzError::Io(std::io::Error::new(
                    std::io::ErrorKind::ConnectionAborted,
                    e.to_string(),
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ws_connect_refused() {
        let result = WsTransport::connect("ws://127.0.0.1:29999").await;
        assert!(result.is_err());
    }
}
