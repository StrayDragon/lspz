# Tasks: fix-pool-spawn-without-global-await

- [x] 1. Add get_or_spawn on Arc<Mutex<LspPool>> without await under lock
- [x] 2. Update daemon handle_spawn + MCP callers
- [x] 3. Tests + just qa + validate --strict
