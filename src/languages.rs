//! Default language → LSP server mappings.

/// A language-to-LSP-server mapping entry.
pub struct LanguageMapping {
    /// Language identifier (used as `language` param in MCP tools).
    pub language: &'static str,
    /// File extensions this language applies to.
    pub extensions: &'static [&'static str],
    /// LSP server binary name (used for `which` checks and display).
    pub backend: &'static str,
    /// Arguments required to start the server in stdio mode (e.g. `--stdio`).
    pub spawn_args: &'static [&'static str],
    /// Default extra arguments users can override (e.g. config flags).
    pub default_args: &'static [&'static str],
}

/// Built-in default language mappings.
///
/// These are compiled into the binary and used by `lspz init` to generate
/// the language table in LSPZ.md. Users can override via config file.
pub const DEFAULT_LANGUAGES: &[LanguageMapping] = &[
    LanguageMapping {
        language: "rust",
        extensions: &["rs"],
        backend: "rust-analyzer",
        spawn_args: &[],
        default_args: &[],
    },
    LanguageMapping {
        language: "go",
        extensions: &["go"],
        backend: "gopls",
        spawn_args: &[],
        default_args: &[],
    },
    LanguageMapping {
        language: "typescript",
        extensions: &["ts", "tsx"],
        backend: "typescript-language-server",
        spawn_args: &["--stdio"],
        default_args: &[],
    },
    LanguageMapping {
        language: "javascript",
        extensions: &["js", "jsx"],
        backend: "typescript-language-server",
        spawn_args: &["--stdio"],
        default_args: &[],
    },
    LanguageMapping {
        language: "python",
        extensions: &["py"],
        backend: "basedpyright-langserver",
        spawn_args: &["--stdio"],
        default_args: &[],
    },
    LanguageMapping {
        language: "c",
        extensions: &["c", "h"],
        backend: "clangd",
        spawn_args: &[],
        default_args: &[],
    },
    LanguageMapping {
        language: "cpp",
        extensions: &["cpp", "cc", "cxx", "hpp", "hxx"],
        backend: "clangd",
        spawn_args: &[],
        default_args: &[],
    },
];

/// Look up a language mapping by file extension.
///
/// Returns `(language, spawn_command)` if the extension is recognized.
/// The `spawn_command` includes any required arguments (e.g. `--stdio`).
pub fn lookup_by_extension(ext: &str) -> Option<(&'static str, String)> {
    for mapping in DEFAULT_LANGUAGES {
        if mapping.extensions.contains(&ext) {
            let cmd = spawn_cmd(mapping);
            return Some((mapping.language, cmd));
        }
    }
    None
}

/// Look up default extra args for a file extension.
pub fn default_args_by_extension(ext: &str) -> &'static [&'static str] {
    for mapping in DEFAULT_LANGUAGES {
        if mapping.extensions.contains(&ext) {
            return mapping.default_args;
        }
    }
    &[]
}

/// Build the full spawn command string for a mapping.
fn spawn_cmd(mapping: &LanguageMapping) -> String {
    if mapping.spawn_args.is_empty() {
        mapping.backend.to_string()
    } else {
        format!("{} {}", mapping.backend, mapping.spawn_args.join(" "))
    }
}

/// Generate a language mapping table for LSPZ.md.
///
/// Format: markdown table with extensions, language, and backend.
/// Shows availability status based on PATH check.
pub fn generate_language_table() -> String {
    let mut rows = vec!["| Extensions | Language | Backend | Status |".to_string()];
    rows.push("|---|---|---|---|".to_string());
    for mapping in DEFAULT_LANGUAGES {
        let available = which(mapping.backend);
        let exts: Vec<String> = mapping.extensions.iter().map(|e| format!(".{e}")).collect();
        let status = if available { "ok" } else { "not installed" };
        rows.push(format!(
            "| {} | `{}` | `{}` | {} |",
            exts.join(", "),
            mapping.language,
            mapping.backend,
            status,
        ));
    }
    rows.join("\n")
}

/// Check if a backend binary is resolvable via PATH or default install layouts.
fn which(cmd: &str) -> bool {
    crate::tool_path::resolve_tool(cmd).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn touch_exe(path: &std::path::Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"#!/bin/sh\n").unwrap();
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    #[test]
    fn test_which_finds_local_bin_with_lean_path() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let empty = root.path().join("empty");
        fs::create_dir_all(&empty).unwrap();
        let backend = "basedpyright-langserver";
        touch_exe(&home.join(".local").join("bin").join(backend));

        let prev_home = std::env::var_os("HOME");
        let prev_path = std::env::var_os("PATH");
        // SAFETY: serialized by ENV_MUTEX; restored before unlock.
        unsafe {
            std::env::set_var("HOME", &home);
            std::env::set_var("PATH", &empty);
        }
        let ok = which(backend);
        unsafe {
            match prev_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
            match prev_path {
                Some(v) => std::env::set_var("PATH", v),
                None => std::env::remove_var("PATH"),
            }
        }
        assert!(ok, "lean PATH should still resolve ~/.local/bin backend");
    }

    #[test]
    fn test_generate_language_table_contains_status_column() {
        let table = generate_language_table();
        assert!(table.contains("| Extensions | Language | Backend | Status |"));
        assert!(table.contains("`rust`"));
    }
}
