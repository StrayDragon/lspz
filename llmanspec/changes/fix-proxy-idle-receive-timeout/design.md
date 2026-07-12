# Design: fix-proxy-idle-receive-timeout

```text
handshake / recv_response_by_id: keep timeout(RECEIVE_TIMEOUT, receive)
message_loop select! server branch: bare receive()  // or idle-aware deadline that only logs
```

可选：仅在存在 in-flight pending_requests 时对 server receive 施加超时。
