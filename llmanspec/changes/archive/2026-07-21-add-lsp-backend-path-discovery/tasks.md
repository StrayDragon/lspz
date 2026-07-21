# Tasks: add-lsp-backend-path-discovery

> 每个任务 ≤ 2h。涉及文件与覆盖率目标写明。

- [x] 1. 实现 `resolve_tool` 与默认布局搜索顺序
  - 文件：新建 `src/tool_path.rs`（或等价），在 `src/lib.rs` / `src/main` 模块树导出
  - 覆盖：PATH 优先、`~/.local/bin`、uv tools、cargo、go、未找到 → None
  - 单测：临时 `HOME` / env mock，目标该模块行覆盖 ≥ 80%

- [x] 2. 接入 `languages.rs` 可用性检查
  - 文件：`src/languages.rs`（替换裸 `which`）
  - 单测：精简 PATH + 仅 `~/.local/bin` 时表状态为 ok

- [x] 3. 接入 MCP / daemon 后端解析与 spawn
  - 文件：`src/transport/stdio.rs`（`rewrite_cmd_program`；MCP/daemon/`LspPool` 均经 `StdioTransport::spawn`）
  - 行为：解析到绝对路径则用其 spawn；失败语义不变
  - 单测：`tool_path::test_rewrite_cmd_program_resolves_first_token`

- [x] 4. 文档：README 安装节
  - 文件：`README.md`（必要时 `src/init/constants.rs` 一句）
  - 内容：推荐 `uv tool install`；默认布局无需 export PATH

- [x] 5. 归档前主 spec 元数据（若 archive 未自动补全）
  - 文件：`llmanspec/specs/tool-path-discovery/spec.toon`
  - `purpose` / `valid_scope`：`src/tool_path.rs`、`src/languages.rs`、`src/mcp/`、`tests/`（按实际落点调整）

- [x] 6. 门禁
  - `llman sdd validate add-lsp-backend-path-discovery --strict --no-interactive`
  - 实现完成后：`just qa`
