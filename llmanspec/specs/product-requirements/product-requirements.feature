# language: zh-CN
# capability: product-requirements
# purpose: 产品需求规范，定义 lspz 的核心目标、用户场景和 MVP 范围。
# scope: src/, tests/

功能: product-requirements

  @req:r20
  规则: AI 友好代理
    系统 MUST 构建对 AI Coding Agent 友好的 LSP 代理层。
    场景: agent-friendly
      假如 AI Agent 使用 lspz
      当 Agent 发送 LSP 请求
      那么 返回压缩后的响应

  @req:r30
  规则: Token 压缩
    系统 MUST 对 Server→Client 方向的 LSP 消息进行 Token 敏感的压缩（诊断去重合并等以语义等价为目标；hover 等可做有意截断以节省 token）。
    场景: token-reduce
      假如 50 个诊断的 LSP 响应
      当 压缩处理
      那么 token 数量明显减少且诊断语义可还原

  @req:r39
  规则: 诊断压缩优先
    系统 MUST 优先压缩 textDocument/publishDiagnostics 诊断消息。
    场景: diagnostics-first
      假如 publishDiagnostics 消息
      当 Proxy 接收
      那么 优先进行压缩处理

  @req:r55
  规则: 三模态支持
    系统 MUST 支持库、CLI 代理、MCP 服务器（含 daemon 模式）三种使用模式。
    场景: daemon-mode
      假如 运行 lspz daemon
      当 MCP 客户端连接
      那么 复用 daemon 会话并返回压缩结果
    场景: library-mode
      假如 Cargo.toml 添加 lspz 依赖
      当 编译
      那么 成功链接 lspz 库
    场景: cli-mode
      假如 运行 lspz proxy --backend ra
      当 启动
      那么 透明代理 LSP 通信
    场景: mcp-mode
      假如 运行 lspz mcp
      当 启动
      那么 MCP 服务器监听请求

  @req:r62
  规则: LSP 兼容性
    系统 MUST 保持所有 LSP 标准接口的兼容性。
    场景: e2e-handshake
      假如 真实 LSP 服务器经 lspz proxy 启动
      当 Agent 发送 initialize 握手并订阅诊断
      那么 握手成功且通知按 LSP 3.17 语义送达

    场景: passthrough-unmodified
      假如 非目标 LSP 消息
      当 Proxy 接收
      那么 透明转发且字节不改写

    场景: response-id-preserved
      假如 目标响应被压缩
      当 压缩处理
      那么 JSON-RPC id 保持不变
