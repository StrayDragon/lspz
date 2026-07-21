//! MCP workspace resolution via Roots, session bind, and tool parameters.
//!
//! Coding agents may advertise workspace folders through MCP `roots/list`
//! (best-effort; Roots is deprecated by SEP-2577). Preferred anchors are
//! explicit `workspace` tool args and session `set_workspace` binds so
//! project-relative paths resolve without guessing `$HOME`.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rmcp::model::ClientInfo;
use rmcp::service::{Peer, RoleServer};
use tracing::{info, warn};

use crate::languages::DEFAULT_LANGUAGES;
use crate::uri::{path_from_file_uri, path_to_file_uri};

/// Soft cap on files opened during a no-target workspace diagnostics scan.
pub const MAX_SCAN_FILES: usize = 8;

/// Max directory depth when discovering source files under a workspace root.
pub const MAX_SCAN_DEPTH: usize = 6;

/// Project markers used for auto-detect and trusted-cwd checks.
pub const ROOT_MARKERS: &[&str] = &[
    "pyproject.toml",
    "pyrightconfig.json",
    "Cargo.toml",
    "go.mod",
    "tsconfig.json",
    "package.json",
    "compile_commands.json",
    ".clangd",
    ".git",
];

/// Result of resolving a workspace for an MCP tool call.
#[derive(Debug, Clone)]
pub struct ResolvedWorkspace {
    /// Canonical workspace root path used for scanning / spawn.
    pub root: String,
    /// MCP root directories when `roots/list` succeeded (for path confinement).
    pub mcp_roots: Vec<PathBuf>,
    /// How this workspace was obtained.
    ///
    /// One of: `explicit` | `session` | `mcp_roots` | `fallback` | `process_cwd`.
    pub source: &'static str,
}

/// Session-scoped workspace bind for one MCP server connection.
#[derive(Clone, Default)]
pub struct SessionWorkspace {
    inner: Arc<Mutex<Option<String>>>,
}

impl SessionWorkspace {
    /// Create an empty session bind.
    pub fn new() -> Self {
        Self::default()
    }

    /// Current bound workspace root, if any.
    pub fn get(&self) -> Option<String> {
        self.inner.lock().ok().and_then(|g| g.clone())
    }

    /// Bind (or replace) the session workspace root (must be an existing dir).
    pub fn set(&self, root: &str) -> Result<String, String> {
        let normalized = normalize_workspace_input(root)?;
        let path = PathBuf::from(&normalized);
        if !path.is_dir() {
            return Err(format!("workspace is not a directory: {normalized}"));
        }
        let root = normalize_root(&normalized);
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "session workspace lock poisoned".to_string())?;
        *guard = Some(root.clone());
        info!(workspace = %root, "session workspace bound");
        Ok(root)
    }
}

/// Directory names skipped while scanning (build/deps/vcs noise).
const SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    ".venv",
    "venv",
    "dist",
    "build",
    "__pycache__",
    ".tox",
    ".mypy_cache",
];

/// Inputs that may identify target files for diagnostics / symbols.
#[derive(Debug, Clone, Default)]
pub struct TargetPathInputs<'a> {
    /// Optional `file://` URI or path-like string.
    pub uri: Option<&'a str>,
    /// Optional single filesystem path (relative or absolute).
    pub path: Option<&'a str>,
    /// Optional list of filesystem paths.
    pub paths: Option<&'a [String]>,
}

