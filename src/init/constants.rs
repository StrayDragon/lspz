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

/// Slim awareness content embedded from the template file.
pub const LSPZ_SLIM: &str = include_str!("../../hooks/claude/lspz-awareness.md");
