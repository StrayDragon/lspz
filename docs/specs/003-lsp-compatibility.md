# lspz LSP 兼容性规范

**版本**: v0.1.0
**LSP 版本**: 3.17
**状态**: 定稿
**最后更新**: 2026-05-09

## 概述

lspz 必须完全符合 [Language Server Protocol 3.17](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/) 规范，确保与任何标准 LSP 客户端和服务器兼容。

### 兼容性原则

1. **不破坏标准**: 永不违反 LSP 规范
2. **透明转发**: 非目标消息必须透明转发
3. **能力协商**: 正确处理 server/client capabilities
4. **错误隔离**: 压缩失败不影响 LSP 通信

---

## 现有 AI Agent 处理诊断的方式

### OpenCode 参考实现

通过对 OpenCode 代码的分析，我们看到 AI Agent 通常会自己格式化诊断：

| 步骤 | OpenCode 做法 | 说明 |
|------|-------------|------|
| 过滤 | 只保留 `severity === 1` (ERROR) | WARNING/INFO/HINT 全部丢弃 |
| 截断 | 每文件最多 `MAX_PER_FILE = 20` 条 | 超出部分用 `"... and N more"` |
| 格式化 | `"ERROR [line:col] message"` | 单行文本，丢失结构化信息 |
| 存储 | `Map<string, Diagnostic[]>` 内存映射 | 每个 LSP client 实例独立存储 |
| 输出 | 包裹在 `<diagnostics file="uri">` XML 标签中 | 添加到 edit/write 等工具的输出中 |
| 去重 | 基于 JSON.stringify({code, severity, message, source, range}) | 确保 push 和 pull 诊断不重复 |

**关键结论**: Agent 端的格式化是**有损的**——它丢弃 WARNING，截断超出部分，丢失结构化信息。lspz 可以在协议层以**无损紧凑形式**提供更多信号，而 Agent 可以根据需要选择使用多少。

### lspz 在 Agent 生态中的定位

```mermaid
flowchart LR
    subgraph LSP["LSP Layer"]
        SERVER["LSP Server"]
        LSPZ["lspz Proxy<br/>compact diagnostics"]
    end
    subgraph AGENT["AI Agent Internal"]
        CLIENT["LSP Client<br/>receives diagnostics"]
        FORMAT["Formatter<br/>(opencode: 有损压缩)"]
        LLM["LLM Prompt<br/>context"]
    end

    SERVER -->|"raw JSON (verbose)"| LSPZ
    LSPZ -->|"compact (40%+ savings)"| CLIENT
    CLIENT --> FORMAT
    FORMAT -->|"text (lossy)"| LLM

    classDef lsp fill:#4A90E2,stroke:#2E5C8A,stroke-width:2px,color:#fff
    classDef agent fill:#9013FE,stroke:#6A0DAD,stroke-width:2px,color:#fff
    class LSPZ lsp
    class CLIENT,FORMAT,LLM agent
```

---

## 支持的 LSP 消息

### 完全支持（透明转发）

| 类别 | 消息 | 说明 |
|------|------|------|
| **生命周期** | `initialize` | 客户端初始化 |
| | `initialized` | 初始化完成通知 |
| | `shutdown` | 关闭请求 |
| | `exit` | 退出通知 |
| **文档同步** | `textDocument/didOpen` | 打开文档 |
| | `textDocument/didChange` | 文档变更 |
| | `textDocument/didClose` | 关闭文档 |
| | `textDocument/didSave` | 保存文档 |
| **语言特性** | `textDocument/hover` | 悬停提示 |
| | `textDocument/completion` | 代码补全 |
| | `textDocument/definition` | 跳转定义 |
| | `textDocument/references` | 查找引用 |
| | `textDocument/documentSymbol` | 文档符号 |
| | `textDocument/codeAction` | 代码操作 |
| **工作区** | `workspace/didChangeConfiguration` | 配置变更 |
| | `workspace/executeCommand` | 执行命令 |

### 拦截处理（压缩后转发）

| 消息 | 处理方式 | 压缩策略 |
|------|----------|----------|
| `textDocument/publishDiagnostics` | 压缩后转发 | 诊断压缩（详见 [002-compression-format.md](002-compression-format.md)） |

---

## LSP 初始化流程

```mermaid
sequenceDiagram
    participant C as Client (Agent)
    participant P as lspz (Proxy)
    participant S as Server

    Note over C,S: Phase 1: 初始化
    C->>P: initialize (params)
    P->>S: initialize (params)

    S-->>P: initialize result (capabilities)
    P-->>C: initialize result (capabilities)

    C->>P: initialized notification
    P->>S: initialized notification

    Note over C,S: Phase 2: 正常工作
    C->>P: textDocument/didOpen
    P->>S: textDocument/didOpen

    S-->>P: textDocument/publishDiagnostics
    Note over P: 压缩诊断
    P-->>C: textDocument/publishDiagnostics (compressed)
```

### Initialize 参数

**Client → Server** (lspz 透明转发):

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "processId": 12345,
    "rootUri": "file:///path/to/project",
    "capabilities": {
      "textDocument": {
        "publishDiagnostics": {
          "relatedInformation": true,
          "codeDescriptionSupport": true,
          "dataSupport": true
        }
      }
    }
  }
}
```

**Server → Client** (lspz 透明转发):

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "capabilities": {
      "textDocumentSync": 1,
      "hoverProvider": true,
      "completionProvider": {
        "resolveProvider": false
      },
      "diagnosticProvider": {}
    }
  }
}
```

---

## 能力协商

### lspz 在能力协商中的角色

**lspz 不修改 capabilities**:

