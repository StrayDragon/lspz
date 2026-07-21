//! # tool_path
//!
//! Resolve LSP backend binaries from PATH and common package-manager layouts
//! without requiring the user to export PATH.
//!
//! ## Search order
//!
//! 1. Process `PATH`
//! 2. `$HOME/.local/bin/<name>`
//! 3. `$XDG_DATA_HOME/uv/tools/*/bin/<name>` (default `~/.local/share/uv/tools`)
//! 4. `$CARGO_HOME/bin/<name>` or `~/.cargo/bin/<name>`
//! 5. `$GOPATH/bin/<name>` or `~/go/bin/<name>`
//! 6. Optional: `~/.local/share/npm/bin/<name>`, `~/.bun/bin/<name>`

use std::path::{Path, PathBuf};

/// Environment inputs for [`resolve_tool_in`].
///
/// Production code uses [`ResolveEnv::from_process`]; tests pass explicit roots.
#[derive(Debug, Clone, Default)]
pub struct ResolveEnv {
    /// `PATH` value (`:`-separated on Unix).
    pub path: Option<String>,
    /// `HOME` directory.
    pub home: Option<PathBuf>,
    /// `XDG_DATA_HOME` (defaults to `$HOME/.local/share` when unset).
    pub xdg_data_home: Option<PathBuf>,
    /// `CARGO_HOME` (defaults to `$HOME/.cargo` when unset).
    pub cargo_home: Option<PathBuf>,
    /// `GOPATH` (defaults to `$HOME/go` when unset).
    pub gopath: Option<PathBuf>,
}

impl ResolveEnv {
    /// Read discovery roots from the current process environment.
    pub fn from_process() -> Self {
        Self {
            path: std::env::var("PATH").ok(),
            home: std::env::var_os("HOME").map(PathBuf::from),
            xdg_data_home: std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
            cargo_home: std::env::var_os("CARGO_HOME").map(PathBuf::from),
            gopath: std::env::var_os("GOPATH").map(PathBuf::from),
        }
    }
}

/// Resolve `name` using the process environment.
///
/// Returns the first existing executable path, or `None` if not found.
pub fn resolve_tool(name: &str) -> Option<PathBuf> {
    resolve_tool_in(&ResolveEnv::from_process(), name)
}

/// Resolve `name` against an explicit [`ResolveEnv`] (testable).
pub fn resolve_tool_in(env: &ResolveEnv, name: &str) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }

    let as_path = Path::new(name);
    if as_path.is_absolute() || name.contains(std::path::MAIN_SEPARATOR) {
        return is_executable(as_path).then(|| as_path.to_path_buf());
    }

    if let Some(path_env) = env.path.as_deref()
        && let Some(found) = lookup_in_path_env(path_env, name)
    {
        return Some(found);
    }

    let home = env.home.as_deref();

    if let Some(home) = home {
        let local_bin = home.join(".local").join("bin").join(name);
        if is_executable(&local_bin) {
            return Some(local_bin);
        }
    }

    let data_home = env
        .xdg_data_home
        .clone()
        .or_else(|| home.map(|h| h.join(".local").join("share")));
    if let Some(data_home) = data_home.as_ref() {
        if let Some(found) = lookup_uv_tools(data_home, name) {
            return Some(found);
        }
        let npm_bin = data_home.join("npm").join("bin").join(name);
        if is_executable(&npm_bin) {
            return Some(npm_bin);
        }
    }

    let cargo_bin = env
        .cargo_home
        .clone()
        .or_else(|| home.map(|h| h.join(".cargo")))
        .map(|c| c.join("bin").join(name));
    if let Some(cargo_bin) = cargo_bin.as_ref()
        && is_executable(cargo_bin)
    {
        return Some(cargo_bin.clone());
    }

    let go_bin = env
        .gopath
        .clone()
        .or_else(|| home.map(|h| h.join("go")))
        .map(|g| g.join("bin").join(name));
    if let Some(go_bin) = go_bin.as_ref()
        && is_executable(go_bin)
    {
        return Some(go_bin.clone());
    }

    if let Some(home) = home {
        let bun_bin = home.join(".bun").join("bin").join(name);
        if is_executable(&bun_bin) {
            return Some(bun_bin);
        }
    }

    None
}

/// Rewrite the program token of a shell command to an absolute path when resolvable.
///
/// Used by stdio spawn so MCP / proxy / daemon all benefit from discovery.
pub fn rewrite_cmd_program(cmd: &str) -> String {
    rewrite_cmd_program_in(&ResolveEnv::from_process(), cmd)
}

/// Like [`rewrite_cmd_program`] but with an explicit [`ResolveEnv`].
pub fn rewrite_cmd_program_in(env: &ResolveEnv, cmd: &str) -> String {
    let Ok(mut parts) = shell_words::split(cmd) else {
        return cmd.to_string();
    };
    let Some(program) = parts.first_mut() else {
        return cmd.to_string();
    };
    if let Some(resolved) = resolve_tool_in(env, program) {
        *program = resolved.to_string_lossy().into_owned();
        return shell_words::join(&parts);
    }
    cmd.to_string()
}

