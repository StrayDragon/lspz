# language: en
# capability: proxy
# purpose: 代理核心，管理 LSP 会话生命周期、消息路由和拦截器链执行。
# scope: src/proxy.rs, tests/

Feature: proxy

  @req:r21 @human
  Scenario: 生命周期管理
    - 系统 MUST 支持完整的 Proxy 生命周期：Created→Initializing→Ready→Working→ShuttingDown→Exited。

  @req:r31 @human
  Scenario: 消息路由
    - 系统 MUST 将 Client→Server 消息透明转发，Server→Client 消息经过拦截器链处理。

  @req:r40 @human
  Scenario: LSP 握手
    - 系统 MUST 正确处理 initialize/initialized 握手流程。

  @req:r49 @human
  Scenario: 配置驱动输出格式
    - 系统 MUST 使用 Config 控制运行时行为，包括 OutputFormat：passthrough 原样转发，toon 对通知与响应均输出 TOON 且响应保留 id，json 输出 compact JSON。

  @req:r56 @human
  Scenario: 优雅关闭
    - 系统 MUST 支持 shutdown 请求和 exit 通知的优雅关闭流程。

  @req:r63 @human
  Scenario: Cancel-safe 消息循环
    - 系统 MUST 在 select 竞争下以 cancel-safe 方式读取完整 LSP 帧，禁止中途取消导致字节丢失与帧错位。

  @req:r69 @human
  Scenario: 按 id 匹配握手与关闭响应
    - 系统 MUST 在 initialize 与 shutdown 流程中按 JSON-RPC id 匹配服务器响应，并将交错的通知转发给客户端。

  @req:r72 @human
  Scenario: Receive 超时
    - 系统 MUST 对 Proxy 侧等待服务器帧的 receive 使用有限超时（默认 30s），超时返回可观测错误而非无限阻塞。

  @req:r74 @human
  Scenario: pending_requests 回收
    - Proxy MUST 在收到 $/cancelRequest 时移除对应 pending id；MUST 按 TTL 剪枝超时未响应的 pending 条目，防止 HashMap 无限增长。

  @req:r24 @human
  Scenario: 空闲不因 receive 超时退出
    - Proxy message_loop MUST NOT 因服务器侧空闲超过 RECEIVE_TIMEOUT 而退出；超时仅用于握手或已发出请求后的等待。空闲 select! 中的 server receive MUST 无限期等待（或与客户端事件一并取消），禁止把空闲超时当成致命错误。

  @req:r21 @human
  Scenario: proxy-create
    - MUST hold: Given Config 实例; When Proxy::new(); Then 状态变为 Created.
    Given Config 实例
    When Proxy::new()
    Then 状态变为 Created

  @req:r21 @human
  Scenario: proxy-initialize
    - MUST hold: Given Created 状态的 Proxy; When 调用 initialize(); Then 状态变为 Initializing.
    Given Created 状态的 Proxy
    When 调用 initialize()
    Then 状态变为 Initializing

  @req:r31 @human
  Scenario: forward-client-msg
    - MUST hold: Given Client→Server 消息; When 消息循环; Then 透明转发给 LSP 服务器.
    Given Client→Server 消息
    When 消息循环
    Then 透明转发给 LSP 服务器

  @req:r31 @human
  Scenario: intercept-server-msg
    - MUST hold: Given Server→Client 消息; When 消息循环; Then 经过拦截器链处理后发送给客户端.
    Given Server→Client 消息
    When 消息循环
    Then 经过拦截器链处理后发送给客户端

  @req:r40 @human
  Scenario: initialize-handshake
    - MUST hold: Given LSP 客户端发送 initialize; When Proxy 处理; Then 转发给服务器并返回 capabilities.
    Given LSP 客户端发送 initialize
    When Proxy 处理
    Then 转发给服务器并返回 capabilities

  @req:r49 @human
  Scenario: passthrough-honored
    - MUST hold: Given output_format=passthrough; When 服务器发送 publishDiagnostics; Then 客户端收到未经压缩的原始 LSP JSON.
    Given output_format=passthrough
    When 服务器发送 publishDiagnostics
    Then 客户端收到未经压缩的原始 LSP JSON

  @req:r49 @human
  Scenario: toon-response-keeps-id
    - MUST hold: Given output_format=toon 且 pending completion 请求 id=7; When 服务器返回 completion 响应; Then 客户端收到带 id=7 且 result.format=toon 的 JSON-RPC 响应.
    Given output_format=toon 且 pending completion 请求 id=7
    When 服务器返回 completion 响应
    Then 客户端收到带 id=7 且 result.format=toon 的 JSON-RPC 响应

  @req:r56 @human
  Scenario: shutdown-flow
    - MUST hold: Given 调用 shutdown(); When Proxy 处理; Then 发送 shutdown 请求和 exit 通知.
    Given 调用 shutdown()
    When Proxy 处理
    Then 发送 shutdown 请求和 exit 通知

  @req:r63 @human
  Scenario: cancel-safe-loop
    - MUST hold: Given stdin 与 server 同时有不完整帧数据; When message_loop 运行 select; Then 任一侧只在完整帧到达后投递，另一侧未读字节保留在其 reader.
    Given stdin 与 server 同时有不完整帧数据
    When message_loop 运行 select
    Then 任一侧只在完整帧到达后投递，另一侧未读字节保留在其 reader

  @req:r69 @human
  Scenario: handshake-id-match
    - MUST hold: Given initialize 请求 id=1 且服务器先发 window/logMessage; When 执行握手; Then 通知先转发给客户端，随后转发 id=1 的 initialize 响应.
    Given initialize 请求 id=1 且服务器先发 window/logMessage
    When 执行握手
    Then 通知先转发给客户端，随后转发 id=1 的 initialize 响应

  @req:r72 @human
  Scenario: receive-timeout
    - MUST hold: Given 服务器不再发送数据; When 握手或消息循环等待 receive; Then 在超时后返回错误并记录日志.
    Given 服务器不再发送数据
    When 握手或消息循环等待 receive
    Then 在超时后返回错误并记录日志

  @req:r74 @human
  Scenario: cancel-prunes
    - MUST hold: Given pending 中有 id=7 的请求; When 客户端发送 $/cancelRequest id=7; Then pending_requests 不再包含 7.
    Given pending 中有 id=7 的请求
    When 客户端发送 $/cancelRequest id=7
    Then pending_requests 不再包含 7

  @req:r74 @human
  Scenario: ttl-prunes
    - MUST hold: Given pending 条目超过 TTL; When 跟踪新请求触发剪枝; Then 过期条目被移除.
    Given pending 条目超过 TTL
    When 跟踪新请求触发剪枝
    Then 过期条目被移除

  @req:r24 @human
  Scenario: idle-survives
    - MUST hold: Given Proxy Ready 且 30s 内无任何消息; When message_loop 继续运行; Then 不返回 Timeout 且进程保持 Ready.
    Given Proxy Ready 且 30s 内无任何消息
    When message_loop 继续运行
    Then 不返回 Timeout 且进程保持 Ready
