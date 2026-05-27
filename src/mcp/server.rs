use std::sync::Arc;

use crate::codec::{compact, toon};
use crate::interceptors::completions::compress_completions;
use crate::interceptors::symbols::compress_symbols;
use crate::languages::lookup_by_extension;
use rmcp::{
    ErrorData, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, Content, ErrorCode, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    service::{RequestContext, RoleServer},
};
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
struct GetDiagnosticsInput {
    uri: String,
    backend: Option<String>,
    language: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct GetCompletionsInput {
    uri: String,
    backend: Option<String>,
    language: Option<String>,
    line: u32,
    character: u32,
}

#[derive(Debug, serde::Deserialize)]
struct GetSymbolsInput {
    uri: String,
    backend: Option<String>,
    language: Option<String>,
}

/// Extract file extension from a `file://` URI.
fn extension_from_uri(uri: &str) -> Option<&str> {
    let path = uri.strip_prefix("file://")?;
    let name = path.rsplit('/').next()?;
    let (before, after) = name.rsplit_once('.')?;
    if before.is_empty() || after.is_empty() {
        return None;
    }
    Some(after)
}

/// Resolve backend and language from explicit values or URI extension auto-detection.
///
/// Returns `(language, backend)` on success, or an error if neither explicit values
/// nor auto-detection can determine them.
fn resolve_language_backend(
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
        let be = backend.unwrap_or(detected_be).to_string();
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
}

impl McpServer {
    /// Create a new MCP server with an empty connection pool.
    pub fn new() -> Self {
        Self {
            pool: Arc::new(Mutex::new(LspPool::new())),
        }
    }

    async fn handle_diagnostics(&self, input: GetDiagnosticsInput) -> Result<String, ErrorData> {
        let (language, backend) = resolve_language_backend(
            &input.uri,
            input.language.as_deref(),
            input.backend.as_deref(),
        )?;

        let mut pool = self.pool.lock().await;
        let session = pool
            .get_or_spawn(&language, &backend)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let path = input
            .uri
            .strip_prefix("file://")
            .ok_or_else(|| ErrorData::invalid_request("URI must start with file://", None))?;
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        session
            .send_notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": input.uri,
                        "languageId": "",
                        "version": 1,
                        "text": content,
                    }
                }),
            )
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let params = session
            .wait_for_notification("textDocument/publishDiagnostics")
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

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

        let mut pool = self.pool.lock().await;
        let session = pool
            .get_or_spawn(&language, &backend)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let path = input
            .uri
            .strip_prefix("file://")
            .ok_or_else(|| ErrorData::invalid_request("URI must start with file://", None))?;
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        session
            .send_notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": input.uri,
                        "languageId": language,
                        "version": 1,
                        "text": content,
                    }
                }),
            )
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

        let mut pool = self.pool.lock().await;
        let session = pool
            .get_or_spawn(&language, &backend)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let path = input
            .uri
            .strip_prefix("file://")
            .ok_or_else(|| ErrorData::invalid_request("URI must start with file://", None))?;
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        session
            .send_notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": input.uri,
                        "languageId": language,
                        "version": 1,
                        "text": content,
                    }
                }),
            )
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
        ServerInfo {
            instructions: Some(
                "lspz MCP server — exposes LSP diagnostics, completions, and symbols as MCP tools. "
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
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

trait JsonSchema {
    fn json_schema() -> serde_json::Value;
}

impl JsonSchema for GetDiagnosticsInput {
    fn json_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "uri": { "type": "string", "description": "File URI (e.g. file:///path/to/file.go)" },
                "backend": { "type": "string", "description": "Backend LSP server command (e.g. gopls, rust-analyzer). Auto-detected from file extension if omitted." },
                "language": { "type": "string", "description": "Language identifier (e.g. go, rust). Auto-detected from file extension if omitted." }
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
                "character": { "type": "integer", "description": "Character offset (0-based)" }
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
                "language": { "type": "string", "description": "Language identifier. Auto-detected from file extension if omitted." }
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
        let (lang, be) = resolve_language_backend(
            "file:///test.py",
            Some("python"),
            Some("basedpyright"),
        )
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
        assert!(
            resolve_language_backend("file:///data.xyz", Some("custom"), None).is_err()
        );
    }

    #[test]
    fn resolve_unknown_ext_backend_only() {
        assert!(
            resolve_language_backend("file:///data.xyz", None, Some("myserver")).is_err()
        );
    }
}