/// Resolve the primary workspace directory for an MCP tool call.
///
/// Priority:
/// 1. Explicit `workspace` tool parameter
/// 2. Session `set_workspace` bind
/// 3. MCP `roots/list` when the client advertised `roots`
/// 4. Explicit fallback (daemon cwd) — only if plausible
/// 5. `std::env::current_dir()` — only if plausible
///
/// Returns `Err` when no trusted/plausible anchor exists (callers must not scan).
pub async fn resolve_workspace(
    peer: &Peer<RoleServer>,
    fallback: Option<&str>,
    session: Option<&SessionWorkspace>,
    explicit: Option<&str>,
) -> Result<ResolvedWorkspace, String> {
    let mut mcp_roots = Vec::new();
    let roots_ws = try_list_roots(peer).await;
    if let Some(ref ws) = roots_ws {
        mcp_roots = ws.mcp_roots.clone();
    }

    if let Some(raw) = explicit.map(str::trim).filter(|s| !s.is_empty()) {
        let root = normalize_workspace_input(raw)?;
        let root = normalize_root(&root);
        let ws = ResolvedWorkspace {
            root,
            mcp_roots,
            source: "explicit",
        };
        info!(
            workspace = %ws.root,
            source = ws.source,
            "workspace resolved (explicit tool parameter)"
        );
        return Ok(ws);
    }

    if let Some(session) = session
        && let Some(root) = session.get()
    {
        let ws = ResolvedWorkspace {
            root,
            mcp_roots,
            source: "session",
        };
        info!(
            workspace = %ws.root,
            source = ws.source,
            "workspace resolved (session bind)"
        );
        return Ok(ws);
    }

    if let Some(ws) = roots_ws {
        info!(
            workspace = %ws.root,
            source = ws.source,
            mcp_roots = ws.mcp_roots.len(),
            "workspace resolved (MCP roots)"
        );
        return Ok(ws);
    }

    if let Some(fb) = fallback.filter(|s| !s.is_empty()) {
        let root = normalize_root(fb);
        let ws = ResolvedWorkspace {
            root,
            mcp_roots: Vec::new(),
            source: "fallback",
        };
        ensure_plausible_for_untrusted(&ws)?;
        info!(
            workspace = %ws.root,
            source = ws.source,
            "workspace resolved (no MCP roots / session; using fallback)"
        );
        return Ok(ws);
    }

    let cwd = std::env::current_dir().map_err(|e| {
        format!(
            "Cannot resolve workspace (no roots, session, or cwd: {e}). \
             Pass `workspace`, call `set_workspace`, or pass absolute `uri`/`path`."
        )
    })?;
    let ws = ResolvedWorkspace {
        root: normalize_root(&cwd.to_string_lossy()),
        mcp_roots: Vec::new(),
        source: "process_cwd",
    };
    ensure_plausible_for_untrusted(&ws)?;
    info!(
        workspace = %ws.root,
        source = ws.source,
        "workspace resolved (process cwd)"
    );
    Ok(ws)
}

/// Reject untrusted anchors that look like `$HOME` / `/` / non-projects.
pub fn ensure_plausible_for_untrusted(ws: &ResolvedWorkspace) -> Result<(), String> {
    if ws.source == "explicit" || ws.source == "session" || ws.source == "mcp_roots" {
        return Ok(());
    }
    if let Some(reason) = disallowed_untrusted_root(Path::new(&ws.root)) {
        return Err(format!(
            "Cannot use workspace `{}` (source={}): {reason}. \
             Pass `workspace` (absolute project root), call `set_workspace`, \
             pass absolute `uri`/`path`, or ensure the MCP client advertises workspace roots.",
            ws.root, ws.source
        ));
    }
    Ok(())
}

/// Whether an untrusted root must not be scanned.
pub fn disallowed_untrusted_root(path: &Path) -> Option<&'static str> {
    let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if canon == Path::new("/") {
        return Some("filesystem root is not a project workspace");
    }
    if let Some(home) = dirs::home_dir() {
        let home = std::fs::canonicalize(&home).unwrap_or(home);
        if canon == home {
            return Some("home directory is not a project workspace");
        }
    }
    if !has_project_marker(&canon) {
        return Some("no project markers (Cargo.toml, package.json, go.mod, .git, …) at this path");
    }
    None
}

/// True if `dir` contains a known project marker (file or `.git` dir/file).
pub fn has_project_marker(dir: &Path) -> bool {
    for marker in ROOT_MARKERS {
        let p = dir.join(marker);
        if p.is_file() || (*marker == ".git" && p.exists()) {
            return true;
        }
    }
    false
}

