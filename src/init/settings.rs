//! Settings.json read/write utilities for Claude Code integration.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::constants::{CLAUDE_DIR, CLAUDE_DIR_ENV, CLAUDE_JSON, MCP_SERVER_KEY, SETTINGS_JSON};
use crate::error::LspzError;

/// Result of a settings patching operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchResult {
    /// Entry was added successfully.
    Patched,
    /// Entry was already present with the same value.
    AlreadyPresent,
    /// Dry-run: would have patched.
    WouldPatch,
}

/// Resolve the Claude config directory.
///
/// Checks `LSPZ_CLAUDE_DIR` env var first, then falls back to `~/.claude/`.
pub fn resolve_claude_dir() -> Result<PathBuf, LspzError> {
    if let Ok(dir) = std::env::var(CLAUDE_DIR_ENV) {
        return Ok(PathBuf::from(dir));
    }
    dirs::home_dir()
        .map(|home| home.join(CLAUDE_DIR))
        .ok_or_else(|| LspzError::Config("Cannot determine home directory".into()))
}

/// Resolve the absolute path of the running binary.
pub fn resolve_binary_path() -> Result<PathBuf, LspzError> {
    std::env::current_exe().map_err(|e| LspzError::Config(format!("Cannot determine binary path: {e}")))
}

/// Atomic write: write to a tempfile in the same directory, then rename.
pub fn atomic_write(path: &Path, content: &[u8]) -> Result<(), LspzError> {
    let dir = path
        .parent()
        .ok_or_else(|| LspzError::Config(format!("No parent directory for {}", path.display())))?;

    let mut tmp = tempfile::NamedTempFile::new_in(dir)
        .map_err(|e| LspzError::Config(format!("Failed to create tempfile: {e}")))?;

    tmp.write_all(content)
        .map_err(|e| LspzError::Config(format!("Failed to write tempfile: {e}")))?;

    tmp.persist(path)
        .map_err(|e| LspzError::Config(format!("Failed to persist tempfile: {e}")))?;

    Ok(())
}

/// Read settings.json, or return an empty object if the file doesn't exist.
fn read_settings(path: &Path) -> Result<Value, LspzError> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }

    let content = fs::read_to_string(path)
        .map_err(|e| LspzError::Config(format!("Failed to read {}: {e}", path.display())))?;

    serde_json::from_str(&content)
        .map_err(|e| LspzError::Config(format!("Invalid JSON in {}: {e}", path.display())))
}

/// Check if the lspz MCP server entry is already present with the correct binary path.
fn mcp_server_already_present(settings: &Value, binary_path: &Path) -> bool {
    let cmd = binary_path.to_string_lossy();
    settings
        .get("mcpServers")
        .and_then(|m| m.get(MCP_SERVER_KEY))
        .and_then(|e| e.get("command"))
        .and_then(|c| c.as_str())
        .map(|c| c == cmd)
        .unwrap_or(false)
}

/// Patch settings.json with the lspz MCP server entry.
///
/// Returns `PatchResult::AlreadyPresent` if the entry already exists with the same path.
pub fn patch_mcp_server(
    settings_path: &Path,
    binary_path: &Path,
    dry_run: bool,
) -> Result<PatchResult, LspzError> {
    let mut root = read_settings(settings_path)?;

    if mcp_server_already_present(&root, binary_path) {
        return Ok(PatchResult::AlreadyPresent);
    }

    if dry_run {
        return Ok(PatchResult::WouldPatch);
    }

    let mcp_entry = serde_json::json!({
        "command": binary_path.to_string_lossy(),
        "args": ["mcp"]
    });

    let mcp_servers = root
        .as_object_mut()
        .ok_or_else(|| LspzError::Config("settings.json is not an object".into()))?
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));

    mcp_servers
        .as_object_mut()
        .ok_or_else(|| LspzError::Config("mcpServers is not an object".into()))?
        .insert(MCP_SERVER_KEY.to_string(), mcp_entry);

    let content = serde_json::to_string_pretty(&root)
        .map_err(|e| LspzError::Config(format!("Failed to serialize settings: {e}")))?;

    // Backup original if it exists
    if settings_path.exists() {
        let backup = settings_path.with_extension("json.bak");
        fs::copy(settings_path, &backup)
            .map_err(|e| LspzError::Config(format!("Failed to backup settings: {e}")))?;
    }

    atomic_write(settings_path, content.as_bytes())?;

    Ok(PatchResult::Patched)
}

/// Remove the lspz MCP server entry from settings.json.
///
/// Returns `true` if an entry was removed, `false` if none existed.
pub fn remove_mcp_server(settings_path: &Path, dry_run: bool) -> Result<bool, LspzError> {
    let mut root = read_settings(settings_path)?;

    let removed = root
        .get_mut("mcpServers")
        .and_then(|m| m.as_object_mut())
        .map(|obj| obj.remove(MCP_SERVER_KEY).is_some())
        .unwrap_or(false);

    if !removed || dry_run {
        return Ok(removed);
    }

    let content = serde_json::to_string_pretty(&root)
        .map_err(|e| LspzError::Config(format!("Failed to serialize settings: {e}")))?;

    atomic_write(settings_path, content.as_bytes())?;

    Ok(true)
}

/// Check if the lspz MCP server is registered in settings.json.
pub fn is_mcp_registered(settings_path: &Path, binary_path: &Path) -> bool {
    read_settings(settings_path)
        .map(|s| mcp_server_already_present(&s, binary_path))
        .unwrap_or(false)
}

