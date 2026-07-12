use std::sync::Arc;

use crate::codec::{compact, toon};
use crate::interceptors::completions::compress_completions;
use crate::interceptors::symbols::compress_symbols;
use crate::languages::{default_args_by_extension, lookup_by_extension};
use rmcp::{
    ErrorData, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, Content, ErrorCode, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    service::{RequestContext, RoleServer},
};
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::info;

use super::pool::LspPool;

// ─── Tool Definitions ──────────────────────────────────────────────────────

fn tool_definitions() -> Vec<Tool> {
    vec![
        Tool::new(
            "get_diagnostics",
            "Get LSP diagnostics for a file. Returns TOON format (token-optimized tabular) \
             with severity, message, code, range, count. \
             Language and backend are auto-detected from file extension if omitted.",
            rmcp::model::object(GetDiagnosticsInput::json_schema()),
        ),
        Tool::new(
            "get_completions",
            "Request LSP completions at a cursor position. Returns TOON format \
             with label, kind, detail, documentation. \
             Language and backend are auto-detected from file extension if omitted.",
            rmcp::model::object(GetCompletionsInput::json_schema()),
        ),
        Tool::new(
            "get_symbols",
            "Retrieve document symbols. Returns TOON format \
             with name, kind, range, detail, container. \
             Language and backend are auto-detected from file extension if omitted.",
            rmcp::model::object(GetSymbolsInput::json_schema()),
        ),
    ]
}

