# Agent SDK 集成能力评估

> 目标场景：用 Rust 写一个类似 pi-coding-agent 的 coding agent，通过 lspz `agent-sdk` feature 作为 LSP 工具层集成。

## 当前状态

`agent-sdk` 提供两个入口：

- **`AgentHandle`** — 单语言 LSP session，builder 模式启动
- **`AgentPool`** — 多语言 session pool，按 language id 路由

### 已支持的查询方法（10 个）

| 方法 | LSP 方法 | 压缩 |
|------|----------|------|
| `get_diagnostics` | `textDocument/publishDiagnostics` | ✅ |
| `get_completions` | `textDocument/completion` | ✅ |
| `get_symbols` | `textDocument/documentSymbol` | ✅ |
| `get_hover` | `textDocument/hover` | ✅ |
| `get_references` | `textDocument/references` | ✅ |
| `get_definition` | `textDocument/definition` | ✅ |
| `get_implementation` | `textDocument/implementation` | ✅ |
| `get_type_definition` | `textDocument/typeDefinition` | ✅ |
| `get_workspace_symbols` | `workspace/symbol` | ✅ |
| `get_workspace_diagnostics` | `workspace/diagnostic` | ✅ |

### 设计意图（已确认）

- 定位为 **工具集成**（client 模式），不是 proxy 模式
- 返回 `String` 是有意为之：压缩后的 TOON 文本直接作为 agent 下一轮对话上下文使用
- 不需要下游再做 JSON 反序列化

## 阻塞性缺口

### 1. 文件同步 — `didChange` / `didClose` / `didSave`

当前只有 `didOpen`（`open_file` 私有方法）。agent 编辑文件后 LSP server 看到的内容是 stale 的，后续查询全部不准。

**需要**：公开 `notify_change(language, uri, content)` / `notify_close(language, uri)` 方法。

### 2. Workspace Root 配置

`LspSession::initialize` 写死 `rootUri: null`。很多 LSP server（rust-analyzer、gopls）没有 workspace root 不工作或功能降级。

**需要**：`AgentBuilder::workspace_root(path)` / `AgentPoolBuilder::workspace_root(path)`。

### 3. 通用请求逃生舱口

没有 `raw_request(method, params)` 方法。下游无法发送当前 SDK 未覆盖的 LSP 请求。

**需要**：`AgentHandle::send_raw(method, params) -> Result<String>`。

## 后续优先支持

### 4. 重构操作（高优先级）

| 方法 | LSP 方法 | 用途 |
|------|----------|------|
| `rename` | `textDocument/rename` | 重命名符号 |
| `code_action` | `textDocument/codeAction` | quick fix / organize imports |
| `formatting` | `textDocument/formatting` | 代码格式化 |

这些对 coding agent 的完整度很重要，优先支持。

### 5. 架构改进建议

| 问题 | 现状 | 建议 |
|------|------|------|
| `AgentPool` 重复实现 | 和 `AgentHandle` 几乎复制粘贴逻辑 | Pool 持有 `HashMap<String, AgentHandle>`，委托调用 |
| `&mut self` 独占 | 无法跨 tokio task 并发查询 | 内部用 `Arc<Mutex<...>>` 或 channel，对外暴露 `&self` |
| 错误恢复 | LSP server crash 后无重连 | 加 restart/reconnect 机制 |

## 不需要改的

- 返回 `String`（TOON 文本）— 符合"压缩后直接喂给 LLM 下一轮"的设计意图
- 拦截器链 — 已经统一到 `InterceptorChain`，工作正常
- 压缩/解压工具方法 `compress` / `inflate` — 已有
