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
        CallToolRequestParams, CallToolResult, ContentBlock, ErrorCode, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    service::{RequestContext, RoleServer},
};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use super::server::{
    GetCompletionsInput, GetDiagnosticsInput, GetSymbolsInput, JsonSchema, SetWorkspaceInput,
    detect_workspace_root, merge_backend_args, resolve_language_backend,
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
    /// Session-scoped workspace bind (`set_workspace` / explicit workspace param).
    session_workspace: super::workspace::SessionWorkspace,
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
            session_workspace: super::workspace::SessionWorkspace::new(),
        }
    }

    fn apply_explicit_workspace(&self, workspace: Option<&str>) -> Result<(), ErrorData> {
        if let Some(ws) = workspace.map(str::trim).filter(|s| !s.is_empty()) {
            self.session_workspace
                .set(ws)
                .map_err(|e| ErrorData::invalid_request(e, None))?;
        }
        Ok(())
    }

    async fn resolve_ws(
        &self,
        peer: &rmcp::service::Peer<RoleServer>,
        explicit: Option<&str>,
    ) -> Result<super::workspace::ResolvedWorkspace, ErrorData> {
        super::workspace::resolve_workspace(
            peer,
            Some(self.workspace_root.as_str()),
            Some(&self.session_workspace),
            explicit,
        )
        .await
        .map_err(|e| ErrorData::invalid_request(e, None))
    }

    fn handle_set_workspace(&self, input: SetWorkspaceInput) -> Result<String, ErrorData> {
        let root = self
            .session_workspace
            .set(&input.workspace)
            .map_err(|e| ErrorData::invalid_request(e, None))?;
        Ok(super::workspace::format_set_workspace_toon(&root))
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

    async fn handle_diagnostics(
        &self,
        input: GetDiagnosticsInput,
        peer: &rmcp::service::Peer<RoleServer>,
    ) -> Result<String, ErrorData> {
        self.apply_explicit_workspace(input.workspace.as_deref())?;
        let specs =
            crate::mcp::workspace::collect_target_specs(&crate::mcp::workspace::TargetPathInputs {
                uri: input.uri.as_deref(),
                path: input.path.as_deref(),
                paths: input.paths.as_deref(),
            });
        let ws = self.resolve_ws(peer, input.workspace.as_deref()).await;
        let workspace_root = ws.as_ref().ok().map(|w| w.root.as_str());
        let mcp_roots = ws
            .as_ref()
            .ok()
            .map(|w| w.mcp_roots.clone())
            .unwrap_or_default();

        if specs.is_empty() {
            let workspace = ws?;
            crate::mcp::workspace::ensure_plausible_for_untrusted(&workspace)
                .map_err(|e| ErrorData::invalid_request(e, None))?;
            return self
                .handle_diagnostics_workspace_scan(&input, workspace)
                .await;
        }

        let uris = crate::mcp::workspace::resolve_target_uris(&specs, workspace_root)
            .map_err(|e| ErrorData::invalid_request(e, None))?;

        if uris.len() == 1 {
            return self
                .handle_diagnostics_for_uri(
                    &uris[0],
                    input.language.as_deref(),
                    input.backend.as_deref(),
                    input.backend_args.as_deref(),
                    &mcp_roots,
                )
                .await;
        }

        let workspace = match ws {
            Ok(w) => w,
            Err(_) => {
                let root = detect_workspace_root(&uris[0]).unwrap_or_else(|| {
                    crate::uri::path_from_file_uri(&uris[0])
                        .ok()
                        .and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned()))
                        .unwrap_or_default()
                });
                crate::mcp::workspace::ResolvedWorkspace {
                    root,
                    mcp_roots: mcp_roots.clone(),
                    source: "explicit",
                }
            }
        };
        self.handle_diagnostics_for_uris(&uris, &input, &workspace)
            .await
    }

    async fn handle_diagnostics_for_uris(
        &self,
        uris: &[String],
        input: &GetDiagnosticsInput,
        workspace: &crate::mcp::workspace::ResolvedWorkspace,
    ) -> Result<String, ErrorData> {
        let root_path = std::path::Path::new(&workspace.root);
        let mut rows = Vec::new();
        let mut bodies = Vec::new();
        for uri in uris {
            let rel = crate::mcp::workspace::display_rel_path(root_path, uri);
            match self
                .handle_diagnostics_for_uri(
                    uri,
                    input.language.as_deref(),
                    input.backend.as_deref(),
                    input.backend_args.as_deref(),
                    &workspace.mcp_roots,
                )
                .await
            {
                Ok(body) => {
                    let (e, w, i, h) = crate::mcp::workspace::count_diag_severities_in_toon(&body);
                    rows.push((rel, e, w, i, h));
                    if e + w + i + h > 0 {
                        bodies.push(body);
                    }
                }
                Err(err) => {
                    warn!(file = %rel, error = %err.message, "multi-path diagnostics skipped file");
                    rows.push((rel, 0, 0, 0, 0));
                }
            }
        }
        Ok(crate::mcp::workspace::format_workspace_scan_toon(
            &workspace.root,
            workspace.source,
            &rows,
            &bodies,
        ))
    }

    async fn handle_diagnostics_workspace_scan(
        &self,
        input: &GetDiagnosticsInput,
        workspace: crate::mcp::workspace::ResolvedWorkspace,
    ) -> Result<String, ErrorData> {
        let root_path = std::path::Path::new(&workspace.root);
        let files = crate::mcp::workspace::discover_source_files(
            root_path,
            crate::mcp::workspace::MAX_SCAN_FILES,
        );
        if files.is_empty() {
            return Ok(crate::mcp::workspace::format_workspace_scan_toon(
                &workspace.root,
                workspace.source,
                &[],
                &[],
            ));
        }

        let mut rows = Vec::new();
        let mut bodies = Vec::new();
        for path in files {
            let uri = crate::mcp::workspace::file_uri_for_path(&path);
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
                    let (e, w, i, h) = crate::mcp::workspace::count_diag_severities_in_toon(&body);
                    rows.push((rel, e, w, i, h));
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
        Ok(crate::mcp::workspace::format_workspace_scan_toon(
            &workspace.root,
            workspace.source,
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

        // Read file content first — fail fast if file doesn't exist,
        // avoiding orphan LSP sessions for invalid URIs.
        let path =
            crate::uri::path_from_file_uri(uri).map_err(|e| ErrorData::invalid_request(e, None))?;
        crate::mcp::workspace::ensure_path_under_roots(&path, mcp_roots)
            .map_err(|e| ErrorData::invalid_request(e, None))?;
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        let root = self.cached_workspace_root(uri);
        let extra = merge_backend_args(uri, backend_args);

        let session_key = self
            .ensure_session(&language, &backend, root, &extra)
            .await?;

        {
            let mut guard = self.client.lock().await;
            let client = guard.as_mut().unwrap();
            client
                .lsp_sync_document(&session_key, uri, &language, &content)
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
                    Some(uri),
                    Some(DIAGNOSTIC_WAIT_MS),
                )
                .await
                .unwrap_or(serde_json::Value::Null)
        };

        if params.is_null() {
            return Ok(toon::diagnostics_to_toon(&compact::CompactDiagnostics {
                version: 1,
                uri: uri.to_string(),
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
        self.apply_explicit_workspace(input.workspace.as_deref())?;
        let specs =
            crate::mcp::workspace::collect_target_specs(&crate::mcp::workspace::TargetPathInputs {
                uri: input.uri.as_deref(),
                path: input.path.as_deref(),
                paths: None,
            });
        if specs.is_empty() {
            return Err(ErrorData::invalid_request(
                "get_completions requires `uri` or `path`".to_string(),
                None,
            ));
        }
        if specs.len() > 1 {
            return Err(ErrorData::invalid_request(
                "get_completions accepts a single `uri` or `path`".to_string(),
                None,
            ));
        }
        let workspace_root = input
            .workspace
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                crate::mcp::workspace::normalize_workspace_input(s)
                    .map_err(|e| ErrorData::invalid_request(e, None))
            })
            .transpose()?
            .or_else(|| self.session_workspace.get());
        let uri =
            crate::mcp::workspace::resolve_target_to_file_uri(&specs[0], workspace_root.as_deref())
                .map_err(|e| ErrorData::invalid_request(e, None))?;

        let (language, backend) =
            resolve_language_backend(&uri, input.language.as_deref(), input.backend.as_deref())?;

        // Read file content first — fail fast if file doesn't exist,
        // avoiding orphan LSP sessions for invalid URIs.
        let path = crate::uri::path_from_file_uri(&uri)
            .map_err(|e| ErrorData::invalid_request(e, None))?;
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        let root = self.cached_workspace_root(&uri);
        let extra = merge_backend_args(&uri, input.backend_args.as_deref());

        let session_key = self
            .ensure_session(&language, &backend, root, &extra)
            .await?;

        {
            let mut guard = self.client.lock().await;
            let client = guard.as_mut().unwrap();
            client
                .lsp_sync_document(&session_key, &uri, &language, &content)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

            let result = client
                .lsp_request(
                    &session_key,
                    "textDocument/completion",
                    serde_json::json!({
                        "textDocument": { "uri": uri },
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
    }

    async fn handle_symbols(
        &self,
        input: GetSymbolsInput,
        peer: &rmcp::service::Peer<RoleServer>,
    ) -> Result<String, ErrorData> {
        self.apply_explicit_workspace(input.workspace.as_deref())?;
        let specs =
            crate::mcp::workspace::collect_target_specs(&crate::mcp::workspace::TargetPathInputs {
                uri: input.uri.as_deref(),
                path: input.path.as_deref(),
                paths: input.paths.as_deref(),
            });
        let ws = self.resolve_ws(peer, input.workspace.as_deref()).await;
        let workspace_root = ws.as_ref().ok().map(|w| w.root.as_str());
        let mcp_roots = ws
            .as_ref()
            .ok()
            .map(|w| w.mcp_roots.clone())
            .unwrap_or_default();

        if specs.is_empty() {
            let workspace = ws?;
            crate::mcp::workspace::ensure_plausible_for_untrusted(&workspace)
                .map_err(|e| ErrorData::invalid_request(e, None))?;
            return self.handle_symbols_workspace_scan(&input, workspace).await;
        }

        let uris = crate::mcp::workspace::resolve_target_uris(&specs, workspace_root)
            .map_err(|e| ErrorData::invalid_request(e, None))?;

        if uris.len() == 1 {
            return self
                .handle_symbols_for_uri(
                    &uris[0],
                    input.language.as_deref(),
                    input.backend.as_deref(),
                    input.backend_args.as_deref(),
                    &mcp_roots,
                )
                .await;
        }

        let workspace = match ws {
            Ok(w) => w,
            Err(_) => {
                let root = detect_workspace_root(&uris[0]).unwrap_or_else(|| {
                    crate::uri::path_from_file_uri(&uris[0])
                        .ok()
                        .and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned()))
                        .unwrap_or_default()
                });
                crate::mcp::workspace::ResolvedWorkspace {
                    root,
                    mcp_roots: mcp_roots.clone(),
                    source: "explicit",
                }
            }
        };
        self.handle_symbols_for_uris(&uris, &input, &workspace)
            .await
    }

    async fn handle_symbols_for_uris(
        &self,
        uris: &[String],
        input: &GetSymbolsInput,
        workspace: &crate::mcp::workspace::ResolvedWorkspace,
    ) -> Result<String, ErrorData> {
        let root_path = std::path::Path::new(&workspace.root);
        let mut rows = Vec::new();
        let mut bodies = Vec::new();
        for uri in uris {
            let rel = crate::mcp::workspace::display_rel_path(root_path, uri);
            match self
                .handle_symbols_for_uri(
                    uri,
                    input.language.as_deref(),
                    input.backend.as_deref(),
                    input.backend_args.as_deref(),
                    &workspace.mcp_roots,
                )
                .await
            {
                Ok(body) => {
                    let n = crate::mcp::workspace::count_symbol_rows_in_toon(&body);
                    rows.push((rel, n));
                    if n > 0 {
                        bodies.push(body);
                    }
                }
                Err(err) => {
                    warn!(file = %rel, error = %err.message, "multi-path symbols skipped file");
                    rows.push((rel, 0));
                }
            }
        }
        Ok(crate::mcp::workspace::format_workspace_symbols_scan_toon(
            &workspace.root,
            workspace.source,
            &rows,
            &bodies,
        ))
    }

    async fn handle_symbols_workspace_scan(
        &self,
        input: &GetSymbolsInput,
        workspace: crate::mcp::workspace::ResolvedWorkspace,
    ) -> Result<String, ErrorData> {
        let root_path = std::path::Path::new(&workspace.root);
        let files = crate::mcp::workspace::discover_source_files(
            root_path,
            crate::mcp::workspace::MAX_SCAN_FILES,
        );
        if files.is_empty() {
            return Ok(crate::mcp::workspace::format_workspace_symbols_scan_toon(
                &workspace.root,
                workspace.source,
                &[],
                &[],
            ));
        }

        let mut rows = Vec::new();
        let mut bodies = Vec::new();
        for path in files {
            let uri = crate::mcp::workspace::file_uri_for_path(&path);
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
                    let n = crate::mcp::workspace::count_symbol_rows_in_toon(&body);
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
        Ok(crate::mcp::workspace::format_workspace_symbols_scan_toon(
            &workspace.root,
            workspace.source,
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

        let path =
            crate::uri::path_from_file_uri(uri).map_err(|e| ErrorData::invalid_request(e, None))?;
        crate::mcp::workspace::ensure_path_under_roots(&path, mcp_roots)
            .map_err(|e| ErrorData::invalid_request(e, None))?;
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        let root = self.cached_workspace_root(uri);
        let extra = merge_backend_args(uri, backend_args);

        let session_key = self
            .ensure_session(&language, &backend, root, &extra)
            .await?;

        {
            let mut guard = self.client.lock().await;
            let client = guard.as_mut().unwrap();
            client
                .lsp_sync_document(&session_key, uri, &language, &content)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

            let result = client
                .lsp_request(
                    &session_key,
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
            "Get LSP diagnostics via lspz daemon. Returns dense TOON. \
             Prefer `path`/`paths` (project-relative OK) with `workspace` or prior `set_workspace`. \
             Absolute `uri` (file://) still works. Omit targets only when workspace is known \
             (roots / session / explicit project root). \
             Language and backend are auto-detected from file extension if omitted.",
            rmcp::model::object(GetDiagnosticsInput::json_schema()),
        ),
        Tool::new(
            "get_completions",
            "Request LSP completions at a cursor position via lspz daemon. Returns TOON format. \
             Pass `uri` (file:// or absolute) or `path` (relative needs workspace). \
             Language and backend are auto-detected from file extension if omitted.",
            rmcp::model::object(GetCompletionsInput::json_schema()),
        ),
        Tool::new(
            "get_symbols",
            "Retrieve document symbols via lspz daemon. Returns TOON format. \
             Prefer `path`/`paths` with `workspace` or prior `set_workspace`. \
             Language and backend are auto-detected from file extension if omitted.",
            rmcp::model::object(GetSymbolsInput::json_schema()),
        ),
        Tool::new(
            "set_workspace",
            "Bind this MCP session to a project workspace root (absolute path or file://). \
             Subsequent get_diagnostics/get_symbols/get_completions can use project-relative paths.",
            rmcp::model::object(SetWorkspaceInput::json_schema()),
        ),
    ]
}

impl ServerHandler for DaemonMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "lspz MCP server (daemon mode) — auto-connects to a running lspz daemon. \
             Call `set_workspace` with the project root (or pass `workspace` on each tool call), \
             then use project-relative `path`/`paths`. Absolute `file://` URIs still work.",
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
        context: RequestContext<RoleServer>,
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
                let result = self.handle_diagnostics(input, &context.peer).await?;
                info!("get_diagnostics completed (daemon)");
                Ok(CallToolResult::success(vec![ContentBlock::text(result)]))
            }
            "get_completions" => {
                let input: GetCompletionsInput = serde_json::from_value(serde_json::Value::Object(
                    arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_request(e.to_string(), None))?;
                let result = self.handle_completions(input).await?;
                info!("get_completions completed (daemon)");
                Ok(CallToolResult::success(vec![ContentBlock::text(result)]))
            }
            "get_symbols" => {
                let input: GetSymbolsInput = serde_json::from_value(serde_json::Value::Object(
                    arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_request(e.to_string(), None))?;
                let result = self.handle_symbols(input, &context.peer).await?;
                info!("get_symbols completed (daemon)");
                Ok(CallToolResult::success(vec![ContentBlock::text(result)]))
            }
            "set_workspace" => {
                let input: SetWorkspaceInput = serde_json::from_value(serde_json::Value::Object(
                    arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_request(e.to_string(), None))?;
                let result = self.handle_set_workspace(input)?;
                info!("set_workspace completed (daemon)");
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
