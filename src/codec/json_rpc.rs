//! JSON-RPC 2.0 Content-Length frame parser.
//!
//! Implements the LSP 3.17 wire format:
//! `Content-Length: N\r\n\r\n{body}`.
//!
//! ## Frame Parsing Algorithm
//!
//! Header until `\r\n\r\n`, then exactly `Content-Length` body bytes.

use serde::Serialize;
use serde_json::Value;
use std::fmt;

use crate::error::LspzError;

/// Maximum body size: 16 MB (SSOT: framing::MAX_BODY_SIZE).
use crate::transport::framing::MAX_BODY_SIZE;

/// A parsed JSON-RPC 2.0 message.
#[derive(Debug, Clone, PartialEq)]
pub enum LspMessage {
    /// A request with method + params + id.
    Request {
        id: i64,
        method: String,
        params: Value,
    },
    /// A response to a previous request.
    Response {
        id: i64,
        result: Option<Value>,
        error: Option<JsonRpcError>,
    },
    /// A notification (method + params, no id).
    Notification { method: String, params: Value },
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl fmt::Display for LspMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LspMessage::Request { id, method, .. } => {
                write!(f, "Request(id={}, method={})", id, method)
            }
            LspMessage::Response { id, .. } => write!(f, "Response(id={})", id),
            LspMessage::Notification { method, .. } => {
                write!(f, "Notification(method={})", method)
            }
        }
    }
}

// ─── Serialize ───────────────────────────────────────────────────────────────

impl LspMessage {
    /// Serialize this message into a Content-Length framed byte buffer.
    pub fn to_bytes(&self) -> Result<Vec<u8>, LspzError> {
        let body = self.to_json_value()?;
        serialize_frame(&body)
    }

    /// Convert to a [`serde_json::Value`] (JSON-RPC 2.0 request/response/notification).
    pub fn to_json_value(&self) -> Result<Value, LspzError> {
        let val = match self {
            LspMessage::Request { id, method, params } => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            }),
            LspMessage::Response { id, result, error } => {
                let mut obj = serde_json::Map::new();
                obj.insert("jsonrpc".into(), Value::String("2.0".into()));
                obj.insert("id".into(), Value::Number((*id).into()));
                if let Some(result) = result {
                    obj.insert("result".into(), result.clone());
                }
                if let Some(error) = error {
                    obj.insert("error".into(), serde_json::to_value(error)?);
                }
                Value::Object(obj)
            }
            LspMessage::Notification { method, params } => serde_json::json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": params,
            }),
        };
        Ok(val)
    }
}

// ─── Deserialize ─────────────────────────────────────────────────────────────

/// Parse a JSON-RPC 2.0 message from a [`serde_json::Value`].
pub fn parse_message(val: &Value) -> Result<LspMessage, LspzError> {
    let obj = val
        .as_object()
        .ok_or_else(|| LspzError::Protocol("message is not a JSON object".into()))?;

    // JSON-RPC version check (optional but spec says should be present)
    if let Some(v) = obj.get("jsonrpc")
        && v.as_str() != Some("2.0")
    {
        return Err(LspzError::Protocol(format!("invalid jsonrpc version: {v}")));
    }

    let method = obj.get("method").and_then(|v| v.as_str()).map(String::from);
    let has_id = obj.contains_key("id");
    let has_result = obj.contains_key("result");
    let has_error = obj.contains_key("error");

    match (has_id, method, has_result, has_error) {
        // Request: has id + method
        (true, Some(method), false, false) => {
            let params = obj.get("params").cloned().unwrap_or(Value::Null);
            let id = parse_id(obj)?;
            Ok(LspMessage::Request { id, method, params })
        }
        // Notification: no id + method
        (false, Some(method), false, false) => {
            let params = obj.get("params").cloned().unwrap_or(Value::Null);
            Ok(LspMessage::Notification { method, params })
        }
        // Response: has id + result xor error
        (true, None, true, false) => {
            let id = parse_id(obj)?;
            let result = obj.get("result").cloned();
            Ok(LspMessage::Response {
                id,
                result,
                error: None,
            })
        }
        (true, None, false, true) => {
            let id = parse_id(obj)?;
            let error: JsonRpcError = serde_json::from_value(obj["error"].clone())?;
            Ok(LspMessage::Response {
                id,
                result: None,
                error: Some(error),
            })
        }
        _ => Err(LspzError::Protocol(format!(
            "unrecognized message structure: {val}"
        ))),
    }
}

