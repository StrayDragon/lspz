# Design: add-lsp-backend-path-discovery

## 背景

`src/languages.rs` 的 `which` 通过 `Command::new("which")` 查 PATH；MCP `resolve_language_backend` 返回命令名字符串，spawn 依赖当时环境 PATH。Cursor agent shell 等精简 PATH 场景下，`~/.local/bin` 与 uv tools 隔离环境不在 PATH → 误判未安装 / spawn 失败。原 tip `docs/tips/00-tool-path-discovery.md` 为 SSOT 备忘，本变更将其契约化后删除 tip。

## 决议

### 1. 搜索顺序（与 dapz 对齐，可复制）

```text
resolve_tool(name) -> Option<PathBuf>
  1. 进程 PATH 上的可执行文件（命中即返回）
  2. $HOME/.local/bin/<name>
  3. $XDG_DATA_HOME/uv/tools/*/bin/<name>（默认 ~/.local/share/uv/tools）
  4. $CARGO_HOME/bin/<name> 或 ~/.cargo/bin/<name>
  5. $GOPATH/bin/<name> 或 ~/go/bin/<name>
  6. （可选，MVP 可后置）npm/bun 全局 bin
```

- 仅检查「同名可执行文件是否存在且可执行」；不做网络、不安装
- `HOME` / `XDG_DATA_HOME` / `CARGO_HOME` / `GOPATH` 从环境读取，单测用临时目录 mock

### 2. 消费点

| 调用方 | 行为 |
|--------|------|
| `generate_language_table` / 可用性 | 用 `resolve_tool(backend).is_some()` 替代裸 `which` |
| MCP / daemon spawn | 解析到 `PathBuf` 时优先用绝对路径 spawn；解析失败保持现有错误语义 |
| 显式 `backend` 参数 | 若为绝对/相对路径且存在则直接用；否则按 name 走 `resolve_tool` |

### 3. 模块落点

- 优先：`src/tool_path.rs`（或 `src/languages/resolve.rs`）+ `languages.rs` 改调
- `src/mcp/server.rs` / pool spawn 路径接入
- 单测：`env` + 临时 `HOME` 布局，不依赖真实语言服务器

### 4. 文档

- README 安装节：推荐 `uv tool install basedpyright` 等；说明默认布局无需改 PATH
- tip 文件在 propose 阶段删除，避免双 SSOT

## 非决议（后续）

- shebang wrapper → `python -m`
- 与 dapz 抽共享 crate
- npm/bun 是否进 MVP：实现时若成本低可一并加入，否则 tasks 标可选
