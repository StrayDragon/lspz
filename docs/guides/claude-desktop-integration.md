# Claude Desktop Integration

This guide explains how to configure Claude Desktop to use lspz's MCP tools for LSP-powered code analysis.

## Prerequisites

- [lspz](https://github.com/straydragon/lspz) installed (`cargo install lspz` or `cargo install --path .` from the repo root)
- LSP servers installed for your languages (e.g., `rust-analyzer`, `gopls`, `basedpyright`, `typescript-language-server`)
- [Claude Desktop](https://claude.ai/download) installed

## Configuration

Add the following to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "lspz": {
      "command": "lspz",
      "args": ["mcp"]
    }
  }
}
```

**File location:**

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Windows | `%APPDATA%\Claude\claude_desktop_config.json` |
| Linux | `~/.config/Claude/claude_desktop_config.json` |

## Tools Available

After restarting Claude Desktop, the following tools will appear in the MCP tool list:

| Tool | Description | Parameters |
|------|-------------|------------|
| `get_diagnostics` | Returns LSP diagnostics for a file (optionally compressed) | `uri`, `backend`, `language` |
| `get_completions` | Returns completions at a cursor position | `uri`, `backend`, `language`, `line`, `character` |
| `get_symbols` | Returns document symbols for a file | `uri`, `backend`, `language` |

## Usage Examples

Once configured, you can ask Claude to analyze your code:

> "Check diagnostics for file:///home/user/project/src/main.go using gopls in Go"

> "Get completions at line 42, character 10 for file:///home/user/project/app.ts using typescript-language-server in typescript"

> "List symbols in file:///home/user/project/src/lib.rs using rust-analyzer in rust"

### URI Format

All tools require `file://` URIs. Note that the path must be absolute:

- Correct: `file:///home/user/project/main.go`
- Incorrect: `/home/user/project/main.go`
- Incorrect: `file://relative/path.rs`

## Log Levels

To increase logging verbosity, pass a log level flag:

```json
{
  "mcpServers": {
    "lspz": {
      "command": "lspz",
      "args": ["mcp", "--log-level", "debug"]
    }
  }
}
```

Available levels: `trace`, `debug`, `info` (default), `warn`, `error`.

## Troubleshooting

### "backend not found"
Ensure the LSP server is installed and available in your `PATH`. Test with `which rust-analyzer` (or your server's command).

### "file not accessible"
The file path must exist and be readable by the lspz process. Claude Desktop's sandboxed process may have restricted filesystem access. Use absolute paths.

### "URI must start with file://"
Ensure you prefix the file path with `file://` as shown in the examples above.

### No tools appearing
- Verify the MCP server is running: run `lspz mcp` directly in a terminal to check for startup errors.
- Check Claude Desktop's MCP logs for connection issues.
- Restart Claude Desktop after modifying the configuration.

## References

- [lspz README](../../README.md)
- [ROADMAP.md](../../ROADMAP.md)
- [LSP Compatibility Spec](../specs/003-lsp-compatibility.md)
