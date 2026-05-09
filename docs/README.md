# lspz 开发者文档

> **⚠️ 重要：开始开发前请按顺序阅读以下文档**

## 必读顺序（请严格按此顺序阅读）

1. **[ROADMAP.md](../ROADMAP.md)** - 项目总览和路线图
   - 了解项目愿景、三模态架构和研究结论
   - 理解各阶段交付目标和检查点

2. **[AGENTS.md](../AGENTS.md)** - 开发规范和 SSOT 框架
   - 阅读 Rust 编码约定
   - **必须理解**: SSOT Harness 节（代码即 SSOT、架构不变量、检查点规则）

3. **[specs/001-tri-modal-architecture.md](specs/001-tri-modal-architecture.md)** - 三模态架构规格
   - 深入理解三种产品形态的设计
   - 掌握核心 Trait（Transport, Interceptor, LspMessage）和状态机
   - 阅读所有 Mermaid 流程图

4. **[plan/01-mvp-phase.md](plan/01-mvp-phase.md)** - MVP 实施计划
   - v0.1 的具体实施计划（Task A→D）
   - 技术选型（自实现 JSON-RPC，四步压缩策略优先级）

5. **[specs/002-compression-format.md](specs/002-compression-format.md)** - 压缩格式规范
   - 理解紧凑格式 Schema
   - **必须理解**: 去重合并是 #1 收益策略

6. **[specs/003-lsp-compatibility.md](specs/003-lsp-compatibility.md)** - LSP 兼容性规范
   - 理解 OpenCode 等 Agent 如何消费诊断
   - 理解 lspz 的 Fail-open 错误处理

7. **[specs/004-ssot-rules.md](specs/004-ssot-rules.md)** - 文档生成和 SSOT 规则
   - 理解代码→文档的生成流程
   - 掌握 `.gen.` 文件的使用规范

## 文档结构

```
docs/
├── README.md                    # 本文档（开发者导航）
├── plan/                        # 阶段性实施计划
│   ├── 00-prd.md               # 产品需求文档（含调研结论）
│   └── 01-mvp-phase.md         # v0.1 MVP 阶段（当前）
├── specs/                       # 技术规格文档（核心 SSOT）
│   ├── 001-tri-modal-architecture.md  # 三模态架构（含 Mermaid 图）
│   ├── 002-compression-format.md      # 压缩格式规范（含调研数据）
│   ├── 003-lsp-compatibility.md       # LSP 兼容性规范（含 Agent 分析）
│   └── 004-ssot-rules.md              # 文档生成和 SSOT 规则
├── guides/                      # 开发指南
│   ├── coding-conventions.md    # 编码约定
│   ├── testing-guide.md         # 测试指南
│   └── contributing.md          # 贡献指南
├── api/                         # 自动生成（.gen. 文件，不手改）
│   ├── modules.gen.md          # 模块索引（从 //! 注释生成）
│   └── *.gen.md                # API 文档（从 /// 注释生成）
└── reference/                   # 自动生成（.gen. 文件，不手改）
    ├── config.gen.md           # 配置参考（从 Config 生成）
    └── error-types.gen.md      # 错误类型（从 LspzError 生成）
```

## 快速导航

| 我想... | 查看文档 |
|---------|----------|
| 了解项目整体规划 | [ROADMAP.md](../ROADMAP.md) |
| 理解架构设计 | [specs/001-tri-modal-architecture.md](specs/001-tri-modal-architecture.md) |
| 开始 MVP 开发 | [plan/01-mvp-phase.md](plan/01-mvp-phase.md) |
| 查看编码规范 | [guides/coding-conventions.md](guides/coding-conventions.md) |
| 了解压缩格式 | [specs/002-compression-format.md](specs/002-compression-format.md) |
| 理解文档生成规则 | [specs/004-ssot-rules.md](specs/004-ssot-rules.md) |

## 关键设计原则

### 向后兼容性承诺
- **lspz-core 公共 API**: 在主版本号不变的情况下承诺向后兼容
- **压缩格式**: Agent 端解压库必须支持所有历史格式版本
- **LSP 兼容性**: 永远不破坏标准 LSP 协议

### 扩展点预留
为避免后期重构，当前实现必须预留以下扩展点：
1. **拦截器链**: 支持动态注册和优先级排序
2. **传输层**: trait 抽象，支持未来实现 TCP/WebSocket
3. **压缩策略**: 可配置、可插拔的压缩算法
4. **指标收集**: 预留 metrics hook 点

### 开发前检查清单
- [ ] 已阅读 ROADMAP.md 并理解整体规划
- [ ] 已阅读 specs/001-tri-modal-architecture.md 并理解三模态架构
- [ ] 已阅读当前阶段的 plan/ 文档
- [ ] 已确认当前任务不会破坏扩展点预留
- [ ] 已为新增功能添加对应的规格文档

## 版本说明

本文档随项目演进更新，当前版本对应项目 v0.1 MVP 阶段。
