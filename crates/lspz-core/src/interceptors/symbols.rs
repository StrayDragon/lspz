//! DocumentSymbol / SymbolInformation compression interceptor.
//!
//! Compresses `textDocument/documentSymbol` responses by:
//!
//! 1. Compacting field names — `name`→`n`, `kind`→`k`, `range`→`r`, `children`→`c`, `detail`→`d`
//! 2. Reducing `SymbolKind` (1-26) to single-char codes
//! 3. Dropping non-essential fields — `deprecated`, `tags`, `selectionRange`
//! 4. Recursively compressing tree structures (hierarchical `DocumentSymbol` with `children`)
//! 5. Handling both `DocumentSymbol` (hierarchical) and `SymbolInformation` (flat) response types

use serde_json::Value;

use crate::error::LspzError;
use crate::interceptors::{Direction, Interceptor};

/// Compression interceptor for `textDocument/documentSymbol`.
///
/// Transforms LSP document symbol responses into a compact format.
/// On any error, logs a WARN and returns `Err` (fail-open in the chain).
pub struct DocumentSymbolCompressor;

impl Default for DocumentSymbolCompressor {
    fn default() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Interceptor for DocumentSymbolCompressor {
    fn name(&self) -> &str {
        "document_symbol_compressor"
    }

    fn applies_to(&self, method: &str, direction: Direction) -> bool {
        method == "textDocument/documentSymbol" && direction == Direction::ServerToClient
    }

    async fn intercept(
        &self,
        _method: &str,
        params: Value,
        _direction: Direction,
    ) -> Result<Option<Value>, LspzError> {
        let compact = compress_symbols(&params)?;
        Ok(Some(compact))
    }
}

/// Compress a `textDocument/documentSymbol` response value.
///
/// Handles both `DocumentSymbol[]` (hierarchical, has `range`) and
/// `SymbolInformation[]` (flat, has `location`) on a per-element basis.
fn compress_symbols(params: &Value) -> Result<Value, LspzError> {
    let arr = params
        .as_array()
        .ok_or_else(|| LspzError::Protocol("documentSymbol params not an array".into()))?;

    let items: Vec<Value> = arr.iter().map(compress_symbol).collect();

    let mut result = serde_json::Map::new();
    result.insert("version".into(), Value::Number(1.into()));
    result.insert("items".into(), Value::Array(items));
    Ok(Value::Object(result))
}

/// Compress a single symbol, detecting whether it is a DocumentSymbol or SymbolInformation.
fn compress_symbol(value: &Value) -> Value {
    match value {
        Value::Object(obj) if obj.contains_key("range") || obj.contains_key("selectionRange") => {
            compress_document_symbol(value)
        }
        Value::Object(obj) if obj.contains_key("location") => compress_symbol_information(value),
        other => other.clone(),
    }
}

/// Compress a hierarchical DocumentSymbol entry.
///
/// Field mapping:
/// - `name` → `n`, `kind` → `k` (char), `range` → `r`, `detail` → `d`, `children` → `c`
/// - Dropped: `deprecated`, `tags`, `selectionRange`
fn compress_document_symbol(value: &Value) -> Value {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return value.clone(),
    };

    let mut out = serde_json::Map::new();

    // name → n
    if let Some(name) = obj.get("name") {
        out.insert("n".into(), name.clone());
    }

    // kind → k (single char)
    if let Some(k) = obj
        .get("kind")
        .and_then(|v| v.as_u64())
        .and_then(encode_symbol_kind)
    {
        out.insert("k".into(), Value::String(k.to_string()));
    }

    // range → r
    if let Some(range) = obj.get("range") {
        out.insert("r".into(), compress_range(range));
    }

    // detail → d
    if let Some(detail) = obj.get("detail") {
        out.insert("d".into(), detail.clone());
    }

