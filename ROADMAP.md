# lspz Project Roadmap

> **lsp** **z**ip — 对 AI Coding Agent 极其友好的 LSP 压缩代理

## 项目愿景

构建一个**三模态 LSP 压缩代理系统**，通过 Token 敏感的智能压缩，让 AI Coding Agent
用更少上下文理解更多代码问题。

### 核心价值

- **Token 节省**: 4 种 LSP 消息类型压缩，平均 40–80%
- **透明集成**: Agent 无需感知，像正常使用 LSP 一样
- **灵活部署**: 支持作为库、独立代理、MCP 服务器三种形态
- **标准兼容**: 永不破坏 LSP 协议标准（fail-open 保证）

---

## 三种产品形态

```
                    ┌─────────────────┐
                    │    lspz-core    │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│Library Mode  │    │ Proxy Mode   │    │  MCP Mode    │
│  #1 优先     │    │  #2 优先     │    │  #3 优先     │
└──────────────┘    └──────────────┘    └──────────────┘
```

| 模式 | 目标用户 | 典型场景 | 集成方式 |
|------|----------|----------|----------|
| **Library** | 自研 Agent CLI | 完全控制，零开销 | `use lspz_core::Proxy` |
| **Proxy** | Claude Code/Continue/Cody | 即插即用，透明代理 | `lspz --backend gopls` |
| **MCP** | 快速实验/多工具协同 | 融入生态，按需查询 | MCP server 配置 |

---

## 版本历史

| 版本 | 阶段 | 主要变更 |
|------|------|----------|
| **v0.11.0** *(当前)* | Workspace Symbol + Diagnostic 压缩 | 全部 8 个压缩器完成 |
| v0.10.0 | Workspace Symbol 压缩 | 复用 SymbolKind 编码 + URI 去重 |
| v0.9.0 | Location 压缩 | references/definition/impl/typeDef |
| v0.8.0 | Response Capping | 截断 + 压缩正交叠加，Proxy 响应拦截 |
| v0.7.0 | TOON 输出格式 | Token-Oriented Object Notation |
| v0.6.0 | 4 个压缩器全部完成 | DocumentSymbol 压缩 |
| v0.5.0 | Hover 压缩 | Markdown 紧凑 + Hover 字段压缩 |
| v0.4.0 | 补全压缩 | CompletionItemKind 编码 + doc 去重 |
| v0.3.0 | Agent SDK | AgentHandle + AgentPool |
| v0.2.0 | MCP 集成 | 3 tools (get_diagnostics/completions/symbols) |
| v0.1.0 | MVP | Proxy + Diagnostics 压缩 |

---

## 已完成 (Phase 0–6)

### Phase 0: 基础设施 ✅
- [x] 项目初始化
- [x] CI/CD 配置 (GitHub Actions)
- [x] SSOT 规则建立
- [x] 开发环境搭建

### Phase 1: MVP — v0.1 ✅
- [x] JSON-RPC codec + Transport
- [x] Proxy 核心 + CLI
- [x] Interceptor 链 + Diagnostics 压缩
- [x] 测试 + 文档 + 发布

### Phase 2: MCP 集成 — v0.2 ✅
- [x] lspz-mcp crate
- [x] get_diagnostics / get_completions / get_symbols tools
- [x] MCP 配置指南

### Phase 3: Agent SDK — v0.3 ✅
- [x] lspz-agent-sdk crate (AgentHandle + AgentPool)
- [x] Agent 集成模板

### Phase 4: Completion 压缩 — v0.4 ✅
- [x] CompletionItemKind 枚举缩减 (1-25 → 单 char)
- [x] 字段裁剪 + doc 去重
- [x] E2E 测试框架 (LspTestHarness)

### Phase 5: Hover 压缩 — v0.5 ✅
- [x] Markdown 空白行折叠 + code fence 缩短
- [x] MarkupKind 缩减 (markdown → "m", plaintext → "p")
- [x] Hover 字段压缩

### Phase 6: DocumentSymbol 压缩 — v0.5 ✅
- [x] DocumentSymbol (分层) + SymbolInformation (扁平) 双格式支持
- [x] SymbolKind 1-26 单字符编码
- [x] 递归 children 压缩

