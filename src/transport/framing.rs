//! Shared Content-Length framing for LSP transports.
//!
//! LSP uses a HTTP-like header `Content-Length: N\r\n\r\n` followed by N bytes of JSON body.
//! This module provides cancel-safe frame-reading logic shared by stdio and TCP transports.

use tokio::io::{AsyncRead, AsyncReadExt, BufReader};

use crate::error::LspzError;

/// Maximum LSP message body size: 16 MiB (shared with codec layer).
pub const MAX_BODY_SIZE: usize = 16 * 1024 * 1024;

/// Maximum header size before `\r\n\r\n` (DoS guard).
const MAX_HEADER_SIZE: usize = 64 * 1024;

/// Cancel-safe partial-frame accumulator.
///
/// Progress is stored here so a cancelled `read_frame` future (e.g. from
/// `tokio::select!`) does not lose bytes already pulled from the underlying reader.
#[derive(Debug, Default)]
pub struct FrameState {
    header: Vec<u8>,
    body: Option<Vec<u8>>,
    body_pos: usize,
    content_length: Option<usize>,
}

impl FrameState {
    /// Create an empty frame state.
    pub fn new() -> Self {
        Self::default()
    }

    fn clear(&mut self) {
        self.header.clear();
        self.body = None;
        self.body_pos = 0;
        self.content_length = None;
    }
}

/// Cancel-safe frame read using persistent [`FrameState`].
///
/// Prefer keeping a long-lived [`FrameState`] on the transport so cancelled
/// `select!` branches do not lose bytes.
pub(crate) async fn read_frame_with_state<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    state: &mut FrameState,
) -> Result<Vec<u8>, LspzError> {
    // Phase 1: header until \r\n\r\n
    while state.content_length.is_none() {
        let byte = reader.read_u8().await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                LspzError::ServerExited
            } else {
                LspzError::Io(e)
            }
        })?;
        state.header.push(byte);

        if state.header.len() > MAX_HEADER_SIZE {
            state.clear();
            return Err(LspzError::Protocol("header too large".into()));
        }

        if state.header.len() >= 4 && state.header[state.header.len() - 4..] == *b"\r\n\r\n" {
            let header = std::str::from_utf8(&state.header)
                .map_err(|_| LspzError::Protocol("header is not valid UTF-8".into()))?;

            let content_length = parse_content_length(header)?;
            if content_length as usize > MAX_BODY_SIZE {
                state.clear();
                return Err(LspzError::Protocol(format!(
                    "body too large: {content_length} > {MAX_BODY_SIZE}"
                )));
            }
            let len = content_length as usize;
            state.content_length = Some(len);
            state.body = Some(vec![0u8; len]);
            state.body_pos = 0;
        }
    }

    // Phase 2: body (read() is cancel-safe: cancelled polls leave BufReader intact)
    let content_length = state.content_length.expect("set in phase 1");
    while state.body_pos < content_length {
        let body = state
            .body
            .as_mut()
            .expect("body allocated when content_length set");
        let n = reader
            .read(&mut body[state.body_pos..])
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    LspzError::ServerExited
                } else {
                    LspzError::Io(e)
                }
            })?;
        if n == 0 {
            return Err(LspzError::ServerExited);
        }
        state.body_pos += n;
    }

    let body = state.body.take().expect("body present after fill");
    let mut frame = Vec::with_capacity(state.header.len() + body.len());
    frame.extend_from_slice(&state.header);
    frame.extend_from_slice(&body);
    state.clear();
    Ok(frame)
}

/// Parse `Content-Length` from the header string.
pub(crate) fn parse_content_length(header: &str) -> Result<u64, LspzError> {
    for line in header.lines() {
        let line = line.trim();
        if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
            let value = value.trim();
            return value
                .parse::<u64>()
                .map_err(|_| LspzError::Protocol(format!("invalid Content-Length: {value}")));
        }
    }
    Err(LspzError::Protocol("missing Content-Length header".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content_length_basic() {
        let header = "Content-Length: 47\r\n\r\n";
        assert_eq!(parse_content_length(header).unwrap(), 47);
    }

    #[test]
    fn test_parse_content_length_case_insensitive() {
        let header = "content-length: 128\r\n\r\n";
        assert_eq!(parse_content_length(header).unwrap(), 128);
    }

    #[test]
    fn test_parse_content_length_whitespace() {
        let header = "Content-Length:   99   \r\n\r\n";
        assert_eq!(parse_content_length(header).unwrap(), 99);
    }

    #[test]
    fn test_missing_content_length() {
        let header = "\r\n\r\n";
        match parse_content_length(header) {
            Err(LspzError::Protocol(msg)) => assert!(msg.contains("missing")),
            _ => panic!("expected Protocol error"),
        }
    }

    #[test]
    fn test_invalid_content_length() {
        let header = "Content-Length: abc\r\n\r\n";
        match parse_content_length(header) {
            Err(LspzError::Protocol(msg)) => assert!(msg.contains("invalid")),
            _ => panic!("expected Protocol error"),
        }
    }

    /// Read a frame from an in-memory buffer.
    #[tokio::test]
    async fn test_read_frame_from_buf() {
        use std::io::Cursor;
        let frame = b"Content-Length: 6\r\n\r\nhello!";
        let mut reader = BufReader::new(Cursor::new(frame));
        let mut state = FrameState::new();
        let result = read_frame_with_state(&mut reader, &mut state)
            .await
            .unwrap();
        assert_eq!(result, frame);
    }

    #[tokio::test]
    async fn test_read_frame_truncated_body() {
        use std::io::Cursor;
        let frame = b"Content-Length: 10\r\n\r\nhi";
        let mut reader = BufReader::new(Cursor::new(frame));
        let mut state = FrameState::new();
        let result = read_frame_with_state(&mut reader, &mut state).await;
        assert!(matches!(result, Err(LspzError::ServerExited)));
    }

    #[tokio::test]
    async fn test_read_frame_rejects_oversized_body() {
        use std::io::Cursor;
        let huge = MAX_BODY_SIZE + 1;
        let header = format!("Content-Length: {huge}\r\n\r\n");
        let mut reader = BufReader::new(Cursor::new(header.into_bytes()));
        let mut state = FrameState::new();
        let result = read_frame_with_state(&mut reader, &mut state).await;
        match result {
            Err(LspzError::Protocol(msg)) => assert!(msg.contains("body too large")),
            other => panic!("expected body too large, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_frame_state_survives_partial_header() {
        use std::io::Cursor;
        let part1 = b"Content-Length: 5\r\n";
        let part2 = b"\r\nhello";
        let mut state = FrameState::new();

        let mut reader = BufReader::new(Cursor::new(part1.as_slice()));
        // Drain part1 into state (will await more and hit EOF on Cursor end)
        let r1 = read_frame_with_state(&mut reader, &mut state).await;
        assert!(matches!(r1, Err(LspzError::ServerExited)));
        assert!(!state.header.is_empty());

        // Continue with part2 on a fresh reader but same state
        let mut reader2 = BufReader::new(Cursor::new(part2.as_slice()));
        let frame = read_frame_with_state(&mut reader2, &mut state)
            .await
            .unwrap();
        assert_eq!(frame, b"Content-Length: 5\r\n\r\nhello");
    }
}
