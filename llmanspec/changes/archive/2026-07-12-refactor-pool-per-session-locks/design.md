# Design: refactor-pool-per-session-locks

```text
Arc<Mutex<LspPool>>          // map metadata only
  sessions: HashMap<Key, Arc<Mutex<LspSession>>>

lookup: lock pool → clone Arc → unlock pool → lock session → I/O
```

Spawn still initializes under pool lock (rare); subsequent requests do not.
