# Design: fix-daemon-graceful-shutdown

## Approach

`tokio::sync::watch::channel(false)`（或 `Notify`）在 DaemonServer 内共享：

1. Ctrl-C task → `send(true)`
2. Idle reaper → `send(true)` when TTL hit
3. `daemon/shutdown` handler → `send(true)` after responding ok

`start()` accept loop:

```text
select! {
  _ = shutdown_rx.changed() if *shutdown_rx.borrow() => break,
  accept = listener.accept() => handle client,
}
```

After break:

1. `pool.lock().await.clear()` — drops all `LspSession` → `StdioTransport::Drop` → `start_kill`
2. `remove_file(socket_path)`
3. `return Ok(())` (process exits normally from main)

## Alternatives

- Keep `process::exit` after explicit `pool.clear()` — works but skips other Drops; prefer clean return.
