//! Location / LocationLink compression interceptor.
//!
//! Compresses responses from `textDocument/references`, `textDocument/definition`,
//! `textDocument/implementation`, and `textDocument/typeDefinition` by:
//!
//! 1. URI pooling — unique URIs collected into a pool, each item references by index
//! 2. Range compression — `{start:{line,character}, end:{line,character}}` → `{s:{l,c}, e:{l,c}}`
//! 3. Field pruning — Location: `uri`→`u`(pool idx), `range`→`r`; LocationLink: `targetUri`→`u`,
//!    `targetRange`→`r`, `targetSelectionRange`→`s`, `originSelectionRange`→`o`
//! 4. Normalize to array — single object → array of 1, null → empty array
//!
//! All 4 methods share the same `Location` / `LocationLink` response types.

use serde_json::Value;

use crate::error::LspzError;
use crate::interceptors::{Direction, Interceptor};

/// Compression interceptor for Location / LocationLink responses.
///
/// Covers `textDocument/references`, `textDocument/definition`,
/// `textDocument/implementation`, and `textDocument/typeDefinition`.
/// On any error, logs a WARN and returns `Err` (fail-open in the chain).
pub struct LocationCompressor;

impl Default for LocationCompressor {
    fn default() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Interceptor for LocationCompressor {
    fn name(&self) -> &str {
        "location_compressor"
    }

    fn applies_to(&self, method: &str, direction: Direction) -> bool {
        direction == Direction::ServerToClient
            && matches!(
                method,
                "textDocument/references"
                    | "textDocument/definition"
                    | "textDocument/implementation"
                    | "textDocument/typeDefinition"
            )
    }

    async fn intercept(
        &self,
        _method: &str,
        params: Value,
        _direction: Direction,
    ) -> Result<Option<Value>, LspzError> {
        let compact = compress_locations(&params)?;
        Ok(Some(compact))
    }
}

// ─── Compression Logic ─────────────────────────────────────────────────────────

/// Top-level compression entry point.
///
/// Normalizes the input to an array, builds a URI pool, then compresses each item.
fn compress_locations(params: &Value) -> Result<Value, LspzError> {
    let items = normalize_to_array(params);

    if items.is_empty() {
        let mut result = serde_json::Map::new();
        result.insert("version".into(), Value::Number(1.into()));
        result.insert("uris".into(), Value::Array(Vec::new()));
        result.insert("items".into(), Value::Array(Vec::new()));
        return Ok(Value::Object(result));
    }

    let uri_pool = build_uri_pool(&items);
    let compressed: Vec<Value> = items
        .iter()
        .map(|item| compress_item(item, &uri_pool))
        .collect();

    let mut uris_json = Vec::new();
    for uri in &uri_pool {
        uris_json.push(Value::String(uri.clone()));
    }

    let mut result = serde_json::Map::new();
    result.insert("version".into(), Value::Number(1.into()));
    result.insert("uris".into(), Value::Array(uris_json));
    result.insert("items".into(), Value::Array(compressed));
    Ok(Value::Object(result))
}

/// Normalize the params value to a Vec of references.
///
/// - `Value::Null` → empty vec
/// - single object with `uri` or `targetUri` → vec of one element
/// - `Value::Array` → cloned elements
fn normalize_to_array(params: &Value) -> Vec<Value> {
    match params {
        Value::Null => vec![],
        Value::Array(arr) => arr.clone(),
        Value::Object(obj) if obj.contains_key("targetUri") || obj.contains_key("uri") => {
            vec![params.clone()]
        }
        _ => vec![],
    }
}

/// Extract unique URI strings from an iterator of values.
///
/// Each value is expected to be an object with either `uri` (Location) or
/// `targetUri` (LocationLink). URIs are collected in insertion order; duplicates
/// are skipped. If an item has neither field, it contributes no URI.
fn build_uri_pool(items: &[Value]) -> Vec<String> {
    let mut pool = Vec::new();
    for item in items {
        let uri = item
            .as_object()
            .and_then(|obj| {
                obj.get("targetUri")
                    .or_else(|| obj.get("uri"))
                    .and_then(|v| v.as_str())
            })
            .map(String::from);
        if let Some(uri) = uri
            && !pool.contains(&uri)
        {
            pool.push(uri);
        }
    }
    pool
}

/// Compress a single Location or LocationLink value.
///
/// Detects LocationLink by checking for `targetUri` field.
fn compress_item(value: &Value, uri_pool: &[String]) -> Value {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return value.clone(),
    };

