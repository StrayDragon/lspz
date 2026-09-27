# language: zh-CN
# capability: interceptors
# purpose: 拦截器链架构，按顺序执行拦截器处理 Server→Client 消息，支持配置驱动的启用/禁用。
# scope: src/interceptors/, tests/

功能: interceptors

  @req:r17
  规则: Interceptor trait
    系统 MUST 定义 Interceptor trait 包含 name、applies_to 和 intercept 方法。
    场景: diagnostics-compressor
      假如 DiagnosticsCompressor
      当 applies_to(textDocument/publishDiagnostics)
      那么 返回 true
    场景: hover-compressor
      假如 HoverCompressor
      当 applies_to(textDocument/hover)
      那么 返回 true

  @req:r27
  规则: InterceptorChain
    系统 MUST 实现 InterceptorChain 按顺序执行所有匹配的拦截器。
    场景: chain-execute
      假如 包含 3 个拦截器的链
      当 处理消息
      那么 按顺序执行所有拦截器

  @req:r45
  规则: 配置驱动
    系统 MUST 根据 Config 配置启用或禁用特定拦截器。
    场景: config-toggle
      假如 配置禁用某个拦截器
      当 InterceptorChain 处理
      那么 跳过该拦截器

  @req:r52
  规则: 方向过滤
    拦截器 MUST 仅处理 ServerToClient 方向的消息。
    场景: client-to-server
      假如 ClientToServer 方向消息
      当 InterceptorChain 处理
      那么 直接透传不执行拦截器

  @req:r59
  规则: Completion MarkupContent
    CompletionCompressor MUST 从 documentation 的纯字符串与 MarkupContent.value 提取文档文本；不得因 documentation 为对象而静默丢弃。
    场景: markup-doc
      假如 completion item documentation 为 MarkupContent 对象
      当 compress_completions
      那么 compact 项含 doc 字段且等于 MarkupContent.value

  @req:r66
  规则: 补全截断配置驱动
    CompletionCompressor MUST 将 max_items=0 视为不截断；默认 MUST 为 0。数量截断 MUST 由 CappingInterceptor 的 Config.capping.max_completions 控制，禁止在压缩机中硬编码 50。
    场景: unlimited-default
      假如 CompletionCompressor 默认构造且无 capping
      当 压缩含 100 项的 completion
      那么 输出保留全部 100 项

  @req:r70
  规则: Capping 读热更新配置
    CappingInterceptor MUST 在每次 intercept 时从共享 Config 读取 max_diags/max_completions/max_symbols，使热重载后的上限立即生效。
    场景: live-cap
      假如 运行中把 max_diags 从 100 改为 5
      当 下一条 publishDiagnostics
      那么 仅保留 5 条诊断
