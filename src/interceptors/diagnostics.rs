//! Diagnostic compression interceptor.
//!
//! Implements the 5-step compression pipeline:
//!
//! 1. **Field pruning** — drop `source`, `data`, `codeDescription`, `relatedInformation`
//! 2. **Message normalization** — strip variable-specific details to enable dedup
//! 3. **Enum reduction** — `severity` → `E`/`W`/`I`/`H`, compact field names
//! 4. **Dedup** — group by (normalized_message, severity, code)
//! 5. **Range encoding** — `[line, col, line, col]` with delta for multiple ranges
//!
//! [MermaidChart:./docs/mmd/compression-pipeline.mmd]

use serde_json::Value;

use crate::codec::compact::{
    self, CompactSeverity, CompressedEntry, encode_range, encode_ranges_with_delta, encode_tags,
    group_by_key,
};
use crate::error::LspzError;
use crate::interceptors::{Direction, Interceptor};

/// Compression interceptor for `textDocument/publishDiagnostics`.
///
/// Transforms LSP diagnostics into the compact format.
/// On any error, logs a WARN and returns `Err` (fail-open in the chain).
pub struct DiagnosticsCompressor {
    /// Whether to apply message normalisation before dedup.
    pub enable_normalisation: bool,
}

impl Default for DiagnosticsCompressor {
    fn default() -> Self {
        Self {
            enable_normalisation: true,
        }
    }
}

#[async_trait::async_trait]
impl Interceptor for DiagnosticsCompressor {
    fn name(&self) -> &str {
        "diagnostics_compressor"
    }

    fn applies_to(&self, method: &str, direction: Direction) -> bool {
        method == "textDocument/publishDiagnostics" && direction == Direction::ServerToClient
    }

    async fn intercept(
        &self,
        _method: &str,
        params: Value,
        _direction: Direction,
    ) -> Result<Option<Value>, LspzError> {
        // ── Step 1 & 2: Parse, field-prune, normalize ──────────────
        let uri = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LspzError::Protocol("missing uri".into()))?
            .to_string();

        let diagnostics = params
            .get("diagnostics")
            .and_then(|v| v.as_array())
            .ok_or_else(|| LspzError::Protocol("missing diagnostics".into()))?;

        let mut entries: Vec<CompressedEntry> = Vec::with_capacity(diagnostics.len());

        for diag in diagnostics {
            // Step 1: Field pruning — keep only message, severity, range, code
            let message = diag
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Step 2: Normalise message — extract string or numeric code for normalization
            let norm_code = diag.get("code").and_then(|v| {
                v.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            });
            let normalized = if self.enable_normalisation {
                normalize_message(&message, norm_code.as_deref())
            } else {
                message.clone()
            };

            let severity = match diag.get("severity").and_then(|v| v.as_u64()) {
                Some(s) => CompactSeverity::from_lsp(s).unwrap_or(CompactSeverity::W),
                None => CompactSeverity::W,
            };

            let code = diag.get("code").and_then(|v| v.as_str()).map(String::from);

            let tags = diag
                .get("tags")
                .and_then(|v| v.as_array())
                .and_then(|t| encode_tags(t));

            // Encode range → [line, col, line, col]
            let range = diag
                .get("range")
                .and_then(encode_range)
                .unwrap_or([0, 0, 0, 0]);

            entries.push(CompressedEntry {
                message: normalized,
                severity,
                code,
                tags,
                ranges: vec![range],
                count: 1,
            });
        }

        // ── Step 4: Dedup by (message, severity, code) ──────────────
        let grouped = group_by_key(entries);

        // ── Step 5: Build compact diagnostics with range encoding ───
        let compact_diags: Vec<compact::CompactDiagnostic> = grouped
            .into_iter()
            .map(|entry| {
                let r = if entry.ranges.len() > 1 && entry.ranges.len() <= 100 {
                    // Encode with delta: first absolute, rest delta
                    let as_json_ranges: Vec<Value> = entry
                        .ranges
                        .iter()
                        .map(|&[ls, cs, le, ce]| {
                            serde_json::json!({
                                "start": {"line": ls, "character": cs},
                                "end": {"line": le, "character": ce}
                            })
                        })
                        .collect();
                    encode_ranges_with_delta(as_json_ranges)
                } else {
                    entry.ranges
                };

                compact::CompactDiagnostic {
                    m: entry.message,
                    s: entry.severity,
                    r,
                    c: entry.code,
                    t: entry.tags,
                    n: entry.count,
                }
            })
            .collect();

        let compact = compact::CompactDiagnostics {
            version: 1,
            uri,
            diagnostics: compact_diags,
        };