---

---

## Phase 7: TOON 输出格式 — v0.7 ✅ 已完成

> **核心洞察**: lspz 的消费端 100% 是 LLM，不是标准 LSP Client。
> Compact JSON 使用缩写字段名（`m`, `s`, `r`）节省 token，但可能让 LLM 困惑。
> TOON 格式在保持 token 效率的同时使用完整自解释字段名，是 LLM 消费的最佳选择。

### 目标

引入 [TOON (Token-Oriented Object Notation)](docs/specs/005-toon-format.md) 作为第二输出格式，
在 token 效率和 LLM 可读性之间取得平衡。

### 任务

```
[P7-A] lspz-core: codec/toon.rs 新模块 ✅ 已完成
  - diagnostics_to_toon: 诊断 TOON 格式化
  - completions_to_toon: 补全 TOON 格式化
  - hover_to_toon: Hover TOON 格式化
  - symbols_to_toon: 符号 TOON 格式化

[P7-B] Proxy 支持 --output 参数 ✅ 已完成
  - --output toon（默认，当前 TOON 格式）
  - --output json（紧凑 JSON，兼容模式）
  - --output toon (新 TOON 格式)
  - --output passthrough (标准 LSP JSON, 透明转发)

[P7-C] Agent SDK TOON API ⏸️ 搁置
  - get_diagnostics_toon() 等方法
  - to_toon() / from_toon() 格式转换
  - 决策: Proxy 层输出 TOON 已满足需求，Agent SDK 直出 TOON 无实际用例

[P7-D] Token 节省验证 ✅ 已完成
  - cargo bench 更新（新增 TOON vs Compact JSON vs 标准 LSP 对比列）
  - 发布 TOON vs Compact JSON vs 标准 LSP 对比数据
```

### Token 节省目标

| 格式 | 单条诊断 | 30条去重诊断 | 4个补全项 |
|------|---------|-------------|----------|
| Compact JSON | 51t (基准) | 51t (基准) | ~250t (基准) |
| TOON | **38t (−25%)** | **38t (−25%)** | **~120t (−52%)** |

---

## Phase 8: Response Capping — v0.8 ✅ 已完成

> **核心洞察**: LLM 不需要 LSP 返回的**全部**条目。比如 200 个 diagnostics 中，前 20 个就足以反映问题全貌。
> 在压缩**之前**截断，比压缩本身更省 token。

### 目标

在 Interceptor 链中增加可选的 response 条目上限功能，对大返回（diagnostics、completions、symbols）
在压缩前进行截断。

### 任务

```
[P8-A] Config 扩展  ✅ 已完成
  - CappingConfig 结构体 + ConfigBuilder.capping() 方法
  - --max-diags / --max-completions / --max-symbols CLI flags
  - 环境变量 LSPZ_MAX_DIAGS / LSPZ_MAX_COMPLETIONS / LSPZ_MAX_SYMBOLS

[P8-B] 新增 CappingInterceptor  ✅ 已完成
  - crates/lspz-core/src/interceptors/capping.rs
  - 位于 Interceptor 链最前端（优先于压缩器执行）
  - 支持 diagnostics / completions / symbols 三种类型截断
  - 10 个单元测试全部通过
  - Fail-open 保障

[P8-C] Proxy 响应拦截增强  ✅ 已完成
  - proxy.rs 新增 pending_requests 跟踪机制
  - 支持将 server→client 的 response 消息送入 InterceptorChain
  - CompletionCompressor / HoverCompressor / DocumentSymbolCompressor
    现在在 proxy 模式下也能正常工作

[P8-D] 集成与测试  ✅ 已完成
  - CLI 参数装配到 Config + InterceptorChain
  - 141 个测试通过 (lib + integration)
  - clippy 零警告

[P8-E] 文档  ✅ 已完成
  - ROADMAP.md Phase 8 标记完成
  - _HANDOFF.md 更新
  - CLI --help 自动更新（clap derive）
```

### Token 节省估算