- lspz 透明转发 Server 的 capabilities
- lspz 不声明任何额外的能力
- lspz 不过滤 Client 的 capabilities

### 原因

保持 lspz 对 LSP 客户端和服务器完全透明：
- 客户端认为直接与 Server 通信
- 服务器认为直接与 Client 通信
- lspz 仅作为中间压缩层

---

## 诊断消息处理

### 标准 publishDiagnostics

**Server → lspz** (标准格式):

```json
{
  "jsonrpc": "2.0",
  "method": "textDocument/publishDiagnostics",
  "params": {
    "uri": "file:///path/to/file.rs",
    "diagnostics": [
      {
        "range": {
          "start": {"line": 10, "character": 5},
          "end": {"line": 10, "character": 15}
        },
        "severity": 2,
        "message": "unused variable"
      }
    ]
  }
}
```

**lspz → Client** (压缩格式):

```json
{
  "jsonrpc": "2.0",
  "method": "textDocument/publishDiagnostics",
  "params": {
    "version": 1,
    "uri": "file:///path/to/file.rs",
    "diagnostics": [
      {
        "m": "unused variable",
        "s": "W",
        "r": [[10, 5, 10, 15]]
      }
    ]
  }
}
```

### 关键约束

1. **消息结构不变**: 仍然是 JSON-RPC 2.0 通知
2. **方法名不变**: `textDocument/publishDiagnostics`
3. **URI 不变**: `params.uri` 保持原值
4. **只压缩 diagnostics**: `params.diagnostics` 内容压缩

---

## 紧凑格式兼容性

### 未感知压缩的 Client

当 Client 未感知压缩时（如标准 LSP Client 或旧版 Agent）：

1. lspz 可以通过配置禁用压缩 → 透明转发标准 LSP 消息
2. 或者：Client 收到紧凑格式后，通过 `params.version` 字段检测到非标准格式

### 感知压缩的 Agent

Agent 端可以主动声明支持压缩：

```rust
// 自研 Agent 在初始化时配置
let config = Config::builder()
    .enable_diag_compress(true)  // 启用压缩输出
    .build();
```

---

## 错误处理

### JSON-RPC 错误

lspz 正确转发和处理 JSON-RPC 错误：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32601,
    "message": "Method not found"
  }
}
```

### LSP 错误响应

lspz 透明转发 LSP 错误响应：

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "error": {
    "code": -32800,
    "message": "Request cancelled"
  }
}
```

### lspz 内部错误

**原则**: lspz 内部错误不应中断 LSP 通信

| 错误类型 | 处理方式 | 日志级别 |
|---------|----------|----------|
| 压缩失败 | 降级到透明转发 | WARN |
| 配置错误 | 记录日志，继续运行 | ERROR |
| 传输错误 | 返回 JSON-RPC 错误 | ERROR |
| 解析错误 | 返回 JSON-RPC 错误 | ERROR |

**实现**:

```rust
// 压缩失败时的降级处理 — 符合 "Fail-open" 不变量
match compress_diagnostics(diagnostics) {
    Ok(compressed) => send(compressed),
    Err(e) => {
        tracing::warn!(error = %e, "Diagnostic compression failed, falling back to transparent forward");
        send_original(diagnostics);  // 透明转发原始消息
    }
}
```

---

## 服务器兼容性测试

### 目标 LSP 服务器

| LSP Server | 语言 | 测试优先级 | 诊断字段数 | 调用方式 |
|-----------|------|-----------|-----------|----------|
| rust-analyzer | Rust | P0 | 6 (range, severity, message, code, source, tags) | `rust-analyzer` |
| basedpyright | Python | P0 | 5 (range, severity, message, code, source) | `basedpyright --language-server` |
| typescript-language-server | TypeScript | P1 | 7 (range, severity, message, code, source, relatedInformation, tags) | `typescript-language-server --stdio` |
| gopls | Go | P2 | 4 (range, severity, message, source) | `gopls` |

### 测试矩阵

| LSP Server | initialize | diagnostics | hover | completion | 字段裁剪验证 | 去重合并验证 |
|-----------|-----------|-------------|-------|-----------|-------------|-------------|
| rust-analyzer | ✅ | ✅ | ✅ | ✅ | ⏳ | ⏳ |
| gopls | ✅ | ✅ | ✅ | ✅ | ⏳ | ⏳ |
| basedpyright | ✅ | ✅ | ✅ | ✅ | ⏳ | ⏳ |
| typescript-language-server | ✅ | ✅ | ✅ | ✅ | ⏳ | ⏳ |

---

## 实现检查清单

### LSP 规范合规

- [ ] 正确处理 JSON-RPC 2.0 基础协议
- [ ] 支持请求、响应、通知三种消息类型
- [ ] 正确处理消息 id
- [ ] 正确转发 JSON-RPC 错误
- [ ] 实现完整的 initialize/initialized 握手
- [ ] 透明转发所有非诊断消息
- [ ] 正确处理 capabilities
- [ ] 压缩失败时降级到透明转发

### 测试覆盖

- [ ] rust-analyzer 集成测试通过
- [ ] gopls 集成测试通过
- [ ] at least 2 个其他 LSP 服务器测试通过
- [ ] 压缩格式可正确还原
- [ ] Token 节省 ≥ 40%

---

## 参考文档

- [LSP 3.17 规范](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)
- [JSON-RPC 2.0 规范](https://www.jsonrpc.org/specification)
- [specs/002-compression-format.md](002-compression-format.md) - 压缩格式规范
- [specs/001-tri-modal-architecture.md](001-tri-modal-architecture.md) - 三模态架构
