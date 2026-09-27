# language: zh-CN
# capability: codec
# purpose: 编解码层，实现 JSON-RPC 2.0 协议编解码和多种输出格式（TOON、compact、passthrough）。
# scope: src/codec/, tests/

功能: codec

  @req:r15
  规则: JSON-RPC 编解码
    系统 MUST 正确编解码 JSON-RPC 2.0 协议消息。
    场景: encode-request
      假如 JSON-RPC 请求
      当 编码
      那么 输出符合 JSON-RPC 2.0 规范的字节
    场景: decode-response
      假如 JSON-RPC 响应字节
      当 解码
      那么 输出 serde_json::Value

  @req:r25
  规则: TOON 格式
    系统 MUST 支持 TOON 输出：通知与响应均可编码为 TOON；响应 MUST 保留原 JSON-RPC id 并将 TOON 文本置于 result。
    场景: toon-response-shape
      假如 已压缩的 completion result 与请求 id
      当 编码为 TOON 响应
      那么 JSON-RPC 含相同 id 且 result 含 format=toon 与 text

  @req:r34
  规则: Compact 格式
    系统 MUST 支持 compact 格式，使用缩短的字段名。
    场景: compact-diagnostics
      假如 诊断数据
      当 compact 编码
      那么 输出缩短字段名的 JSON

  @req:r43
  规则: Passthrough 格式
    系统 MUST 支持 passthrough：Proxy 在该模式下不得改写消息体，原样转发原始 LSP JSON。
    场景: proxy-passthrough
      假如 Config.output_format=passthrough
      当 拦截路径处理任意 ServerToClient 消息
      那么 输出字节与输入 raw 帧语义一致且无压缩字段
