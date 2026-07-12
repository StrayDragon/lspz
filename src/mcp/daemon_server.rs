//! MCP server backed by a lspz daemon.
//!
//! Unlike [`McpServer`](super::McpServer) which manages LSP processes directly,
//! this server delegates all LSP operations to a running lspz daemon via
//! Unix socket. This eliminates duplicate LSP server startups.

use std::sync::Arc;

use crate::codec::{compact, toon};
use crate::daemon::DaemonClient;
use crate::daemon::protocol::SpawnParams;
use crate::interceptors::completions::compress_completions;
use crate::interceptors::symbols::compress_symbols;
use rmcp::{
    ErrorData, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, Content, ErrorCode, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    service::{RequestContext, RoleServer},
};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use super::server::{
    GetCompletionsInput, GetDiagnosticsInput, GetSymbolsInput, JsonSchema, detect_workspace_root,
    merge_backend_args, resolve_language_backend,
};
use crate::daemon::resolve_workspace_root;

/// MCP server that delegates to a lspz daemon.
///
/// When the daemon is not running, it auto-starts one in the background.
#[derive(Clone)]
pub struct DaemonMcpServer {
    client: Arc<Mutex<Option<DaemonClient>>>,
    root_cache: Arc<std::sync::Mutex<std::collections::HashMap<String, Option<String>>>>,
    /// Workspace root passed from CLI/lazily detected.
    workspace_root: Arc<String>,
}

impl DaemonMcpServer {
    /// Create a new daemon-backed MCP server.
    ///
    /// `workspace_root` should be a canonicalized absolute path.
    pub fn new(workspace_root: String) -> Self {
        let workspace_root = resolve_workspace_root(&workspace_root);
        Self {
            client: Arc::new(Mutex::new(None)),
            root_cache: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            workspace_root: Arc::new(workspace_root),
        }
    }

    /// Lazily connect to or spawn the daemon.
    async fn ensure_connected(&self) -> Result<(), ErrorData> {
        let mut guard = self.client.lock().await;
        if guard.is_some() {
            return Ok(());
        }
        match DaemonClient::connect_or_start(&self.workspace_root).await {
            Ok(client) => {
                info!("Connected to lspz daemon for MCP");
                *guard = Some(client);
                Ok(())
            }
            Err(e) => {
                warn!("Daemon connect failed, falling back to in-process: {e}");
                // Returning an error here means the MCP tool call will fail with
                // a helpful message. The agent can retry or use fallback.
                Err(ErrorData::internal_error(
                    format!("Cannot connect to lspz daemon: {e}. Try running 'lspz daemon' first.",),
                    None,
                ))
            }
        }
    }

