# Tasks: refactor-pool-per-session-locks

- [x] 1. LspPool stores Arc<Mutex<LspSession>>; get_or_spawn/get_by_key return Arc
- [x] 2. Update daemon handlers to release pool lock before LSP await
- [x] 3. Update McpServer handlers similarly
- [x] 4. Tests + just qa
