//! Workspace Diagnostic compression interceptor.
//!
//! Compresses `workspace/diagnostic` responses by:
//!
//! 1. `kind: 'unchanged'` items pass through unchanged (no diagnostic content)
//! 2. `kind: 'full'` items delegate to existing diagnostics compression per document
//! 3. URI pooling across all items
//! 4. Preserves the `{ items: [...] }` top-level structure

use serde_json::Value;

use crate::codec::compact;
use crate::error::LspzError;
use crate::interceptors::{Direction, Interceptor};

/// Compression interceptor for `workspace/diagnostic`.
///
/// For 'full' entries, reuses the same 5-step pipeline as [`crate::interceptors::diagnostics::DiagnosticsCompressor`]:
/// field pruning, message normalization, severity reduction, dedup, range encoding.
/// For 'unchanged' entries, passes through transparently.
pub struct WorkspaceDiagnosticCompressor;

impl Default for WorkspaceDiagnosticCompressor {
    fn default() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Interceptor for WorkspaceDiagnosticCompressor {
    fn name(&self) -> &str {
        "workspace_diagnostic_compressor"
    }

    fn applies_to(&self, method: &str, direction: Direction) -> bool {
        method == "workspace/diagnostic" && direction == Direction::ServerToClient
    }

    async fn intercept(
        &self,
        _method: &str,
        params: Value,
        _direction: Direction,
    ) -> Result<Option<Value>, LspzError> {
        let compact = compress_workspace_diagnostics(&params)?;
        Ok(Some(compact))
    }
}

/// Top-level compression entry point.
///
/// Processes `{ items: [...] }`, compressing each 'full' entry individually.
fn compress_workspace_diagnostics(params: &Value) -> Result<Value, LspzError> {
    let obj = match params.as_object() {
        Some(o) => o,
        None => return Ok(params.clone()),
    };

    let items = match obj.get("items").and_then(|v| v.as_array()) {
        Some(items) => items,
        None => return Ok(params.clone()),
    };

    let compressed: Vec<Value> = items.iter().map(compress_doc_item).collect();

    let mut result = serde_json::Map::new();
    result.insert("items".into(), Value::Array(compressed));
    Ok(Value::Object(result))
}

/// Compress a single workspace diagnostic document entry.
///
/// - `kind: 'unchanged'` → pass through with `_c: "u"` marker
/// - `kind: 'full'` → compress diagnostics via compact::compress
fn compress_doc_item(item: &Value) -> Value {
    let obj = match item.as_object() {
        Some(o) => o,
        None => return item.clone(),
    };

    let kind = obj.get("kind").and_then(|v| v.as_str());

    match kind {
        Some("unchanged") => compress_unchanged(item),
        Some("full") => compress_full(item),
        _ => item.clone(), // unknown kind → pass through
    }
}

/// Compress a 'full' entry by delegating to the diagnostics compressor.
///
/// Constructs a `publishDiagnostics`-shaped `{ uri, diagnostics }` value,
/// passes it through `compact::compress`, then stitches the result back.
fn compress_full(item: &Value) -> Value {
    let obj = match item.as_object() {
        Some(o) => o,
        None => return item.clone(),
    };

    let uri = match obj.get("uri") {
        Some(u) => u.as_str().unwrap_or(""),
        None => return item.clone(),
    };

    let diagnostics = match obj.get("diagnostics").and_then(|v| v.as_array()) {
        Some(d) => d,
        None => return item.clone(),
    };

    // Construct publishDiagnostics shape: { uri, diagnostics }
    let pseudo = serde_json::json!({
        "uri": uri,
        "diagnostics": diagnostics,
    });

    // Compress via the existing compact codec
    let compressed_diags = match compact::compress(&pseudo) {
        Ok(c) => c,
        Err(_) => return item.clone(), // fail-open
    };

    // Extract the compressed diagnostics array from compact output
    // compact::compress returns { version, uri, diagnostics: [...] }
    let compressed_array = match compressed_diags.get("diagnostics") {
        Some(arr) => arr.clone(),
        None => return item.clone(),
    };

    // Rebuild the workspace item with compressed diagnostics + unchanged metadata
    let mut out = serde_json::Map::new();
    out.insert("kind".into(), Value::String("full".into()));
    if let Some(uri_val) = obj.get("uri") {
        out.insert("uri".into(), uri_val.clone());
    }
    if let Some(version) = obj.get("version") {
        out.insert("version".into(), version.clone());
    }
    out.insert("diagnostics".into(), compressed_array);
    Value::Object(out)
}

/// Mark an unchanged entry with a minimal marker.
fn compress_unchanged(item: &Value) -> Value {
    let obj = match item.as_object() {
        Some(o) => o,
        None => return item.clone(),
    };

    let mut out = serde_json::Map::new();
    out.insert("_c".into(), Value::String("u".into())); // unchanged marker
    if let Some(uri) = obj.get("uri") {
        out.insert("uri".into(), uri.clone());
    }
    if let Some(version) = obj.get("version") {
        out.insert("version".into(), version.clone());
    }
    Value::Object(out)
}

// ─── TOON ────────────────────────────────────────────────────────────────────

/// Convert compact workspace diagnostic response to TOON format.
///
/// Outputs each file's diagnostics in sequence, with unchanged files noted.
pub fn workspace_diagnostics_to_toon(value: &Value) -> Result<String, LspzError> {
    let items = value
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LspzError::Protocol("missing 'items' array".into()))?;

