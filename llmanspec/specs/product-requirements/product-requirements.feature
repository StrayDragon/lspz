# language: en
# capability: product-requirements
# purpose: 产品需求规范，定义 lspz 的核心目标、用户场景和 MVP 范围。
# scope: src/, tests/

Feature: product-requirements

  @req:r20 @human
  Scenario: AI 友好代理
    - 系统 MUST 构建对 AI Coding Agent 友好的 LSP 代理层。

  @req:r30 @human
  Scenario: Token 压缩
    - 系统 MUST 对 Server→Client 方向的 LSP 消息进行 Token 敏感的压缩（诊断去重合并等以语义等价为目标；hover 等可做有意截断以节省 token）。

  @req:r39 @human
  Scenario: 诊断压缩优先
    - 系统 MUST 优先压缩 textDocument/publishDiagnostics 诊断消息。

  @req:r48 @human
  Scenario: 去重合并
    - 系统 MUST 将同一文件中相同 message+severity+code 的诊断合并为一个条目。

  @req:r55 @human
  Scenario: 三模态支持
    - 系统 MUST 支持库、CLI 代理、MCP 服务器（含 daemon 模式）三种使用模式。

  @req:r62 @human
  Scenario: LSP 兼容性
    - 系统 MUST 保持所有 LSP 标准接口的兼容性。

  @req:r20 @human
  Scenario: agent-friendly
    - MUST hold: Given AI Agent 使用 lspz; When Agent 发送 LSP 请求; Then 返回压缩后的响应.
    Given AI Agent 使用 lspz
    When Agent 发送 LSP 请求
    Then 返回压缩后的响应

  @req:r30 @human
  Scenario: token-reduce
    - MUST hold: Given 50 个诊断的 LSP 响应; When 压缩处理; Then token 数量明显减少且诊断语义可还原.
    Given 50 个诊断的 LSP 响应
    When 压缩处理
    Then token 数量明显减少且诊断语义可还原

  @req:r39 @human
  Scenario: diagnostics-first
    - MUST hold: Given publishDiagnostics 消息; When Proxy 接收; Then 优先进行压缩处理.
    Given publishDiagnostics 消息
    When Proxy 接收
    Then 优先进行压缩处理

  @req:r48 @human
  Scenario: dedup-with-code
    - MUST hold: Given 30 个 unused variable 诊断且 code 相同; When 压缩处理; Then 合并为 1 条 + 30 个 range.
    Given 30 个 unused variable 诊断且 code 相同
    When 压缩处理
    Then 合并为 1 条 + 30 个 range

  @req:r55 @human
  Scenario: daemon-mode
    - MUST hold: Given 运行 lspz daemon; When MCP 客户端连接; Then 复用 daemon 会话并返回压缩结果.
    Given 运行 lspz daemon
    When MCP 客户端连接
    Then 复用 daemon 会话并返回压缩结果

  @req:r62 @human
  Scenario: lsp-compatible
    - MUST hold: Given 标准 LSP 服务器; When Proxy 连接; Then 正确处理所有 LSP 消息.
    Given 标准 LSP 服务器
    When Proxy 连接
    Then 正确处理所有 LSP 消息

  @req:r55 @human
  Scenario: library-mode
    - MUST hold: Given Cargo.toml 添加 lspz 依赖; When 编译; Then 成功链接 lspz 库.
    Given Cargo.toml 添加 lspz 依赖
    When 编译
    Then 成功链接 lspz 库

  @req:r55 @human
  Scenario: cli-mode
    - MUST hold: Given 运行 lspz proxy --backend ra; When 启动; Then 透明代理 LSP 通信.
    Given 运行 lspz proxy --backend ra
    When 启动
    Then 透明代理 LSP 通信

  @req:r55 @human
  Scenario: mcp-mode
    - MUST hold: Given 运行 lspz mcp; When 启动; Then MCP 服务器监听请求.
    Given 运行 lspz mcp
    When 启动
    Then MCP 服务器监听请求
