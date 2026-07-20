//! MCP workspace resolution via Roots (with cwd fallback).
//!
//! Coding agents such as Cursor may advertise workspace folders through the
//! MCP `roots/list` request after initialize. Roots is deprecated by SEP-2577
//! but remains the practical way to learn the client workspace over stdio
//! until a replacement lands — we keep using it with a cwd fallback.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use rmcp::model::ClientInfo;
use rmcp::service::{Peer, RoleServer};
use tracing::{info, warn};

use crate::languages::DEFAULT_LANGUAGES;
use crate::uri::{path_from_file_uri, path_to_file_uri};

/// Soft cap on files opened during a no-`uri` workspace diagnostics scan.
pub const MAX_SCAN_FILES: usize = 8;

/// Max directory depth when discovering source files under a workspace root.
pub const MAX_SCAN_DEPTH: usize = 6;

/// Result of resolving a workspace for an MCP tool call.
#[derive(Debug, Clone)]
pub struct ResolvedWorkspace {
    /// Canonical workspace root path used for scanning / spawn.
    pub root: String,
    /// MCP root directories when `roots/list` succeeded (for path confinement).
    pub mcp_roots: Vec<PathBuf>,
    /// How this workspace was obtained (`mcp_roots` | `fallback` | `process_cwd`).
    pub source: &'static str,
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

/// Resolve the primary workspace directory for an MCP tool call.
///
/// Order:
/// 1. MCP `roots/list` when the client advertised `roots` capability
/// 2. Explicit fallback (daemon cwd / process cwd)
/// 3. `std::env::current_dir()`
pub async fn resolve_workspace(
    peer: &Peer<RoleServer>,
    fallback: Option<&str>,
) -> Option<ResolvedWorkspace> {
    if let Some(ws) = try_list_roots(peer).await {
        info!(
            workspace = %ws.root,
            source = ws.source,
            mcp_roots = ws.mcp_roots.len(),
            "workspace resolved"
        );
        return Some(ws);
    }
    if let Some(fb) = fallback.filter(|s| !s.is_empty()) {
        let ws = ResolvedWorkspace {
            root: normalize_root(fb),
            mcp_roots: Vec::new(),
            source: "fallback",
        };
        info!(
            workspace = %ws.root,
            source = ws.source,
            "workspace resolved (no usable MCP roots; using fallback)"
        );
        return Some(ws);
    }
    let ws = std::env::current_dir().ok().map(|p| ResolvedWorkspace {
        root: normalize_root(&p.to_string_lossy()),
        mcp_roots: Vec::new(),
        source: "process_cwd",
    })?;
    info!(
        workspace = %ws.root,
        source = ws.source,
        "workspace resolved (no MCP roots / fallback; using process cwd)"
    );
    Some(ws)
}

async fn try_list_roots(peer: &Peer<RoleServer>) -> Option<ResolvedWorkspace> {
    let info = peer.peer_info()?;
    log_client_info(info.as_ref());
    if info.capabilities.roots.is_none() {
        info!(
            client = %info.client_info.name,
            "MCP client did not advertise roots capability; will use fallback/cwd"
        );
        return None;
    }

    #[allow(deprecated)]
    match peer.list_roots().await {
        Ok(result) => {
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
            warn!(error = %e, "roots/list failed; falling back to cwd");
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

/// Build a dense TOON overview for a no-`uri` symbols workspace scan.
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
        // Coding agents often omit uri entirely when workspace is known via roots.
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
    }
}
