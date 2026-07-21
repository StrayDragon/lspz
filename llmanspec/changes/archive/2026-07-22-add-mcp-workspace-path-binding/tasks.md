# Tasks: add-mcp-workspace-path-binding

- [x] 1. 扩展 `src/mcp/workspace.rs`：锚点解析、相对路径、安全阀、会话绑定 API、单测
- [x] 2. 更新 `GetDiagnosticsInput` / `GetSymbolsInput` / `GetCompletionsInput`（path/paths/workspace + deny_unknown_fields）与 JSON Schema
- [x] 3. 新增 `set_workspace` 工具；`McpServer` / `DaemonMcpServer` 会话状态与 handlers
- [x] 4. 多目标诊断/符号走 scan TOON；不可信 fallback 报错；增强 Roots 日志
- [x] 5. `llman sdd validate add-mcp-workspace-path-binding --strict --no-interactive`
- [x] 6. `just qa`
