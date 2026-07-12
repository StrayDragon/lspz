# Design: fix-document-sync-unification

## SSOT

`LspSession::{open_or_update_document, close_document}` owns version map.

## Daemon wire

```json
{"id":1,"method":"lsp/sync_document","params":{
  "session_key":"...",
  "uri":"file://...",
  "language_id":"rust",
  "content":"..."
}}
```

Handler calls `session.open_or_update_document(...)`.

## AgentHandle

- `open_file` → read + `session.open_or_update_document`
- `notify_change` → same
- `notify_close` → `didClose` + `close_document`
- diagnostics wait uses URI predicate
