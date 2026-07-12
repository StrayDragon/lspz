//! Response capping interceptor.
//!
//! Truncates LSP server responses before compression, keeping only the first N
//! items. Capping and compression are orthogonal: truncation eliminates tail noise,
//! compression condenses the retained signal.
//!
//! Limits are read from the shared [`Config`] on each intercept so hot-reload
//! of `max_diags` / `max_completions` / `max_symbols` takes effect immediately.
//!
//! Supported message types:
//! - `textDocument/publishDiagnostics` — cap `diagnostics` array
//! - `textDocument/completion` — cap `items` array (or bare array)
//! - `textDocument/documentSymbol` — cap symbol array

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::error::LspzError;
use crate::interceptors::{Direction, Interceptor};

/// Response capping interceptor.
///
/// Placed at the front of the interceptor chain (before compressors).
/// A limit of 0 means no cap for that type.
pub struct CappingInterceptor {
    config: Arc<RwLock<Config>>,
}

impl CappingInterceptor {
    /// Create a capping interceptor that reads limits from `config` live.
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        Self { config }
    }

    /// Test helper: static limits wrapped in a private config.
    #[cfg(test)]
    pub fn with_limits(max_diags: usize, max_completions: usize, max_symbols: usize) -> Self {
        let mut cfg = Config::default();
        cfg.capping.max_diags = max_diags;
        cfg.capping.max_completions = max_completions;
        cfg.capping.max_symbols = max_symbols;
        Self::new(Arc::new(RwLock::new(cfg)))
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
        let config = self.config.read().await;
        let max_diags = config.capping.max_diags;
        let max_completions = config.capping.max_completions;
        let max_symbols = config.capping.max_symbols;
        drop(config);

        match method {
            "textDocument/publishDiagnostics" => {
                cap_array_field(&mut params, "diagnostics", max_diags)?;
            }
            "textDocument/completion" => {
                cap_completions(&mut params, max_completions)?;
            }
            "textDocument/documentSymbol" => {
                cap_array(params.as_array_mut(), max_symbols)?;
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
    if let Some(arr) = params.as_array_mut() {
        let original = arr.len();
        if original > limit {
            arr.truncate(limit);
            tracing::info!(original, capped = limit, "Capped completions array");
        }
        return Ok(());
    }
    cap_array_field(params, "items", limit)
}

fn cap_array(arr: Option<&mut Vec<Value>>, limit: usize) -> Result<(), LspzError> {
    match arr {
        Some(arr) => {
            if limit > 0 {
                let original = arr.len();
                if original > limit {
                    arr.truncate(limit);
                    tracing::info!(original, capped = limit, "Capped symbols");
                }
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
        CappingInterceptor::with_limits(diags, completions, symbols)
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
    async fn test_live_config_update_changes_cap() {
        let config = Arc::new(RwLock::new({
            let mut c = Config::default();
            c.capping.max_diags = 100;
            c
        }));
        let interceptor = CappingInterceptor::new(config.clone());
        let params = json!({
            "uri": "file:///t.rs",
            "diagnostics": [
                {"message": "a", "severity": 1, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}}},
                {"message": "b", "severity": 1, "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 1}}},
                {"message": "c", "severity": 1, "range": {"start": {"line": 2, "character": 0}, "end": {"line": 2, "character": 1}}},
            ]
        });

        config.write().await.capping.max_diags = 1;
        let result = interceptor
            .intercept(
                "textDocument/publishDiagnostics",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result["diagnostics"].as_array().unwrap().len(), 1);
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
    }

    #[tokio::test]
    async fn test_cap_symbols() {
        let interceptor = make_interceptor(0, 0, 2);
        let params = json!([
            {"name": "a", "kind": 12},
            {"name": "b", "kind": 12},
            {"name": "c", "kind": 12},
        ]);
        let result = interceptor
            .intercept(
                "textDocument/documentSymbol",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_cap_within_limit_no_truncation() {
        let interceptor = make_interceptor(10, 10, 10);
        let params = json!({
            "uri": "file:///t.rs",
            "diagnostics": [
                {"message": "only", "severity": 1, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}}},
            ]
        });
        let result = interceptor
            .intercept(
                "textDocument/publishDiagnostics",
                params.clone(),
                Direction::ServerToClient,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result["diagnostics"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_cap_empty_array() {
        let interceptor = make_interceptor(5, 5, 5);
        let params = json!({"uri": "file:///t.rs", "diagnostics": []});
        let result = interceptor
            .intercept(
                "textDocument/publishDiagnostics",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap()
            .unwrap();
        assert!(result["diagnostics"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_cap_missing_field_fail_open() {
        let interceptor = make_interceptor(1, 0, 0);
        let err = interceptor
            .intercept(
                "textDocument/publishDiagnostics",
                json!({"uri": "file:///t.rs"}),
                Direction::ServerToClient,
            )
            .await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_cap_preserves_other_fields() {
        let interceptor = make_interceptor(1, 0, 0);
        let params = json!({
            "uri": "file:///t.rs",
            "version": 3,
            "diagnostics": [
                {"message": "a", "severity": 1, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}}},
                {"message": "b", "severity": 1, "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 1}}},
            ]
        });
        let result = interceptor
            .intercept(
                "textDocument/publishDiagnostics",
                params,
                Direction::ServerToClient,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result["version"], 3);
        assert_eq!(result["uri"], "file:///t.rs");
    }

    #[test]
    fn test_cap_applies_to() {
        let i = make_interceptor(1, 1, 1);
        assert!(i.applies_to("textDocument/publishDiagnostics", Direction::ServerToClient));
        assert!(i.applies_to("textDocument/completion", Direction::ServerToClient));
        assert!(i.applies_to("textDocument/documentSymbol", Direction::ServerToClient));
        assert!(!i.applies_to("textDocument/hover", Direction::ServerToClient));
        assert!(!i.applies_to("textDocument/publishDiagnostics", Direction::ClientToServer));
    }
}