    // Detect LocationLink vs Location
    if obj.contains_key("targetUri") {
        compress_location_link(value, uri_pool)
    } else if obj.contains_key("uri") {
        compress_location(value, uri_pool)
    } else {
        value.clone()
    }
}

/// Compress a Location: `{ uri, range }` → `{ u: idx, r: { s: { l, c }, e: { l, c } } }`.
fn compress_location(value: &Value, uri_pool: &[String]) -> Value {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return value.clone(),
    };

    let mut out = serde_json::Map::new();

    // uri → u (pool index)
    if let Some(uri) = obj.get("uri").and_then(|v| v.as_str()) {
        let idx = uri_pool.iter().position(|u| u == uri).unwrap_or(0);
        out.insert("u".into(), Value::Number(idx.into()));
    }

    // range → r
    if let Some(range) = obj.get("range") {
        out.insert("r".into(), compress_range(range));
    }

    Value::Object(out)
}

/// Compress a LocationLink: `{ targetUri, targetRange, targetSelectionRange, originSelectionRange? }`
/// → `{ u: idx, r, s, o? }`.
fn compress_location_link(value: &Value, uri_pool: &[String]) -> Value {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return value.clone(),
    };

    let mut out = serde_json::Map::new();

    // targetUri → u (pool index)
    if let Some(uri) = obj.get("targetUri").and_then(|v| v.as_str()) {
        let idx = uri_pool.iter().position(|u| u == uri).unwrap_or(0);
        out.insert("u".into(), Value::Number(idx.into()));
    }

    // targetRange → r
    if let Some(range) = obj.get("targetRange") {
        out.insert("r".into(), compress_range(range));
    }

    // targetSelectionRange → s
    if let Some(range) = obj.get("targetSelectionRange") {
        out.insert("s".into(), compress_range(range));
    }

    // originSelectionRange → o (optional — only if present)
    if let Some(range) = obj.get("originSelectionRange") {
        out.insert("o".into(), compress_range(range));
    }

    Value::Object(out)
}

// ─── Range Compression (local copies, same pattern as symbols.rs) ──────────────

