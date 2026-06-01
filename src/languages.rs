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

/// Check if a command exists in PATH.
fn which(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
