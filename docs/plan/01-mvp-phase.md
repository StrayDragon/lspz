# lspz MVP 阶段计划 (v0.1)

**阶段**: Phase 1 - MVP
**版本**: v0.1.0
**状态**: ⏳ 待开始
**预计周期**: 4-6 周

## 目标

实现 lspz 的核心功能，支持 **Library Mode** 和 **Proxy Mode** 两种产品形态。

### 核心交付物

1. **lspz-core** (v0.1.0) - 核心库 crate
2. **lspz** (v0.1.0) - CLI 二进制
3. 基础测试覆盖（rust-analyzer, gopls）
4. 使用示例和文档

---

## 架构概览

```
┌─────────────────────────────────────────────┐
│              lspz MVP (v0.1)                  │
├─────────────────────────────────────────────┤
│                                              │
│  ┌─────────────┐      ┌─────────────┐       │
│  │  lspz CLI   │      │  lspz-core  │       │
│  │  (Binary)   │      │  (Library)  │       │
│  └──────┬──────┘      └──────┬──────┘       │
│         │                    │              │
│         │              ┌─────▼─────┐        │
│         │              │   Proxy   │        │
│         │              │   Core    │        │
│         │              └─────┬─────┘        │
│         │                    │              │
│         │         ┌──────────┼──────────┐   │
│         │         │          │          │   │
│         │    ┌────▼───┐ ┌───▼───┐ ┌───▼───▼│
│         │    │Codec   │ │Inter- │ │Trans- │
│         │    │Layer   │ │ceptor│ │port  │
│         │    └────────┘ └───────┘ └───────┘│
│         │                              │     │
└─────────┼──────────────────────────────┼─────┘
          │                              │
          │          ┌──────────┐        │
          └──────────►stdio     ◄───────┘
                     │Transport│
                     └────┬────┘
                          │
                     ┌────▼─────┐
                     │  LSP     │
                     │  Server  │
                     └──────────┘
```

---

## 技术栈

### 核心依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| Rust | 2021 edition | 基础语言 |
| tokio | 1.35+ | 异步运行时 |
| serde | 1.0+ | 序列化框架 |
| serde_json | 1.0+ | JSON 支持 |
| async-trait | 0.1+ | 异步 trait |

### 评估中依赖

| 依赖 | 用途 | 决策 |
|------|------|------|
| tower-lsp | LSP 协议 | 待评估 vs 自实现 |
| tokio-util | codec 支持 | 待定 |
| tracing | 日志/追踪 | 倾向使用 |

### 开发依赖

| 依赖 | 用途 |
|------|------|
| anyhow | 错误处理（开发时） |
| rstest | 参数化测试 |
| tiktoken-rs | Token 计数验证 |

---

## 实施计划

### Week 1-2: 基础设施

**目标**: 搭建项目骨架，实现基础通信

#### 任务清单

- [ ] **项目初始化**
  - [ ] 创建 workspace 结构
  - [ ] 配置 Cargo.toml
  - [ ] 设置 CI/CD (GitHub Actions)

- [ ] **lspz-core 基础**
  - [ ] 定义公共 API 接口
  - [ ] 实现 `Config` 结构体和构建器
  - [ ] 定义 `LspzError` 错误类型
  - [ ] 设置基础日志框架

- [ ] **Codec Layer**
  - [ ] 实现 JSON-RPC 2.0 消息解析
  - [ ] 实现 LSP 消息类型定义
  - [ ] 消息序列化/反序列化测试

- [ ] **Transport Layer**
  - [ ] 定义 `Transport` trait
  - [ ] 实现 `StdioTransport`
  - [ ] 进程启动和生命周期管理

#### 验收标准

- [x] 可以编译通过 `cargo build`
- [ ] 可以启动一个子进程并通信
- [ ] JSON-RPC 消息可以正确解析

---

### Week 3-4: Proxy 核心

**目标**: 实现 LSP 代理核心逻辑

#### 任务清单

- [ ] **Proxy Core**
  - [ ] 实现 `Proxy` 结构体
  - [ ] LSP 初始化握手逻辑
  - [ ] 能力协商处理
  - [ ] 消息路由和转发

- [ ] **LSP 兼容性**
  - [ ] 实现 `initialize` 请求处理
  - [ ] 实现 `initialized` 通知处理
  - [ ] 实现透明转发逻辑
  - [ ] 错误处理和降级

- [ ] **CLI 基础**
  - [ ] 实现 `main.rs`
  - [ ] 命令行参数解析
  - [ ] 环境变量配置加载
  - [ ] 日志级别配置

#### 验收标准

- [ ] `lspz --backend rust-analyzer` 可以启动
- [ ] LSP 初始化握手成功
- [ ] 可以处理基本的 LSP 请求（如 hover）

---

### Week 5-6: 诊断压缩

**目标**: 实现诊断压缩拦截器

#### 任务清单

- [ ] **Interceptor 框架**
  - [ ] 定义 `Interceptor` trait
  - [ ] 实现拦截器链管理
  - [ ] 拦截器优先级排序
  - [ ] 错误隔离和降级