/// Parse the `id` field from a JSON object.
fn parse_id(obj: &serde_json::Map<String, Value>) -> Result<i64, LspzError> {
    match obj.get("id").and_then(|v| v.as_i64()) {
        Some(id) => Ok(id),
        None => Err(LspzError::Protocol(
            "message id must be a valid integer".into(),
        )),
    }
}

// ─── Frame Serialization ─────────────────────────────────────────────────────

/// Wrap a JSON value in a `Content-Length: N\r\n\r\n` frame.
pub fn serialize_frame(body: &Value) -> Result<Vec<u8>, LspzError> {
    let body_bytes = serde_json::to_vec(body)?;
    let header = format!("Content-Length: {}\r\n\r\n", body_bytes.len());
    let mut buf = Vec::with_capacity(header.len() + body_bytes.len());
    buf.extend_from_slice(header.as_bytes());
    buf.extend_from_slice(&body_bytes);
    Ok(buf)
}

// ─── Frame Deserialization ──────────────────────────────────────────────────

/// A partially parsed frame: header + body bytes.
#[derive(Debug)]
pub struct Frame {
    pub body: Vec<u8>,
}

/// Parse the next complete frame from a buffer.
///
/// Returns the `Frame` and the number of bytes consumed.
/// Returns `None` if more data is needed.
pub fn parse_frame(buf: &[u8]) -> Result<Option<(Frame, usize)>, LspzError> {
    // Find the header terminator \r\n\r\n
    let header_end = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4);

    let header_end = match header_end {
        Some(pos) => pos,
        None => return Ok(None), // Need more data
    };

    // Parse Content-Length from header
    let header = std::str::from_utf8(&buf[..header_end])
        .map_err(|_| LspzError::Protocol("header is not valid UTF-8".into()))?;

    let content_length = crate::transport::framing::parse_content_length(header)?;

    if content_length as usize > MAX_BODY_SIZE {
        return Err(LspzError::Protocol(format!(
            "body too large: {content_length} > {MAX_BODY_SIZE}"
        )));
    }

    // Check if we have the full body
    let total_len = header_end + content_length as usize;
    if buf.len() < total_len {
        return Ok(None); // Need more data
    }

    let body = buf[header_end..total_len].to_vec();
    let consumed = total_len;

    Ok(Some((Frame { body }, consumed)))
}

// ─── Stream Parser ──────────────────────────────────────────────────────────

/// A streaming frame parser that handles partial reads and concatenation.
pub struct FrameReader {
    buffer: Vec<u8>,
}