/// Collect non-empty path/URI specs from tool inputs (order preserved, de-duped).
pub fn collect_target_specs(inputs: &TargetPathInputs<'_>) -> Vec<String> {
    let mut out = Vec::new();
    let mut push = |raw: &str| {
        let t = raw.trim();
        if t.is_empty() {
            return;
        }
        if !out.iter().any(|s: &String| s == t) {
            out.push(t.to_string());
        }
    };
    if let Some(u) = inputs.uri {
        push(u);
    }
    if let Some(p) = inputs.path {
        push(p);
    }
    if let Some(paths) = inputs.paths {
        for p in paths {
            push(p);
        }
    }
    out
}

/// Resolve a single path/URI spec to a `file://` URI.
///
/// Relative paths require `workspace_root`.
pub fn resolve_target_to_file_uri(
    spec: &str,
    workspace_root: Option<&str>,
) -> Result<String, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("empty path/uri".into());
    }
    if spec.starts_with("file://") {
        let path = path_from_file_uri(spec)?;
        let abs = std::fs::canonicalize(&path).unwrap_or(path);
        return Ok(path_to_file_uri(&abs));
    }
    let path = PathBuf::from(spec);
    let abs = if path.is_absolute() {
        path
    } else {
        let root = workspace_root.ok_or_else(|| {
            format!(
                "Relative path `{spec}` requires a workspace anchor. \
                 Pass `workspace`, call `set_workspace`, or use an absolute path / file:// URI."
            )
        })?;
        PathBuf::from(root).join(&path)
    };
    let abs = std::fs::canonicalize(&abs).unwrap_or(abs);
    Ok(path_to_file_uri(&abs))
}

/// Resolve all target specs to `file://` URIs.
pub fn resolve_target_uris(
    specs: &[String],
    workspace_root: Option<&str>,
) -> Result<Vec<String>, String> {
    specs
        .iter()
        .map(|s| resolve_target_to_file_uri(s, workspace_root))
        .collect()
}

/// Normalize a workspace tool argument (`file://` or filesystem path) to an absolute path string.
pub fn normalize_workspace_input(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("workspace must not be empty".into());
    }
    let path = if raw.starts_with("file://") {
        path_from_file_uri(raw)?
    } else {
        PathBuf::from(raw)
    };
    let abs = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|e| format!("cannot resolve relative workspace: {e}"))?
            .join(path)
    };
    let abs = std::fs::canonicalize(&abs).unwrap_or(abs);
    Ok(abs.to_string_lossy().into_owned())
}

/// Format `set_workspace` success TOON.
pub fn format_set_workspace_toon(workspace: &str) -> String {
    format!("workspace: {workspace}\nworkspace_source: session\n")
}

async fn try_list_roots(peer: &Peer<RoleServer>) -> Option<ResolvedWorkspace> {
    let info = match peer.peer_info() {
        Some(i) => i,
        None => {
            info!("MCP peer_info unavailable; cannot query roots");
            return None;
        }
    };
    log_client_info(info.as_ref());
    if info.capabilities.roots.is_none() {
        info!(
            client = %info.client_info.name,
            version = %info.client_info.version,
            "MCP client did not advertise roots capability; will use session/explicit/cwd"
        );
        return None;
    }

    #[allow(deprecated)]
    match peer.list_roots().await {
        Ok(result) => {
            if result.roots.is_empty() {
                warn!(
                    client = %info.client_info.name,
                    "roots/list returned empty list; not binding mcp_roots"
                );
                return None;
            }
            let mut mcp_roots = Vec::new();
            for root in &result.roots {
                match path_from_file_uri(&root.uri) {
                    Ok(path) => {
                        let canon = std::fs::canonicalize(&path).unwrap_or(path);
                        info!(
                            uri = %root.uri,
                            path = %canon.display(),
                            name = ?root.name,
                            "MCP root received"
                        );
                        mcp_roots.push(canon);
                    }
                    Err(e) => {
                        warn!(uri = %root.uri, error = %e, "Skipping unusable MCP root URI");
                    }
                }
            }
            let first = mcp_roots.first()?.clone();
            let s = normalize_root(&first.to_string_lossy());
            Some(ResolvedWorkspace {
                root: s,
                mcp_roots,
                source: "mcp_roots",
            })
        }
        Err(e) => {
            warn!(
                client = %info.client_info.name,
                error = %e,
                "roots/list failed; falling back to session/explicit/cwd"
            );
            None
        }
    }
}

