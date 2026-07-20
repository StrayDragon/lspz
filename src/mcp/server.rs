use std::sync::Arc;

use crate::codec::{compact, toon};
use crate::interceptors::completions::compress_completions;
use crate::interceptors::symbols::compress_symbols;
use crate::languages::{default_args_by_extension, lookup_by_extension};
use rmcp::{
    ErrorData, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, ContentBlock, ErrorCode, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    service::{RequestContext, RoleServer},
};
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::{info, warn};

use super::pool::LspPool;

// ─── Tool Definitions ──────────────────────────────────────────────────────

fn tool_definitions() -> Vec<Tool> {
    vec![
        Tool::new(
            "get_diagnostics",
            "Get LSP diagnostics for a file. Returns dense TOON (token-optimized tabular). \
             `uri` is optional: when omitted, resolve the client workspace (MCP roots, else cwd), \
             scan a small set of source files, and return a max-column overview plus per-file details. \
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
            "Retrieve document symbols. Returns dense TOON. \
             `uri` is optional: omit to scan the MCP workspace (roots → cwd) \
             and return a max-column overview plus per-file symbol tables. \
             Language and backend are auto-detected from file extension if omitted.",
            rmcp::model::object(GetSymbolsInput::json_schema()),
        ),
    ]
}

// ─── Input Types ───────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct GetDiagnosticsInput {
    /// Target file URI. When omitted/empty, lspz resolves the MCP workspace
    /// (roots → cwd) and scans a capped set of source files.
    pub uri: Option<String>,
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
    /// Target file URI. When omitted/empty, resolve MCP workspace and scan
    /// a capped set of source files for document symbols.
    pub uri: Option<String>,
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
    let path = crate::uri::path_from_file_uri(uri).ok()?;
    let mut dir = path.as_path();
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
    async fn handle_diagnostics(
        &self,
        input: GetDiagnosticsInput,
        peer: &rmcp::service::Peer<RoleServer>,
    ) -> Result<String, ErrorData> {
        let uri = input
            .uri
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let ws = super::workspace::resolve_workspace(peer, None).await;
        let mcp_roots = ws.as_ref().map(|w| w.mcp_roots.clone()).unwrap_or_default();
        match uri {
            Some(uri) => {
                self.handle_diagnostics_for_uri(
                    uri,
                    input.language.as_deref(),
                    input.backend.as_deref(),
                    input.backend_args.as_deref(),
                    &mcp_roots,
                )
                .await
            }
            None => {
                self.handle_diagnostics_workspace_scan(input, peer, ws)
                    .await
            }
        }
    }

    async fn handle_diagnostics_workspace_scan(
        &self,
        input: GetDiagnosticsInput,
        _peer: &rmcp::service::Peer<RoleServer>,
        ws: Option<super::workspace::ResolvedWorkspace>,
    ) -> Result<String, ErrorData> {
        let workspace = match ws {
            Some(w) => w,
            None => {
                return Err(ErrorData::invalid_request(
                    "Cannot resolve workspace (no MCP roots and no cwd). Pass `uri` explicitly."
                        .to_string(),
                    None,
                ));
            }
        };
        let root_path = std::path::Path::new(&workspace.root);
        let files =
            super::workspace::discover_source_files(root_path, super::workspace::MAX_SCAN_FILES);
        if files.is_empty() {
            return Ok(super::workspace::format_workspace_scan_toon(
                &workspace.root,
                &[],
                &[],
            ));
        }

        let mut rows = Vec::new();
        let mut bodies = Vec::new();
        for path in files {
            let uri = super::workspace::file_uri_for_path(&path);
            let rel = path
                .strip_prefix(root_path)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            match self
                .handle_diagnostics_for_uri(
                    &uri,
                    input.language.as_deref(),
                    input.backend.as_deref(),
                    input.backend_args.as_deref(),
                    &workspace.mcp_roots,
                )
                .await
            {
                Ok(body) => {
                    let (e, w, i, h) = super::workspace::count_diag_severities_in_toon(&body);
                    rows.push((rel, e, w, i, h));
                    // Only attach per-file bodies that have findings to keep
                    // agent context dense (overview table is always present).
                    if e + w + i + h > 0 {
                        bodies.push(body);
                    }
                }
                Err(err) => {
                    warn!(file = %rel, error = %err.message, "workspace scan skipped file");
                    rows.push((rel, 0, 0, 0, 0));
                }
            }
        }
        Ok(super::workspace::format_workspace_scan_toon(
            &workspace.root,
            &rows,
            &bodies,
        ))
    }

    async fn handle_diagnostics_for_uri(
        &self,
        uri: &str,
        language: Option<&str>,
        backend: Option<&str>,
        backend_args: Option<&[String]>,
        mcp_roots: &[std::path::PathBuf],
    ) -> Result<String, ErrorData> {
        let (language, backend) = resolve_language_backend(uri, language, backend)?;
        let root = self.cached_workspace_root(uri);
        let extra = merge_backend_args(uri, backend_args);

        let path =
            crate::uri::path_from_file_uri(uri).map_err(|e| ErrorData::invalid_request(e, None))?;
        super::workspace::ensure_path_under_roots(&path, mcp_roots)
            .map_err(|e| ErrorData::invalid_request(e, None))?;
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        let session = {
            LspPool::get_or_spawn(&self.pool, &language, &backend, root.as_deref(), &extra)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        };
        let mut session = session.lock().await;

        // Open the document on first call, or send didChange on subsequent calls.
        // This avoids re-sending didOpen for already-open documents, which can
        // cause the LSP server to reset its state and return stale diagnostics.
        session
            .open_or_update_document(uri, &language, &content)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let target_uri = uri.to_string();
        // Wait for the first publishDiagnostics for this URI.
        //
        // Per LSP, the first notification after didOpen/didChange is the
        // complete set — an empty array is the legitimate terminal state for a
        // clean file. Do not keep waiting for non-empty (that burns the budget
        // on clean files and diverges from DaemonMcpServer).
        let params = {
            let wait_result = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                session.wait_for_notification_where("textDocument/publishDiagnostics", |p| {
                    p.get("uri").and_then(Value::as_str) == Some(&target_uri)
                }),
            )
            .await;

            match wait_result {
                Ok(Ok(p)) => p,
                Ok(Err(e)) => {
                    return Err(ErrorData::internal_error(e.to_string(), None));
                }
                Err(_) => serde_json::Value::Null,
            }
        };

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
            LspPool::get_or_spawn(&self.pool, &language, &backend, root.as_deref(), &extra)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        };
        let mut session = session.lock().await;

        let path = crate::uri::path_from_file_uri(&input.uri)
            .map_err(|e| ErrorData::invalid_request(e, None))?;
        let content = tokio::fs::read_to_string(&path)
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

        let compressed = compress_completions(&result, 0, true)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        toon::completions_to_toon(&compressed)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }

    async fn handle_symbols(
        &self,
        input: GetSymbolsInput,
        peer: &rmcp::service::Peer<RoleServer>,
    ) -> Result<String, ErrorData> {
        let uri = input
            .uri
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let ws = super::workspace::resolve_workspace(peer, None).await;
        let mcp_roots = ws.as_ref().map(|w| w.mcp_roots.clone()).unwrap_or_default();
        match uri {
            Some(uri) => {
                self.handle_symbols_for_uri(
                    uri,
                    input.language.as_deref(),
                    input.backend.as_deref(),
                    input.backend_args.as_deref(),
                    &mcp_roots,
                )
                .await
            }
            None => self.handle_symbols_workspace_scan(input, ws).await,
        }
    }

    async fn handle_symbols_workspace_scan(
        &self,
        input: GetSymbolsInput,
        ws: Option<super::workspace::ResolvedWorkspace>,
    ) -> Result<String, ErrorData> {
        let workspace = match ws {
            Some(w) => w,
            None => {
                return Err(ErrorData::invalid_request(
                    "Cannot resolve workspace (no MCP roots and no cwd). Pass `uri` explicitly."
                        .to_string(),
                    None,
                ));
            }
        };
        let root_path = std::path::Path::new(&workspace.root);
        let files =
            super::workspace::discover_source_files(root_path, super::workspace::MAX_SCAN_FILES);
        if files.is_empty() {
            return Ok(super::workspace::format_workspace_symbols_scan_toon(
                &workspace.root,
                &[],
                &[],
            ));
        }

        let mut rows = Vec::new();
        let mut bodies = Vec::new();
        for path in files {
            let uri = super::workspace::file_uri_for_path(&path);
            let rel = path
                .strip_prefix(root_path)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            match self
                .handle_symbols_for_uri(
                    &uri,
                    input.language.as_deref(),
                    input.backend.as_deref(),
                    input.backend_args.as_deref(),
                    &workspace.mcp_roots,
                )
                .await
            {
                Ok(body) => {
                    let n = super::workspace::count_symbol_rows_in_toon(&body);
                    rows.push((rel, n));
                    if n > 0 {
                        bodies.push(body);
                    }
                }
                Err(err) => {
                    warn!(file = %rel, error = %err.message, "workspace symbols scan skipped file");
                    rows.push((rel, 0));
                }
            }
        }
        Ok(super::workspace::format_workspace_symbols_scan_toon(
            &workspace.root,
            &rows,
            &bodies,
        ))
    }

    async fn handle_symbols_for_uri(
        &self,
        uri: &str,
        language: Option<&str>,
        backend: Option<&str>,
        backend_args: Option<&[String]>,
        mcp_roots: &[std::path::PathBuf],
    ) -> Result<String, ErrorData> {
        let (language, backend) = resolve_language_backend(uri, language, backend)?;
        let root = self.cached_workspace_root(uri);
        let extra = merge_backend_args(uri, backend_args);

        let path =
            crate::uri::path_from_file_uri(uri).map_err(|e| ErrorData::invalid_request(e, None))?;
        super::workspace::ensure_path_under_roots(&path, mcp_roots)
            .map_err(|e| ErrorData::invalid_request(e, None))?;
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        let session = {
            LspPool::get_or_spawn(&self.pool, &language, &backend, root.as_deref(), &extra)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        };
        let mut session = session.lock().await;

        // Open the document on first call, or send didChange on subsequent calls.
        session
            .open_or_update_document(uri, &language, &content)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let result = session
            .send_request(
                "textDocument/documentSymbol",
                serde_json::json!({
                    "textDocument": { "uri": uri },
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
            "lspz MCP server — exposes LSP diagnostics, completions, and symbols as MCP tools. \
             Prefer omitting `uri` on get_diagnostics to scan the client workspace (MCP roots / cwd) \
             and receive a dense TOON overview suitable for coding agents (Cursor, etc.).",
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
        context: RequestContext<RoleServer>,
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
                let result = self.handle_diagnostics(input, &context.peer).await?;
                info!("get_diagnostics completed");
                Ok(CallToolResult::success(vec![ContentBlock::text(result)]))
            }
            "get_completions" => {
                let input: GetCompletionsInput = serde_json::from_value(serde_json::Value::Object(
                    arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_request(e.to_string(), None))?;
                let result = self.handle_completions(input).await?;
                info!("get_completions completed");
                Ok(CallToolResult::success(vec![ContentBlock::text(result)]))
            }
            "get_symbols" => {
                let input: GetSymbolsInput = serde_json::from_value(serde_json::Value::Object(
                    arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_request(e.to_string(), None))?;
                let result = self.handle_symbols(input, &context.peer).await?;
                info!("get_symbols completed");
                Ok(CallToolResult::success(vec![ContentBlock::text(result)]))
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
                "uri": {
                    "type": "string",
                    "description": "File URI (e.g. file:///path/to/file.go). Optional: omit to scan the MCP workspace (roots → cwd) and return a dense overview table."
                },
                "backend": { "type": "string", "description": "Backend LSP server command (e.g. gopls, rust-analyzer). Auto-detected from file extension if omitted." },
                "language": { "type": "string", "description": "Language identifier (e.g. go, rust). Auto-detected from file extension if omitted." },
                "backend_args": { "type": "array", "items": { "type": "string" }, "description": "Extra CLI arguments passed to the backend LSP server (appended to defaults)." }
            }
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
                "uri": {
                    "type": "string",
                    "description": "File URI. Optional: omit to scan the MCP workspace (roots → cwd) and return a dense symbols overview."
                },
                "backend": { "type": "string", "description": "Backend LSP server command. Auto-detected from file extension if omitted." },
                "language": { "type": "string", "description": "Language identifier. Auto-detected from file extension if omitted." },
                "backend_args": { "type": "array", "items": { "type": "string" }, "description": "Extra CLI arguments passed to the backend LSP server (appended to defaults)." }
            }
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
