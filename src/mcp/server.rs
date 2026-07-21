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
            "Get LSP diagnostics. Returns dense TOON. \
             Prefer `path`/`paths` (project-relative OK) with `workspace` or prior `set_workspace`. \
             Absolute `uri` (file://) still works. Omit targets only when workspace is known \
             (roots / session / explicit) to scan a small source set. \
             Language and backend are auto-detected from file extension if omitted.",
            rmcp::model::object(GetDiagnosticsInput::json_schema()),
        ),
        Tool::new(
            "get_completions",
            "Request LSP completions at a cursor position. Returns TOON format \
             with label, kind, detail, documentation. \
             Pass `uri` (file:// or absolute) or `path` (relative needs workspace). \
             Language and backend are auto-detected from file extension if omitted.",
            rmcp::model::object(GetCompletionsInput::json_schema()),
        ),
        Tool::new(
            "get_symbols",
            "Retrieve document symbols. Returns dense TOON. \
             Prefer `path`/`paths` with `workspace` or prior `set_workspace`. \
             Omit targets only when workspace is known to scan a small source set. \
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

// ─── Input Types ───────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetDiagnosticsInput {
    /// Target file URI (`file://…` or absolute path). Optional when using path(s).
    pub uri: Option<String>,
    /// Single filesystem path (relative to workspace or absolute).
    pub path: Option<String>,
    /// Multiple filesystem paths (relative to workspace or absolute).
    pub paths: Option<Vec<String>>,
    /// Project workspace root (absolute path or `file://`). Preferred over session/roots.
    pub workspace: Option<String>,
    pub backend: Option<String>,
    pub language: Option<String>,
    pub backend_args: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetCompletionsInput {
    /// Target file URI (`file://…` or absolute path). Optional when `path` is set.
    pub uri: Option<String>,
    /// Filesystem path (relative needs workspace).
    pub path: Option<String>,
    /// Project workspace root for relative `path`.
    pub workspace: Option<String>,
    pub backend: Option<String>,
    pub language: Option<String>,
    pub line: u32,
    pub character: u32,
    pub backend_args: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetSymbolsInput {
    /// Target file URI. Optional when using path(s).
    pub uri: Option<String>,
    /// Single filesystem path (relative to workspace or absolute).
    pub path: Option<String>,
    /// Multiple filesystem paths.
    pub paths: Option<Vec<String>>,
    /// Project workspace root (absolute path or `file://`).
    pub workspace: Option<String>,
    pub backend: Option<String>,
    pub language: Option<String>,
    pub backend_args: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetWorkspaceInput {
    /// Absolute project root path or `file://` URI.
    pub workspace: String,
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
        if super::workspace::has_project_marker(dir) {
            return std::fs::canonicalize(dir)
                .map(|p| p.to_string_lossy().to_string())
                .ok();
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
    /// Session-scoped workspace bind (`set_workspace` / explicit workspace param).
    session_workspace: super::workspace::SessionWorkspace,
}

impl McpServer {
    /// Create a new MCP server with an empty connection pool.
    pub fn new() -> Self {
        Self {
            pool: Arc::new(Mutex::new(LspPool::new())),
            root_cache: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
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
        super::workspace::resolve_workspace(peer, None, Some(&self.session_workspace), explicit)
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
        let specs = super::workspace::collect_target_specs(&super::workspace::TargetPathInputs {
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
            super::workspace::ensure_plausible_for_untrusted(&workspace)
                .map_err(|e| ErrorData::invalid_request(e, None))?;
            return self
                .handle_diagnostics_workspace_scan(&input, workspace)
                .await;
        }

        let uris = super::workspace::resolve_target_uris(&specs, workspace_root)
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
                super::workspace::ResolvedWorkspace {
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
        workspace: &super::workspace::ResolvedWorkspace,
    ) -> Result<String, ErrorData> {
        let root_path = std::path::Path::new(&workspace.root);
        let mut rows = Vec::new();
        let mut bodies = Vec::new();
        for uri in uris {
            let rel = super::workspace::display_rel_path(root_path, uri);
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
                    let (e, w, i, h) = super::workspace::count_diag_severities_in_toon(&body);
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
        Ok(super::workspace::format_workspace_scan_toon(
            &workspace.root,
            workspace.source,
            &rows,
            &bodies,
        ))
    }

    async fn handle_diagnostics_workspace_scan(
        &self,
        input: &GetDiagnosticsInput,
        workspace: super::workspace::ResolvedWorkspace,
    ) -> Result<String, ErrorData> {
        let root_path = std::path::Path::new(&workspace.root);
        let files =
            super::workspace::discover_source_files(root_path, super::workspace::MAX_SCAN_FILES);
        if files.is_empty() {
            return Ok(super::workspace::format_workspace_scan_toon(
                &workspace.root,
                workspace.source,
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
        self.apply_explicit_workspace(input.workspace.as_deref())?;
        let specs = super::workspace::collect_target_specs(&super::workspace::TargetPathInputs {
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
                super::workspace::normalize_workspace_input(s)
                    .map_err(|e| ErrorData::invalid_request(e, None))
            })
            .transpose()?
            .or_else(|| self.session_workspace.get());
        let uri =
            super::workspace::resolve_target_to_file_uri(&specs[0], workspace_root.as_deref())
                .map_err(|e| ErrorData::invalid_request(e, None))?;

        let (language, backend) =
            resolve_language_backend(&uri, input.language.as_deref(), input.backend.as_deref())?;
        let root = self.cached_workspace_root(&uri);
        let extra = merge_backend_args(&uri, input.backend_args.as_deref());

        let session = {
            LspPool::get_or_spawn(&self.pool, &language, &backend, root.as_deref(), &extra)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        };
        let mut session = session.lock().await;

        let path = crate::uri::path_from_file_uri(&uri)
            .map_err(|e| ErrorData::invalid_request(e, None))?;
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        // Open the document on first call, or send didChange on subsequent calls.
        session
            .open_or_update_document(&uri, &language, &content)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let result = session
            .send_request(
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

    async fn handle_symbols(
        &self,
        input: GetSymbolsInput,
        peer: &rmcp::service::Peer<RoleServer>,
    ) -> Result<String, ErrorData> {
        self.apply_explicit_workspace(input.workspace.as_deref())?;
        let specs = super::workspace::collect_target_specs(&super::workspace::TargetPathInputs {
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
            super::workspace::ensure_plausible_for_untrusted(&workspace)
                .map_err(|e| ErrorData::invalid_request(e, None))?;
            return self.handle_symbols_workspace_scan(&input, workspace).await;
        }

        let uris = super::workspace::resolve_target_uris(&specs, workspace_root)
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
                super::workspace::ResolvedWorkspace {
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
        workspace: &super::workspace::ResolvedWorkspace,
    ) -> Result<String, ErrorData> {
        let root_path = std::path::Path::new(&workspace.root);
        let mut rows = Vec::new();
        let mut bodies = Vec::new();
        for uri in uris {
            let rel = super::workspace::display_rel_path(root_path, uri);
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
                    let n = super::workspace::count_symbol_rows_in_toon(&body);
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
        Ok(super::workspace::format_workspace_symbols_scan_toon(
            &workspace.root,
            workspace.source,
            &rows,
            &bodies,
        ))
    }

    async fn handle_symbols_workspace_scan(
        &self,
        input: &GetSymbolsInput,
        workspace: super::workspace::ResolvedWorkspace,
    ) -> Result<String, ErrorData> {
        let root_path = std::path::Path::new(&workspace.root);
        let files =
            super::workspace::discover_source_files(root_path, super::workspace::MAX_SCAN_FILES);
        if files.is_empty() {
            return Ok(super::workspace::format_workspace_symbols_scan_toon(
                &workspace.root,
                workspace.source,
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
             Call `set_workspace` with the project root (or pass `workspace` on each tool call), \
             then use project-relative `path`/`paths`. Absolute `file://` URIs still work. \
             Omit targets only when workspace is known (roots / session / explicit project root).",
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
            "set_workspace" => {
                let input: SetWorkspaceInput = serde_json::from_value(serde_json::Value::Object(
                    arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_request(e.to_string(), None))?;
                let result = self.handle_set_workspace(input)?;
                info!("set_workspace completed");
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
            "additionalProperties": false,
            "properties": {
                "uri": {
                    "type": "string",
                    "description": "File URI (file://…) or absolute path. Optional when using path/paths."
                },
                "path": {
                    "type": "string",
                    "description": "Project-relative or absolute filesystem path."
                },
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Multiple project-relative or absolute filesystem paths."
                },
                "workspace": {
                    "type": "string",
                    "description": "Project workspace root (absolute path or file://). Preferred over session bind / MCP roots."
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
            "additionalProperties": false,
            "properties": {
                "uri": { "type": "string", "description": "File URI (file://…) or absolute path. Optional when path is set." },
                "path": { "type": "string", "description": "Project-relative or absolute filesystem path." },
                "workspace": { "type": "string", "description": "Project workspace root for relative path." },
                "backend": { "type": "string", "description": "Backend LSP server command. Auto-detected from file extension if omitted." },
                "language": { "type": "string", "description": "Language identifier. Auto-detected from file extension if omitted." },
                "line": { "type": "integer", "description": "Line number (0-based)" },
                "character": { "type": "integer", "description": "Character offset (0-based)" },
                "backend_args": { "type": "array", "items": { "type": "string" }, "description": "Extra CLI arguments passed to the backend LSP server (appended to defaults)." }
            },
            "required": ["line", "character"]
        })
    }
}

impl JsonSchema for GetSymbolsInput {
    fn json_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "uri": {
                    "type": "string",
                    "description": "File URI or absolute path. Optional when using path/paths."
                },
                "path": {
                    "type": "string",
                    "description": "Project-relative or absolute filesystem path."
                },
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Multiple project-relative or absolute filesystem paths."
                },
                "workspace": {
                    "type": "string",
                    "description": "Project workspace root (absolute path or file://)."
                },
                "backend": { "type": "string", "description": "Backend LSP server command. Auto-detected from file extension if omitted." },
                "language": { "type": "string", "description": "Language identifier. Auto-detected from file extension if omitted." },
                "backend_args": { "type": "array", "items": { "type": "string" }, "description": "Extra CLI arguments passed to the backend LSP server (appended to defaults)." }
            }
        })
    }
}

impl JsonSchema for SetWorkspaceInput {
    fn json_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "workspace": {
                    "type": "string",
                    "description": "Absolute project root path or file:// URI to bind for this MCP session."
                }
            },
            "required": ["workspace"]
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
