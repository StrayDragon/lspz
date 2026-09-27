# language: zh-CN
# capability: compression
# purpose: LSP 消息压缩能力，通过去重、字段裁剪、枚举缩减和 Range Delta 编码减少 token 消耗。
# scope: src/interceptors/, src/codec/

功能: compression

  @req:r16
  规则: 去重合并
    系统 MUST 将同一文件中相同 message+severity+code 的诊断合并为一个条目。
    场景: dedup-same-message
      假如 3 个相同 message 的诊断
      当 DiagnosticsCompressor 处理
      那么 输出 1 个条目且 count=3
    场景: dedup-different-message
      假如 2 个不同 message 的诊断
      当 DiagnosticsCompressor 处理
      那么 输出 2 个独立条目
    场景: dedup-with-code
      假如 30 个 unused variable 诊断且 code 相同
      当 压缩处理
      那么 合并为 1 条 + 30 个 range

  @req:r26
  规则: 字段裁剪
    系统 MUST 默认移除 source、data、codeDescription、relatedInformation 等非必要字段。
    场景: prune-default-fields
      假如 包含 source 字段的诊断
      当 压缩处理
      那么 输出不包含 source 字段

  @req:r35
  规则: 枚举缩减
    系统 MUST 将 severity 编码为单字符（E/W/I/H），tags 编码为单字符（U/D）。
    场景: severity-error
      假如 severity=1 的诊断
      当 压缩处理
      那么 输出 severity='E'
    场景: severity-warning
      假如 severity=2 的诊断
      当 压缩处理
      那么 输出 severity='W'

  @req:r44
  规则: Range Delta 编码
    系统 MUST 对多个 range 使用 delta 编码，第一个 range 存绝对值，后续存相对差值。
    场景: delta-multiple-ranges
      假如 3 个连续 range
      当 压缩处理
      那么 第一个 range 绝对值，后续 delta 编码
    场景: delta-single-range
      假如 1 个 range
      当 压缩处理
      那么 直接存绝对值不做 delta

  @req:r5
  规则: 失败开放
    任何压缩失败时系统 MUST 记录 WARN 日志并透明转发原始消息。
    场景: compression-failure
      假如 压缩器抛出异常
      当 InterceptorChain 处理
      那么 返回原始消息且日志包含 WARN
    场景: compression-fail
      假如 压缩器异常
      当 InterceptorChain 处理
      那么 返回原始消息且 LSP 通信继续

  @req:r6
  规则: 诊断 code 保留数字
    系统 MUST 将 LSP Diagnostic.code 的字符串与整数形式均写入 compact 字段 c；不得因类型为数字而丢弃。
    场景: numeric-code
      假如 诊断 code 为整数 6133
      当 DiagnosticsCompressor 或 compact::compress
      那么 输出条目的 c 为字符串 6133

  @req:r7
  规则: UTF-8 安全截断
    系统 MUST 在按字节长度截断诊断 message 时落在合法 UTF-8 字符边界上，禁止 panic 或产生非法 UTF-8。
    场景: utf8-truncate
      假如 含多字节字符且规范化后长度超过 200 字节的 message
      当 normalize_message 截断
      那么 结果为合法 UTF-8 且以省略号结尾
