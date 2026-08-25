# language: en
# capability: lsp-compatibility
# purpose: LSP 兼容性规范，确保输入侧完全符合 LSP 3.17 规范，输出侧以 token 效率优先。
# scope: src/proxy.rs, src/transport/

Feature: lsp-compatibility

  @req:r18 @human
  Scenario: 输入兼容
    - 系统 MUST 完全符合 LSP 3.17 规范处理 Server→lspz 的输入。

  @req:r28 @human
  Scenario: 输出自由
    - 系统输出（lspz→Agent）MUST 不受 LSP 标准约束，以 token 效率优先。

  @req:r37 @human
  Scenario: 透明转发
    - 非目标消息 MUST 透明转发，不干扰 LSP 通信流。

  @req:r46 @human
  Scenario: 能力协商
    - 系统 MUST 正确处理 server/client capabilities，不做篡改。

  @req:r53 @human
  Scenario: 错误隔离
    - 压缩失败 MUST 不影响 LSP 通信。

  @req:r60 @human
  Scenario: 取消安全分帧
    - 系统 MUST 使用可取消安全的分帧状态（FrameState）读取 Content-Length 帧，避免 select! 取消导致字节丢失与协议错位。

  @req:r67 @human
  Scenario: 按 id 匹配握手响应
    - 系统 MUST 在 initialize/shutdown 握手中按 JSON-RPC id 匹配响应，期间转发交错的服务器通知，禁止将下一条任意消息当作响应。

  @req:r18 @human
  Scenario: lsp-317-input
    - MUST hold: Given LSP 3.17 服务器响应; When Proxy 接收; Then 正确解析符合规范.
    Given LSP 3.17 服务器响应
    When Proxy 接收
    Then 正确解析符合规范

  @req:r28 @human
  Scenario: toon-output
    - MUST hold: Given 诊断数据; When 压缩后输出; Then 使用 TOON 格式而非标准 LSP JSON.
    Given 诊断数据
    When 压缩后输出
    Then 使用 TOON 格式而非标准 LSP JSON

  @req:r37 @human
  Scenario: forward-non-target
    - MUST hold: Given 非诊断消息; When Proxy 接收; Then 透明转发给客户端.
    Given 非诊断消息
    When Proxy 接收
    Then 透明转发给客户端

  @req:r37 @human
  Scenario: forward-initialize
    - MUST hold: Given initialize 请求; When Proxy 接收; Then 透明转发给服务器.
    Given initialize 请求
    When Proxy 接收
    Then 透明转发给服务器

  @req:r46 @human
  Scenario: capabilities-forward
    - MUST hold: Given server capabilities; When Proxy 接收; Then 原样转发给客户端.
    Given server capabilities
    When Proxy 接收
    Then 原样转发给客户端

  @req:r46 @human
  Scenario: capabilities-no-modify
    - MUST hold: Given client capabilities; When Proxy 接收; Then 不修改直接转发.
    Given client capabilities
    When Proxy 接收
    Then 不修改直接转发

  @req:r53 @human
  Scenario: compression-fail
    - MUST hold: Given 压缩器异常; When InterceptorChain 处理; Then 返回原始消息且 LSP 通信继续.
    Given 压缩器异常
    When InterceptorChain 处理
    Then 返回原始消息且 LSP 通信继续

  @req:r60 @human
  Scenario: cancel-safe-frame
    - MUST hold: Given stdin 与 server 同时可读且一方被取消; When Proxy 继续读帧; Then 已读字节保留在 FrameState 中且后续帧解析正确.
    Given stdin 与 server 同时可读且一方被取消
    When Proxy 继续读帧
    Then 已读字节保留在 FrameState 中且后续帧解析正确

  @req:r67 @human
  Scenario: handshake-by-id
    - MUST hold: Given initialize 请求 id=1 且服务器先发 window/logMessage; When Proxy 等待 initialize 响应; Then 转发通知并仅在 id=1 的响应到达后完成握手.
    Given initialize 请求 id=1 且服务器先发 window/logMessage
    When Proxy 等待 initialize 响应
    Then 转发通知并仅在 id=1 的响应到达后完成握手
