# language: zh-CN
# capability: codec
# purpose: 编解码层，实现 JSON-RPC 2.0 协议编解码和多种输出格式（TOON、compact、passthrough）。
# scope: src/codec/, tests/

功能: codec

  @req:r15
  规则: JSON-RPC 编解码
    系统 MUST 正确编解码 JSON-RPC 2.0 协议消息。
  @req:r25
  规则: TOON 格式
    系统 MUST 支持 TOON 输出：通知与响应均可编码为 TOON；响应 MUST 保留原 JSON-RPC id 并将 TOON 文本置于 result。
  @req:r34
  规则: Compact 格式
    系统 MUST 支持 compact 格式，使用缩短的字段名。
  @req:r43
  规则: Passthrough 格式
    系统 MUST 支持 passthrough：Proxy 在该模式下不得改写消息体，原样转发原始 LSP JSON。
  @req:r15
  规则: encode-request
    MUST hold: Given JSON-RPC 请求; When 编码; Then 输出符合 JSON-RPC 2.0 规范的字节.
  @req:r15
  规则: decode-response
    MUST hold: Given JSON-RPC 响应字节; When 解码; Then 输出 serde_json::Value.
  @req:r25
  规则: toon-response-shape
    MUST hold: Given 已压缩的 completion result 与请求 id; When 编码为 TOON 响应; Then JSON-RPC 含相同 id 且 result 含 format=toon 与 text.
  @req:r34
  规则: compact-diagnostics
    MUST hold: Given 诊断数据; When compact 编码; Then 输出缩短字段名的 JSON.
  @req:r43
  规则: proxy-passthrough
    MUST hold: Given Config.output_format=passthrough; When 拦截路径处理任意 ServerToClient 消息; Then 输出字节与输入 raw 帧语义一致且无压缩字段.