/// Get the path to the MCP server config file for the given scope.
///
/// - Global: `~/.claude.json` (user-level, alongside other MCP servers)
/// - Project: `.claude/settings.json` (project-level)
pub fn settings_path(global: bool) -> Result<PathBuf, LspzError> {
    if global {
        // MCP servers live in ~/.claude.json, not ~/.claude/settings.json
        dirs::home_dir()
            .map(|home| home.join(CLAUDE_JSON))
            .ok_or_else(|| LspzError::Config("Cannot determine home directory".into()))
    } else {
        Ok(Path::new(".").join(CLAUDE_DIR).join(SETTINGS_JSON))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn atomic_write_creates_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.json");
        atomic_write(&path, b"{\"hello\": \"world\"}").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"hello\": \"world\"}");
    }

    #[test]
    fn read_settings_missing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let settings = read_settings(&path).unwrap();
        assert_eq!(settings, serde_json::json!({}));
    }

    #[test]
    fn read_settings_empty_object() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{}").unwrap();
        let settings = read_settings(&path).unwrap();
        assert_eq!(settings, serde_json::json!({}));
    }

    #[test]
    fn read_settings_invalid_json() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "not json").unwrap();
        let result = read_settings(&path);
        assert!(result.is_err());
    }

    #[test]
    fn patch_mcp_server_creates_entry() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        let binary = PathBuf::from("/usr/bin/lspz");

        let result = patch_mcp_server(&path, &binary, false).unwrap();
        assert_eq!(result, PatchResult::Patched);

        let content = fs::read_to_string(&path).unwrap();
        let settings: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            settings["mcpServers"]["lspz"]["command"],
            "/usr/bin/lspz"
        );
        assert_eq!(
            settings["mcpServers"]["lspz"]["args"],
            serde_json::json!(["mcp"])
        );
    }

    #[test]
    fn patch_mcp_server_idempotent() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        let binary = PathBuf::from("/usr/bin/lspz");

        patch_mcp_server(&path, &binary, false).unwrap();
        let result = patch_mcp_server(&path, &binary, false).unwrap();
        assert_eq!(result, PatchResult::AlreadyPresent);
    }

    #[test]
    fn patch_mcp_server_preserves_existing_entries() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"mcpServers": {"other": {"command": "/usr/bin/other"}}}"#,
        )
        .unwrap();

        let binary = PathBuf::from("/usr/bin/lspz");
        patch_mcp_server(&path, &binary, false).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let settings: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(settings["mcpServers"]["other"]["command"], "/usr/bin/other");
        assert_eq!(settings["mcpServers"]["lspz"]["command"], "/usr/bin/lspz");
    }

    #[test]
    fn patch_mcp_server_dry_run() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        let binary = PathBuf::from("/usr/bin/lspz");

        let result = patch_mcp_server(&path, &binary, true).unwrap();
        assert_eq!(result, PatchResult::WouldPatch);
        assert!(!path.exists());
    }

    #[test]
    fn remove_mcp_server_removes_entry() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"mcpServers": {"lspz": {"command": "/usr/bin/lspz", "args": ["mcp"]}, "other": {"command": "/usr/bin/other"}}}"#,
        )
        .unwrap();

        let removed = remove_mcp_server(&path, false).unwrap();
        assert!(removed);

        let content = fs::read_to_string(&path).unwrap();
        let settings: Value = serde_json::from_str(&content).unwrap();
        assert!(settings["mcpServers"].get("lspz").is_none());
        assert_eq!(settings["mcpServers"]["other"]["command"], "/usr/bin/other");
    }

    #[test]
    fn remove_mcp_server_no_entry() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"mcpServers": {}}"#).unwrap();

        let removed = remove_mcp_server(&path, false).unwrap();
        assert!(!removed);
    }

    #[test]
    fn remove_mcp_server_missing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");

        let removed = remove_mcp_server(&path, false).unwrap();
        assert!(!removed);
    }

    #[test]
    fn is_mcp_registered_true() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        let binary = PathBuf::from("/usr/bin/lspz");
        fs::write(
            &path,
            r#"{"mcpServers": {"lspz": {"command": "/usr/bin/lspz"}}}"#,
        )
        .unwrap();

        assert!(is_mcp_registered(&path, &binary));
    }

    #[test]
    fn is_mcp_registered_wrong_path() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        let binary = PathBuf::from("/usr/bin/lspz");
        fs::write(
            &path,
            r#"{"mcpServers": {"lspz": {"command": "/other/path/lspz"}}}"#,
        )
        .unwrap();

        assert!(!is_mcp_registered(&path, &binary));
    }

    #[test]
    fn is_mcp_registered_no_entry() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        let binary = PathBuf::from("/usr/bin/lspz");
        fs::write(&path, r#"{"mcpServers": {}}"#).unwrap();

        assert!(!is_mcp_registered(&path, &binary));
    }

    #[test]
    fn backup_created_on_patch() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"existing": true}"#).unwrap();

        let binary = PathBuf::from("/usr/bin/lspz");
        patch_mcp_server(&path, &binary, false).unwrap();

        let backup = path.with_extension("json.bak");
        assert!(backup.exists());
        let backup_content = fs::read_to_string(&backup).unwrap();
        let backup_settings: Value = serde_json::from_str(&backup_content).unwrap();
        assert_eq!(backup_settings["existing"], true);
    }
}
