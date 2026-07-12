# Design: fix-markup-docs-and-daemon-status

## MarkupContent

```rust
fn documentation_text(doc: &Value) -> Option<&str> {
  doc.as_str()
    .or_else(|| doc.get("value").and_then(Value::as_str))
}
```

Use in frequency pass and item emit.

## Daemon counters

```rust
struct DaemonStatus {
  started_at_unix: u64, // serde skip or include
  ...
}
fn refresh_uptime(&mut self) { self.uptime_secs = now - started_at_unix; }
fn record_connection(&mut self) { self.total_connections += 1; }
fn record_request(&mut self) { self.total_requests += 1; }
```

Call sites: accept loop; start of `dispatch` (all methods including status).