impl FrameReader {
    /// Create a new [`FrameReader`] with an empty internal buffer.
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(4096),
        }
    }

    /// Feed new data into the parser and try to extract a complete frame.
    ///
    /// Returns the parsed JSON [`Value`] if a complete frame was found.
    /// The remaining unconsumed data stays in the buffer for the next call.
    pub fn feed(&mut self, data: &[u8]) -> Result<Option<Value>, LspzError> {
        self.buffer.extend_from_slice(data);

        match parse_frame(&self.buffer)? {
            Some((frame, consumed)) => {
                self.buffer.drain(..consumed);
                let value: Value = serde_json::from_slice(&frame.body)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }
}

impl Default for FrameReader {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Utility ────────────────────────────────────────────────────────────────

impl LspMessage {
    /// Parse a [`LspMessage`] from a raw Content-Length framed byte slice.
    pub fn from_frame_bytes(bytes: &[u8]) -> Result<LspMessage, LspzError> {
        let (frame, _consumed) =
            parse_frame(bytes)?.ok_or_else(|| LspzError::Protocol("incomplete frame".into()))?;
        let value: Value = serde_json::from_slice(&frame.body)?;
        parse_message(&value)
    }

    /// Serialize and return the JSON body bytes (without Content-Length header).
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, LspzError> {
        let value = self.to_json_value()?;
        Ok(serde_json::to_vec(&value)?)
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a Content-Length frame from a JSON body string.
    fn frame(body: &str) -> Vec<u8> {
        format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
    }

    /// Helper: request body
    fn req_body(method: &str, id: i64) -> String {
        format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{{}}}}"#)
    }

    /// Helper: notification body
    fn not_body(method: &str) -> String {
        format!(r#"{{"jsonrpc":"2.0","method":"{method}","params":null}}"#)
    }

    // ─── Frame Parsing ───────────────────────────────────────────────────

    #[test]
    fn test_simple_request_frame() {
        let raw = frame(&req_body("test", 1));
        let (f, _) = parse_frame(&raw).unwrap().unwrap();
        let msg = parse_message(&serde_json::from_slice(&f.body).unwrap()).unwrap();
        match msg {
            LspMessage::Request { id, method, .. } => {
                assert_eq!(id, 1);
                assert_eq!(method, "test");
            }
            _ => panic!("expected Request"),
        }
    }

    #[test]
    fn test_notification_frame() {
        let raw = frame(&not_body("exit"));
        let (f, consumed) = parse_frame(&raw).unwrap().unwrap();
        assert_eq!(consumed, raw.len());
        let msg = parse_message(&serde_json::from_slice(&f.body).unwrap()).unwrap();
        match msg {
            LspMessage::Notification { method, .. } => {
                assert_eq!("exit", method);
            }
            _ => panic!("expected Notification"),
        }
    }

    #[test]
    fn test_response_frame() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":null}"#;
        let raw = frame(body);
        let (f, _) = parse_frame(&raw).unwrap().unwrap();
        let msg = parse_message(&serde_json::from_slice(&f.body).unwrap()).unwrap();
        match msg {
            LspMessage::Response { id, result, error } => {
                assert_eq!(id, 1);
                assert!(result.is_some());
                assert!(error.is_none());
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_response_with_error() {
        let body =
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"method not found"}}"#;
        let raw = frame(body);
        let (f, _) = parse_frame(&raw).unwrap().unwrap();
        let msg = parse_message(&serde_json::from_slice(&f.body).unwrap()).unwrap();
        match msg {
            LspMessage::Response { id, result, error } => {
                assert_eq!(id, 1);
                assert!(result.is_none());
                let err = error.unwrap();
                assert_eq!(err.code, -32601);
                assert_eq!(err.message, "method not found");
            }
            _ => panic!("expected Response"),
        }
    }

    // ─── Serialize Round-trip ────────────────────────────────────────────

    #[test]
    fn test_roundtrip_request() {
        let msg = LspMessage::Request {
            id: 42,
            method: "test".into(),
            params: serde_json::json!({"key": "value"}),
        };
        let bytes = msg.to_bytes().unwrap();
        let parsed = LspMessage::from_frame_bytes(&bytes).unwrap();
        assert_eq!(msg, parsed);
    }

    #[test]
    fn test_roundtrip_notification() {
        let msg = LspMessage::Notification {
            method: "exit".into(),
            params: Value::Null,
        };
        let bytes = msg.to_bytes().unwrap();
        let parsed = LspMessage::from_frame_bytes(&bytes).unwrap();
        assert_eq!(msg, parsed);
    }

    #[test]
    fn test_roundtrip_response() {
        let msg = LspMessage::Response {
            id: 7,
            result: Some(serde_json::json!({"ok": true})),
            error: None,
        };
        let bytes = msg.to_bytes().unwrap();
        let parsed = LspMessage::from_frame_bytes(&bytes).unwrap();
        assert_eq!(msg, parsed);
    }

    // ─── Partial / Streaming ─────────────────────────────────────────────

    #[test]
    fn test_partial_frame_returns_none() {
        let raw = b"Content-Length: 100\r\n\r\n{\"partial\"";
        assert!(parse_frame(raw).unwrap().is_none());
    }

    #[test]
    fn test_frame_reader_streaming() {
        let mut reader = FrameReader::new();
        let body = req_body("test", 1);
        let total = frame(&body);
        let mid = total.len() / 2;
        let part1 = &total[..mid];
        let part2 = &total[mid..];

        assert!(reader.feed(part1).unwrap().is_none());
        let result = reader.feed(part2).unwrap();
        assert!(result.is_some());
        let msg = parse_message(&result.unwrap()).unwrap();
        match msg {
            LspMessage::Request { id, method, .. } => {
                assert_eq!(id, 1);
                assert_eq!(method, "test");
            }
            _ => panic!("expected Request"),
        }
    }

    #[test]
    fn test_partial_header_returns_none() {
        let raw = b"Content-Length:";
        assert!(parse_frame(raw).unwrap().is_none());
    }

    #[test]
    fn test_two_frames_in_one_buffer() {
        let raw1 = frame(&req_body("a", 1));
        let raw2 = frame(&req_body("b", 2));
        let combined = [raw1.as_slice(), raw2.as_slice()].concat();

        let (f1, c1) = parse_frame(&combined).unwrap().unwrap();
        let v1: Value = serde_json::from_slice(&f1.body).unwrap();
        assert_eq!(v1["id"], 1);
        let (f2, c2) = parse_frame(&combined[c1..]).unwrap().unwrap();
        let v2: Value = serde_json::from_slice(&f2.body).unwrap();
        assert_eq!(v2["id"], 2);
        assert_eq!(c1 + c2, combined.len());
    }

    // ─── Edge Cases ──────────────────────────────────────────────────────

    #[test]
    fn test_content_length_case_insensitive() {
        let raw = frame(&req_body("t", 1))
            .into_iter()
            .map(|b| b.to_ascii_lowercase())
            .collect::<Vec<_>>();
        assert!(parse_frame(&raw).unwrap().is_some());
    }

    #[test]
    fn test_extra_whitespace_in_header() {
        let body = req_body("t", 1);
        let raw = format!("Content-Length:   {}   \r\n\r\n{}", body.len(), body);
        assert!(parse_frame(raw.as_bytes()).unwrap().is_some());
    }

    #[test]
    fn test_missing_content_length_error() {
        let raw = b"\r\n\r\n{}";
        match parse_frame(raw) {
            Err(LspzError::Protocol(msg)) => {
                assert!(msg.contains("missing"));
            }
            _ => panic!("expected Protocol error"),
        }
    }

    #[test]
    fn test_body_too_large() {
        let huge = MAX_BODY_SIZE + 1;
        let header = format!("Content-Length: {huge}\r\n\r\n");
        match parse_frame(header.as_bytes()) {
            Err(LspzError::Protocol(msg)) => {
                assert!(msg.contains("too large"));
            }
            _ => panic!("expected Protocol error"),
        }
    }

    #[test]
    fn test_invalid_json_errors() {
        let raw = b"Content-Length: 5\r\n\r\n{null}";
        let (frame, _) = parse_frame(raw).unwrap().unwrap();
        let val: Result<Value, _> = serde_json::from_slice(&frame.body);
        assert!(val.is_err());
    }

    #[test]
    fn test_message_no_jsonrpc_version() {
        // Some LSP servers omit "jsonrpc" — should still parse if structure is clear
        let raw = b"Content-Length: 33\r\n\r\n{\"id\":1,\"method\":\"t\",\"params\":{}}";
        let (frame, _) = parse_frame(raw).unwrap().unwrap();
        let val: Value = serde_json::from_slice(&frame.body).unwrap();
        let msg = parse_message(&val).unwrap();
        match msg {
            LspMessage::Request { id, .. } => assert_eq!(id, 1),
            _ => panic!("expected Request"),
        }
    }

    #[test]
    fn test_parse_message_invalid_type() {
        let val = Value::Array(vec![]);
        match parse_message(&val) {
            Err(LspzError::Protocol(msg)) => assert!(msg.contains("not a JSON object")),
            _ => panic!("expected Protocol error"),
        }
    }

    #[test]
    fn test_to_bytes_contains_content_length() {
        let msg = LspMessage::Notification {
            method: "exit".into(),
            params: Value::Null,
        };
        let bytes = msg.to_bytes().unwrap();
        let as_str = String::from_utf8_lossy(&bytes);
        assert!(as_str.starts_with("Content-Length:"));
        assert!(as_str.contains("\r\n\r\n"));
    }
}
