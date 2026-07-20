//! Path constants and embedded content for Claude Code integration.

/// Claude Code configuration directory name.
pub const CLAUDE_DIR: &str = ".claude";

/// Claude Code settings file name (project-level: hooks, permissions, env).
pub const SETTINGS_JSON: &str = "settings.json";

/// Claude Code user config file name (user-level: MCP servers, preferences).
/// MCP servers are registered here, not in settings.json.
pub const CLAUDE_JSON: &str = ".claude.json";

/// LSPZ awareness file name (written to ~/.claude/LSPZ.md).
pub const LSPZ_MD: &str = "LSPZ.md";

/// Claude Code main instruction file name.
pub const CLAUDE_MD: &str = "CLAUDE.md";

/// Reference directive to add to CLAUDE.md.
pub const LSPZ_MD_REF: &str = "@LSPZ.md";

/// MCP server key in settings.json.
pub const MCP_SERVER_KEY: &str = "lspz";

/// Environment variable to override the Claude config directory.
pub const CLAUDE_DIR_ENV: &str = "LSPZ_CLAUDE_DIR";

/// Slim awareness content for Claude Code integration.
pub const LSPZ_SLIM: &str = r#"# lspz — LSP via MCP

Prefer lspz MCP tools over direct CLI (`cargo check`, `gopls check`, `pyright`, etc.) for file diagnostics, symbols, and completions. Same results, fewer tokens.

## Tools

| Tool | Required params | Optional params |
|------|----------------|-----------------|
| `get_diagnostics` | — | `uri`, `backend`, `language`, `backend_args` |
| `get_completions` | `uri`, `line`, `character` | `backend`, `language`, `backend_args` |
| `get_symbols` | — | `uri`, `backend`, `language`, `backend_args` |

- `uri`: `file://` absolute path (e.g. `file:///home/user/src/main.rs`). **Optional** for diagnostics/symbols: omit to scan the MCP workspace (roots → cwd) and get a dense TOON overview.
- `line`/`character`: 0-based
- `language`/`backend`: auto-detected from file extension when omitted
- `backend_args`: extra CLI args passed to the LSP server process (appended to defaults)
"#;