    // children → c (recursive)
    if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
        let compressed: Vec<Value> = children.iter().map(compress_document_symbol).collect();
        out.insert("c".into(), Value::Array(compressed));
    }

    Value::Object(out)
}

/// Compress a flat SymbolInformation entry.
///
/// Field mapping:
/// - `name` → `n`, `kind` → `k` (char), `location` → `l`, `containerName` → `cn`
/// - Dropped: `deprecated`, `tags`
fn compress_symbol_information(value: &Value) -> Value {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return value.clone(),
    };

    let mut out = serde_json::Map::new();

    // name → n
    if let Some(name) = obj.get("name") {
        out.insert("n".into(), name.clone());
    }

    // kind → k (single char)
    if let Some(k) = obj
        .get("kind")
        .and_then(|v| v.as_u64())
        .and_then(encode_symbol_kind)
    {
        out.insert("k".into(), Value::String(k.to_string()));
    }

    // location → l: { uri, range } → { u, r }
    if let Some(location) = obj.get("location") {
        out.insert("l".into(), compress_location(location));
    }

    // containerName → cn
    if let Some(cn) = obj.get("containerName") {
        out.insert("cn".into(), cn.clone());
    }

    Value::Object(out)
}

/// Compress a Location: `{ uri, range }` → `{ u, r }`.
pub(crate) fn compress_location(location: &Value) -> Value {
    match location {
        Value::Object(obj) => {
            let mut out = serde_json::Map::new();
            if let Some(uri) = obj.get("uri") {
                out.insert("u".into(), uri.clone());
            }
            if let Some(range) = obj.get("range") {
                out.insert("r".into(), compress_range(range));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// Compress a Range: `{ start, end }` → `{ s, e }`, each Position compacted to `{ l, c }`.
pub(crate) fn compress_range(range: &Value) -> Value {
    match range {
        Value::Object(obj) => {
            let mut out = serde_json::Map::new();
            if let Some(start) = obj.get("start") {
                out.insert("s".into(), compress_position(start));
            }
            if let Some(end) = obj.get("end") {
                out.insert("e".into(), compress_position(end));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// Compress a Position: `{ line, character }` → `{ l, c }`.
pub(crate) fn compress_position(pos: &Value) -> Value {
    match pos {
        Value::Object(obj) => {
            let mut out = serde_json::Map::new();
            if let Some(line) = obj.get("line") {
                out.insert("l".into(), line.clone());
            }
            if let Some(ch) = obj.get("character") {
                out.insert("c".into(), ch.clone());
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// Map LSP SymbolKind numeric value to a single character.
///
/// See <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#symbolKind>
pub(crate) fn encode_symbol_kind(kind: u64) -> Option<char> {
    Some(match kind {
        1 => 'F',  // File
        2 => 'M',  // Module
        3 => 'N',  // Namespace
        4 => 'P',  // Package
        5 => 's',  // ClasS
        6 => 'm',  // Method
        7 => 'p',  // Property
        8 => 'f',  // Field
        9 => 'c',  // Constructor
        10 => 'E', // Enum
        11 => 'I', // Interface
        12 => 'u', // FUnction
        13 => 'V', // Variable
        14 => 'C', // Constant
        15 => 'S', // String
        16 => 'b', // numBer
        17 => 'B', // Boolean
        18 => 'A', // Array
        19 => 'O', // Object
        20 => 'K', // Key
        21 => 'Z', // Null → Zero
        22 => 'e', // EnumMember
        23 => 't', // sTruct
        24 => 'v', // eVent
        25 => 'r', // opeRator
        26 => 'T', // TypeParameter
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interceptors::Direction;
    use serde_json::json;

    fn make_compressor() -> DocumentSymbolCompressor {
        DocumentSymbolCompressor
    }

    #[test]
    fn test_applies_to_document_symbol() {
        let c = make_compressor();
        assert!(c.applies_to("textDocument/documentSymbol", Direction::ServerToClient));
        assert!(!c.applies_to("textDocument/didOpen", Direction::ServerToClient));
        assert!(!c.applies_to("textDocument/documentSymbol", Direction::ClientToServer));
    }

    #[test]
    fn test_kind_encoding_all() {
        // All 26 known SymbolKind values
        let kinds = [
            (1, 'F'),
            (2, 'M'),
            (3, 'N'),
            (4, 'P'),
            (5, 's'),
            (6, 'm'),
            (7, 'p'),
            (8, 'f'),
            (9, 'c'),
            (10, 'E'),
            (11, 'I'),
            (12, 'u'),
            (13, 'V'),
            (14, 'C'),
            (15, 'S'),
            (16, 'b'),
            (17, 'B'),
            (18, 'A'),
            (19, 'O'),
            (20, 'K'),
            (21, 'Z'),
            (22, 'e'),
            (23, 't'),
            (24, 'v'),
            (25, 'r'),
            (26, 'T'),
        ];
        for (input, expected) in &kinds {
            assert_eq!(
                encode_symbol_kind(*input),
                Some(*expected),
                "kind {} should map to {}",
                input,
                expected
            );
        }
        // Unknown kind returns None
        assert_eq!(encode_symbol_kind(0), None);
        assert_eq!(encode_symbol_kind(99), None);
    }

    #[tokio::test]
    async fn test_document_symbol_compression() {
        let c = make_compressor();
        let params = json!([
            {
                "name": "MyStruct",
                "kind": 23,
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 10, "character": 1 }
                },
                "selectionRange": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 10, "character": 1 }
                },
                "detail": "A test struct",
                "deprecated": false,
                "tags": [],
                "children": [
                    {
                        "name": "field1",
                        "kind": 8,
                        "range": {
                            "start": { "line": 1, "character": 4 },
                            "end": { "line": 1, "character": 11 }
                        },
                        "selectionRange": {
                            "start": { "line": 1, "character": 4 },
                            "end": { "line": 1, "character": 11 }
                        }
                    },
                    {
                        "name": "method1",
                        "kind": 6,
                        "range": {
                            "start": { "line": 3, "character": 4 },
                            "end": { "line": 8, "character": 5 }
                        },
                        "selectionRange": {
                            "start": { "line": 3, "character": 4 },
                            "end": { "line": 8, "character": 5 }
                        }
                    }
                ]
            },
            {
                "name": "myFunction",
                "kind": 12,
                "range": {
                    "start": { "line": 12, "character": 0 },
                    "end": { "line": 20, "character": 1 }
                },
                "selectionRange": {
                    "start": { "line": 12, "character": 0 },
                    "end": { "line": 20, "character": 1 }
                }
            }
        ]);

        let result = c
            .intercept(
                "textDocument/documentSymbol",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let compact = result.unwrap();
        assert_eq!(compact["version"], 1);
        let items = compact["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);

        // First symbol: struct with children
        let s0 = &items[0];
        assert_eq!(s0["n"], "MyStruct");
        assert_eq!(s0["k"], "t"); // Struct → t
        assert_eq!(s0["d"], "A test struct");
        assert!(s0.get("r").is_some(), "range should be present");
        assert_eq!(s0["r"]["s"]["l"], 0);
        assert_eq!(s0["r"]["e"]["c"], 1);

        // Dropped fields
        assert!(s0.get("deprecated").is_none());
        assert!(s0.get("tags").is_none());
        assert!(s0.get("selectionRange").is_none());

        // Children are recursively compressed
        let children = s0["c"].as_array().unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0]["n"], "field1");
        assert_eq!(children[0]["k"], "f"); // Field → f
        assert!(children[0].get("selectionRange").is_none());
        assert_eq!(children[1]["n"], "method1");
        assert_eq!(children[1]["k"], "m"); // Method → m

        // Second symbol: function (no children, no detail)
        let s1 = &items[1];
        assert_eq!(s1["n"], "myFunction");
        assert_eq!(s1["k"], "u"); // Function → u
        assert!(s1.get("c").is_none());
        assert!(s1.get("d").is_none());
    }

    #[tokio::test]
    async fn test_symbol_information_compression() {
        let c = make_compressor();
        let params = json!([
            {
                "name": "SomeClass",
                "kind": 5,
                "location": {
                    "uri": "file:///src/lib.rs",
                    "range": {
                        "start": { "line": 10, "character": 0 },
                        "end": { "line": 50, "character": 1 }
                    }
                },
                "containerName": "my_module",
                "deprecated": false,
                "tags": []
            }
        ]);

        let result = c
            .intercept(
                "textDocument/documentSymbol",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let compact = result.unwrap();
        let items = compact["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);

        let s = &items[0];
        assert_eq!(s["n"], "SomeClass");
        assert_eq!(s["k"], "s"); // Class → s

        // Location compressed
        let loc = s["l"].as_object().unwrap();
        assert_eq!(loc["u"], "file:///src/lib.rs");
        assert_eq!(loc["r"]["s"]["l"], 10);
        assert_eq!(loc["r"]["e"]["l"], 50);

        // Container name
        assert_eq!(s["cn"], "my_module");

        // Dropped fields
        assert!(s.get("deprecated").is_none());
        assert!(s.get("tags").is_none());
    }

    #[tokio::test]
    async fn test_mixed_array() {
        let c = make_compressor();
        let params = json!([
            {
                "name": "myVar",
                "kind": 13,
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 5 }
                },
                "selectionRange": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 5 }
                }
            },
            {
                "name": "myFunc",
                "kind": 12,
                "location": {
                    "uri": "file:///src/main.rs",
                    "range": {
                        "start": { "line": 5, "character": 0 },
                        "end": { "line": 15, "character": 1 }
                    }
                },
                "containerName": "utils"
            }
        ]);

        let result = c
            .intercept(
                "textDocument/documentSymbol",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let compact = result.unwrap();
        let items = compact["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);

        // First: DocumentSymbol (has range) → has `r`, no `l`
        assert!(items[0].get("r").is_some(), "DS should have range");
        assert!(items[0].get("l").is_none(), "DS should not have location");
        assert_eq!(items[0]["k"], "V"); // Variable → V

        // Second: SymbolInformation (has location) → has `l`, no `r`
        assert!(items[1].get("l").is_some(), "SI should have location");
        assert!(items[1].get("r").is_none(), "SI should not have range");
        assert_eq!(items[1]["k"], "u"); // Function → u
    }

    #[tokio::test]
    async fn test_fail_open() {
        let c = make_compressor();
        // Null params should fail — must be an array
        let result = c
            .intercept(
                "textDocument/documentSymbol",
                Value::Null,
                Direction::ServerToClient,
            )
            .await;

        assert!(result.is_err(), "null should fail open");
    }

    #[tokio::test]
    async fn test_roundtrip_preserves_names() {
        let c = make_compressor();
        let params = json!([
            {
                "name": "preservedName",
                "kind": 2,
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 1 }
                },
                "selectionRange": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 1 }
                },
                "deprecated": true,
                "tags": [1]
            }
        ]);

        let result = c
            .intercept(
                "textDocument/documentSymbol",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let compact = result.unwrap();
        let item = &compact["items"][0];

        // Name preserved
        assert_eq!(item["n"], "preservedName");
        // Kind encoded
        assert_eq!(item["k"], "M"); // Module → M
        // Range present
        assert!(item.get("r").is_some());
        // Dropped fields absent
        assert!(
            item.get("deprecated").is_none(),
            "deprecated should be dropped"
        );
        assert!(item.get("tags").is_none(), "tags should be dropped");
        assert!(
            item.get("selectionRange").is_none(),
            "selectionRange should be dropped"
        );
    }
}
