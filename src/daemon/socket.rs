//! Daemon socket path helpers.
//!
//! Centralizes socket path generation so the daemon server and all clients
//! produce the same path for a given workspace.
//!
//! ## Canonicalization (mandatory)
//!
//! [`socket_path_for_workspace`] canonicalizes the workspace path (resolving
//! symlinks, `..`, redundant separators, trailing slashes) **before** hashing.
//! This is what makes daemon reuse actually work: the same directory reached
//! via different strings (`/proj`, `/proj/`, `/home/../proj`, a symlink) must
//! land on the *same* socket. Without it, every spelling variant spawns a
//! fresh daemon, each pinning its own language server, and the detached
//! processes pile up forever.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

/// Resolve a workspace root string into a canonicalized absolute path.
///
/// This is the single normalization point for workspace identity across the
/// whole crate: the CLI (`lspz mcp`, `lspz daemon`, `lspz daemon list`), the
/// MCP server, and the daemon client all derive the daemon socket path from a
/// workspace root, so they must all agree on what "the same workspace" means.
///
/// # Rules
///
/// - If the path exists on disk, it is fully canonicalized (resolves symlinks,
///   `..`, redundant separators, trailing slashes).
/// - If canonicalization fails (e.g. the path does not exist yet, or the
///   caller passed a non-path string in tests), the input is returned as-is so
///   callers still get a deterministic — though possibly non-canonical — key.
///   This keeps startup working on fresh workspaces and keeps the function
///   infallible.
pub fn resolve_workspace_root(workspace_root: &str) -> String {
    std::fs::canonicalize(workspace_root)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| workspace_root.to_string())
}

/// Compute the Unix socket path for a given workspace root.
///
/// The workspace root is first canonicalized via [`resolve_workspace_root`],
/// then hashed deterministically: the same directory always maps to the same
/// socket, regardless of how its path was spelled.
///
/// Format: `~/.cache/lspz/<slug>-<hash>.sock`
pub fn socket_path_for_workspace(workspace_root: &str) -> PathBuf {
    let canonical = resolve_workspace_root(workspace_root);
    let hash = hash_string(&canonical);
    let slug = workspace_slug(&canonical);
    let lspz_dir = lspz_cache_dir();
    lspz_dir.join(format!("{slug}-{hash:016x}.sock"))
}

/// Hash a string for use in socket filenames.
fn hash_string(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Extract a human-friendly slug from a workspace path (its last component).
fn workspace_slug(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// Get the lspz cache directory (`~/.cache/lspz` on Linux, `~/Library/Caches/lspz` on macOS).
pub fn lspz_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("lspz")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_path_stable() {
        let a = socket_path_for_workspace("/home/user/project");
        let b = socket_path_for_workspace("/home/user/project");
        assert_eq!(a, b);
    }

    #[test]
    fn test_socket_path_different_workspaces() {
        let a = socket_path_for_workspace("/home/user/project-a");
        let b = socket_path_for_workspace("/home/user/project-b");
        assert_ne!(a, b);
    }

    #[test]
    fn test_slug_from_path() {
        assert_eq!(workspace_slug("/home/user/my-project"), "my-project");
        assert_eq!(workspace_slug("/"), "unknown"); // root has no filename
        assert_eq!(workspace_slug("/no_trailing_slash"), "no_trailing_slash");
    }

    #[test]
    fn test_hash_deterministic() {
        assert_eq!(hash_string("hello"), hash_string("hello"));
    }

    /// A trailing slash must not change the socket — same directory.
    #[test]
    fn test_socket_path_ignores_trailing_slash() {
        let dir = tempfile::tempdir().unwrap();
        let canonical = dir.path().canonicalize().unwrap();
        let with_slash = format!("{}/", canonical.display());

        let a = socket_path_for_workspace(canonical.to_str().unwrap());
        let b = socket_path_for_workspace(&with_slash);
        assert_eq!(a, b, "trailing slash must not change socket");
    }

    /// `..` segments must not change the socket — they resolve to the same dir.
    #[test]
    fn test_socket_path_resolves_dotdot() {
        let dir = tempfile::tempdir().unwrap();
        // Create `sub` so that `<dir>/sub/..` canonicalizes successfully.
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        let canonical = dir.path().canonicalize().unwrap();
        let via_dotdot = canonical.join("sub").join("..");

        let a = socket_path_for_workspace(canonical.to_str().unwrap());
        let b = socket_path_for_workspace(via_dotdot.to_str().unwrap());
        assert_eq!(a, b, "'..' segments must not change socket");
    }

    /// `resolve_workspace_root` is infallible: unknown paths fall back as-is.
    #[test]
    fn test_resolve_nonexistent_falls_back() {
        let raw = "/this/path/does/not/exist/lspz-xyz";
        let resolved = resolve_workspace_root(raw);
        assert_eq!(resolved, raw);
    }
}