| 场景 | 原始 | 截断后 | 压缩后 | 总节省 |
|------|------|--------|--------|--------|
| 200 diags → 20 | ~4000t | ~400t | ~200t | **−95%** |
| 100 completions → 20 | ~3000t | ~600t | ~300t | **−90%** |
| 50 symbols → 20 | ~600t | ~240t | ~120t | **−80%** |

> 截断和压缩是正交的：截断消除尾部噪音，压缩精简保留的信号。

---


---

## Phase 9: Location 结果压缩 — v0.9 ✅ 已完成

> **核心洞察**: `textDocument/references` 可返回数百个 `Location`，每个包含完整 URI + Range。
> URI 字符串在大型项目中可达 50+ tokens，且跨引用时**同文件 URI 重复出现**。
> URI 去重 + Range 压缩可节省 70-85%。

### 覆盖的方法

| 方法 | 响应类型 | 数据量 | AI Agent 典型场景 |
|------|----------|--------|-------------------|
| `textDocument/references` | `Location[]` | 高 — 数百条 | "找出所有引用此符号的位置" |
| `textDocument/definition` | Location / Location[] / LocationLink[] | 低 — 1~5 条 | "跳转到定义" |
| `textDocument/implementation` | Location / Location[] / LocationLink[] | 中 — 接口实现 | "找到此接口的所有实现" |
| `textDocument/typeDefinition` | Location / Location[] / LocationLink[] | 低 — 少量 | "查看类型定义" |

**4 个方法共享同一个复合响应类型 `Location`，用 1 个 LocationCompressor 覆盖所有。**

### 压缩策略

1. **URI 去重** — `uri` → 池化 ID（`u1`, `u2`, ...），同文件多条结果共享 1 个 URI
2. **Range 编码** — 复用 diagnostics delta-encoding
3. **字段缩减** — `Location`: `uri`→`u`, `range`→`r`; `LocationLink`: 同理
4. **TOON 格式** — 表格式: `uri,range`

### 任务

```
[P9-A] LocationCompressor 实现 — locations.rs, 4 个 applies_to, 11 测试 ✅ 已完成
[P9-B] TOON 输出 — locations_to_toon() ✅ 已完成
[P9-C] Config + CLI — --compress-location / -L, LSPZ_ENABLE_LOCATION_COMPRESS ✅ 已完成
[P9-D] 集成测试 + 文档 ✅ 已完成
```

### Token 节省估算

| 场景 | 原始 | 压缩后 | 节省 |
|------|------|--------|------|
| 50 references（10 个不同文件） | ~2500t | ~400t | **−84%** |
| 5 definition + LocationLink | ~300t | ~120t | **−60%** |
| 20 个接口实现 | ~1000t | ~200t | **−80%** |

---

## Phase 10: Workspace Symbol 压缩 — v0.10 ✅ 已完成

> **核心洞察**: `workspace/symbol` 空查询可返回数千个符号。DocumentSymbol 的
> SymbolKind 编码 (1-26 单字符) 完全复用。

### 压缩策略

1. **复用 `encode_symbol_kind()`** — 已实现的 1-26 单字符编码
2. **URI 去重** — 同 LocationCompressor 的 pool 模式
3. **丢弃字段**: `deprecated`、`tags`、`data`
4. **字段缩减**: `name`→`n`, `kind`→`k`, `containerName`→`c`

### 任务

```
[P10-A] WorkspaceSymbolCompressor 实现 — workspace_symbols.rs, 8 测试 ✅ 已完成
[P10-B] TOON + Compact 格式 ✅ 已完成
[P10-C] Config + CLI ✅ 已完成
[P10-D] 集成测试 + 文档 ✅ 已完成
```

### Token 节省估算

| 场景 | 原始 | 压缩后 | 节省 |
|------|------|--------|------|
| 100 workspace symbols | ~5000t | ~1500t | **−70%** |
| 500 空查询结果 | ~25000t | ~6000t | **−76%** |

---

## Phase 11: Workspace Diagnostic 压缩 — v0.11 ✅ 已完成

