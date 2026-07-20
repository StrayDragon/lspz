# Tasks: add-mcp-workspace-roots

> 每个任务 ≤ 2h。主线已完成 `rmcp` 2.2 升级与 Roots 接线；下列勾选反映当前落地进度。

- [x] 1. 确认/对接 rmcp Roots + peer/client info API
  - 文件：`src/mcp/workspace.rs`、`server.rs`、`daemon_server.rs`；`Cargo.toml` `rmcp = "2"` → 2.2.0
  - 产出：`peer.list_roots()` + `peer_info` 日志；`Content` → `ContentBlock`

- [x] 2. 实现 `resolve_workspace`（Roots → cwd fallback）
  - 文件：`src/mcp/workspace.rs`（`ResolvedWorkspace`）
  - 单测：discover / format / outside-roots

- [x] 3. 工具 schema：`get_diagnostics` / `get_symbols` 的 `uri` 改为可选；空/缺省走 resolve
  - 文件：`GetDiagnosticsInput` / `GetSymbolsInput`（daemon 共用）
  - `get_completions`：仍要求 `uri` + line/character（符合 design）

- [x] 4. 有 Roots 时 `file://` 越界校验（相关 QA 沙箱切片）
  - 文件：`ensure_path_under_roots`；MCP/daemon diagnostics/symbols 路径

- [x] 5. 无 URI `get_diagnostics` / `get_symbols` 默认流：扫描/触发 LSP + max-column compact TOON 摘要
  - 文件：`workspace.rs` + handlers；上限 `MAX_SCAN_FILES=8`

- [x] 6. Cursor 类客户端场景测试（模拟 roots + peer_info）
  - 文件：`src/mcp/workspace.rs` tests
  - 覆盖：空 uri 反序列化；schema 无 required；越界拒绝；symbols/diagnostics dense 格式

- [x] 7. 文档：`///` 更新工具说明；必要时 README MCP 段一句
  - 文件：`src/mcp/mod.rs`、`README.md`、`src/init/constants.rs`（LSPZ.md 模板）

- [x] 8. 门禁
  - `llman sdd validate add-mcp-workspace-roots --strict --no-interactive`
  - `just qa`（实现完成后）
