# language: en
# capability: codec
# purpose: 编解码层，实现 JSON-RPC 2.0 协议编解码和多种输出格式（TOON、compact、passthrough）。
# scope: src/codec/, tests/

Feature: codec

  @req:r15 @human
  Scenario: JSON-RPC 编解码
    - 系统 MUST 正确编解码 JSON-RPC 2.0 协议消息。

  @req:r25 @human
  Scenario: TOON 格式
    - 系统 MUST 支持 TOON 输出：通知与响应均可编码为 TOON；响应 MUST 保留原 JSON-RPC id 并将 TOON 文本置于 result。

  @req:r34 @human
  Scenario: Compact 格式
    - 系统 MUST 支持 compact 格式，使用缩短的字段名。

  @req:r43 @human
  Scenario: Passthrough 格式
    - 系统 MUST 支持 passthrough：Proxy 在该模式下不得改写消息体，原样转发原始 LSP JSON。

  @req:r15 @human
  Scenario: encode-request
    - MUST hold: Given JSON-RPC 请求; When 编码; Then 输出符合 JSON-RPC 2.0 规范的字节.
    Given JSON-RPC 请求
    When 编码
    Then 输出符合 JSON-RPC 2.0 规范的字节

  @req:r15 @human
  Scenario: decode-response
    - MUST hold: Given JSON-RPC 响应字节; When 解码; Then 输出 serde_json::Value.
    Given JSON-RPC 响应字节
    When 解码
    Then 输出 serde_json::Value

  @req:r25 @human
  Scenario: toon-response-shape
    - MUST hold: Given 已压缩的 completion result 与请求 id; When 编码为 TOON 响应; Then JSON-RPC 含相同 id 且 result 含 format=toon 与 text.
    Given 已压缩的 completion result 与请求 id
    When 编码为 TOON 响应
    Then JSON-RPC 含相同 id 且 result 含 format=toon 与 text

  @req:r34 @human
  Scenario: compact-diagnostics
    - MUST hold: Given 诊断数据; When compact 编码; Then 输出缩短字段名的 JSON.
    Given 诊断数据
    When compact 编码
    Then 输出缩短字段名的 JSON

  @req:r43 @human
  Scenario: proxy-passthrough
    - MUST hold: Given Config.output_format=passthrough; When 拦截路径处理任意 ServerToClient 消息; Then 输出字节与输入 raw 帧语义一致且无压缩字段.
    Given Config.output_format=passthrough
    When 拦截路径处理任意 ServerToClient 消息
    Then 输出字节与输入 raw 帧语义一致且无压缩字段
