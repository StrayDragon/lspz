# Design: fix-agent-skip-disk-reread-when-open

```text
open_file(uri):
  if session.is_open(uri): return  // keep buffer
  else: read disk → open_or_update_document
```

`notify_close` clears open state so the next query re-reads disk.
Explicit reopen API is out of scope; close+query is the reopen path.