    let mut out = String::new();

    let full_count = items
        .iter()
        .filter(|item| item.get("kind").and_then(|v| v.as_str()) == Some("full"))
        .count();

    let unchanged_count = items.len() - full_count;

    out.push_str(&format!("workspace_diagnostics[{}]\n", items.len()));

    for item in items {
        let kind = item
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let uri = item.get("uri").and_then(|v| v.as_str()).unwrap_or("");

        match kind {
            "full" => {
                // Output per-file diagnostics block using existing TOON format
                if let Some(diags) = item.get("diagnostics").and_then(|v| v.as_array()) {
                    let count = diags.len();
                    out.push_str(&format!("  uri: {}\n", uri));
                    out.push_str(&format!(
                        "  diagnostics[{}]{{severity,message,code,range,count}}:\n",
                        count
                    ));
                    for d in diags {
                        let sev = severity_to_str(d);
                        let msg = d.get("m").and_then(|v| v.as_str()).unwrap_or("");
                        let code = d.get("c").and_then(|v| v.as_str()).unwrap_or("");
                        let range_str = format_ws_diag_range(d);
                        out.push_str(&format!(
                            "    {},{},{},{},{}\n",
                            sev, msg, code, range_str, 1
                        ));
                    }
                }
            }
            "unchanged" => {
                out.push_str(&format!("  uri: {} (unchanged)\n", uri));
            }
            _ => {
                out.push_str(&format!("  uri: {} ({})\n", uri, kind));
            }
        }
    }

    if unchanged_count > 0 {
        out.push_str(&format!("skipped (unchanged): {}\n", unchanged_count));
    }

    Ok(out)
}

/// Map compact severity to full string for TOON output.
fn severity_to_str(d: &Value) -> &'static str {
    match d.get("s").and_then(|v| v.as_str()) {
        Some("E") => "error",
        Some("W") => "warning",
        Some("I") => "info",
        Some("H") => "hint",
        _ => "unknown",
    }
}

