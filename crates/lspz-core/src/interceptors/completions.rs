//! Completion compression interceptor.
//!
//! Compresses `textDocument/completion` responses by:
//!
//! 1. Truncating to `max_items` (default 50)
//! 2. Pruning fields — drop `command`, `data`, `additionalTextEdits`, `commitCharacters`
//! 3. Reducing `CompletionItemKind` (1-25) to single-char codes
//! 4. Interning identical documentation strings
//! 5. Using compact field names
//!
//! [MermaidChart:./docs/mmd/completion-compression.mmd]

use serde_json::Value;

use crate::error::LspzError;
use crate::interceptors::{Direction, Interceptor};

/// Compression interceptor for `textDocument/completion`.
///
/// Transforms LSP completion responses into a compact format.
/// On any error, logs a WARN and returns `Err` (fail-open in the chain).
pub struct CompletionCompressor {
    /// Maximum number of completion items to keep (default: 50).
    pub max_items: usize,
    /// Whether to deduplicate identical documentation strings (default: true).
    pub enable_doc_dedup: bool,
}

impl Default for CompletionCompressor {
    fn default() -> Self {
        Self {
            max_items: 50,
            enable_doc_dedup: true,
        }
    }
}

#[async_trait::async_trait]
impl Interceptor for CompletionCompressor {
    fn name(&self) -> &str {
        "completion_compressor"
    }

    fn applies_to(&self, method: &str, direction: Direction) -> bool {
        method == "textDocument/completion" && direction == Direction::ServerToClient
    }

    async fn intercept(
        &self,
        _method: &str,
        params: Value,
        _direction: Direction,
    ) -> Result<Option<Value>, LspzError> {
        let compact = compress_completions(&params, self.max_items, self.enable_doc_dedup)?;
        Ok(Some(compact))
    }
}

/// Compress a completion response value.
fn compress_completions(
    params: &Value,
    max_items: usize,
    doc_dedup: bool,
) -> Result<Value, LspzError> {
    // Normalise: wrap bare array into CompletionList form
    let (items, is_incomplete) = if let Some(items) = params.as_array() {
        // Response is a bare CompletionItem[]
        (items.clone(), false)
    } else if let Some(list) = params.as_object() {
        let incomplete = list
            .get("isIncomplete")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let items = list
            .get("items")
            .and_then(|v| v.as_array())
            .ok_or_else(|| LspzError::Protocol("missing completion items".into()))?
            .clone();
        (items, incomplete)
    } else {
        // Null or other — return as-is
        return Ok(params.clone());
    };

    // Truncate
    let items: Vec<Value> = items.into_iter().take(max_items).collect();

    // Collect documentation strings for dedup — only pool docs used >1 time
    let doc_pool: Vec<String> = if doc_dedup {
        // First pass: count frequencies
        let mut freq: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for item in &items {
            if let Some(doc) = item.get("documentation").and_then(|v| v.as_str()) {
                *freq.entry(doc).or_default() += 1;
            }
        }
        // Second pass: collect docs that appear >1 time, preserving insertion order
        let mut pool: Vec<String> = Vec::new();
        for item in &items {
            if let Some(doc) = item.get("documentation").and_then(|v| v.as_str())
                && freq.get(doc).copied().unwrap_or(0) > 1
                && !pool.contains(&doc.to_string())
            {
                pool.push(doc.to_string());
            }
        }
        pool
    } else {
        Vec::new()
    };

    let compact_items: Vec<Value> = items
        .into_iter()
        .map(|item| {
            let mut c = serde_json::Map::new();

            // label -> l (required)
            if let Some(label) = item.get("label").and_then(|v| v.as_str()) {
                c.insert("l".into(), Value::String(label.to_string()));
            }

            // kind -> k (single char)
            if let Some(kind) = item.get("kind").and_then(|v| v.as_u64())
                && let Some(k) = encode_completion_kind(kind)
            {
                c.insert("k".into(), Value::String(k.to_string()));
            }

            // detail -> d
            if let Some(detail) = item.get("detail").and_then(|v| v.as_str())
                && !detail.is_empty()
            {
                c.insert("d".into(), Value::String(detail.to_string()));
            }

            // documentation -> doc (or doc_id if dedup enabled)
            if let Some(doc) = item.get("documentation").and_then(|v| v.as_str()) {
                if doc_dedup {
                    if let Some(idx) = doc_pool.iter().position(|d| d == doc) {
                        c.insert("doc_id".into(), Value::Number(idx.into()));
                    } else {
                        c.insert("doc".into(), Value::String(doc.to_string()));
                    }
                } else {
                    c.insert("doc".into(), Value::String(doc.to_string()));
                }
            }

            // insertText -> i
            if let Some(ins) = item.get("insertText").and_then(|v| v.as_str()) {
                c.insert("i".into(), Value::String(ins.to_string()));
            }

            // sortText -> st
            if let Some(st) = item.get("sortText").and_then(|v| v.as_str()) {
                c.insert("st".into(), Value::String(st.to_string()));
            }

            // deprecated -> dep
            if let Some(true) = item.get("deprecated").and_then(|v| v.as_bool()) {
                c.insert("dep".into(), Value::Bool(true));
            }

            // textEdit / insertTextFormat — keep as-is (valuable for AI agents)
            if let Some(te) = item.get("textEdit") {
                c.insert("te".into(), te.clone());
            }

            Value::Object(c)
        })
        .collect();

    let mut result = serde_json::Map::new();
    result.insert("version".into(), Value::Number(1.into()));
    result.insert("items".into(), Value::Array(compact_items));
    result.insert("incomplete".into(), Value::Bool(is_incomplete));

    // Inline doc pool for dedup reference
    if doc_dedup && !doc_pool.is_empty() {
        let pool_values: Vec<Value> = doc_pool.into_iter().map(Value::String).collect();
        result.insert("docs".into(), Value::Array(pool_values));
    }

    Ok(Value::Object(result))
}

