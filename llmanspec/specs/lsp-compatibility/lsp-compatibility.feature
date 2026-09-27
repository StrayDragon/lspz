# language: zh-CN
# capability: lsp-compatibility
# purpose: LSP 兼容性规范，确保输入侧完全符合 LSP 3.17 规范，输出侧以 token 效率优先。
# scope: src/proxy.rs, src/transport/

功能: lsp-compatibility

  @req:r18
  规则: 输入兼容
    系统 MUST 完全符合 LSP 3.17 规范处理 Server→lspz 的输入。
    场景: lsp-317-input
      假如 LSP 3.17 服务器响应
      当 Proxy 接收
      那么 正确解析符合规范

  @req:r28
  规则: 输出自由
    系统输出（lspz→Agent）MUST 不受 LSP 标准约束，以 token 效率优先。
    场景: toon-output
      假如 诊断数据
      当 压缩后输出
      那么 使用 TOON 格式而非标准 LSP JSON

  @req:r37
  规则: 透明转发
    非目标消息 MUST 透明转发，不干扰 LSP 通信流。
    场景: forward-non-target
      假如 非诊断消息
      当 Proxy 接收
      那么 透明转发给客户端
    场景: forward-initialize
      假如 initialize 请求
      当 Proxy 接收
      那么 透明转发给服务器

  @req:r46
  规则: 能力协商
    系统 MUST 正确处理 server/client capabilities，不做篡改。
    场景: capabilities-forward
      假如 server capabilities
      当 Proxy 接收
      那么 原样转发给客户端
    场景: capabilities-no-modify
      假如 client capabilities
      当 Proxy 接收
      那么 不修改直接转发

  @req:r60
  规则: 取消安全分帧
    系统 MUST 使用可取消安全的分帧状态（FrameState）读取 Content-Length 帧，避免 select! 取消导致字节丢失与协议错位。
    场景: cancel-safe-frame
      假如 stdin 与 server 同时可读且一方被取消
      当 Proxy 继续读帧
      那么 已读字节保留在 FrameState 中且后续帧解析正确
    场景: cancel-safe-loop
      假如 stdin 与 server 同时有不完整帧数据
      当 message_loop 运行 select
      那么 任一侧只在完整帧到达后投递，另一侧未读字节保留在其 reader
