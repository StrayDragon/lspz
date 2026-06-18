//! Daemon socket path helpers.
//!
//! Centralizes socket path generation so the daemon server and
//! all clients produce the same path for a given workspace.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

/// Compute the Unix socket path for a given workspace root.
///
/// Hash-based deterministic: same path every time for the same workspace.
/// Format: `~/.cache/lspz/<slug>-<hash>.sock`
pub fn socket_path_for_workspace(workspace_root: &str) -> PathBuf {
    let hash = hash_string(workspace_root);
    let slug = workspace_slug(workspace_root);
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
}
