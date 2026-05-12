//! Response capping interceptor.
//!
//! Truncates LSP server responses before compression, keeping only the first N
//! items. Capping and compression are orthogonal: truncation eliminates tail noise,
//! compression condenses the retained signal.
//!
//! Supported message types:
//! - `textDocument/publishDiagnostics` — cap `diagnostics` array
//! - `textDocument/completion` — cap `items` array (or bare array)
//! - `textDocument/documentSymbol` — cap symbol array

use serde_json::Value;

use crate::error::LspzError;
use crate::interceptors::{Direction, Interceptor};

/// Response capping interceptor.
///
/// Placed at the front of the interceptor chain (before compressors).
/// A limit of 0 means no cap for that type.
pub struct CappingInterceptor {
    /// Maximum diagnostics to keep (0 = unlimited).
    pub max_diags: usize,
    /// Maximum completion items to keep (0 = unlimited).
    pub max_completions: usize,
    /// Maximum document symbols to keep (0 = unlimited).
    pub max_symbols: usize,
}

impl CappingInterceptor {
    /// Create a new `CappingInterceptor` with the given limits.
    pub fn new(max_diags: usize, max_completions: usize, max_symbols: usize) -> Self {
        Self {
            max_diags,
            max_completions,
            max_symbols,
        }
    }
}

#[async_trait::async_trait]
impl Interceptor for CappingInterceptor {
    fn name(&self) -> &str {
        "capping"
    }

    fn applies_to(&self, method: &str, direction: Direction) -> bool {
        direction == Direction::ServerToClient
            && matches!(
                method,
                "textDocument/publishDiagnostics"
                    | "textDocument/completion"
                    | "textDocument/documentSymbol"
            )
    }

    async fn intercept(
        &self,
        method: &str,
        mut params: Value,
        _direction: Direction,
    ) -> Result<Option<Value>, LspzError> {
        match method {
            "textDocument/publishDiagnostics" => {
                cap_array_field(&mut params, "diagnostics", self.max_diags)?;
            }
            "textDocument/completion" => {
                cap_completions(&mut params, self.max_completions)?;
            }
            "textDocument/documentSymbol" => {
                cap_array(params.as_array_mut(), self.max_symbols)?;
            }
            _ => {}
        }
        Ok(Some(params))
    }
}

/// Truncate an array field in a JSON object to at most `limit` items.
///
/// If `limit` is 0, no truncation is performed.
fn cap_array_field(params: &mut Value, field: &str, limit: usize) -> Result<(), LspzError> {
    if limit == 0 {
        return Ok(());
    }
    let arr = params
        .get_mut(field)
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| LspzError::Protocol(format!("missing field '{field}'")))?;

    let original = arr.len();
    if original > limit {
        arr.truncate(limit);
        tracing::info!(field, original, capped = limit, "Capped response");
    }
    Ok(())
}

/// Truncate a bare array or `CompletionList` items to at most `limit` items.
fn cap_completions(params: &mut Value, limit: usize) -> Result<(), LspzError> {
    if limit == 0 {
        return Ok(());
    }
    // Bare array: `[{...}, ...]`
    if let Some(arr) = params.as_array_mut() {
        let original = arr.len();
        if original > limit {
            arr.truncate(limit);
            tracing::info!(
                field = "completions",
                original,
                capped = limit,
                "Capped response"
            );
        }
        return Ok(());
    }
    // CompletionList: `{"items": [...], "isIncomplete": ...}`
    cap_array_field(params, "items", limit)
}

