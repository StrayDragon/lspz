# language: zh-CN
# capability: interceptors
# purpose: 拦截器链架构，按顺序执行拦截器处理 Server→Client 消息，支持配置驱动的启用/禁用。
# scope: src/interceptors/, tests/

功能: interceptors

  @req:r17
  规则: Interceptor trait
    系统 MUST 定义 Interceptor trait 包含 name、applies_to 和 intercept 方法。
  @req:r27
  规则: InterceptorChain
    系统 MUST 实现 InterceptorChain 按顺序执行所有匹配的拦截器。
  @req:r36
  规则: 失败开放
    拦截器失败时系统 MUST 记录 WARN 日志并返回原始参数。
  @req:r45
  规则: 配置驱动
    系统 MUST 根据 Config 配置启用或禁用特定拦截器。
  @req:r52
  规则: 方向过滤
    拦截器 MUST 仅处理 ServerToClient 方向的消息。
  @req:r59
  规则: Completion MarkupContent
    CompletionCompressor MUST 从 documentation 的纯字符串与 MarkupContent.value 提取文档文本；不得因 documentation 为对象而静默丢弃。
  @req:r66
  规则: 补全截断配置驱动
    CompletionCompressor MUST 将 max_items=0 视为不截断；默认 MUST 为 0。数量截断 MUST 由 CappingInterceptor 的 Config.capping.max_completions 控制，禁止在压缩机中硬编码 50。
  @req:r70
  规则: Capping 读热更新配置
    CappingInterceptor MUST 在每次 intercept 时从共享 Config 读取 max_diags/max_completions/max_symbols，使热重载后的上限立即生效。
  @req:r17
  规则: diagnostics-compressor
    MUST hold: Given DiagnosticsCompressor; When applies_to(textDocument/publishDiagnostics); Then 返回 true.
  @req:r17
  规则: hover-compressor
    MUST hold: Given HoverCompressor; When applies_to(textDocument/hover); Then 返回 true.
  @req:r27
  规则: chain-execute
    MUST hold: Given 包含 3 个拦截器的链; When 处理消息; Then 按顺序执行所有拦截器.
  @req:r27
  规则: chain-skip-disabled
    MUST hold: Given 拦截器被配置禁用; When 处理消息; Then 跳过该拦截器.
  @req:r36
  规则: interceptor-fail
    MUST hold: Given 拦截器返回错误; When InterceptorChain 处理; Then 返回原始消息且日志包含 WARN.
  @req:r45
  规则: config-toggle
    MUST hold: Given 配置禁用某个拦截器; When InterceptorChain 处理; Then 跳过该拦截器.
  @req:r52
  规则: client-to-server
    MUST hold: Given ClientToServer 方向消息; When InterceptorChain 处理; Then 直接透传不执行拦截器.
  @req:r59
  规则: markup-doc
    MUST hold: Given completion item documentation 为 MarkupContent 对象; When compress_completions; Then compact 项含 doc 字段且等于 MarkupContent.value.
  @req:r66
  规则: unlimited-default
    MUST hold: Given CompletionCompressor 默认构造且无 capping; When 压缩含 100 项的 completion; Then 输出保留全部 100 项.
  @req:r70
  规则: live-cap
    MUST hold: Given 运行中把 max_diags 从 100 改为 5; When 下一条 publishDiagnostics; Then 仅保留 5 条诊断.