        let result = serde_json::to_value(compact)?;
        Ok(Some(result))
    }
}

// ─── Message Normalization ─────────────────────────────────────────────────

/// Normalize a diagnostic message by stripping variable-specific content.
///
/// This is the critical enabler for dedup: without normalization, messages like
/// `"declared and not used: foo"` and `"declared and not used: bar"` can't merge.
///
/// ## Known patterns
///
/// | code | Raw message | Normalised |
/// |------|-------------|------------|
/// | `unused_var` / `UnusedVar` | `"declared and not used: x"` | `"unused variable"` |
/// | `unused_import` / `UnusedImport` | `"\"os\" imported and not used"` | `"unused import"` |
/// | `reportUnusedVariable` | `"Variable \"x\" is not used"` | `"unused variable"` |
/// | `reportUnusedImport` | `"Import \"os\" is unused"` | `"unused import"` |
/// | `6133` (TypeScript) | `"'temp' is declared but never read"` | `"unused variable"` |
/// | _any_ with backtick identifiers | `` "use `foo`" `` | `` "use `<ident>`" `` |
pub fn normalize_message(message: &str, code: Option<&str>) -> String {
    // Try code-based normalisation first
    if let Some(code) = code {
        let code_lower = code.to_lowercase();

        // gopls / rust-analyzer / basedpyright: unused variable
        if is_unused_var_code(&code_lower) {
            return "unused variable".to_string();
        }

        // gopls / rust-analyzer / basedpyright: unused import
        if is_unused_import_code(&code_lower) {
            return "unused import".to_string();
        }

        // Unused parameter
        if code_lower.contains("unusedparam") {
            return "unused parameter".to_string();
        }

        // Type mismatch / cannot assign / etc.
        if code_lower.contains("assignment") || code_lower.contains("mismatch") {
            return "type mismatch".to_string();
        }
    }

    // Try numeric-code-based normalisation (TypeScript uses integer codes)
    if let Some(num) = code.and_then(|c| c.parse::<i64>().ok())
        && let Some(normalized) = normalize_by_numeric_code(num)
    {
        return normalized.to_string();
    }

    // Fallback: replace backtick identifiers with `<ident>`
    let normalized = replace_backtick_idents(message);

    // Truncate overly long messages
    if normalized.len() > 200 {
        normalized[..197].to_string() + "..."
    } else {
        normalized
    }
}

/// Normalize by TypeScript-style numeric diagnostic codes.
fn normalize_by_numeric_code(code: i64) -> Option<&'static str> {
    match code {
        6133 => Some("unused variable"),
        2322 => Some("type mismatch"),
        2339 => Some("missing property"),
        2552 => Some("unresolved reference"),
        _ => None,
    }
}

/// Check if the code represents an "unused variable" diagnostic.
fn is_unused_var_code(code: &str) -> bool {
    matches!(
        code,
        "unused_variable"
            | "unused_variables"
            | "unused_var"
            | "unusedvar"
            | "unused_variable_warning"
            | "unused_variable_error"
            | "var_not_used"
            | "reportunusedvariable" // basedpyright
    )
}

/// Check if the code represents an "unused import" diagnostic.
fn is_unused_import_code(code: &str) -> bool {
    matches!(
        code,
        "unused_import"
            | "unusedimport"
            | "unused_imports"
            | "import_not_used"
            | "reportunusedimport" // basedpyright
    )
}

