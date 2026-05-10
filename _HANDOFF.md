# _HANDOFF.md — lspz v0.5.0 (Phase 6-7 completed)

> 创建: 2026-05-09
> 更新: 2026-05-11
> 状态: **v0.5.0** — 4 个压缩器 + Benchmark & Report 系统

## 已完成

| Phase | Tag | 说明 |
|-------|-----|------|
| 0 | — | 项目初始化、SSOT 规则、CI/CD、许可证 |
| 1 | `v0.1.0` | MVP: Library + Proxy 模式, JSON-RPC codec, 诊断压缩 |
| 2 | `v0.2.0` | MCP 集成: McpServer + LspPool + 3 tools |
| 3 | `v0.3.0` | Agent SDK: AgentHandle + AgentPool + 单元测试 |
| 4 | `v0.4.0` | 生产加固 & 补全压缩 |
| 5 | `v0.5.0` | HoverCompressor + DocumentSymbolCompressor |
| 6 | — | 文档清理 + 一键验证脚本 + README/ROADMAP 重写 |
| 7 | — | Benchmark & Report 系统 (Criterion + 压缩比报告) |

## 交付物

### Phase 6 — DocumentSymbolCompressor

| 文件 | 说明 |
|------|------|
| `crates/lspz-core/src/interceptors/symbols.rs` | 拦截器实现 + 7 单元测试 |
| `crates/lspz-core/src/interceptors/mod.rs` | `pub mod symbols;` |
| `crates/lspz-core/src/lib.rs` | `pub use DocumentSymbolCompressor;` |
| `crates/lspz-core/src/config.rs` | `enable_document_symbol_compress` + builder + env-var |
| `crates/lspz/src/main.rs` | `--compress-document-symbol` / `-S` flag + chain 注册 |

### Phase 7 — Benchmark & Report System

| 文件 | 说明 |
|------|------|
| `fixtures/bench/diagnostics.json` | 诊断压测数据 (3 组: small/medium/large) |
| `fixtures/bench/completions.json` | 补全压测数据 (3 组) |
| `fixtures/bench/hover.json` | Hover 压测数据 (3 组) |
| `fixtures/bench/symbols.json` | 符号压测数据 (3 组) |
| `crates/lspz-core/benches/compression.rs` | Criterion 吞吐量基准 |
| `crates/lspz-core/examples/bench-report.rs` | 压缩比报告 (bytes + tokens) |
| `scripts/run-bench.sh` | 一键运行脚本 |
| `docs/reports/latest.md` | 最新报告 |

## 最新测试统计

```
cargo test --workspace:   97 passed
cargo clippy:             零警告
cargo fmt --check:        通过
gen-check:                15/15 文件同步
```

## 当前版本号

| Crate | 版本 |
|-------|------|
| `lspz-core` | 0.1.0 |
| `lspz-mcp` | 0.2.0 |
| `lspz-agent-sdk` | 0.3.0 |
| `lspz` (CLI) | 0.2.0 |

## 关键架构不变式 (不可违反)

1. Interceptor chain 是唯一的 Server→Client 消息转换入口
2. Fail-open: 任何压缩失败 → WARN 日志 + 透明转发原始消息
3. lspz-core 不依赖任何特定传输实现
4. 所有运行时行为通过 Config 控制
5. SSOT: 代码注释是唯一真相源, .gen.md 都是生成物

## 开发命令

```bash
just qa                    # fmt + lint + test + gen-check
just gen-docs              # 从代码注释重新生成 .gen.md
just gen-check             # 检查 .gen.md 是否与代码同步
just bench                 # Criterion 吞吐量基准
just bench-report          # 压缩比报告 (快速)
just bench-all             # 全量基准 + 保存报告
cargo test --workspace     # 全量测试
git tag -l 'v*'            # 查看所有版本 tag
```

## 版本标签

```
v0.1.0 → MVP (Library + Proxy)
v0.2.0 → MCP 集成 (McpServer + LspPool + 3 tools)
v0.3.0 → Agent SDK (AgentHandle + AgentPool)
v0.4.0 → 生产加固 & 补全压缩
v0.5.0 → Hover + DocumentSymbol 压缩
```

## 剩余项 (低优先级)

参见 `_HANDOFF_PHASE5.md` 获取完整上下文。包括:
- TCP/WebSocket Transport (无外部需求)
- Metrics & Tracing 增强 (按需实施)
- Config 热重载 (低优先级)
- Python/TypeScript 解压缩客户端库 (等待格式稳定)
- proc-macro 拦截器派生 (不值得, 除非 10+ 拦截器)