fn lookup_in_path_env(path_env: &str, name: &str) -> Option<PathBuf> {
    for dir in std::env::split_paths(path_env) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn lookup_uv_tools(data_home: &Path, name: &str) -> Option<PathBuf> {
    let tools_dir = data_home.join("uv").join("tools");
    let entries = std::fs::read_dir(&tools_dir).ok()?;
    for entry in entries.flatten() {
        let bin = entry.path().join("bin").join(name);
        if is_executable(&bin) {
            return Some(bin);
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn touch_exe(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"#!/bin/sh\n").unwrap();
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    fn touch_file(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"not-exe").unwrap();
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(path, perms).unwrap();
    }

    #[test]
    fn test_resolve_tool_path_wins_over_local_bin() {
        let root = tempfile::tempdir().unwrap();
        let path_dir = root.path().join("pathbin");
        let home = root.path().join("home");
        let name = "fake-ls";
        let path_exe = path_dir.join(name);
        let local_exe = home.join(".local").join("bin").join(name);
        touch_exe(&path_exe);
        touch_exe(&local_exe);

        let env = ResolveEnv {
            path: Some(path_dir.to_string_lossy().into_owned()),
            home: Some(home),
            ..Default::default()
        };
        let found = resolve_tool_in(&env, name).unwrap();
        assert_eq!(found, path_exe);
    }

    #[test]
    fn test_resolve_tool_local_bin_when_path_lean() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let empty_path = root.path().join("empty");
        fs::create_dir_all(&empty_path).unwrap();
        let name = "basedpyright-langserver";
        let local_exe = home.join(".local").join("bin").join(name);
        touch_exe(&local_exe);

        let env = ResolveEnv {
            path: Some(empty_path.to_string_lossy().into_owned()),
            home: Some(home),
            ..Default::default()
        };
        assert_eq!(resolve_tool_in(&env, name).unwrap(), local_exe);
    }

    #[test]
    fn test_resolve_tool_uv_tools_layout() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let data = home.join(".local").join("share");
        let empty_path = root.path().join("empty");
        fs::create_dir_all(&empty_path).unwrap();
        let name = "basedpyright-langserver";
        let uv_exe = data
            .join("uv")
            .join("tools")
            .join("basedpyright")
            .join("bin")
            .join(name);
        touch_exe(&uv_exe);

        let env = ResolveEnv {
            path: Some(empty_path.to_string_lossy().into_owned()),
            home: Some(home),
            xdg_data_home: Some(data),
            ..Default::default()
        };
        assert_eq!(resolve_tool_in(&env, name).unwrap(), uv_exe);
    }

    #[test]
    fn test_resolve_tool_cargo_bin() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let empty_path = root.path().join("empty");
        fs::create_dir_all(&empty_path).unwrap();
        let name = "rust-analyzer";
        let cargo_exe = home.join(".cargo").join("bin").join(name);
        touch_exe(&cargo_exe);

        let env = ResolveEnv {
            path: Some(empty_path.to_string_lossy().into_owned()),
            home: Some(home),
            ..Default::default()
        };
        assert_eq!(resolve_tool_in(&env, name).unwrap(), cargo_exe);
    }

    #[test]
    fn test_resolve_tool_go_bin() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let empty_path = root.path().join("empty");
        fs::create_dir_all(&empty_path).unwrap();
        let name = "gopls";
        let go_exe = home.join("go").join("bin").join(name);
        touch_exe(&go_exe);

        let env = ResolveEnv {
            path: Some(empty_path.to_string_lossy().into_owned()),
            home: Some(home),
            ..Default::default()
        };
        assert_eq!(resolve_tool_in(&env, name).unwrap(), go_exe);
    }

    #[test]
    fn test_resolve_tool_not_found() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let empty_path = root.path().join("empty");
        fs::create_dir_all(&empty_path).unwrap();
        fs::create_dir_all(&home).unwrap();

        let env = ResolveEnv {
            path: Some(empty_path.to_string_lossy().into_owned()),
            home: Some(home),
            ..Default::default()
        };
        assert!(resolve_tool_in(&env, "no-such-backend-xyz").is_none());
    }

    #[test]
    fn test_resolve_tool_ignores_non_executable() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let empty_path = root.path().join("empty");
        fs::create_dir_all(&empty_path).unwrap();
        let name = "not-exe";
        touch_file(&home.join(".local").join("bin").join(name));

        let env = ResolveEnv {
            path: Some(empty_path.to_string_lossy().into_owned()),
            home: Some(home),
            ..Default::default()
        };
        assert!(resolve_tool_in(&env, name).is_none());
    }

    #[test]
    fn test_rewrite_cmd_program_resolves_first_token() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let empty_path = root.path().join("empty");
        fs::create_dir_all(&empty_path).unwrap();
        let name = "tsserver-fake";
        let local_exe = home.join(".local").join("bin").join(name);
        touch_exe(&local_exe);

        let env = ResolveEnv {
            path: Some(empty_path.to_string_lossy().into_owned()),
            home: Some(home),
            ..Default::default()
        };
        let rewritten = rewrite_cmd_program_in(&env, &format!("{name} --stdio"));
        assert!(
            rewritten.starts_with(&local_exe.to_string_lossy().into_owned()),
            "rewritten={rewritten}"
        );
        assert!(rewritten.contains("--stdio"));
    }
}
