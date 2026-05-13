//! Workspace Symbol compression interceptor.
//!
//! Compresses `workspace/symbol` responses by:
//!
//! 1. URI pooling — unique URIs collected into a pool, each item references by index
//! 2. SymbolKind encoding — reuse `encode_symbol_kind` 1-26 single-char mapping
//! 3. Field pruning — `name`→`n`, `kind`→`k`, `containerName`→`c`, `location`→`l`
//! 4. Dropped fields — `deprecated`, `tags`, `data`

use serde_json::Value;

use crate::error::LspzError;
use crate::interceptors::symbols::{compress_range, encode_symbol_kind};
use crate::interceptors::{Direction, Interceptor};

/// Compression interceptor for `workspace/symbol` responses.
///
/// Reuses SymbolKind encoding from [`crate::interceptors::symbols::DocumentSymbolCompressor`] and URI pooling
/// from [`crate::interceptors::locations::LocationCompressor`] for maximum token savings.
pub struct WorkspaceSymbolCompressor;

impl Default for WorkspaceSymbolCompressor {
    fn default() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Interceptor for WorkspaceSymbolCompressor {
    fn name(&self) -> &str {
        "workspace_symbol_compressor"
    }

    fn applies_to(&self, method: &str, direction: Direction) -> bool {
        method == "workspace/symbol" && direction == Direction::ServerToClient
    }

    async fn intercept(
        &self,
        _method: &str,
        params: Value,
        _direction: Direction,
    ) -> Result<Option<Value>, LspzError> {
        let compact = compress_workspace_symbols(&params)?;
        Ok(Some(compact))
    }
}

/// Top-level compression entry point.
fn compress_workspace_symbols(params: &Value) -> Result<Value, LspzError> {
    let items = match params {
        Value::Array(arr) => arr.clone(),
        Value::Null => return Ok(empty_result()),
        _ => return Ok(empty_result()),
    };

    if items.is_empty() {
        return Ok(empty_result());
    }

    let uri_pool = build_uri_pool(&items);
    let compressed: Vec<Value> = items
        .iter()
        .map(|item| compress_symbol_item(item, &uri_pool))
        .collect();

    let uris_json: Vec<Value> = uri_pool.into_iter().map(Value::String).collect();

    let mut result = serde_json::Map::new();
    result.insert("version".into(), Value::Number(1.into()));
    result.insert("uris".into(), Value::Array(uris_json));
    result.insert("items".into(), Value::Array(compressed));
    Ok(Value::Object(result))
}

fn empty_result() -> Value {
    let mut result = serde_json::Map::new();
    result.insert("version".into(), Value::Number(1.into()));
    result.insert("uris".into(), Value::Array(Vec::new()));
    result.insert("items".into(), Value::Array(Vec::new()));
    Value::Object(result)
}

/// Build URI pool from SymbolInformation items (extract `location.uri`).
fn build_uri_pool(items: &[Value]) -> Vec<String> {
    let mut pool = Vec::new();
    for item in items {
        if let Some(uri) = item
            .as_object()
            .and_then(|obj| obj.get("location"))
            .and_then(|loc| loc.as_object())
            .and_then(|loc| loc.get("uri"))
            .and_then(|v| v.as_str())
            && !pool.iter().any(|u| u == uri)
        {
            pool.push(uri.to_string());
        }
    }
    pool
}

/// Compress a single SymbolInformation entry.
///
/// Field mapping:
/// - `name` → `n`
/// - `kind` → `k` (single char via encode_symbol_kind)
/// - `containerName` → `c`
/// - `location` → `l`: `{ uri, range }` → `{ u: pool_idx, r: { s, e } }`
/// - Dropped: `deprecated`, `tags`, `data`
fn compress_symbol_item(value: &Value, uri_pool: &[String]) -> Value {
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

    // containerName → c
    if let Some(cn) = obj.get("containerName") {
        out.insert("c".into(), cn.clone());
    }

    // location → l: { u: pool_idx, r: { s, e } }
    if let Some(location) = obj.get("location")
        && let Some(loc_obj) = location.as_object()
    {
        let mut l_out = serde_json::Map::new();

        // uri → u (pool index)
        if let Some(uri) = loc_obj.get("uri").and_then(|v| v.as_str()) {
            let idx = uri_pool.iter().position(|u| u == uri).unwrap_or(0);
            l_out.insert("u".into(), Value::Number(idx.into()));
        }

        // range → r
        if let Some(range) = loc_obj.get("range") {
            l_out.insert("r".into(), compress_range(range));
        }

        out.insert("l".into(), Value::Object(l_out));
    }

    Value::Object(out)
}

// ─── TOON ────────────────────────────────────────────────────────────────────

/// Convert compact workspace symbol response to TOON tabular format.
pub fn workspace_symbols_to_toon(value: &Value) -> Result<String, LspzError> {
    let uris = value
        .get("uris")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LspzError::Protocol("missing 'uris' array".into()))?;

