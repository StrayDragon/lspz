# language: en
# capability: agent-sdk
# purpose: Agent SDK，提供 AgentHandle 和 AgentPool API 将 LSP 能力嵌入 Rust 智能体应用。
# scope: src/agent_sdk/, tests/

Feature: agent-sdk

  @req:r1 @human
  Scenario: AgentHandle
    - 系统 MUST 提供 AgentHandle 结构体用于启动和管理单个 LSP 会话。

  @req:r2 @human
  Scenario: AgentPool
    - 系统 MUST 提供 AgentPool 用于管理多个 AgentHandle 实例。

  @req:r3 @human
  Scenario: 查询方法
    - AgentHandle MUST 支持 get_diagnostics、get_completions、get_symbols 等查询方法。

  @req:r4 @human
  Scenario: 文件同步
    - AgentHandle MUST 通过 LspSession::open_or_update_document 同步文档；get_diagnostics MUST 按 URI 过滤 publishDiagnostics；notify_change MUST 不产生与 didOpen 重复的 version。

  @req:r8 @human
  Scenario: 已打开文档不强制磁盘回读
    - AgentHandle 在文档已 open 时 MUST NOT 在查询前无磁盘内容覆盖会话缓冲；notify_change 写入的未保存内容 MUST 在后续 get_* 调用中保持，除非显式重新从磁盘打开。

  @req:r9 @human
  Scenario: file URI 百分号解码
    - AgentHandle MUST 在磁盘读取前对 file:// URI 做百分号解码。

  @req:r1 @human
  Scenario: agent-start
    - MUST hold: Given AgentHandle::builder(); When 调用 start(); Then 返回可用的 AgentHandle 实例.
    Given AgentHandle::builder()
    When 调用 start()
    Then 返回可用的 AgentHandle 实例

  @req:r1 @human
  Scenario: agent-shutdown
    - MUST hold: Given AgentHandle 实例; When 调用 shutdown(); Then 优雅关闭 LSP 会话.
    Given AgentHandle 实例
    When 调用 shutdown()
    Then 优雅关闭 LSP 会话

  @req:r2 @human
  Scenario: pool-manage
    - MUST hold: Given AgentPool 实例; When 多个请求; Then 自动管理 AgentHandle 生命周期.
    Given AgentPool 实例
    When 多个请求
    Then 自动管理 AgentHandle 生命周期

  @req:r3 @human
  Scenario: get-diagnostics
    - MUST hold: Given AgentHandle 实例; When 调用 get_diagnostics(); Then 返回压缩后的诊断.
    Given AgentHandle 实例
    When 调用 get_diagnostics()
    Then 返回压缩后的诊断

  @req:r3 @human
  Scenario: get-completions
    - MUST hold: Given AgentHandle 实例; When 调用 get_completions(); Then 返回压缩后的补全.
    Given AgentHandle 实例
    When 调用 get_completions()
    Then 返回压缩后的补全

  @req:r4 @human
  Scenario: open-or-update
    - MUST hold: Given 已打开的文件; When 再次 open_file 或 notify_change; Then 发送 didChange 且 version 严格递增.
    Given 已打开的文件
    When 再次 open_file 或 notify_change
    Then 发送 didChange 且 version 严格递增

  @req:r4 @human
  Scenario: diag-uri-filter
    - MUST hold: Given 多个文档已打开; When get_diagnostics(uri); Then 仅返回该 uri 的 publishDiagnostics.
    Given 多个文档已打开
    When get_diagnostics(uri)
    Then 仅返回该 uri 的 publishDiagnostics

  @req:r8 @human
  Scenario: keep-unsaved
    - MUST hold: Given notify_change 写入未保存缓冲; When 随后 get_diagnostics; Then 会话仍持有未保存内容而非磁盘旧版本.
    Given notify_change 写入未保存缓冲
    When 随后 get_diagnostics
    Then 会话仍持有未保存内容而非磁盘旧版本

  @req:r9 @human
  Scenario: decode-space
    - MUST hold: Given URI 含 %20; When open_file 读盘; Then 解码后路径可读.
    Given URI 含 %20
    When open_file 读盘
    Then 解码后路径可读