// ─── Input Types ───────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct GetDiagnosticsInput {
    pub uri: String,
    pub backend: Option<String>,
    pub language: Option<String>,
    pub backend_args: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct GetCompletionsInput {
    pub uri: String,
    pub backend: Option<String>,
    pub language: Option<String>,
    pub line: u32,
    pub character: u32,
    pub backend_args: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct GetSymbolsInput {
    pub uri: String,
    pub backend: Option<String>,
    pub language: Option<String>,
    pub backend_args: Option<Vec<String>>,
}

/// Extract file extension from a `file://` URI.
pub(crate) fn extension_from_uri(uri: &str) -> Option<&str> {
    let path = uri.strip_prefix("file://")?;
    let name = path.rsplit('/').next()?;
    let (before, after) = name.rsplit_once('.')?;
    if before.is_empty() || after.is_empty() {
        return None;
    }
    Some(after)
}

/// Merge default backend args (from language mapping) with user-provided args.
pub(crate) fn merge_backend_args(uri: &str, user_args: Option<&[String]>) -> Vec<String> {
    let ext = extension_from_uri(uri).unwrap_or("");
    let defaults = default_args_by_extension(ext);
    let mut merged: Vec<String> = defaults.iter().map(|s| s.to_string()).collect();
    if let Some(args) = user_args {
        merged.extend_from_slice(args);
    }
    merged
}

/// Project root marker files to look for when detecting workspace root.
const ROOT_MARKERS: &[&str] = &[
    "pyproject.toml",
    "pyrightconfig.json",
    "Cargo.toml",
    "go.mod",
    "tsconfig.json",
    "package.json",
    "compile_commands.json",
    ".clangd",
];

/// Detect workspace root by walking up from a file path looking for project markers.
///
/// Returns a canonicalized absolute path (resolves symlinks) so that different
/// paths pointing to the same directory produce the same cache key.
pub(crate) fn detect_workspace_root(uri: &str) -> Option<String> {
    let path = uri.strip_prefix("file://")?;
    let mut dir = std::path::Path::new(path);
    if dir.is_file() {
        dir = dir.parent()?;
    }
    loop {
        for marker in ROOT_MARKERS {
            if dir.join(marker).exists() {
                return std::fs::canonicalize(dir)
                    .map(|p| p.to_string_lossy().to_string())
                    .ok();
            }
        }
        dir = dir.parent()?;
    }
}

/// Resolve backend and language from explicit values or URI extension auto-detection.
///
/// Returns `(language, backend)` on success, or an error if neither explicit values
/// nor auto-detection can determine them.
pub(crate) fn resolve_language_backend(
    uri: &str,
    language: Option<&str>,
    backend: Option<&str>,
) -> Result<(String, String), ErrorData> {
    // Both explicitly provided — use as-is.
    if let (Some(lang), Some(be)) = (language, backend) {
        return Ok((lang.to_string(), be.to_string()));
    }

    // Try auto-detection from file extension.
    if let Some(ext) = extension_from_uri(uri)
        && let Some((detected_lang, detected_be)) = lookup_by_extension(ext)
    {
        let lang = language.unwrap_or(detected_lang).to_string();
        let be = backend.map(|s| s.to_string()).unwrap_or(detected_be);
        return Ok((lang, be));
    }

    // Fallback: use whatever was explicitly provided.
    // At this point, at most one of language/backend is Some (both-Some was handled above).
    if let Some(lang) = language {
        return Err(ErrorData::invalid_request(
            format!(
                "Cannot auto-detect backend for '{lang}'. \
                 Please provide the `backend` parameter (e.g. \"rust-analyzer\")."
            ),
            None,
        ));
    }
    if let Some(be) = backend {
        return Err(ErrorData::invalid_request(
            format!(
                "Cannot auto-detect language for backend '{be}'. \
                 Please provide the `language` parameter (e.g. \"rust\")."
            ),
            None,
        ));
    }
    Err(ErrorData::invalid_request(
        "Cannot auto-detect language/backend from URI. \
         Please provide `language` and `backend` parameters."
            .to_string(),
        None,
    ))
}

// ─── MCP Server ────────────────────────────────────────────────────────────

/// MCP server that exposes LSP functionality as tools.
#[derive(Clone)]
pub struct McpServer {
    pool: Arc<Mutex<LspPool>>,
    root_cache: Arc<std::sync::Mutex<std::collections::HashMap<String, Option<String>>>>,
}

impl McpServer {
    /// Create a new MCP server with an empty connection pool.
    pub fn new() -> Self {
        Self {
            pool: Arc::new(Mutex::new(LspPool::new())),
            root_cache: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    fn cached_workspace_root(&self, uri: &str) -> Option<String> {
        if let Ok(cache) = self.root_cache.lock()
            && let Some(cached) = cache.get(uri)
        {
            return cached.clone();
        }
        let result = detect_workspace_root(uri);
        if let Ok(mut cache) = self.root_cache.lock() {
            cache.insert(uri.to_string(), result.clone());
        }
        result
    }
    async fn handle_diagnostics(&self, input: GetDiagnosticsInput) -> Result<String, ErrorData> {
        let (language, backend) = resolve_language_backend(
            &input.uri,
            input.language.as_deref(),
            input.backend.as_deref(),
        )?;
        let root = self.cached_workspace_root(&input.uri);
        let extra = merge_backend_args(&input.uri, input.backend_args.as_deref());

        let session = {
            let mut pool = self.pool.lock().await;
            pool.get_or_spawn(&language, &backend, root.as_deref(), &extra)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        };
        let mut session = session.lock().await;

        let path = input
            .uri
            .strip_prefix("file://")
            .ok_or_else(|| ErrorData::invalid_request("URI must start with file://", None))?;
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        // Open the document on first call, or send didChange on subsequent calls.
        // This avoids re-sending didOpen for already-open documents, which can
        // cause the LSP server to reset its state and return stale diagnostics.
        session
            .open_or_update_document(&input.uri, &language, &content)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let target_uri = input.uri.clone();
        // Wait for the first non-empty diagnostic notification, with a 10s budget.
        // Some LSP servers send initial empty notifications before analysis completes.
        let mut params = serde_json::Value::Null;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }

            let wait_result = tokio::time::timeout(
                remaining,
                session.wait_for_notification_where("textDocument/publishDiagnostics", |p| {
                    p.get("uri").and_then(Value::as_str) == Some(&target_uri)
                }),
            )
            .await;

            match wait_result {
                Ok(Ok(p)) => {
                    params = p;
                    let diags = params.get("diagnostics").and_then(Value::as_array);
                    if diags.is_some_and(|a| !a.is_empty()) {
                        break;
                    }
                    // Empty diagnostics — keep waiting within budget
                }
                Ok(Err(e)) => {
                    return Err(ErrorData::internal_error(e.to_string(), None));
                }
                Err(_) => {
                    // Timeout — return whatever we have (may be empty)
                    break;
                }
            }
        }

        // If no matching notification arrived (timeout), return empty diagnostics
        // instead of passing Value::Null to compress(), which would produce a
        // misleading "missing 'uri'" error.
        if params.is_null() {
            return Ok(toon::diagnostics_to_toon(&compact::CompactDiagnostics {
                version: 1,
                uri: target_uri,
                diagnostics: vec![],
            }));
        }

        let compressed = compact::compress(&params)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let diags: compact::CompactDiagnostics = serde_json::from_value(compressed)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(toon::diagnostics_to_toon(&diags))
    }

    async fn handle_completions(&self, input: GetCompletionsInput) -> Result<String, ErrorData> {
        let (language, backend) = resolve_language_backend(
            &input.uri,
            input.language.as_deref(),
            input.backend.as_deref(),
        )?;
        let root = self.cached_workspace_root(&input.uri);
        let extra = merge_backend_args(&input.uri, input.backend_args.as_deref());

        let session = {
            let mut pool = self.pool.lock().await;
            pool.get_or_spawn(&language, &backend, root.as_deref(), &extra)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        };
        let mut session = session.lock().await;

        let path = input
            .uri
            .strip_prefix("file://")
            .ok_or_else(|| ErrorData::invalid_request("URI must start with file://", None))?;
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        // Open the document on first call, or send didChange on subsequent calls.
        session
            .open_or_update_document(&input.uri, &language, &content)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let result = session
            .send_request(
                "textDocument/completion",
                serde_json::json!({
                    "textDocument": { "uri": input.uri },
                    "position": { "line": input.line, "character": input.character },
                }),
            )
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let compressed = compress_completions(&result, 50, true)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        toon::completions_to_toon(&compressed)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }

    async fn handle_symbols(&self, input: GetSymbolsInput) -> Result<String, ErrorData> {
        let (language, backend) = resolve_language_backend(
            &input.uri,
            input.language.as_deref(),
            input.backend.as_deref(),
        )?;
        let root = self.cached_workspace_root(&input.uri);
        let extra = merge_backend_args(&input.uri, input.backend_args.as_deref());

        let session = {
            let mut pool = self.pool.lock().await;
            pool.get_or_spawn(&language, &backend, root.as_deref(), &extra)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        };
        let mut session = session.lock().await;

        let path = input
            .uri
            .strip_prefix("file://")
            .ok_or_else(|| ErrorData::invalid_request("URI must start with file://", None))?;
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        // Open the document on first call, or send didChange on subsequent calls.
        session
            .open_or_update_document(&input.uri, &language, &content)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let result = session
            .send_request(
                "textDocument/documentSymbol",
                serde_json::json!({
                    "textDocument": { "uri": input.uri },
                }),
            )
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let compressed = compress_symbols(&result)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        toon::symbols_to_toon(&compressed)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

// ─── ServerHandler Implementation ──────────────────────────────────────────

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "lspz MCP server — exposes LSP diagnostics, completions, and symbols as MCP tools. ",
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(tool_definitions()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let CallToolRequestParams {
            name, arguments, ..
        } = request;

        match name.as_ref() {
            "get_diagnostics" => {
                let input: GetDiagnosticsInput = serde_json::from_value(serde_json::Value::Object(
                    arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_request(e.to_string(), None))?;
                let result = self.handle_diagnostics(input).await?;
                info!("get_diagnostics completed");
                Ok(CallToolResult::success(vec![Content::text(result)]))
            }
            "get_completions" => {
                let input: GetCompletionsInput = serde_json::from_value(serde_json::Value::Object(
                    arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_request(e.to_string(), None))?;
                let result = self.handle_completions(input).await?;
                info!("get_completions completed");
                Ok(CallToolResult::success(vec![Content::text(result)]))
            }
            "get_symbols" => {
                let input: GetSymbolsInput = serde_json::from_value(serde_json::Value::Object(
                    arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_request(e.to_string(), None))?;
                let result = self.handle_symbols(input).await?;
                info!("get_symbols completed");
                Ok(CallToolResult::success(vec![Content::text(result)]))
            }
            _ => Err(ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                format!("Unknown tool: {name}"),
                None,
            )),
        }
    }
}

// ─── JSON Schema Helpers ───────────────────────────────────────────────────

pub(crate) trait JsonSchema {
    fn json_schema() -> serde_json::Value;
}

impl JsonSchema for GetDiagnosticsInput {
    fn json_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "uri": { "type": "string", "description": "File URI (e.g. file:///path/to/file.go)" },
                "backend": { "type": "string", "description": "Backend LSP server command (e.g. gopls, rust-analyzer). Auto-detected from file extension if omitted." },
                "language": { "type": "string", "description": "Language identifier (e.g. go, rust). Auto-detected from file extension if omitted." },
                "backend_args": { "type": "array", "items": { "type": "string" }, "description": "Extra CLI arguments passed to the backend LSP server (appended to defaults)." }
            },
            "required": ["uri"]
        })
    }
}

