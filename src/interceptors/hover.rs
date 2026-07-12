//! Hover compression interceptor.
//!
//! Compresses `textDocument/hover` responses by:
//!
//! 1. Compacting markdown — collapsing blank lines, shortening code fences
//! 2. Reducing `MarkupKind` — `"markdown"` → `"m"`, `"plaintext"` → `"p"`
//! 3. Using compact field names — `contents`→`c`, `kind`→`k`, `value`→`v`, `range`→`r`
//!

use serde_json::Value;

use crate::error::LspzError;
use crate::interceptors::{Direction, Interceptor};

/// Compression interceptor for `textDocument/hover`.
///
/// Transforms LSP hover responses into a compact format.
/// On any error, logs a WARN and returns `Err` (fail-open in the chain).
pub struct HoverCompressor {
    /// Whether to compact markdown content (collapse blank lines, shorten fences).
    pub enable_markdown_compact: bool,
}

impl Default for HoverCompressor {
    fn default() -> Self {
        Self {
            enable_markdown_compact: true,
        }
    }
}

#[async_trait::async_trait]
impl Interceptor for HoverCompressor {
    fn name(&self) -> &str {
        "hover_compressor"
    }

    fn applies_to(&self, method: &str, direction: Direction) -> bool {
        method == "textDocument/hover" && direction == Direction::ServerToClient
    }

    async fn intercept(
        &self,
        _method: &str,
        params: Value,
        _direction: Direction,
    ) -> Result<Option<Value>, LspzError> {
        let compact = compress_hover(&params, self.enable_markdown_compact)?;
        Ok(Some(compact))
    }
}

/// Compress a hover response value.
fn compress_hover(params: &Value, markdown_compact: bool) -> Result<Value, LspzError> {
    let obj = params
        .as_object()
        .ok_or_else(|| LspzError::Protocol("hover params not an object".into()))?;

    let mut compact = serde_json::Map::new();

    // Compress contents
    if let Some(contents) = obj.get("contents") {
        compact.insert("c".into(), compress_contents(contents, markdown_compact)?);
    }

    // Compress range if present
    if let Some(range) = obj.get("range") {
        compact.insert("r".into(), compress_range(range));
    }

    Ok(Value::Object(compact))
}

/// Compress the `contents` field of a Hover response.
///
/// Handles three formats:
/// - `MarkupContent`: `{ kind, value }` → `{ k, v }` (with markdown compaction)
/// - `MarkedString` (string form): `"string"` → `"string"`
/// - `MarkedString` (object form): `{ language, value }` → `{ l, v }`
fn compress_contents(contents: &Value, markdown_compact: bool) -> Result<Value, LspzError> {
    match contents {
        // MarkupContent: { kind: "markdown", value: "..." }
        Value::Object(m) if m.contains_key("kind") => {
            let mut c = serde_json::Map::new();

            // kind → k
            if let Some(kind) = m.get("kind").and_then(|v| v.as_str()) {
                c.insert(
                    "k".into(),
                    Value::String(encode_markup_kind(kind).to_string()),
                );
            }

            // value → v (with optional markdown compaction)
            if let Some(value) = m.get("value").and_then(|v| v.as_str()) {
                let v = if markdown_compact {
                    compact_markdown(value)
                } else {
                    value.to_string()
                };
                c.insert("v".into(), Value::String(v));
            }

            Ok(Value::Object(c))
        }
        // MarkedString object: { language: "rust", value: "fn main()" }
        Value::Object(m) if m.contains_key("language") => {
            let mut c = serde_json::Map::new();
            if let Some(lang) = m.get("language").and_then(|v| v.as_str()) {
                c.insert("l".into(), Value::String(lang.to_string()));
            }
            if let Some(value) = m.get("value").and_then(|v| v.as_str()) {
                c.insert("v".into(), Value::String(value.to_string()));
            }
            Ok(Value::Object(c))
        }
        // Array of MarkedString / MarkupContent
        Value::Array(arr) => {
            let compressed: Vec<Value> = arr
                .iter()
                .map(|item| compress_contents(item, markdown_compact))
                .collect::<Result<Vec<_>, LspzError>>()?;
            Ok(Value::Array(compressed))
        }
        // Plain string — pass through
        Value::String(s) => Ok(Value::String(s.clone())),
        // Null or other — pass through
        other => Ok(other.clone()),
    }
}

/// Compress a range value.
fn compress_range(range: &Value) -> Value {
    match range {
        Value::Object(m) => {
            let mut c = serde_json::Map::new();
            if let Some(start) = m.get("start") {
                c.insert("s".into(), compress_position(start));
            }
            if let Some(end) = m.get("end") {
                c.insert("e".into(), compress_position(end));
            }
            Value::Object(c)
        }
        other => other.clone(),
    }
}