    /// Get or create a session key via the daemon.
    async fn ensure_session(
        &self,
        language: &str,
        backend: &str,
        root_path: Option<String>,
        extra_args: &[String],
    ) -> Result<String, ErrorData> {
        self.ensure_connected().await?;

        let mut guard = self.client.lock().await;
        let client = guard
            .as_mut()
            .ok_or_else(|| ErrorData::internal_error("Daemon not connected", None))?;

        let params = SpawnParams {
            language: language.to_string(),
            backend: backend.to_string(),
            root_path: root_path.map(|s| s.to_string()),
            extra_args: extra_args.to_vec(),
        };

        client.spawn_session(&params).await.map_err(|e| {
            ErrorData::internal_error(format!("Failed to create LSP session: {e}"), None)
        })
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

        // Read file content first — fail fast if file doesn't exist,
        // avoiding orphan LSP sessions for invalid URIs.
        let path = input
            .uri
            .strip_prefix("file://")
            .ok_or_else(|| ErrorData::invalid_request("URI must start with file://", None))?;
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        let root = self.cached_workspace_root(&input.uri);
        let extra = merge_backend_args(&input.uri, input.backend_args.as_deref());

        let session_key = self
            .ensure_session(&language, &backend, root, &extra)
            .await?;

        {
            let mut guard = self.client.lock().await;
            let client = guard.as_mut().unwrap();
            client
                .lsp_sync_document(&session_key, &input.uri, &language, &content)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        }

        // Wait for the single `publishDiagnostics` notification for this file.
        //
        // Per the LSP spec, the first notification for a URI after `didOpen`
        // carries the *complete* diagnostic set — an empty array is the
        // legitimate terminal state for a clean file. We must NOT re-issue the
        // wait hoping for non-empty diagnostics: that second request gets
        // cancelled by our own deadline, leaving an orphan response line that
        // desyncs the daemon's newline-delimited protocol (the root cause of
        // the `spawn failed: Wait for notification failed: ...` and `missing
        // 'uri' in publishDiagnostics` errors).
        //
        // The daemon enforces `timeout_ms` (well below the client read
        // backstop), so it always writes a response before we could time out —
        // no client-side cancellation, no orphan. A null result (timeout/slow
        // server) yields empty diagnostics below.
        // Budget for the daemon-side wait, in ms. Kept below the client read
        // timeout so the daemon always replies first.
        const DIAGNOSTIC_WAIT_MS: u64 = 10_000;
        let params = {
            let mut guard = self.client.lock().await;
            let client = guard.as_mut().unwrap();
            client
                .lsp_wait_notify(
                    &session_key,
                    "textDocument/publishDiagnostics",
                    Some(&input.uri),
                    Some(DIAGNOSTIC_WAIT_MS),
                )
                .await
                .unwrap_or(serde_json::Value::Null)
        };

        if params.is_null() {
            return Ok(toon::diagnostics_to_toon(&compact::CompactDiagnostics {
                version: 1,
                uri: input.uri.clone(),
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

        // Read file content first — fail fast if file doesn't exist,
        // avoiding orphan LSP sessions for invalid URIs.
        let path = input
            .uri
            .strip_prefix("file://")
            .ok_or_else(|| ErrorData::invalid_request("URI must start with file://", None))?;
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        let root = self.cached_workspace_root(&input.uri);
        let extra = merge_backend_args(&input.uri, input.backend_args.as_deref());

        let session_key = self
            .ensure_session(&language, &backend, root, &extra)
            .await?;

        {
            let mut guard = self.client.lock().await;
            let client = guard.as_mut().unwrap();
            client
                .lsp_sync_document(&session_key, &input.uri, &language, &content)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

            let result = client
                .lsp_request(
                    &session_key,
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
    }

    async fn handle_symbols(&self, input: GetSymbolsInput) -> Result<String, ErrorData> {
        let (language, backend) = resolve_language_backend(
            &input.uri,
            input.language.as_deref(),
            input.backend.as_deref(),
        )?;

        // Read file content first — fail fast if file doesn't exist,
        // avoiding orphan LSP sessions for invalid URIs.
        let path = input
            .uri
            .strip_prefix("file://")
            .ok_or_else(|| ErrorData::invalid_request("URI must start with file://", None))?;
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        let root = self.cached_workspace_root(&input.uri);
        let extra = merge_backend_args(&input.uri, input.backend_args.as_deref());

        let session_key = self
            .ensure_session(&language, &backend, root, &extra)
            .await?;

        {
            let mut guard = self.client.lock().await;
            let client = guard.as_mut().unwrap();
            client
                .lsp_sync_document(&session_key, &input.uri, &language, &content)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

            let result = client
                .lsp_request(
                    &session_key,
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
}

impl Default for DaemonMcpServer {
    fn default() -> Self {
        Self::new(
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".into()),
        )
    }
}

// ─── MCP ServerHandler ─────────────────────────────────────────────────────

fn daemon_tool_definitions() -> Vec<Tool> {
    vec![
        Tool::new(
            "get_diagnostics",
            "Get LSP diagnostics for a file via lspz daemon. Returns TOON format. \
             Language and backend are auto-detected from file extension if omitted.",
            rmcp::model::object(GetDiagnosticsInput::json_schema()),
        ),
        Tool::new(
            "get_completions",
            "Request LSP completions at a cursor position via lspz daemon. Returns TOON format. \
             Language and backend are auto-detected from file extension if omitted.",
            rmcp::model::object(GetCompletionsInput::json_schema()),
        ),
        Tool::new(
            "get_symbols",
            "Retrieve document symbols via lspz daemon. Returns TOON format. \
             Language and backend are auto-detected from file extension if omitted.",
            rmcp::model::object(GetSymbolsInput::json_schema()),
        ),
    ]
}

impl ServerHandler for DaemonMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "lspz MCP server (daemon mode) — auto-connects to a running lspz daemon. \
             If no daemon is running, one is started automatically.",
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(daemon_tool_definitions()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let CallToolRequestParams {
            name, arguments, ..
        } = request;

        debug!(tool = %name, "Daemon MCP tool call");
        match name.as_ref() {
            "get_diagnostics" => {
                let input: GetDiagnosticsInput = serde_json::from_value(serde_json::Value::Object(
                    arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_request(e.to_string(), None))?;
                let result = self.handle_diagnostics(input).await?;
                info!("get_diagnostics completed (daemon)");
                Ok(CallToolResult::success(vec![Content::text(result)]))
            }
            "get_completions" => {
                let input: GetCompletionsInput = serde_json::from_value(serde_json::Value::Object(
                    arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_request(e.to_string(), None))?;
                let result = self.handle_completions(input).await?;
                info!("get_completions completed (daemon)");
                Ok(CallToolResult::success(vec![Content::text(result)]))
            }
            "get_symbols" => {
                let input: GetSymbolsInput = serde_json::from_value(serde_json::Value::Object(
                    arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_request(e.to_string(), None))?;
                let result = self.handle_symbols(input).await?;
                info!("get_symbols completed (daemon)");
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