> **核心洞察**: LSP 3.17 拉式诊断 `workspace/diagnostic` 返回所有文件的诊断。
> `kind: 'unchanged'` 文档直接跳过，`kind: 'full'` 调用现有 DiagnosticsCompressor。

### 压缩策略

1. **`kind: 'unchanged'` 跳过** — 无诊断内容，直接透传
2. **每文档调用现有 DiagnosticsCompressor** — 复用完整 5 步管道
3. **URI 去重** — workspace 级别共享
4. **TOON 格式** — 按文件分组的诊断表

### 任务

```
[P11-A] WorkspaceDiagnosticCompressor — workspace_diagnostics.rs ✅ 已完成
[P11-B] TOON 输出 ✅ 已完成
[P11-C] Config + CLI ✅ 已完成
[P11-D] 测试 + 文档 ✅ 已完成
```

### Token 节省估算

| 场景 | 原始 | 压缩后 | 节省 |
|------|------|--------|------|
| 10 文件 x 30 条诊断 | ~15000t | ~1500t | **−90%** |
| 5 文件变更 + 20 unchanged | ~3000t | ~500t | **−83%** |

---

## 调研评估: 已排除的方法

以下 LSP 方法不适合当前路线图：

| 方法 | 排除原因 |
|------|----------|
| `textDocument/codeAction` | 交互性操作，人来触发。action item 对 LLM 有用但 token 量不大，ROI 低 |
| `textDocument/semanticTokens/full` | 响应已是 delta 压缩后的 `uinteger[]` 扁平数组 |
| `textDocument/signatureHelp` | 通常只有 1-5 个 signature，数据量极小 |
| `textDocument/inlayHint/codeLens/documentHighlight` | UI 辅助功能，AI agent 使用频率低 |

---

## 路线图总览

| Phase | 版本 | 说明 | 状态 |
|-------|------|------|------|
| 0-7 | v0.7 | 4 压缩器 + TOON + Benchmark | :white_check_mark: |
| 8 | v0.8 | Response Capping + Proxy 响应拦截 | :white_check_mark: |
| 9 | v0.9 | Location 压缩 (references/definition/impl) | :white_check_mark: |
| 10 | v0.10 | Workspace Symbol 压缩 | :white_check_mark: |
| 11 | v0.11 | Workspace Diagnostic 压缩 | :white_check_mark: 当前 |

## 剩余事项 (低优先级)



### Metrics & Tracing 增强
埋点记录压缩率 / 延迟 / 节省 token 数。不违反 fail-open 原则。

### TCP/WebSocket Transport
当前仅支持 stdio。无实际需求，除非需要远程 LSP server。

### Config 热重载
`notify` crate + `Arc<RwLock<Config>>`。但有状态一致性问题，重启即可。

### Python/TypeScript 客户端库
等待压缩格式稳定（连续 3 个 phase 无变更）。

---

## 技术栈

- **Rust**: 2024 edition
- **Tokio**: 异步运行时
- **Serde**: 序列化框架
- **Clap**: CLI 参数解析

---

## 文档导航

### 新手入门
1. [README.md](README.md) — 项目概览和快速开始
2. [docs/README.md](docs/README.md) — 开发者前导
3. [docs/specs/001-tri-modal-architecture.md](docs/specs/001-tri-modal-architecture.md) — 架构规格

### 技术规格
- [docs/specs/002-compression-format.md](docs/specs/002-compression-format.md) — 压缩格式规范
- [docs/specs/003-lsp-compatibility.md](docs/specs/003-lsp-compatibility.md) — LSP 兼容性
- [docs/specs/004-ssot-rules.md](docs/specs/004-ssot-rules.md) — SSOT 规则
- [docs/specs/005-toon-format.md](docs/specs/005-toon-format.md) — TOON 输出格式

### API 文档（自动生成）
- [docs/api/modules.gen.md](docs/api/modules.gen.md) — 模块索引
- [docs/api/config.gen.md](docs/api/config.gen.md) — 配置参考
- [docs/reference/config.gen.md](docs/reference/config.gen.md) — 配置字段说明
- [docs/specs/interceptors.gen.md](docs/specs/interceptors.gen.md) — 拦截器列表

---

## 许可证

MIT
