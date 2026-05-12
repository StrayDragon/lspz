//! Shared Content-Length framing for LSP transports.
//!
//! LSP uses a HTTP-like header `Content-Length: N\r\n\r\n` followed by N bytes of JSON body.
//! This module provides the common frame-reading logic shared by stdio and TCP transports.

use tokio::io::{AsyncRead, AsyncReadExt, BufReader};

use crate::error::LspzError;

/// Read one complete Content-Length framed LSP message.
///
/// Reads headers byte-by-byte until `\r\n\r\n`, parses `Content-Length`,
/// then reads exactly that many body bytes. Returns the full frame (header + body).
pub(crate) async fn read_frame<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
) -> Result<Vec<u8>, LspzError> {
    let mut buffer = Vec::with_capacity(4096);

    // Read headers until \r\n\r\n
    loop {
        let byte = reader.read_u8().await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                LspzError::ServerExited
            } else {
                LspzError::Io(e)
            }
        })?;
        buffer.push(byte);

        if buffer.len() >= 4 && buffer[buffer.len() - 4..] == [b'\r', b'\n', b'\r', b'\n'] {
            break;
        }
    }

    let header = std::str::from_utf8(&buffer)
        .map_err(|_| LspzError::Protocol("header is not valid UTF-8".into()))?;

    let content_length = parse_content_length(header)?;

    // Read the body
    let mut body = vec![0u8; content_length as usize];
    reader.read_exact(&mut body).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            LspzError::ServerExited
        } else {
            LspzError::Io(e)
        }
    })?;

    // Reconstruct full frame: header + body
    let mut frame = Vec::with_capacity(buffer.len() + body.len());
    frame.extend_from_slice(&buffer);
    frame.extend_from_slice(&body);
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
        let result = read_frame(&mut reader).await.unwrap();
        assert_eq!(result, frame);
    }

    #[tokio::test]
    async fn test_read_frame_truncated_body() {
        use std::io::Cursor;
        let frame = b"Content-Length: 10\r\n\r\nhi";
        let mut reader = BufReader::new(Cursor::new(frame));
        let result = read_frame(&mut reader).await;
        assert!(matches!(result, Err(LspzError::ServerExited)));
    }
}
