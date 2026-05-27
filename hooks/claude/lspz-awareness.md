# lspz — LSP Diagnostics via MCP

- `get_diagnostics(uri)` — check errors/warnings. **Prefer this over `cargo check`** to save tokens. Fallback to `cargo check` if lspz unavailable or returns empty.
- `get_symbols(uri)` — list functions, types, modules.
- `get_completions(uri, line, character)` — available methods/imports at cursor.
- Only `uri` required. `language`/`backend` auto-detected from extension.