- [ ] **诊断压缩拦截器**
  - [ ] 去重合并算法
  - [ ] 字段裁剪逻辑
  - [ ] 枚举缩减实现
  - [ ] Range delta 编码

- [ ] **紧凑格式**
  - [ ] 定义紧凑格式 schema
  - [ ] 实现序列化/反序列化
  - [ ] 版本兼容性处理

- [ ] **测试**
  - [ ] 单元测试（各压缩步骤）
  - [ ] 集成测试（与 rust-analyzer）
  - [ ] Token 节省验证（目标 ≥40%）

#### 验收标准

- [ ] 诊断压缩功能正常工作
- [ ] 压缩后内容可正确还原
- [ ] Token 节省 ≥ 40%
- [ ] 压缩失败自动降级

---

### Week 7-8: 测试和文档

**目标**: 完善测试覆盖和文档

#### 任务清单

- [ ] **集成测试**
  - [ ] rust-analyzer 测试
  - [ ] gopls 测试
  - [ ] basedpyright 测试

- [ ] **示例代码**
  - [ ] 作为库使用示例
  - [ ] CLI 使用示例
  - [ ] 配置示例

- [ ] **文档**
  - [ ] README.md
  - [ ] API 文档（rustdoc）
  - [ ] 使用指南

- [ ] **发布准备**
  - [ ] 版本号标记
  - [ ] CHANGELOG.md
  - [ ] 发布到 crates.io（可选）

#### 验收标准

- [ ] 测试覆盖 ≥ 80%
- [ ] 文档完整且准确
- [ ] 可以发布 v0.1.0

---

## 模块依赖关系

```
┌─────────┐
│ main.rs │ (CLI)
└────┬────┘
     │
     ▼
┌─────────┐
│ lib.rs  │ (lspz-core 入口)
└────┬────┘
     │
     ├─────────────────────────────────┐
     │                                 │
     ▼                                 ▼
┌─────────┐                     ┌──────────┐
│ proxy.rs│                     │ config.rs│
└────┬────┘                     └──────────┘
     │
     ├───────────────────────────────────┐
   │   │   │           │           │
   ▼   ▼   ▼           ▼           ▼
codec/  trans-  inter-   error.rs   config.rs
       port   ceptors
```

---

## 扩展点预留

为了避免后期重构，当前实现必须预留以下扩展点：

### 1. 拦截器链

```rust
// 当前: 固定拦截器列表
let interceptors: Vec<Box<dyn Interceptor>> = vec![
    Box::new(DiagnosticsCompressor::new()),
];

// 未来: 支持动态注册
proxy.register_interceptor(Box::new(CustomInterceptor::new()))?;
proxy.set_interceptor_order(vec!["custom", "diagnostics"])?;
```

### 2. 传输层抽象

```rust
// 当前: 只有 stdio
let transport = StdioTransport::new()?;

// 未来: 支持 TCP/WebSocket
let transport = match config.transport_type {
    TransportType::Stdio => Box::new(StdioTransport::new()?),
    TransportType::Tcp => Box::new(TcpTransport::new(&config.tcp_addr)?),
    TransportType::WebSocket => Box::new(WebSocketTransport::new(&config.ws_url)?),
};
```

### 3. 配置热加载

```rust
// 当前: 配置固定
let config = Config::from_env()?;

// 未来: 支持热加载
let hot_reload = HotReloadConfig::new("lspz.toml")?;
hot_reload.watch(|new_config| {
    proxy.update_config(new_config);
});
```

---

## 测试策略

### 单元测试

- **Codec 层**: 消息解析、序列化测试
- **压缩算法**: 各压缩步骤的单元测试
- **配置**: 配置解析和验证测试

### 集成测试

- **LSP 服务器兼容性**: 测试多个真实 LSP 服务器
- **端到端流程**: 从初始化到诊断压缩的完整流程

### 性能测试

- **压缩率**: Token 节省 ≥ 40%
- **延迟**: 压缩增加的延迟 ≤ 10ms
- **内存**: 内存占用 ≤ 50MB

---

## 风险和缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| LSP 协议兼容性问题 | 高 | 中 | 严格遵循 LSP 3.17 规范，充分测试 |
| 压缩算法性能问题 | 中 | 低 | 基准测试，必要时优化 |
| 不同 LSP server 差异 | 高 | 高 | 测试多个 server，抽象差异 |

---

## 交付物清单

### 代码

- [ ] `lspz-core` crate (src/)
- [ ] `lspz` CLI (src/)
- [ ] 集成测试 (tests/)
- [ ] 示例代码 (examples/)

### 文档

- [ ] README.md
- [ ] API 文档 (rustdoc)
- [ ] 使用指南 (docs/guides/usage.md)
- [ ] CHANGELOG.md

### 配置

- [ ] Cargo.toml
- [ ] .github/workflows/ (CI)
- [ ] .gitignore

---

## 下一步

完成 MVP 后，进入 [Phase 2: MCP 集成](../plan/02-mcp-phase.md)（待创建）

---

## 参考文档

- [ROADMAP.md](../../ROADMAP.md) - 项目路线图
- [specs/001-tri-modal-architecture.md](../specs/001-tri-modal-architecture.md) - 三模态架构
- [docs/README.md](../README.md) - 开发者文档导航
