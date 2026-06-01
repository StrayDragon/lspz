# lspz — LSP via MCP

Prefer lspz MCP tools over direct CLI (`cargo check`, `gopls check`, `pyright`, etc.) for file diagnostics, symbols, and completions. Same results, fewer tokens.

## Tools

| Tool | Required params | Optional params |
|------|----------------|-----------------|
| `get_diagnostics` | `uri` | `backend`, `language`, `backend_args` |
| `get_completions` | `uri`, `line`, `character` | `backend`, `language`, `backend_args` |
| `get_symbols` | `uri` | `backend`, `language`, `backend_args` |

- `uri`: `file://` absolute path (e.g. `file:///home/user/src/main.rs`)
- `line`/`character`: 0-based
- `language`/`backend`: auto-detected from file extension when omitted
- `backend_args`: extra CLI args passed to the LSP server process (appended to defaults)