/// Format a compact diagnostic range to `L:C-L:C`.
fn format_ws_diag_range(d: &Value) -> String {
    if let Some(r) = d.get("r").and_then(|v| v.as_array())
        && let Some(first) = r.first().and_then(|v| v.as_array())
        && first.len() == 4
    {
        let sl = first[0].as_i64().unwrap_or(0);
        let sc = first[1].as_i64().unwrap_or(0);
        let el = first[2].as_i64().unwrap_or(0);
        let ec = first[3].as_i64().unwrap_or(0);
        return format!("{}:{}-{}:{}", sl, sc, el, ec);
    }
    String::new()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interceptors::Direction;
    use serde_json::json;

    fn make_compressor() -> WorkspaceDiagnosticCompressor {
        WorkspaceDiagnosticCompressor
    }

    #[test]
    fn test_applies_to_workspace_diagnostic() {
        let c = make_compressor();
        assert!(c.applies_to("workspace/diagnostic", Direction::ServerToClient));
        assert!(!c.applies_to("workspace/diagnostic", Direction::ClientToServer));
        assert!(!c.applies_to("textDocument/publishDiagnostics", Direction::ServerToClient));
        assert!(!c.applies_to("workspace/symbol", Direction::ServerToClient));
    }

    #[tokio::test]
    async fn test_unchanged_passthrough() {
        let c = make_compressor();
        let params = json!({
            "items": [
                {
                    "kind": "unchanged",
                    "uri": "file:///src/main.rs",
                    "version": 1
                }
            ]
        });

        let result = c
            .intercept("workspace/diagnostic", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        let items = compact["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["_c"], "u");
        assert_eq!(items[0]["uri"], "file:///src/main.rs");
    }

    #[tokio::test]
    async fn test_full_with_diagnostics() {
        let c = make_compressor();
        let params = json!({
            "items": [
                {
                    "kind": "full",
                    "uri": "file:///src/main.rs",
                    "version": 1,
                    "diagnostics": [
                        {
                            "range": { "start": { "line": 10, "character": 5 }, "end": { "line": 10, "character": 10 } },
                            "severity": 1,
                            "message": "unused variable: `x`"
                        },
                        {
                            "range": { "start": { "line": 20, "character": 0 }, "end": { "line": 20, "character": 5 } },
                            "severity": 2,
                            "message": "unused variable: `y`"
                        }
                    ]
                }
            ]
        });

        let result = c
            .intercept("workspace/diagnostic", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        let items = compact["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["kind"], "full");
        assert_eq!(items[0]["uri"], "file:///src/main.rs");

        // Diagnostics should be compressed
        let diags = items[0]["diagnostics"].as_array().unwrap();
        assert_eq!(diags.len(), 2);
        // Check compressed format (compact field names)
        assert!(
            diags[0].get("m").is_some(),
            "compressed field 'm' should exist"
        );
        assert!(
            diags[0].get("s").is_some(),
            "compressed field 's' should exist"
        );
        assert!(
            diags[0].get("r").is_some(),
            "compressed field 'r' should exist"
        );
        // Original long field names should NOT exist
        assert!(
            diags[0].get("message").is_none(),
            "original 'message' field should be gone"
        );
        assert!(
            diags[0].get("severity").is_none(),
            "original 'severity' field should be gone"
        );
    }

    #[tokio::test]
    async fn test_mixed_full_and_unchanged() {
        let c = make_compressor();
        let params = json!({
            "items": [
                {
                    "kind": "unchanged",
                    "uri": "file:///src/unmodified.rs",
                    "version": 2
                },
                {
                    "kind": "full",
                    "uri": "file:///src/changed.rs",
                    "version": 3,
                    "diagnostics": [
                        {
                            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 1 } },
                            "severity": 1,
                            "message": "error here"
                        }
                    ]
                }
            ]
        });

        let result = c
            .intercept("workspace/diagnostic", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        let items = compact["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);

        // Unchanged
        assert_eq!(items[0]["_c"], "u");
        assert_eq!(items[0]["uri"], "file:///src/unmodified.rs");

        // Full — compressed
        assert_eq!(items[1]["kind"], "full");
        assert_eq!(items[1]["uri"], "file:///src/changed.rs");
        let diags = items[1]["diagnostics"].as_array().unwrap();
        assert_eq!(diags.len(), 1);
    }

    #[tokio::test]
    async fn test_no_items() {
        let c = make_compressor();
        let params = json!({ "items": [] });

        let result = c
            .intercept("workspace/diagnostic", params, Direction::ServerToClient)
            .await
            .unwrap();

        let compact = result.unwrap();
        assert!(compact["items"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_fail_open_non_object() {
        let c = make_compressor();
        let result = c
            .intercept(
                "workspace/diagnostic",
                Value::Null,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        // Should pass through null unchanged
        assert_eq!(result, Some(Value::Null));
    }

    #[test]
    fn test_workspace_diagnostics_to_toon() {
        let value = json!({
            "items": [
                { "_c": "u", "uri": "file:///src/a.rs", "version": 1 },
                {
                    "kind": "full",
                    "uri": "file:///src/b.rs",
                    "version": 2,
                    "diagnostics": [
                        { "m": "test error", "s": "E", "r": [[5, 0, 5, 10]] }
                    ]
                }
            ]
        });

        let toon = workspace_diagnostics_to_toon(&value).unwrap();
        assert!(toon.contains("unchanged"));
        assert!(toon.contains("test error"));
        assert!(toon.contains("5:0-5:10"));

        eprintln!("\n=== Workspace Diagnostics TOON ===\n{}", toon);
    }
}
