# Design: fix-diag-code-utf8-notify-buffer

## Code extraction

Shared helper (in `compact` or diagnostics):

```rust
fn diagnostic_code_to_string(code: &Value) -> Option<String> {
  code.as_str().map(str::to_string)
    .or_else(|| code.as_i64().map(|n| n.to_string()))
    .or_else(|| code.as_u64().map(|n| n.to_string()))
}
```

Used by `DiagnosticsCompressor` and `compact::compress`.

## UTF-8 truncate

Floor `max` to `str::floor_char_boundary` (Rust 1.93+ / Edition 2024) or manual loop on `is_char_boundary`.

## Notification buffer

`VecDeque<(String, Value)>` on `LspSession`, cap ~64. `send_request` pushes; `wait_for_notification_where` drains match first.