    let items = value
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LspzError::Protocol("missing 'items' array".into()))?;

    let mut out = String::new();

    // URI pool header
    out.push_str(&format!("uris[{}]:\n", uris.len()));
    for uri in uris {
        if let Some(u) = uri.as_str() {
            out.push_str(&format!("  {}\n", u));
        }
    }

    // Table header
    let fields = ["name", "kind", "uri", "range", "container"];
    out.push_str(&format!(
        "symbols[{}]{{{}}}:\n",
        items.len(),
        fields.join(",")
    ));

    // Table rows
    for item in items {
        let name = item.get("n").and_then(|v| v.as_str()).unwrap_or("");
        let kind = item
            .get("k")
            .and_then(|v| v.as_str())
            .map(symbol_kind_from_char)
            .unwrap_or("unknown");

        let uri_idx = item
            .get("l")
            .and_then(|l| l.get("u"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let range_str = item
            .get("l")
            .and_then(|l| l.get("r"))
            .map(format_ws_range)
            .unwrap_or_default();

        let container = item.get("c").and_then(|v| v.as_str()).unwrap_or("");

        let name_col = if name.contains(',') {
            escape_csv(name)
        } else {
            name.to_string()
        };

        out.push_str(&format!(
            "  {},{},{},{},{}\n",
            name_col, kind, uri_idx, range_str, container,
        ));
    }

    Ok(out)
}

/// Map compact symbol kind char back to full string.
fn symbol_kind_from_char(k: &str) -> &'static str {
    match k {
        "F" => "file",
        "M" => "module",
        "N" => "namespace",
        "P" => "package",
        "s" => "class",
        "m" => "method",
        "p" => "property",
        "f" => "field",
        "c" => "constructor",
        "E" => "enum",
        "I" => "interface",
        "u" => "function",
        "V" => "variable",
        "C" => "constant",
        "S" => "string",
        "b" => "number",
        "B" => "boolean",
        "A" => "array",
        "O" => "object",
        "K" => "key",
        "Z" => "null",
        "e" => "enum_member",
        "t" => "struct",
        "v" => "event",
        "r" => "operator",
        "T" => "type_parameter",
        _ => "unknown",
    }
}

/// Format a compact range `{ s: { l, c }, e: { l, c } }` to `L:C-L:C`.
fn format_ws_range(range: &Value) -> String {
    let s = match range.get("s") {
        Some(s) => s,
        None => return String::new(),
    };
    let e = match range.get("e") {
        Some(e) => e,
        None => return String::new(),
    };
    let sl = s.get("l").and_then(|v| v.as_i64()).unwrap_or(0);
    let sc = s.get("c").and_then(|v| v.as_i64()).unwrap_or(0);
    let el = e.get("l").and_then(|v| v.as_i64()).unwrap_or(0);
    let ec = e.get("c").and_then(|v| v.as_i64()).unwrap_or(0);
    format!("{}:{}-{}:{}", sl, sc, el, ec)
}

