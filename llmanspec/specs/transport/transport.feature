# language: zh-CN
# capability: transport
# purpose: 传输层抽象，定义所有传输层必须实现的 I/O trait，支持 stdio、TCP、WebSocket 等协议。
# scope: src/transport/, tests/

功能: transport

  @req:r23
  规则: Transport trait
    系统 MUST 定义 Transport trait 包含 receive 和 send 方法。
    场景: stdio-transport
      假如 StdioTransport 实例
      当 调用 receive()
      那么 返回 Content-Length 分帧的消息
    场景: tcp-transport
      假如 TcpTransport 实例
      当 调用 receive()
      那么 返回 Content-Length 分帧的消息
    场景: ws-transport
      假如 WsTransport 实例
      当 调用 receive()
      那么 返回 Content-Length 分帧的消息

  @req:r33
  规则: Content-Length 分帧
    所有传输层 MUST 处理 Content-Length 分帧协议。
    场景: content-length-raw
      假如 原始 LSP 消息
      当 发送前
      那么 自动添加 Content-Length 头
    场景: content-length-parse
      假如 带 Content-Length 的消息
      当 接收时
      那么 自动解析并提取消息体

  @req:r42
  规则: 多协议支持
    系统 MUST 支持 StdioTransport、TcpTransport、WsTransport 三种传输协议。
    场景: multiple-protocols
      假如 配置使用不同传输协议
      当 Proxy 创建
      那么 成功创建对应的 Transport 实例

  @req:r51
  规则: 进程退出检测
    Transport trait MUST 提供 try_wait 方法检测底层进程退出状态。
    场景: process-exit
      假如 子进程已退出
      当 调用 try_wait()
      那么 返回 Some(ExitStatus)

  @req:r58
  规则: Content-Length 上限
    所有 Content-Length 分帧读取路径 MUST 拒绝超过 16MiB 的 body，与 codec 层上限一致。
    场景: reject-oversized
      假如 Content-Length 声明大于 16MiB
      当 read_frame
      那么 返回协议错误且不分配完整 body 缓冲

  @req:r65
  规则: WebSocket Text 帧
    WsTransport MUST 接受 Text 帧并将其 UTF-8 字节作为 LSP 消息载荷，不得无限跳过 Text。
    场景: accept-text
      假如 对端发送 WebSocket Text LSP 帧
      当 receive
      那么 返回对应字节内容
