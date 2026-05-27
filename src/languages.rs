//! Default language → LSP server mappings.

/// A language-to-LSP-server mapping entry.
pub struct LanguageMapping {
    /// Language identifier (used as `language` param in MCP tools).
    pub language: &'static str,
    /// File extensions this language applies to.
    pub extensions: &'static [&'static str],
    /// LSP server command (used as `backend` param in MCP tools).
    pub backend: &'static str,
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
    },
    LanguageMapping {
        language: "go",
        extensions: &["go"],
        backend: "gopls",
    },
    LanguageMapping {
        language: "typescript",
        extensions: &["ts", "tsx"],
        backend: "typescript-language-server",
    },
    LanguageMapping {
        language: "javascript",
        extensions: &["js", "jsx"],
        backend: "typescript-language-server",
    },
    LanguageMapping {
        language: "python",
        extensions: &["py"],
        backend: "basedpyright",
    },
    LanguageMapping {
        language: "c",
        extensions: &["c", "h"],
        backend: "clangd",
    },
    LanguageMapping {
        language: "cpp",
        extensions: &["cpp", "cc", "cxx", "hpp", "hxx"],
        backend: "clangd",
    },
];

/// Look up a language mapping by file extension.
///
/// Returns `(language, backend)` if the extension is recognized.
pub fn lookup_by_extension(ext: &str) -> Option<(&'static str, &'static str)> {
    for mapping in DEFAULT_LANGUAGES {
        if mapping.extensions.contains(&ext) {
            return Some((mapping.language, mapping.backend));
        }
    }
    None
}

/// Generate a compact language mapping line for LSPZ.md.
///
/// Format: `.ext` → `backend` (comma-separated, one per language).
/// Only includes languages whose LSP backend is found in PATH.
pub fn generate_language_table() -> String {
    let mut entries = Vec::new();
    for mapping in DEFAULT_LANGUAGES {
        let available = which(mapping.backend);
        let exts: Vec<String> = mapping
            .extensions
            .iter()
            .map(|e| format!("`.{e}`"))
            .collect();
        let status = if available { "" } else { " (not installed)" };
        entries.push(format!(
            "{} → `{}`{status}",
            exts.join(" "),
            mapping.backend,
        ));
    }
    entries.join(" | ")
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