/// When MCP roots are known, reject paths that fall outside every root.
pub fn ensure_path_under_roots(path: &Path, mcp_roots: &[PathBuf]) -> Result<(), String> {
    if mcp_roots.is_empty() {
        return Ok(());
    }
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    for root in mcp_roots {
        if abs.starts_with(root) {
            return Ok(());
        }
    }
    Err(format!(
        "Path {} is outside MCP workspace roots; pass a file under the client workspace",
        abs.display()
    ))
}

fn log_client_info(info: &ClientInfo) {
    info!(
        client = %info.client_info.name,
        version = %info.client_info.version,
        protocol = ?info.protocol_version,
        roots_capability = info.capabilities.roots.is_some(),
        "MCP peer info"
    );
}

fn normalize_root(raw: &str) -> String {
    let path = PathBuf::from(raw);
    std::fs::canonicalize(&path)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// Prefer extensions that match project markers at `root`, else all known langs.
fn preferred_extensions(root: &Path) -> Vec<&'static str> {
    let mut preferred: Vec<&'static str> = Vec::new();
    if root.join("Cargo.toml").is_file() {
        preferred.extend(["rs"]);
    }
    if root.join("go.mod").is_file() {
        preferred.extend(["go"]);
    }
    if root.join("tsconfig.json").is_file() || root.join("package.json").is_file() {
        preferred.extend(["ts", "tsx", "js", "jsx"]);
    }
    if root.join("pyproject.toml").is_file()
        || root.join("pyrightconfig.json").is_file()
        || root.join("setup.py").is_file()
    {
        preferred.extend(["py"]);
    }
    if root.join("compile_commands.json").is_file() || root.join(".clangd").is_file() {
        preferred.extend(["c", "h", "cpp", "cc", "cxx", "hpp", "hxx"]);
    }

    if preferred.is_empty() {
        for mapping in DEFAULT_LANGUAGES {
            preferred.extend(mapping.extensions.iter().copied());
        }
    }
    preferred.sort_unstable();
    preferred.dedup();
    preferred
}

/// Discover up to `max` source files under `root` for a workspace scan.
pub fn discover_source_files(root: &Path, max: usize) -> Vec<PathBuf> {
    if max == 0 {
        return Vec::new();
    }
    let exts = preferred_extensions(root);
    let mut found = Vec::new();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((root.to_path_buf(), 0));

    while let Some((dir, depth)) = queue.pop_front() {
        if found.len() >= max || depth > MAX_SCAN_DEPTH {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            if found.len() >= max {
                break;
            }
            let path = entry.path();
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if SKIP_DIRS.iter().any(|s| *s == name) || name.starts_with('.') {
                    continue;
                }
                queue.push_back((path, depth + 1));
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if exts.contains(&ext) {
                found.push(path);
            }
        }
    }
    found
}

/// Count severity words in a diagnostics TOON body (best-effort for overview rows).
pub(crate) fn count_diag_severities_in_toon(toon: &str) -> (u32, u32, u32, u32) {
    let mut e = 0u32;
    let mut w = 0u32;
    let mut i = 0u32;
    let mut h = 0u32;
    for line in toon.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("error,") {
            e += 1;
        } else if trimmed.starts_with("warning,") {
            w += 1;
        } else if trimmed.starts_with("info,") {
            i += 1;
        } else if trimmed.starts_with("hint,") {
            h += 1;
        }
    }
    (e, w, i, h)
}

