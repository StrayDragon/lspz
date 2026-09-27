# language: zh-CN
# capability: proxy
# purpose: 代理核心，管理 LSP 会话生命周期、消息路由和拦截器链执行。
# scope: src/proxy.rs, tests/

功能: proxy

  @req:r21
  规则: 生命周期管理
    系统 MUST 支持完整的 Proxy 生命周期：Created→Initializing→Ready→Working→ShuttingDown→Exited。
    场景: proxy-create
      假如 Config 实例
      当 Proxy::new()
      那么 状态变为 Created
    场景: proxy-initialize
      假如 Created 状态的 Proxy
      当 调用 initialize()
      那么 状态变为 Initializing

  @req:r31
  规则: 消息路由
    系统 MUST 将 Client→Server 消息透明转发，Server→Client 消息经过拦截器链处理。
    场景: forward-client-msg
      假如 Client→Server 消息
      当 消息循环
      那么 透明转发给 LSP 服务器
    场景: intercept-server-msg
      假如 Server→Client 消息
      当 消息循环
      那么 经过拦截器链处理后发送给客户端

  @req:r40
  规则: LSP 握手
    系统 MUST 正确处理 initialize/initialized 握手流程。
    场景: initialize-handshake
      假如 LSP 客户端发送 initialize
      当 Proxy 处理
      那么 转发给服务器并返回 capabilities

  @req:r49
  规则: 配置驱动输出格式
    系统 MUST 使用 Config 控制运行时行为，包括 OutputFormat：passthrough 原样转发，toon 对通知与响应均输出 TOON 且响应保留 id，json 输出 compact JSON。
    场景: passthrough-honored
      假如 output_format=passthrough
      当 服务器发送 publishDiagnostics
      那么 客户端收到未经压缩的原始 LSP JSON
    场景: toon-response-keeps-id
      假如 output_format=toon 且 pending completion 请求 id=7
      当 服务器返回 completion 响应
      那么 客户端收到带 id=7 且 result.format=toon 的 JSON-RPC 响应

  @req:r56
  规则: 优雅关闭
    系统 MUST 支持 shutdown 请求和 exit 通知的优雅关闭流程。
    场景: shutdown-flow
      假如 调用 shutdown()
      当 Proxy 处理
      那么 发送 shutdown 请求和 exit 通知

  @req:r69
  规则: 按 id 匹配握手与关闭响应
    系统 MUST 在 initialize 与 shutdown 流程中按 JSON-RPC id 匹配服务器响应，并将交错的通知转发给客户端。
    场景: handshake-id-match
      假如 initialize 请求 id=1 且服务器先发 window/logMessage
      当 执行握手
      那么 通知先转发给客户端，随后转发 id=1 的 initialize 响应（不将下一条任意消息当作响应）

  @req:r72
  规则: Receive 超时
    系统 MUST 对 Proxy 侧等待服务器帧的 receive 使用有限超时（默认 30s），超时返回可观测错误而非无限阻塞。
    场景: receive-timeout
      假如 服务器不再发送数据
      当 握手或消息循环等待 receive
      那么 在超时后返回错误并记录日志

  @req:r74
  规则: pending_requests 回收
    Proxy MUST 在收到 $/cancelRequest 时移除对应 pending id；MUST 按 TTL 剪枝超时未响应的 pending 条目，防止 HashMap 无限增长。
    场景: cancel-prunes
      假如 pending 中有 id=7 的请求
      当 客户端发送 $/cancelRequest id=7
      那么 pending_requests 不再包含 7
    场景: ttl-prunes
      假如 pending 条目超过 TTL
      当 跟踪新请求触发剪枝
      那么 过期条目被移除

  @req:r24
  规则: 空闲不因 receive 超时退出
    Proxy message_loop MUST NOT 因服务器侧空闲超过 RECEIVE_TIMEOUT 而退出；超时仅用于握手或已发出请求后的等待。空闲 select! 中的 server receive MUST 无限期等待（或与客户端事件一并取消），禁止把空闲超时当成致命错误。
    场景: idle-survives
      假如 Proxy Ready 且 30s 内无任何消息
      当 message_loop 继续运行
      那么 不返回 Timeout 且进程保持 Ready
