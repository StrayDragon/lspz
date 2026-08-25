# language: en
# capability: compression
# purpose: LSP 消息压缩能力，通过去重、字段裁剪、枚举缩减和 Range Delta 编码减少 token 消耗。
# scope: src/interceptors/, src/codec/

Feature: compression

  @req:r16 @human
  Scenario: 去重合并
    - 系统 MUST 将同一文件中相同 message+severity+code 的诊断合并为一个条目。

  @req:r26 @human
  Scenario: 字段裁剪
    - 系统 MUST 默认移除 source、data、codeDescription、relatedInformation 等非必要字段。

  @req:r35 @human
  Scenario: 枚举缩减
    - 系统 MUST 将 severity 编码为单字符（E/W/I/H），tags 编码为单字符（U/D）。

  @req:r44 @human
  Scenario: Range Delta 编码
    - 系统 MUST 对多个 range 使用 delta 编码，第一个 range 存绝对值，后续存相对差值。

  @req:r5 @human
  Scenario: 失败开放
    - 任何压缩失败时系统 MUST 记录 WARN 日志并透明转发原始消息。

  @req:r6 @human
  Scenario: 诊断 code 保留数字
    - 系统 MUST 将 LSP Diagnostic.code 的字符串与整数形式均写入 compact 字段 c；不得因类型为数字而丢弃。

  @req:r7 @human
  Scenario: UTF-8 安全截断
    - 系统 MUST 在按字节长度截断诊断 message 时落在合法 UTF-8 字符边界上，禁止 panic 或产生非法 UTF-8。

  @req:r16 @human
  Scenario: dedup-same-message
    - MUST hold: Given 3 个相同 message 的诊断; When DiagnosticsCompressor 处理; Then 输出 1 个条目且 count=3.
    Given 3 个相同 message 的诊断
    When DiagnosticsCompressor 处理
    Then 输出 1 个条目且 count=3

  @req:r16 @human
  Scenario: dedup-different-message
    - MUST hold: Given 2 个不同 message 的诊断; When DiagnosticsCompressor 处理; Then 输出 2 个独立条目.
    Given 2 个不同 message 的诊断
    When DiagnosticsCompressor 处理
    Then 输出 2 个独立条目

  @req:r26 @human
  Scenario: prune-default-fields
    - MUST hold: Given 包含 source 字段的诊断; When 压缩处理; Then 输出不包含 source 字段.
    Given 包含 source 字段的诊断
    When 压缩处理
    Then 输出不包含 source 字段

  @req:r35 @human
  Scenario: severity-error
    - MUST hold: Given severity=1 的诊断; When 压缩处理; Then 输出 severity='E'.
    Given severity=1 的诊断
    When 压缩处理
    Then 输出 severity='E'

  @req:r35 @human
  Scenario: severity-warning
    - MUST hold: Given severity=2 的诊断; When 压缩处理; Then 输出 severity='W'.
    Given severity=2 的诊断
    When 压缩处理
    Then 输出 severity='W'

  @req:r44 @human
  Scenario: delta-multiple-ranges
    - MUST hold: Given 3 个连续 range; When 压缩处理; Then 第一个 range 绝对值，后续 delta 编码.
    Given 3 个连续 range
    When 压缩处理
    Then 第一个 range 绝对值，后续 delta 编码

  @req:r44 @human
  Scenario: delta-single-range
    - MUST hold: Given 1 个 range; When 压缩处理; Then 直接存绝对值不做 delta.
    Given 1 个 range
    When 压缩处理
    Then 直接存绝对值不做 delta

  @req:r5 @human
  Scenario: compression-failure
    - MUST hold: Given 压缩器抛出异常; When InterceptorChain 处理; Then 返回原始消息且日志包含 WARN.
    Given 压缩器抛出异常
    When InterceptorChain 处理
    Then 返回原始消息且日志包含 WARN

  @req:r6 @human
  Scenario: numeric-code
    - MUST hold: Given 诊断 code 为整数 6133; When DiagnosticsCompressor 或 compact::compress; Then 输出条目的 c 为字符串 6133.
    Given 诊断 code 为整数 6133
    When DiagnosticsCompressor 或 compact::compress
    Then 输出条目的 c 为字符串 6133

  @req:r7 @human
  Scenario: utf8-truncate
    - MUST hold: Given 含多字节字符且规范化后长度超过 200 字节的 message; When normalize_message 截断; Then 结果为合法 UTF-8 且以省略号结尾.
    Given 含多字节字符且规范化后长度超过 200 字节的 message
    When normalize_message 截断
    Then 结果为合法 UTF-8 且以省略号结尾
