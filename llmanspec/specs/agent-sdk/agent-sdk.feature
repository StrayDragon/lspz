# language: zh-CN
# capability: agent-sdk
# purpose: Agent SDK，提供 AgentHandle 和 AgentPool API 将 LSP 能力嵌入 Rust 智能体应用。
# scope: src/agent_sdk/, tests/

功能: agent-sdk

  @req:r1
  规则: AgentHandle
    系统 MUST 提供 AgentHandle 结构体用于启动和管理单个 LSP 会话。
    场景: agent-start
      假如 AgentHandle::builder()
      当 调用 start()
      那么 返回可用的 AgentHandle 实例
    场景: agent-shutdown
      假如 AgentHandle 实例
      当 调用 shutdown()
      那么 优雅关闭 LSP 会话

  @req:r2
  规则: AgentPool
    系统 MUST 提供 AgentPool 用于管理多个 AgentHandle 实例。
    场景: pool-manage
      假如 AgentPool 实例
      当 多个请求
      那么 自动管理 AgentHandle 生命周期

  @req:r3
  规则: 查询方法
    AgentHandle MUST 支持 get_diagnostics、get_completions、get_symbols 等查询方法。
    场景: get-diagnostics
      假如 AgentHandle 实例
      当 调用 get_diagnostics()
      那么 返回压缩后的诊断
    场景: get-completions
      假如 AgentHandle 实例
      当 调用 get_completions()
      那么 返回压缩后的补全

  @req:r4
  规则: 文件同步
    AgentHandle MUST 通过 LspSession::open_or_update_document 同步文档；get_diagnostics MUST 按 URI 过滤 publishDiagnostics；notify_change MUST 不产生与 didOpen 重复的 version。
    场景: open-or-update
      假如 已打开的文件
      当 再次 open_file 或 notify_change
      那么 发送 didChange 且 version 严格递增
    场景: diag-uri-filter
      假如 多个文档已打开
      当 get_diagnostics(uri)
      那么 仅返回该 uri 的 publishDiagnostics

  @req:r8
  规则: 已打开文档不强制磁盘回读
    AgentHandle 在文档已 open 时 MUST NOT 在查询前无磁盘内容覆盖会话缓冲；notify_change 写入的未保存内容 MUST 在后续 get_* 调用中保持，除非显式重新从磁盘打开。
    场景: keep-unsaved
      假如 notify_change 写入未保存缓冲
      当 随后 get_diagnostics
      那么 会话仍持有未保存内容而非磁盘旧版本

  @req:r9
  规则: file URI 百分号解码
    AgentHandle MUST 在磁盘读取前对 file:// URI 做百分号解码。
    场景: decode-space
      假如 URI 含 %20
      当 open_file 读盘
      那么 解码后路径可读
