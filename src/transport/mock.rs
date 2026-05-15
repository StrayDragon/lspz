//! Mock transport for testing LspSession without real LSP servers.
//!
//! Pre-program responses with `push_response` or `push_message`,
//! then inspect captured requests with `sent_messages`.

use std::sync::Mutex;

use crate::codec::json_rpc::LspMessage;
use crate::error::LspzError;

use super::Transport;

/// A mock transport for testing LspSession and AgentHandle.
///
/// Responses are enqueued with [`push_response`](MockTransport::push_response)
/// and returned in FIFO order by [`receive`](Transport::receive).
/// Messages sent via [`send`](Transport::send) are captured for later inspection.
///
/// # Example
///
/// ```ignore
/// let mock = MockTransport::new();
/// mock.push_message(&LspMessage::Notification {
///     method: "textDocument/publishDiagnostics".into(),
///     params: serde_json::json!({"diagnostics": []}),
/// });
/// ```
pub struct MockTransport {
    incoming: Mutex<Vec<Vec<u8>>>,
    pos: Mutex<usize>,
    sent: Mutex<Vec<Vec<u8>>>,
}

impl MockTransport {
    /// Create a new empty mock transport.
    pub fn new() -> Self {
        Self {
            incoming: Mutex::new(Vec::new()),
            pos: Mutex::new(0),
            sent: Mutex::new(Vec::new()),
        }
    }

    /// Enqueue a raw framed message to be returned by the next `receive()`.
    pub fn push_response(&self, frame: Vec<u8>) {
        self.incoming.lock().unwrap().push(frame);
    }

    /// Enqueue an `LspMessage` to be returned by the next `receive()`.
    ///
    /// Convenience wrapper that serializes the message to bytes first.
    pub fn push_message(&self, msg: &LspMessage) -> Result<(), LspzError> {
        let bytes = msg.to_bytes()?;
        self.push_response(bytes);
        Ok(())
    }

    /// Return all bytes sent via `send()` since creation.
    pub fn sent_messages(&self) -> Vec<Vec<u8>> {
        self.sent.lock().unwrap().clone()
    }

    /// Clear all queued responses (resets the response queue).
    pub fn clear_responses(&self) {
        self.incoming.lock().unwrap().clear();
        *self.pos.lock().unwrap() = 0;
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Transport for MockTransport {
    async fn receive(&mut self) -> Result<Vec<u8>, LspzError> {
        let incoming = self.incoming.lock().unwrap();
        let mut pos = self.pos.lock().unwrap();
        if *pos >= incoming.len() {
            return Err(LspzError::Protocol(
                "mock transport: no more responses".into(),
            ));
        }
        let result = incoming[*pos].clone();
        *pos += 1;
        Ok(result)
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), LspzError> {
        self.sent.lock().unwrap().push(data.to_vec());
        Ok(())
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::json_rpc::LspMessage;

    fn notification_frame(method: &str) -> Vec<u8> {
        LspMessage::Notification {
            method: method.into(),
            params: serde_json::json!({"key": "value"}),
        }
        .to_bytes()
        .unwrap()
    }

    fn request_frame(id: i64, method: &str) -> Vec<u8> {
        LspMessage::Request {
            id,
            method: method.into(),
            params: serde_json::json!({"workspace": "test"}),
        }
        .to_bytes()
        .unwrap()
    }

    #[tokio::test]
    async fn test_send_and_receive() {
        let mut mock = MockTransport::new();
        let frame = notification_frame("test/method");
        mock.push_response(frame.clone());

        mock.send(b"hello").await.unwrap();
        let received = mock.receive().await.unwrap();

        assert_eq!(received, frame);
        assert_eq!(mock.sent_messages(), vec![b"hello".to_vec()]);
    }

    #[tokio::test]
    async fn test_fifo_order() {
        let mut mock = MockTransport::new();
        let a = request_frame(1, "req/a");
        let b = request_frame(2, "req/b");
        mock.push_response(a.clone());
        mock.push_response(b.clone());

        assert_eq!(mock.receive().await.unwrap(), a);
        assert_eq!(mock.receive().await.unwrap(), b);
    }

    #[tokio::test]
    async fn test_receive_empty_returns_err() {
        let mut mock = MockTransport::new();
        let result = mock.receive().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_push_message() {
        let mut mock = MockTransport::new();
        let msg = LspMessage::Notification {
            method: "test".into(),
            params: serde_json::json!({"x": 1}),
        };
        mock.push_message(&msg).unwrap();

        let received = mock.receive().await.unwrap();
        let parsed = LspMessage::from_frame_bytes(&received).unwrap();
        assert_eq!(
            parsed,
            LspMessage::Notification {
                method: "test".into(),
                params: serde_json::json!({"x": 1}),
            }
        );
    }

    #[tokio::test]
    async fn test_sent_capture() {
        let mut mock = MockTransport::new();
        mock.send(b"frame1").await.unwrap();
        mock.send(b"frame2").await.unwrap();

        let sent = mock.sent_messages();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0], b"frame1");
        assert_eq!(sent[1], b"frame2");
    }

    #[tokio::test]
    async fn test_clear_responses() {
        let mut mock = MockTransport::new();
        mock.push_response(vec![1, 2, 3]);
        mock.clear_responses();

        let result = mock.receive().await;
        assert!(result.is_err());
    }
}