/// Compress a Range: `{ start, end }` → `{ s, e }`, each position: `{ line, character }` → `{ l, c }`.
fn compress_range(range: &Value) -> Value {
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
fn compress_position(pos: &Value) -> Value {
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

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interceptors::Direction;
    use serde_json::json;

    fn make_compressor() -> LocationCompressor {
        LocationCompressor
    }

    #[test]
    fn test_applies_to_all_methods() {
        let c = make_compressor();
        // All 4 methods should match in ServerToClient direction
        assert!(c.applies_to("textDocument/references", Direction::ServerToClient));
        assert!(c.applies_to("textDocument/definition", Direction::ServerToClient));
        assert!(c.applies_to("textDocument/implementation", Direction::ServerToClient));
        assert!(c.applies_to("textDocument/typeDefinition", Direction::ServerToClient));
        // Should NOT match in ClientToServer direction
        assert!(!c.applies_to("textDocument/references", Direction::ClientToServer));
        // Should NOT match unknown methods
        assert!(!c.applies_to("textDocument/completion", Direction::ServerToClient));
        assert!(!c.applies_to("textDocument/documentSymbol", Direction::ServerToClient));
    }

    #[tokio::test]
    async fn test_compress_single_location() {
        let c = make_compressor();
        let params = json!({
            "uri": "file:///src/main.rs",
            "range": {
                "start": { "line": 10, "character": 5 },
                "end": { "line": 10, "character": 10 }
            }
        });

        let result = c
            .intercept("textDocument/definition", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        assert_eq!(compact["version"], 1);
        let uris = compact["uris"].as_array().unwrap();
        assert_eq!(uris.len(), 1);
        assert_eq!(uris[0], "file:///src/main.rs");

        let items = compact["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["u"], 0);
        assert_eq!(items[0]["r"]["s"]["l"], 10);
        assert_eq!(items[0]["r"]["s"]["c"], 5);
        assert_eq!(items[0]["r"]["e"]["l"], 10);
        assert_eq!(items[0]["r"]["e"]["c"], 10);
    }

    #[tokio::test]
    async fn test_compress_location_array() {
        let c = make_compressor();
        let params = json!([
            {
                "uri": "file:///src/main.rs",
                "range": {
                    "start": { "line": 10, "character": 0 },
                    "end": { "line": 10, "character": 5 }
                }
            },
            {
                "uri": "file:///src/lib.rs",
                "range": {
                    "start": { "line": 20, "character": 0 },
                    "end": { "line": 20, "character": 10 }
                }
            },
            {
                "uri": "file:///src/main.rs",
                "range": {
                    "start": { "line": 30, "character": 0 },
                    "end": { "line": 30, "character": 15 }
                }
            }
        ]);

        let result = c
            .intercept("textDocument/references", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        let uris = compact["uris"].as_array().unwrap();
        assert_eq!(uris.len(), 2);
        assert_eq!(uris[0], "file:///src/main.rs");
        assert_eq!(uris[1], "file:///src/lib.rs");

        let items = compact["items"].as_array().unwrap();
        assert_eq!(items.len(), 3);
        // First and third items reference main.rs (index 0)
        assert_eq!(items[0]["u"], 0);
        assert_eq!(items[0]["r"]["s"]["l"], 10);
        assert_eq!(items[2]["u"], 0);
        // Second item references lib.rs (index 1)
        assert_eq!(items[1]["u"], 1);
    }

    #[tokio::test]
    async fn test_compress_location_link() {
        let c = make_compressor();
        let params = json!({
            "targetUri": "file:///src/lib.rs",
            "targetRange": {
                "start": { "line": 5, "character": 0 },
                "end": { "line": 50, "character": 1 }
            },
            "targetSelectionRange": {
                "start": { "line": 5, "character": 0 },
                "end": { "line": 15, "character": 10 }
            },
            "originSelectionRange": {
                "start": { "line": 3, "character": 5 },
                "end": { "line": 3, "character": 10 }
            }
        });

        let result = c
            .intercept("textDocument/definition", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        let uris = compact["uris"].as_array().unwrap();
        assert_eq!(uris.len(), 1);
        assert_eq!(uris[0], "file:///src/lib.rs");

        let items = compact["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item["u"], 0);
        // targetRange → r
        assert_eq!(item["r"]["s"]["l"], 5);
        assert_eq!(item["r"]["e"]["l"], 50);
        // targetSelectionRange → s
        assert_eq!(item["s"]["s"]["l"], 5);
        assert_eq!(item["s"]["e"]["l"], 15);
        // originSelectionRange → o
        assert_eq!(item["o"]["s"]["c"], 5);
        assert_eq!(item["o"]["e"]["c"], 10);
    }

    #[tokio::test]
    async fn test_compress_mixed_location_and_link() {
        let c = make_compressor();
        let params = json!([
            {
                "uri": "file:///src/main.rs",
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 1, "character": 1 }
                }
            },
            {
                "targetUri": "file:///src/lib.rs",
                "targetRange": {
                    "start": { "line": 10, "character": 0 },
                    "end": { "line": 20, "character": 1 }
                },
                "targetSelectionRange": {
                    "start": { "line": 10, "character": 5 },
                    "end": { "line": 15, "character": 10 }
                }
            }
        ]);

        let result = c
            .intercept(
                "textDocument/implementation",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let compact = result.unwrap();
        let uris = compact["uris"].as_array().unwrap();
        assert_eq!(uris.len(), 2);

        let items = compact["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);

        // First item: Location (has `u` + `r`)
        assert!(items[0].get("u").is_some());
        assert!(items[0].get("r").is_some());
        assert!(items[0].get("s").is_none()); // No selection range for Location

        // Second item: LocationLink (has `u` + `r` + `s`)
        assert!(items[1].get("u").is_some());
        assert!(items[1].get("r").is_some());
        assert!(items[1].get("s").is_some());
    }

    #[tokio::test]
    async fn test_compress_null() {
        let c = make_compressor();
        let result = c
            .intercept(
                "textDocument/references",
                Value::Null,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let compact = result.unwrap();
        assert_eq!(compact["version"], 1);
        assert!(compact["uris"].as_array().unwrap().is_empty());
        assert!(compact["items"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_compress_empty_array() {
        let c = make_compressor();
        let result = c
            .intercept(
                "textDocument/references",
                Value::Array(vec![]),
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let compact = result.unwrap();
        assert_eq!(compact["version"], 1);
        assert!(compact["uris"].as_array().unwrap().is_empty());
        assert!(compact["items"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_uri_pool_dedup() {
        let c = make_compressor();
        let params = json!([
            { "uri": "file:///src/main.rs", "range": { "start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 5} } },
            { "uri": "file:///src/main.rs", "range": { "start": {"line": 2, "character": 0}, "end": {"line": 2, "character": 5} } },
            { "uri": "file:///src/main.rs", "range": { "start": {"line": 3, "character": 0}, "end": {"line": 3, "character": 5} } },
            { "uri": "file:///src/lib.rs",  "range": { "start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 5} } },
            { "uri": "file:///src/main.rs", "range": { "start": {"line": 4, "character": 0}, "end": {"line": 4, "character": 5} } },
            { "uri": "file:///src/lib.rs",  "range": { "start": {"line": 2, "character": 0}, "end": {"line": 2, "character": 5} } },
        ]);

        let result = c
            .intercept("textDocument/references", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        let uris = compact["uris"].as_array().unwrap();
        assert_eq!(uris.len(), 2, "should have exactly 2 unique URIs");

        let items = compact["items"].as_array().unwrap();
        assert_eq!(items.len(), 6);
        // Items 0,1,2,4 are main.rs (index 0)
        assert_eq!(items[0]["u"], 0);
        assert_eq!(items[1]["u"], 0);
        assert_eq!(items[2]["u"], 0);
        assert_eq!(items[4]["u"], 0);
        // Items 3,5 are lib.rs (index 1)
        assert_eq!(items[3]["u"], 1);
        assert_eq!(items[5]["u"], 1);
    }

    #[test]
    fn test_range_compression() {
        let range = json!({
            "start": { "line": 42, "character": 7 },
            "end": { "line": 100, "character": 3 }
        });

        let compressed = compress_range(&range);
        assert_eq!(compressed["s"]["l"], 42);
        assert_eq!(compressed["s"]["c"], 7);
        assert_eq!(compressed["e"]["l"], 100);
        assert_eq!(compressed["e"]["c"], 3);
        // Original field names should NOT be present
        assert!(compressed.get("start").is_none());
        assert!(compressed.get("end").is_none());
    }

    #[tokio::test]
    async fn test_fail_open() {
        let c = make_compressor();
        // Non-object, non-array, non-null input → should produce empty result (not panic)
        let result = c
            .intercept(
                "textDocument/references",
                Value::String("not a location".into()),
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let compact = result.unwrap();
        // Should produce empty result gracefully
        assert_eq!(compact["version"], 1);
        assert!(compact["uris"].as_array().unwrap().is_empty());
        assert!(compact["items"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_roundtrip_preserves_structure() {
        let c = make_compressor();
        let params = json!([
            {
                "uri": "file:///src/a.rs",
                "range": {
                    "start": { "line": 1, "character": 0 },
                    "end": { "line": 1, "character": 5 }
                }
            },
            {
                "uri": "file:///src/b.rs",
                "range": {
                    "start": { "line": 2, "character": 10 },
                    "end": { "line": 5, "character": 15 }
                }
            }
        ]);

        let result = c
            .intercept("textDocument/references", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        // Structure: { version, uris, items }
        assert!(compact.get("version").is_some());
        assert!(compact.get("uris").is_some());
        assert!(compact.get("items").is_some());

        // Items each have u (pool index) and r (range)
        for item in compact["items"].as_array().unwrap() {
            assert!(item.get("u").is_some(), "item should have 'u' (URI index)");
            assert!(item.get("r").is_some(), "item should have 'r' (range)");
            let r = item["r"].as_object().unwrap();
            assert!(r.contains_key("s"), "range should have 's' (start)");
            assert!(r.contains_key("e"), "range should have 'e' (end)");
        }
    }
}
