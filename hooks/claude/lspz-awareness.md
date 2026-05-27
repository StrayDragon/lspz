# lspz - LSP Compression Proxy

**Usage**: AI-friendly LSP diagnostics, completions, and symbols via MCP

## MCP Tools (available automatically)

| Tool | Description |
|------|-------------|
| `get_diagnostics` | Compressed LSP diagnostics for a file |
| `get_completions` | LSP completions at a cursor position |
| `get_symbols` | Document symbols (functions, classes, etc.) |

**Auto-detection**: `language` and `backend` parameters are optional.
When omitted, they are auto-detected from the file extension (e.g. `.rs` → rust/rust-analyzer, `.go` → go/gopls, `.py` → python/basedpyright).

Known mappings:
`.rs` → rust-analyzer, `.go` → gopls, `.ts`/`.tsx` → typescript-language-server, `.js`/`.jsx` → typescript-language-server, `.py` → basedpyright, `.c`/`.h` → clangd, `.cpp`/`.cc`/`.cxx`/`.hpp`/`.hxx` → clangd

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
