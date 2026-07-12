# Design: fix-proxy-framing-output-timeout

## Cancel-safe framing

**问题**: `tokio::select!` 取消正在执行的 `read_frame` / `read_stdin_frame` 会丢失已从 `BufReader` 消费的字节。

**方案**:
1. 启动时 spawn stdin reader task：循环 `read_frame`，完整帧经 `mpsc` 发送
2. spawn server reader task：循环 `transport.receive()`，完整帧经另一 `mpsc` 发送
3. `message_loop` 只 `select!` 两个 `Receiver`（cancel-safe）

握手阶段同样用「按 id 等待响应」循环，期间把非匹配的 server 消息转发给 client。

## Output formats

| Format | Notification | Response |
|--------|--------------|----------|
| `passthrough` | 原样 `raw` | 原样 `raw` |
| `json` | compact params 重建 | compact result 重建 |
| `toon` | `{method, params:{format,text}}` | `{id, result:{format,text}}` |

## Timeouts & size limits

- Proxy receive（handshake / message loop / shutdown）: `tokio::time::timeout(30s)`
- `framing::read_frame` 与 stdin 路径拒绝 `Content-Length > 16MiB`（与 `json_rpc::MAX_BODY_SIZE` 同源常量）

## Alternatives considered

- `tokio_util::codec` Framed：更标准，但改动面大；本 change 用 channel 隔离即可
- 在 proxy 内对 transport 做 peek buffer：侵入 Transport trait，拒绝