/// Replace backtick-wrapped identifiers with a placeholder.
///
/// `` "use `foo`" `` → `` "use `<ident>`" ``
fn replace_backtick_idents(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_backtick = false;
    let mut backtick_content = String::new();

    for ch in s.chars() {
        if ch == '`' {
            if in_backtick {
                // Close backtick block
                if !backtick_content.is_empty() && !backtick_content.contains(' ') {
                    result.push_str("<ident>");
                } else {
                    result.push('`');
                    result.push_str(&backtick_content);
                    result.push('`');
                }
                backtick_content.clear();
                in_backtick = false;
            } else {
                in_backtick = true;
            }
        } else if in_backtick {
            backtick_content.push(ch);
        } else {
            result.push(ch);
        }
    }

    // Unclosed backtick
    if in_backtick {
        result.push('`');
        result.push_str(&backtick_content);
    }

    result
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interceptors::Interceptor;

    fn make_compressor() -> DiagnosticsCompressor {
        DiagnosticsCompressor::default()
    }

    #[test]
    fn test_applies_to_publish_diagnostics() {
        let compressor = make_compressor();
        assert!(
            compressor.applies_to("textDocument/publishDiagnostics", Direction::ServerToClient)
        );
        assert!(!compressor.applies_to("textDocument/didOpen", Direction::ServerToClient));
        assert!(
            !compressor.applies_to("textDocument/publishDiagnostics", Direction::ClientToServer)
        );
    }

    // ─── Normalization ───────────────────────────────────────────────

    #[test]
    fn test_normalize_unused_var_gopls() {
        assert_eq!(
            normalize_message("declared and not used: unusedVar", Some("UnusedVar")),
            "unused variable"
        );
    }

    #[test]
    fn test_normalize_unused_var_rust_analyzer() {
        assert_eq!(
            normalize_message("unused variable: `x`", Some("unused_variables")),
            "unused variable"
        );
    }

    #[test]
    fn test_normalize_unused_import_gopls() {
        assert_eq!(
            normalize_message("\"os\" imported and not used", Some("UnusedImport")),
            "unused import"
        );
    }

    #[test]
    fn test_normalize_backtick_fallback() {
        assert_eq!(
            normalize_message("use `some_ident` here", None),
            "use <ident> here"
        );
    }

    #[test]
    fn test_normalize_no_match() {
        let msg = "some random error message";
        assert_eq!(normalize_message(msg, None), msg);
    }

    // ─── Interceptor ─────────────────────────────────────────────────

    #[tokio::test]
    async fn test_intercept_compress() {
        let compressor = make_compressor();
        let params = serde_json::json!({
            "uri": "file:///test.rs",
            "diagnostics": [
                {
                    "range": {"start": {"line": 10, "character": 5}, "end": {"line": 10, "character": 15}},
                    "severity": 2,
                    "message": "unused variable: `x`",
                    "code": "unused_variables",
                    "source": "rust-analyzer",
                    "tags": [1]
                }
            ]
        });

        let result = compressor
            .intercept(
                "textDocument/publishDiagnostics",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        assert!(result.is_some());
        let compact = result.unwrap();
        assert_eq!(compact["version"], 1);
        assert_eq!(compact["uri"], "file:///test.rs");
        assert_eq!(compact["diagnostics"][0]["m"], "unused variable");
        assert_eq!(compact["diagnostics"][0]["s"], "W");
    }

    #[tokio::test]
    async fn test_intercept_dedup() {
        let compressor = make_compressor();
        let params = serde_json::json!({
            "uri": "file:///test.rs",
            "diagnostics": [
                {
                    "range": {"start": {"line": 10, "character": 5}, "end": {"line": 10, "character": 15}},
                    "severity": 1,
                    "message": "use of undeclared `foo`",
                    "code": "E0425"
                },
                {
                    "range": {"start": {"line": 20, "character": 0}, "end": {"line": 20, "character": 10}},
                    "severity": 1,
                    "message": "use of undeclared `foo`",
                    "code": "E0425"
                }
            ]
        });

        let result = compressor
            .intercept(
                "textDocument/publishDiagnostics",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let compact = result.unwrap();
        let diags = compact["diagnostics"].as_array().unwrap();
        assert_eq!(diags.len(), 1, "should be deduped into 1 entry");
        assert_eq!(diags[0]["n"], 2);
    }

    #[tokio::test]
    async fn test_intercept_different_messages_not_deduped() {
        let compressor = make_compressor();
        let params = serde_json::json!({
            "uri": "file:///test.rs",
            "diagnostics": [
                {
                    "range": {"start": {"line": 10, "character": 5}, "end": {"line": 10, "character": 15}},
                    "severity": 1,
                    "message": "error A",
                    "code": "E001"
                },
                {
                    "range": {"start": {"line": 20, "character": 0}, "end": {"line": 20, "character": 10}},
                    "severity": 2,
                    "message": "error B",
                    "code": "E002"
                }
            ]
        });

        let result = compressor
            .intercept(
                "textDocument/publishDiagnostics",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let compact = result.unwrap();
        assert_eq!(compact["diagnostics"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_intercept_missing_uri_errors() {
        let compressor = make_compressor();
        let params = serde_json::json!({
            "diagnostics": []
        });

        let result = compressor
            .intercept(
                "textDocument/publishDiagnostics",
                params,
                Direction::ServerToClient,
            )
            .await;

        assert!(result.is_err());
    }

    #[test]
    fn test_replace_backtick_idents() {
        assert_eq!(replace_backtick_idents("use `foo`"), "use <ident>");
        assert_eq!(
            replace_backtick_idents("no backticks here"),
            "no backticks here"
        );
        assert_eq!(
            replace_backtick_idents("multi `a` `b` `c`"),
            "multi <ident> <ident> <ident>"
        );
        assert_eq!(
            replace_backtick_idents("unclosed `backtick"),
            "unclosed `backtick"
        );
    }
}