/// Map LSP CompletionItemKind numeric value to a single character.
///
/// See https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#completionItemKind
fn encode_completion_kind(kind: u64) -> Option<char> {
    Some(match kind {
        1 => 'T',  // Text
        2 => 'M',  // Method
        3 => 'F',  // Function
        4 => 'C',  // Constructor
        5 => 'f',  // Field
        6 => 'V',  // Variable
        7 => 'c',  // Class
        8 => 'I',  // Interface
        9 => 'm',  // Module
        10 => 'P', // Property
        11 => 'U', // Unit
        12 => 'v', // Value
        13 => 'e', // Enum
        14 => 'K', // Keyword
        15 => 's', // Snippet
        16 => 'o', // Color
        17 => 'F', // File
        18 => 'r', // Reference
        19 => 'D', // Folder
        20 => 'n', // EnumMember
        21 => 'q', // Constant
        22 => 'S', // Struct
        23 => 'W', // Event
        24 => 'O', // Operator
        25 => 't', // TypeParameter
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interceptors::Direction;

    fn make_compressor() -> CompletionCompressor {
        CompletionCompressor::default()
    }

    #[test]
    fn test_applies_to_completion() {
        let c = make_compressor();
        assert!(c.applies_to("textDocument/completion", Direction::ServerToClient));
        assert!(!c.applies_to("textDocument/didOpen", Direction::ServerToClient));
        assert!(!c.applies_to("textDocument/completion", Direction::ClientToServer));
    }

    #[test]
    fn test_kind_encoding_all() {
        // All 25 known kind values
        let kinds = [
            (1, 'T'),
            (2, 'M'),
            (3, 'F'),
            (4, 'C'),
            (5, 'f'),
            (6, 'V'),
            (7, 'c'),
            (8, 'I'),
            (9, 'm'),
            (10, 'P'),
            (11, 'U'),
            (12, 'v'),
            (13, 'e'),
            (14, 'K'),
            (15, 's'),
            (16, 'o'),
            (17, 'F'),
            (18, 'r'),
            (19, 'D'),
            (20, 'n'),
            (21, 'q'),
            (22, 'S'),
            (23, 'W'),
            (24, 'O'),
            (25, 't'),
        ];
        for (input, expected) in &kinds {
            assert_eq!(
                encode_completion_kind(*input),
                Some(*expected),
                "kind {} should map to {}",
                input,
                expected
            );
        }
        // Unknown kind returns None
        assert_eq!(encode_completion_kind(0), None);
        assert_eq!(encode_completion_kind(99), None);
    }

    #[tokio::test]
    async fn test_field_pruning() {
        let c = make_compressor();
        let params = serde_json::json!([
            {
                "label": "test",
                "kind": 1,
                "detail": "a detail",
                "documentation": "doc text",
                "insertText": "test()",
                "sortText": "000",
                "deprecated": true,
                "command": {"command": "do", "title": "do it"},
                "data": "should be dropped",
                "additionalTextEdits": [{"range": {}, "newText": "x"}],
                "commitCharacters": [","]
            }
        ]);

        let result = c
            .intercept("textDocument/completion", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        let item = &compact["items"][0];

        // Should keep essential fields with compact names
        assert_eq!(item["l"], "test");
        assert_eq!(item["k"], "T");
        assert_eq!(item["d"], "a detail");
        assert_eq!(item["doc"], "doc text");
        assert_eq!(item["i"], "test()");
        assert_eq!(item["st"], "000");
        assert!(item["dep"].as_bool().unwrap());

        // Should drop command, data, additionalTextEdits, commitCharacters
        assert!(item.get("command").is_none());
        assert!(item.get("data").is_none());
        assert!(item.get("additionalTextEdits").is_none());
        assert!(item.get("commitCharacters").is_none());
    }

    #[tokio::test]
    async fn test_doc_dedup() {
        let c = make_compressor();
        let params = serde_json::json!([
            {"label": "a", "documentation": "shared doc"},
            {"label": "b", "documentation": "shared doc"},
            {"label": "c", "documentation": "unique doc"},
        ]);

        let result = c
            .intercept("textDocument/completion", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        let items = compact["items"].as_array().unwrap();

        // Items with shared doc should reference by doc_id
        assert_eq!(items[0].get("doc_id").and_then(|v| v.as_u64()), Some(0));
        assert_eq!(items[1].get("doc_id").and_then(|v| v.as_u64()), Some(0));
        // Unique doc should be inline
        assert_eq!(
            items[2].get("doc").and_then(|v| v.as_str()),
            Some("unique doc")
        );

        // doc pool should have 1 entry
        let docs = compact["docs"].as_array().unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].as_str(), Some("shared doc"));
    }

    #[tokio::test]
    async fn test_max_items_truncation() {
        let c = CompletionCompressor {
            max_items: 3,
            ..Default::default()
        };
        let items: Vec<Value> = (0..10)
            .map(|i| serde_json::json!({"label": format!("item_{}", i)}))
            .collect();

        let result = c
            .intercept(
                "textDocument/completion",
                Value::Array(items),
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let compact = result.unwrap();
        assert_eq!(compact["items"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn test_fail_open() {
        let c = make_compressor();
        // Null params should pass through
        let result = c
            .intercept(
                "textDocument/completion",
                Value::Null,
                Direction::ServerToClient,
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_roundtrip_preserves_labels() {
        let c = make_compressor();
        let params = serde_json::json!([
            {"label": "foo", "kind": 3},
            {"label": "bar", "kind": 6},
        ]);

        let result = c
            .intercept("textDocument/completion", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        let items = compact["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["l"], "foo");
        assert_eq!(items[0]["k"], "F");
        assert_eq!(items[1]["l"], "bar");
        assert_eq!(items[1]["k"], "V");
    }

    #[tokio::test]
    async fn test_completion_list_format() {
        let c = make_compressor();
        let params = serde_json::json!({
            "isIncomplete": false,
            "items": [
                {"label": "foo", "kind": 1},
                {"label": "bar", "kind": 2},
            ]
        });

        let result = c
            .intercept("textDocument/completion", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        assert_eq!(compact["version"], 1);
        assert_eq!(compact["incomplete"], false);
        assert_eq!(compact["items"].as_array().unwrap().len(), 2);
    }
}