impl JsonSchema for GetCompletionsInput {
    fn json_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "uri": { "type": "string", "description": "File URI" },
                "backend": { "type": "string", "description": "Backend LSP server command. Auto-detected from file extension if omitted." },
                "language": { "type": "string", "description": "Language identifier. Auto-detected from file extension if omitted." },
                "line": { "type": "integer", "description": "Line number (0-based)" },
                "character": { "type": "integer", "description": "Character offset (0-based)" },
                "backend_args": { "type": "array", "items": { "type": "string" }, "description": "Extra CLI arguments passed to the backend LSP server (appended to defaults)." }
            },
            "required": ["uri", "line", "character"]
        })
    }
}

impl JsonSchema for GetSymbolsInput {
    fn json_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "uri": { "type": "string", "description": "File URI" },
                "backend": { "type": "string", "description": "Backend LSP server command. Auto-detected from file extension if omitted." },
                "language": { "type": "string", "description": "Language identifier. Auto-detected from file extension if omitted." },
                "backend_args": { "type": "array", "items": { "type": "string" }, "description": "Extra CLI arguments passed to the backend LSP server (appended to defaults)." }
            },
            "required": ["uri"]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_from_uri_basic() {
        assert_eq!(extension_from_uri("file:///home/user/main.rs"), Some("rs"));
        assert_eq!(extension_from_uri("file:///tmp/test.go"), Some("go"));
        assert_eq!(extension_from_uri("file:///src/app.tsx"), Some("tsx"));
    }

    #[test]
    fn extension_from_uri_no_ext() {
        assert_eq!(extension_from_uri("file:///Makefile"), None);
        assert_eq!(extension_from_uri("file:///path/to/file."), None);
    }

    #[test]
    fn extension_from_uri_no_prefix() {
        assert_eq!(extension_from_uri("/home/user/main.rs"), None);
    }

    #[test]
    fn resolve_both_explicit() {
        let (lang, be) =
            resolve_language_backend("file:///test.py", Some("python"), Some("basedpyright"))
                .unwrap();
        assert_eq!(lang, "python");
        assert_eq!(be, "basedpyright");
    }

    #[test]
    fn resolve_auto_detect() {
        let (lang, be) = resolve_language_backend("file:///main.rs", None, None).unwrap();
        assert_eq!(lang, "rust");
        assert_eq!(be, "rust-analyzer");
    }

    #[test]
    fn resolve_partial_override() {
        // Only language provided, backend auto-detected.
        let (lang, be) = resolve_language_backend("file:///main.rs", Some("myrust"), None).unwrap();
        assert_eq!(lang, "myrust");
        assert_eq!(be, "rust-analyzer");

        // Only backend provided, language auto-detected.
        let (lang, be) =
            resolve_language_backend("file:///main.go", None, Some("mygopls")).unwrap();
        assert_eq!(lang, "go");
        assert_eq!(be, "mygopls");
    }

    #[test]
    fn resolve_unknown_ext_no_params() {
        assert!(resolve_language_backend("file:///data.xyz", None, None).is_err());
    }

    #[test]
    fn resolve_unknown_ext_lang_only() {
        assert!(resolve_language_backend("file:///data.xyz", Some("custom"), None).is_err());
    }

    #[test]
    fn resolve_unknown_ext_backend_only() {
        assert!(resolve_language_backend("file:///data.xyz", None, Some("myserver")).is_err());
    }
}