/// Compress a Position { line, character } → { l, c }.
fn compress_position(pos: &Value) -> Value {
    match pos {
        Value::Object(m) => {
            let mut c = serde_json::Map::new();
            if let Some(line) = m.get("line") {
                c.insert("l".into(), line.clone());
            }
            if let Some(ch) = m.get("character") {
                c.insert("c".into(), ch.clone());
            }
            Value::Object(c)
        }
        other => other.clone(),
    }
}

/// Compress markdown text:
/// - Collapse 2+ consecutive blank lines → single blank line
/// - Shorten code fence markers: ` ```language ` → `` `lc `` (first 2 chars), ` ``` ` → `` ` ``
/// - Trim trailing whitespace on each line
fn compact_markdown(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_blank = false;
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim_end();

        if trimmed.is_empty() {
            if prev_blank {
                continue;
            }
            prev_blank = true;
            result.push('\n');
        } else {
            prev_blank = false;
            if let Some(lang) = trimmed.strip_prefix("```") {
                let lang = lang.trim();
                result.push('`');
                if !lang.is_empty() {
                    let short: String = lang.chars().take(2).collect();
                    result.push_str(&short);
                }
                if lines.peek().is_some() {
                    result.push('\n');
                }
            } else {
                result.push_str(trimmed);
                if lines.peek().is_some() {
                    result.push('\n');
                }
            }
        }
    }

    result
}

/// Map MarkupKind to single char.
fn encode_markup_kind(kind: &str) -> &str {
    match kind {
        "markdown" => "m",
        "plaintext" => "p",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interceptors::Direction;
    use serde_json::json;

    fn make_compressor() -> HoverCompressor {
        HoverCompressor::default()
    }

    #[test]
    fn test_applies_to_hover() {
        let c = make_compressor();
        assert!(c.applies_to("textDocument/hover", Direction::ServerToClient));
        assert!(!c.applies_to("textDocument/didOpen", Direction::ServerToClient));
        assert!(!c.applies_to("textDocument/hover", Direction::ClientToServer));
    }

    #[tokio::test]
    async fn test_markdown_compact() {
        let c = make_compressor();
        let params = json!({
            "contents": {
                "kind": "markdown",
                "value": "Some text\n\n\n\nMore text\n```rust\nlet x = 1;\n```\n\n\nEnd"
            }
        });

        let result = c
            .intercept("textDocument/hover", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        let v = compact["c"]["v"].as_str().unwrap();

        // Blank lines collapsed
        assert!(!v.contains("\n\n\n"), "should collapse 3+ blank lines");
        // Code fence shortened: ```rust → `ru
        assert!(v.contains("`ru"), "should shorten ```rust to `ru");
        assert!(!v.contains("```rust"), "should not contain full fence");
    }

    #[tokio::test]
    async fn test_markup_kind_reduction() {
        let c = make_compressor();
        let params = json!({
            "contents": {
                "kind": "markdown",
                "value": "hello"
            }
        });

        let result = c
            .intercept("textDocument/hover", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        assert_eq!(compact["c"]["k"], "m", "markdown should reduce to m");
    }

    #[tokio::test]
    async fn test_plaintext_kind_reduction() {
        let c = make_compressor();
        let params = json!({
            "contents": {
                "kind": "plaintext",
                "value": "hello"
            }
        });

        let result = c
            .intercept("textDocument/hover", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        assert_eq!(compact["c"]["k"], "p", "plaintext should reduce to p");
    }

    #[tokio::test]
    async fn test_fail_open() {
        let c = make_compressor();
        // Null should fail — hover must be an object
        let result = c
            .intercept("textDocument/hover", Value::Null, Direction::ServerToClient)
            .await;

        assert!(result.is_err(), "null hover should fail open");
    }

    #[tokio::test]
    async fn test_roundtrip_preserves_content() {
        let c = make_compressor();
        let params = json!({
            "contents": {
                "kind": "plaintext",
                "value": "Hello World"
            },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 5 }
            }
        });

        let result = c
            .intercept("textDocument/hover", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();

        // Content value preserved
        assert_eq!(compact["c"]["v"], "Hello World");
        // Kind reduced
        assert_eq!(compact["c"]["k"], "p");
        // Range preserved with compact names
        assert_eq!(compact["r"]["s"]["l"], 0);
        assert_eq!(compact["r"]["e"]["c"], 5);
    }

    #[tokio::test]
    async fn test_empty_hover() {
        let c = make_compressor();
        // Minimal hover with empty fields
        let params = json!({
            "contents": {
                "kind": "markdown",
                "value": ""
            }
        });

        let result = c
            .intercept("textDocument/hover", params, Direction::ServerToClient)
            .await;

        assert!(result.is_ok(), "empty hover should not crash");
    }
}