/// Truncate an optional array reference to at most `limit` items.
fn cap_array(arr: Option<&mut Vec<Value>>, limit: usize) -> Result<(), LspzError> {
    if limit == 0 {
        return Ok(());
    }
    match arr {
        Some(items) => {
            let original = items.len();
            if original > limit {
                items.truncate(limit);
                tracing::info!(
                    field = "symbols",
                    original,
                    capped = limit,
                    "Capped response"
                );
            }
            Ok(())
        }
        None => Err(LspzError::Protocol("expected array for symbols".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_interceptor(diags: usize, completions: usize, symbols: usize) -> CappingInterceptor {
        CappingInterceptor::new(diags, completions, symbols)
    }

    #[tokio::test]
    async fn test_cap_diagnostics() {
        let interceptor = make_interceptor(2, 0, 0);
        let params = json!({
            "uri": "file:///test.rs",
            "diagnostics": [
                {"message": "e1", "severity": 1, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}}},
                {"message": "e2", "severity": 1, "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 1}}},
                {"message": "e3", "severity": 1, "range": {"start": {"line": 2, "character": 0}, "end": {"line": 2, "character": 1}}},
                {"message": "e4", "severity": 1, "range": {"start": {"line": 3, "character": 0}, "end": {"line": 3, "character": 1}}},
                {"message": "e5", "severity": 1, "range": {"start": {"line": 4, "character": 0}, "end": {"line": 4, "character": 1}}},
            ]
        });

        let result = interceptor
            .intercept(
                "textDocument/publishDiagnostics",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let caps = result.unwrap();
        let diags = caps["diagnostics"].as_array().unwrap();
        assert_eq!(diags.len(), 2, "should cap to 2 diagnostics");
        assert_eq!(diags[0]["message"], "e1");
        assert_eq!(diags[1]["message"], "e2");
    }

    #[tokio::test]
    async fn test_cap_completions_array() {
        let interceptor = make_interceptor(0, 3, 0);
        let items: Vec<Value> = (0..10)
            .map(|i| json!({"label": format!("item_{}", i)}))
            .collect();
        let params = Value::Array(items);

        let result = interceptor
            .intercept("textDocument/completion", params, Direction::ServerToClient)
            .await
            .unwrap();

        let caps = result.unwrap().as_array().unwrap().to_vec();
        assert_eq!(caps.len(), 3, "should cap to 3 completions");
    }

    #[tokio::test]
    async fn test_cap_completions_list() {
        let interceptor = make_interceptor(0, 3, 0);
        let params = json!({
            "isIncomplete": false,
            "items": [
                {"label": "a"}, {"label": "b"}, {"label": "c"},
                {"label": "d"}, {"label": "e"},
            ]
        });

        let result = interceptor
            .intercept("textDocument/completion", params, Direction::ServerToClient)
            .await
            .unwrap();

        let caps = result.unwrap();
        let items = caps["items"].as_array().unwrap();
        assert_eq!(items.len(), 3, "should cap to 3 completion items");
        assert_eq!(caps["isIncomplete"], false, "should preserve other fields");
    }

    #[tokio::test]
    async fn test_cap_symbols() {
        let interceptor = make_interceptor(0, 0, 2);
        let params = json!([
            {"name": "func1", "kind": 12},
            {"name": "func2", "kind": 12},
            {"name": "func3", "kind": 12},
        ]);

        let result = interceptor
            .intercept(
                "textDocument/documentSymbol",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let caps = result.unwrap().as_array().unwrap().to_vec();
        assert_eq!(caps.len(), 2, "should cap to 2 symbols");
    }

    #[tokio::test]
    async fn test_cap_zero_means_unlimited() {
        let interceptor = make_interceptor(0, 0, 0);
        let params = json!({
            "uri": "file:///test.rs",
            "diagnostics": (0..100).map(|i| json!({"message": format!("e{}", i), "severity": 1, "range": {"start": {"line": i, "character": 0}, "end": {"line": i, "character": 1}}})).collect::<Vec<_>>()
        });

        let result = interceptor
            .intercept(
                "textDocument/publishDiagnostics",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let caps = result.unwrap();
        assert_eq!(
            caps["diagnostics"].as_array().unwrap().len(),
            100,
            "limit=0 should not truncate"
        );
    }

    #[tokio::test]
    async fn test_cap_empty_array() {
        let interceptor = make_interceptor(5, 5, 5);
        let params = json!({
            "uri": "file:///test.rs",
            "diagnostics": []
        });

        let result = interceptor
            .intercept(
                "textDocument/publishDiagnostics",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        assert!(result.is_some(), "empty array should not fail");
        let caps = result.unwrap();
        assert_eq!(
            caps["diagnostics"].as_array().unwrap().len(),
            0,
            "empty array stays empty"
        );
    }

    #[tokio::test]
    async fn test_cap_preserves_other_fields() {
        let interceptor = make_interceptor(2, 0, 0);
        let params = json!({
            "uri": "file:///test.rs",
            "diagnostics": [
                {"message": "e1", "severity": 1, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}}},
                {"message": "e2", "severity": 1, "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 1}}},
                {"message": "e3", "severity": 1, "range": {"start": {"line": 2, "character": 0}, "end": {"line": 2, "character": 1}}},
            ]
        });

        let result = interceptor
            .intercept(
                "textDocument/publishDiagnostics",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let caps = result.unwrap();
        assert_eq!(caps["uri"], "file:///test.rs", "should preserve uri");
        assert_eq!(caps["diagnostics"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_cap_missing_field_fail_open() {
        let interceptor = make_interceptor(5, 0, 0);
        let params = json!({
            "uri": "file:///test.rs"
            // no diagnostics field
        });

        let result = interceptor
            .intercept(
                "textDocument/publishDiagnostics",
                params,
                Direction::ServerToClient,
            )
            .await;

        assert!(result.is_err(), "missing field should error (fail-open)");
    }

    #[tokio::test]
    async fn test_cap_applies_to() {
        let interceptor = make_interceptor(1, 1, 1);

        assert!(
            interceptor.applies_to("textDocument/publishDiagnostics", Direction::ServerToClient)
        );
        assert!(interceptor.applies_to("textDocument/completion", Direction::ServerToClient));
        assert!(interceptor.applies_to("textDocument/documentSymbol", Direction::ServerToClient));

        assert!(!interceptor.applies_to("textDocument/didOpen", Direction::ServerToClient));
        assert!(
            !interceptor.applies_to("textDocument/publishDiagnostics", Direction::ClientToServer)
        );
    }

    #[tokio::test]
    async fn test_cap_within_limit_no_truncation() {
        let interceptor = make_interceptor(10, 0, 0);
        let params = json!({
            "uri": "file:///test.rs",
            "diagnostics": [
                {"message": "e1", "severity": 1, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}}},
            ]
        });

        let result = interceptor
            .intercept(
                "textDocument/publishDiagnostics",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap();

        let caps = result.unwrap();
        assert_eq!(caps["diagnostics"].as_array().unwrap().len(), 1);
    }
}
