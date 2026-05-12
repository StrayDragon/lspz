# API: `src/metrics`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

Configuration for runtime metrics collection.

When `enabled` is false (default), the `MetredInterceptor` wrapper adds zero overhead.

```rust
#[derive(Debug, Default, Clone, serde::Deserialize)]
```

Whether metrics collection is enabled.

```rust
pub enabled: bool,
```

Interval in seconds for periodic metrics logging.
0 means only log on shutdown/drop.

```rust
pub report_interval_secs: u64,
```

Atomic snapshot of metrics for a single interceptor.

```rust
#[derive(Debug)]
```

Total bytes of input params before processing.

```rust
pub total_input_bytes: AtomicU64,
```

Total bytes of output params after processing.

```rust
pub total_output_bytes: AtomicU64,
```

Total latency in microseconds.

```rust
pub total_latency_us: AtomicU64,
```

Number of messages processed.

```rust
pub messages_processed: AtomicU64,
```

Number of failures (fail-open events).

```rust
pub failures: AtomicU64,
```

Create a new, zeroed snapshot.

```rust
pub fn new() -> Self {
```

Compute the compression ratio (1.0 - output/input).

```rust
pub fn compression_ratio(&self) -> f64 {
```

Average latency per processed message in microseconds.

```rust
pub fn avg_latency_us(&self) -> f64 {
```

Build a tracing-friendly summary string.

```rust
pub fn summary(&self, name: &str) -> String {
```

An interceptor wrapper that records metrics.

When metrics are disabled, all methods delegate directly to the inner
interceptor with no measurement overhead.

```rust
pub struct MetredInterceptor {
```

Wrap an interceptor with metrics recording.

```rust
pub fn new(inner: Box<dyn Interceptor>) -> Self {
```

Enable metrics collection for this wrapper.

```rust
pub fn enable(mut self) -> Self {
```

Reference to the metrics snapshot.

```rust
pub fn snapshot(&self) -> &MetricsSnapshot {
```

Dump metrics snapshots via tracing.

Called periodically or on shutdown to log all metrics.

```rust
pub fn log_metrics_summary(snapshots: &[&MetredInterceptor]) {
```
