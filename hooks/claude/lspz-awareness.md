# lspz - LSP Compression Proxy

**Usage**: AI-friendly LSP diagnostics, completions, and symbols via MCP

## MCP Tools (available automatically)

| Tool | Description |
|------|-------------|
| `get_diagnostics` | Compressed LSP diagnostics for a file |
| `get_completions` | LSP completions at a cursor position |
| `get_symbols` | Document symbols (functions, classes, etc.) |

## Proxy Mode

```bash
lspz proxy -b rust-analyzer      # Transparent LSP proxy with compression
lspz proxy -b gopls               # Works with any LSP server
```

## Installation Verification

```bash
lspz --version         # Should show: lspz X.Y.Z
lspz mcp --help        # Should show MCP server help (if built with --features mcp)
```