/// Escape a string for CSV output.
fn escape_csv(s: &str) -> String {
    let s = s.replace('\n', "\\n");
    if s.contains(',') || s.contains('"') {
        let escaped = s.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        s
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interceptors::Direction;
    use serde_json::json;

    fn make_compressor() -> WorkspaceSymbolCompressor {
        WorkspaceSymbolCompressor
    }

    #[test]
    fn test_applies_to_workspace_symbol() {
        let c = make_compressor();
        assert!(c.applies_to("workspace/symbol", Direction::ServerToClient));
        assert!(!c.applies_to("workspace/symbol", Direction::ClientToServer));
        assert!(!c.applies_to("textDocument/documentSymbol", Direction::ServerToClient));
        assert!(!c.applies_to("workspace/diagnostic", Direction::ServerToClient));
    }

    #[tokio::test]
    async fn test_compress_basic() {
        let c = make_compressor();
        let params = json!([
            {
                "name": "myFunction",
                "kind": 12,
                "location": {
                    "uri": "file:///src/lib.rs",
                    "range": { "start": { "line": 10, "character": 0 }, "end": { "line": 20, "character": 1 } }
                },
                "containerName": "utils"
            }
        ]);

        let result = c
            .intercept("workspace/symbol", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        assert_eq!(compact["version"], 1);
        assert_eq!(compact["uris"].as_array().unwrap().len(), 1);
        assert_eq!(compact["uris"][0], "file:///src/lib.rs");

        let items = compact["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["n"], "myFunction");
        assert_eq!(items[0]["k"], "u"); // Function
        assert_eq!(items[0]["c"], "utils");
        assert_eq!(items[0]["l"]["u"], 0);
        assert_eq!(items[0]["l"]["r"]["s"]["l"], 10);
        assert_eq!(items[0]["l"]["r"]["e"]["l"], 20);
    }

    #[tokio::test]
    async fn test_compress_multiple_files() {
        let c = make_compressor();
        let params = json!([
            {
                "name": "foo",
                "kind": 5,
                "location": {
                    "uri": "file:///src/a.rs",
                    "range": { "start": { "line": 1, "character": 0 }, "end": { "line": 1, "character": 5 } }
                }
            },
            {
                "name": "bar",
                "kind": 12,
                "location": {
                    "uri": "file:///src/b.rs",
                    "range": { "start": { "line": 2, "character": 0 }, "end": { "line": 2, "character": 10 } }
                }
            },
            {
                "name": "baz",
                "kind": 13,
                "location": {
                    "uri": "file:///src/a.rs",
                    "range": { "start": { "line": 3, "character": 0 }, "end": { "line": 3, "character": 15 } }
                }
            }
        ]);

        let result = c
            .intercept("workspace/symbol", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        assert_eq!(compact["uris"].as_array().unwrap().len(), 2);

        let items = compact["items"].as_array().unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0]["l"]["u"], 0); // a.rs
        assert_eq!(items[1]["l"]["u"], 1); // b.rs
        assert_eq!(items[2]["l"]["u"], 0); // a.rs
    }

    #[tokio::test]
    async fn test_drop_deprecated_tags_data() {
        let c = make_compressor();
        let params = json!([
            {
                "name": "oldFunc",
                "kind": 12,
                "location": {
                    "uri": "file:///src/main.rs",
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 1 } }
                },
                "deprecated": true,
                "tags": [1],
                "data": "some_extra"
            }
        ]);

        let result = c
            .intercept("workspace/symbol", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        let item = &compact["items"][0];

        assert!(item.get("deprecated").is_none());
        assert!(item.get("tags").is_none());
        assert!(item.get("data").is_none());
        // Core fields preserved
        assert_eq!(item["n"], "oldFunc");
        assert_eq!(item["k"], "u");
    }

    #[tokio::test]
    async fn test_null_params() {
        let c = make_compressor();
        let result = c
            .intercept("workspace/symbol", Value::Null, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        assert_eq!(compact["version"], 1);
        assert!(compact["uris"].as_array().unwrap().is_empty());
        assert!(compact["items"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_empty_array() {
        let c = make_compressor();
        let result = c
            .intercept(
                "workspace/symbol",
                Value::Array(vec![]),
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let compact = result.unwrap();
        assert!(compact["uris"].as_array().unwrap().is_empty());
        assert!(compact["items"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_fail_open() {
        let c = make_compressor();
        // Non-array input should return empty result (not panic)
        let result = c
            .intercept(
                "workspace/symbol",
                Value::String("bad".into()),
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let compact = result.unwrap();
        assert!(compact["items"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_workspace_symbols_to_toon() {
        let value = json!({
            "version": 1,
            "uris": ["file:///src/lib.rs"],
            "items": [
                {
                    "n": "myFunction",
                    "k": "u",
                    "c": "utils",
                    "l": { "u": 0, "r": { "s": { "l": 10, "c": 0 }, "e": { "l": 20, "c": 1 } } }
                }
            ]
        });

        let toon = workspace_symbols_to_toon(&value).unwrap();
        assert!(toon.contains("uris[1]:"));
        assert!(toon.contains("  file:///src/lib.rs"));
        assert!(toon.contains("symbols[1]{"));
        assert!(toon.contains("myFunction,function"));

        eprintln!("\n=== Workspace Symbols TOON ===\n{}", toon);
    }

    #[test]
    fn test_workspace_symbols_to_toon_empty() {
        let value = json!({"version": 1, "uris": [], "items": []});
        let toon = workspace_symbols_to_toon(&value).unwrap();
        assert!(toon.contains("symbols[0]{"));
    }
}