/// Build a dense TOON overview table plus optional per-file bodies.
pub fn format_workspace_scan_toon(
    workspace: &str,
    source: &str,
    rows: &[(String, u32, u32, u32, u32)],
    bodies: &[String],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("workspace: {workspace}\n"));
    out.push_str(&format!("workspace_source: {source}\n"));
    out.push_str(&format!("scanned[{}]", rows.len()));
    out.push_str("{path,errors,warnings,infos,hints}:\n");
    for (path, e, w, i, h) in rows {
        out.push_str(&format!("  {path},{e},{w},{i},{h}\n"));
    }
    if !bodies.is_empty() {
        out.push_str("---\n");
        for (idx, body) in bodies.iter().enumerate() {
            if idx > 0 {
                out.push_str("---\n");
            }
            out.push_str(body);
            if !body.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    out
}

/// Build a dense TOON overview for a no-target symbols workspace scan.
pub fn format_workspace_symbols_scan_toon(
    workspace: &str,
    source: &str,
    rows: &[(String, u32)],
    bodies: &[String],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("workspace: {workspace}\n"));
    out.push_str(&format!("workspace_source: {source}\n"));
    out.push_str(&format!("scanned[{}]", rows.len()));
    out.push_str("{path,symbols}:\n");
    for (path, n) in rows {
        out.push_str(&format!("  {path},{n}\n"));
    }
    if !bodies.is_empty() {
        out.push_str("---\n");
        for (idx, body) in bodies.iter().enumerate() {
            if idx > 0 {
                out.push_str("---\n");
            }
            out.push_str(body);
            if !body.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    out
}

/// Count symbol table rows in a symbols TOON body.
pub(crate) fn count_symbol_rows_in_toon(toon: &str) -> u32 {
    toon.lines()
        .filter(|line| {
            let t = line.trim_start();
            !t.is_empty()
                && !t.starts_with("uri:")
                && !t.starts_with("symbols[")
                && !t.starts_with('#')
                && !t.starts_with("---")
        })
        .count() as u32
}

/// Convert a filesystem path to a `file://` URI (absolute when possible).
pub fn file_uri_for_path(path: &Path) -> String {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    path_to_file_uri(&abs)
}

/// Display path relative to workspace when possible.
pub fn display_rel_path(workspace_root: &Path, uri: &str) -> String {
    let Ok(path) = path_from_file_uri(uri) else {
        return uri.to_string();
    };
    path.strip_prefix(workspace_root)
        .unwrap_or(&path)
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn discovers_rust_sources_under_cargo_project() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[package]\nname=\"t\"\n").unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(tmp.path().join("src/main.rs"), "fn main() {}\n").unwrap();
        fs::write(tmp.path().join("src/lib.rs"), "").unwrap();
        fs::create_dir_all(tmp.path().join("target/debug")).unwrap();
        fs::write(tmp.path().join("target/debug/x.rs"), "should skip\n").unwrap();

        let files = discover_source_files(tmp.path(), 8);
        assert_eq!(files.len(), 2);
        assert!(
            files
                .iter()
                .all(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
        );
    }

    #[test]
    fn format_scan_has_dense_header() {
        let text = format_workspace_scan_toon(
            "/proj",
            "process_cwd",
            &[("src/main.rs".into(), 1, 2, 0, 0)],
            &["uri: file:///proj/src/main.rs\ndiagnostics[0]{severity,message,code,range,count}:\n".into()],
        );
        assert!(text.contains("workspace_source: process_cwd"));
        assert!(text.contains("scanned[1]{path,errors,warnings,infos,hints}:"));
        assert!(text.contains("  src/main.rs,1,2,0,0"));
        assert!(text.contains("---\nuri:"));
    }

    #[test]
    fn rejects_path_outside_mcp_roots() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("proj");
        fs::create_dir_all(&root).unwrap();
        let inside = root.join("src/main.rs");
        fs::create_dir_all(inside.parent().unwrap()).unwrap();
        fs::write(&inside, "fn main(){}\n").unwrap();
        let outside = tmp.path().join("other.rs");
        fs::write(&outside, "x\n").unwrap();

        let roots = vec![std::fs::canonicalize(&root).unwrap()];
        assert!(ensure_path_under_roots(&inside, &roots).is_ok());
        assert!(ensure_path_under_roots(&outside, &roots).is_err());
        assert!(ensure_path_under_roots(&outside, &[]).is_ok());
    }

    #[test]
    fn cursor_like_empty_uri_inputs_deserialize() {
        let diag: super::super::server::GetDiagnosticsInput =
            serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(diag.uri.is_none());
        let sym: super::super::server::GetSymbolsInput =
            serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(sym.uri.is_none());
        let diag_empty: super::super::server::GetDiagnosticsInput =
            serde_json::from_value(serde_json::json!({"uri": ""})).unwrap();
        assert_eq!(diag_empty.uri.as_deref(), Some(""));
    }

    #[test]
    fn unknown_field_is_rejected() {
        let err = serde_json::from_value::<super::super::server::GetDiagnosticsInput>(
            serde_json::json!({"typo_field": "x"}),
        );
        assert!(err.is_err());
    }

    #[test]
    fn symbols_scan_format_is_dense() {
        let text = format_workspace_symbols_scan_toon(
            "/proj",
            "mcp_roots",
            &[("src/lib.rs".into(), 3)],
            &["uri: file:///proj/src/lib.rs\nsymbols[1]{name,kind,range,detail,container}:\n  Foo,class,1:0-10:1,,\n".into()],
        );
        assert!(text.contains("workspace_source: mcp_roots"));
        assert!(text.contains("scanned[1]{path,symbols}:"));
        assert!(text.contains("  src/lib.rs,3"));
        assert_eq!(
            count_symbol_rows_in_toon("uri: x\nsymbols[1]{name}:\n  Foo,class,1:0-1:1,,\n"),
            1
        );
    }

    #[test]
    fn diagnostics_and_symbols_schemas_omit_uri_required() {
        use super::super::server::{GetDiagnosticsInput, GetSymbolsInput, JsonSchema};
        let d = GetDiagnosticsInput::json_schema();
        let s = GetSymbolsInput::json_schema();
        assert!(d.get("required").is_none());
        assert!(s.get("required").is_none());
        assert!(d["properties"].get("paths").is_some());
        assert!(d["properties"].get("workspace").is_some());
    }

    #[test]
    fn home_is_disallowed_untrusted_root() {
        let home = dirs::home_dir().expect("home");
        assert!(disallowed_untrusted_root(&home).is_some());
        let ws = ResolvedWorkspace {
            root: home.to_string_lossy().into_owned(),
            mcp_roots: Vec::new(),
            source: "fallback",
        };
        assert!(ensure_plausible_for_untrusted(&ws).is_err());
        let trusted = ResolvedWorkspace {
            root: home.to_string_lossy().into_owned(),
            mcp_roots: Vec::new(),
            source: "explicit",
        };
        assert!(ensure_plausible_for_untrusted(&trusted).is_ok());
    }

    #[test]
    fn relative_path_needs_workspace() {
        let err = resolve_target_to_file_uri("src/main.rs", None).unwrap_err();
        assert!(err.contains("workspace"));
    }

    #[test]
    fn relative_path_joins_workspace() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(tmp.path().join("src/main.rs"), "fn main(){}\n").unwrap();
        let root = tmp.path().to_string_lossy().into_owned();
        let uri = resolve_target_to_file_uri("src/main.rs", Some(&root)).unwrap();
        assert!(uri.starts_with("file://"));
        assert!(uri.ends_with("src/main.rs") || uri.contains("src/main.rs"));
    }

    #[test]
    fn session_workspace_bind_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let session = SessionWorkspace::new();
        let root = session.set(&tmp.path().to_string_lossy()).unwrap();
        assert_eq!(session.get().as_deref(), Some(root.as_str()));
        assert!(format_set_workspace_toon(&root).contains("workspace_source: session"));
    }

    #[test]
    fn collect_targets_dedupes() {
        let paths = vec!["a.rs".into(), "a.rs".into(), "b.rs".into()];
        let specs = collect_target_specs(&TargetPathInputs {
            uri: Some("file:///x.rs"),
            path: Some("a.rs"),
            paths: Some(&paths),
        });
        assert_eq!(specs, vec!["file:///x.rs", "a.rs", "b.rs"]);
    }

    #[test]
    fn project_with_cargo_is_plausible() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[package]\nname=\"t\"\n").unwrap();
        assert!(has_project_marker(tmp.path()));
        assert!(disallowed_untrusted_root(tmp.path()).is_none());
    }
}
