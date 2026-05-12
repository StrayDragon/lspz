//! Runtime metrics for the interceptor chain.
//!
//! Provides configurable metrics collection using atomic counters.
//! Metrics are recorded per-interceptor via a decorator wrapper.
//!
//! ## Usage
//!
//! ```ignore
//! use lspz::metrics::{MetricsConfig, MetredInterceptor, MetricsSnapshot};
//!
//! let config = MetricsConfig { enabled: true, report_interval_secs: 60 };
//! let wrapped = MetredInterceptor::new(Box::new(DiagnosticsCompressor::default()));
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::error::LspzError;
use crate::interceptors::{Direction, Interceptor};

/// Configuration for runtime metrics collection.
///
/// When `enabled` is false (default), the `MetredInterceptor` wrapper adds zero overhead.
#[derive(Debug, Default, Clone, serde::Deserialize)]
pub struct MetricsConfig {
    /// Whether metrics collection is enabled.
    pub enabled: bool,
    /// Interval in seconds for periodic metrics logging.
    /// 0 means only log on shutdown/drop.
    pub report_interval_secs: u64,
}

/// Atomic snapshot of metrics for a single interceptor.
#[derive(Debug)]
pub struct MetricsSnapshot {
    /// Total bytes of input params before processing.
    pub total_input_bytes: AtomicU64,
    /// Total bytes of output params after processing.
    pub total_output_bytes: AtomicU64,
    /// Total latency in microseconds.
    pub total_latency_us: AtomicU64,
    /// Number of messages processed.
    pub messages_processed: AtomicU64,
    /// Number of failures (fail-open events).
    pub failures: AtomicU64,
}

impl MetricsSnapshot {
    /// Create a new, zeroed snapshot.
    pub fn new() -> Self {
        Self {
            total_input_bytes: AtomicU64::new(0),
            total_output_bytes: AtomicU64::new(0),
            total_latency_us: AtomicU64::new(0),
            messages_processed: AtomicU64::new(0),
            failures: AtomicU64::new(0),
        }
    }

    /// Compute the compression ratio (1.0 - output/input).
    pub fn compression_ratio(&self) -> f64 {
        let input = self.total_input_bytes.load(Ordering::Relaxed);
        let output = self.total_output_bytes.load(Ordering::Relaxed);
        if input == 0 {
            return 0.0;
        }
        1.0 - (output as f64 / input as f64)
    }

    /// Average latency per processed message in microseconds.
    pub fn avg_latency_us(&self) -> f64 {
        let count = self.messages_processed.load(Ordering::Relaxed);
        if count == 0 {
            return 0.0;
        }
        self.total_latency_us.load(Ordering::Relaxed) as f64 / count as f64
    }

    /// Build a tracing-friendly summary string.
    pub fn summary(&self, name: &str) -> String {
        let processed = self.messages_processed.load(Ordering::Relaxed);
        let failures = self.failures.load(Ordering::Relaxed);
        let ratio = self.compression_ratio();
        let avg_lat = self.avg_latency_us();

        format!(
            "Metrics[{name}]: processed={processed}, compression_ratio={:.1}%, avg_latency_us={avg_lat:.0}, failures={failures}",
            ratio * 100.0
        )
    }
}

impl Default for MetricsSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

/// An interceptor wrapper that records metrics.
///
/// When metrics are disabled, all methods delegate directly to the inner
/// interceptor with no measurement overhead.
pub struct MetredInterceptor {
    inner: Box<dyn Interceptor>,
    snapshot: Box<MetricsSnapshot>,
    enabled: bool,
}

impl MetredInterceptor {
    /// Wrap an interceptor with metrics recording.
    pub fn new(inner: Box<dyn Interceptor>) -> Self {
        Self {
            inner,
            snapshot: Box::new(MetricsSnapshot::new()),
            enabled: false,
        }
    }

    /// Enable metrics collection for this wrapper.
    pub fn enable(mut self) -> Self {
        self.enabled = true;
        self
    }

    /// Reference to the metrics snapshot.
    pub fn snapshot(&self) -> &MetricsSnapshot {
        &self.snapshot
    }
}

#[async_trait::async_trait]
impl Interceptor for MetredInterceptor {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn applies_to(&self, method: &str, direction: Direction) -> bool {
        self.inner.applies_to(method, direction)
    }

    async fn intercept(
        &self,
        method: &str,
        params: serde_json::Value,
        direction: Direction,
    ) -> Result<Option<serde_json::Value>, LspzError> {
        if !self.enabled {
            return self.inner.intercept(method, params, direction).await;
        }

        let pre_size = params.to_string().len() as u64;
        let start = Instant::now();

        match self.inner.intercept(method, params, direction).await {
            Ok(Some(new_params)) => {
                let elapsed = start.elapsed().as_micros() as u64;
                let post_size = new_params.to_string().len() as u64;

                self.snapshot
                    .total_input_bytes
                    .fetch_add(pre_size, Ordering::Relaxed);
                self.snapshot
                    .total_output_bytes
                    .fetch_add(post_size, Ordering::Relaxed);
                self.snapshot
                    .total_latency_us
                    .fetch_add(elapsed, Ordering::Relaxed);
                let count = self
                    .snapshot
                    .messages_processed
                    .fetch_add(1, Ordering::Relaxed)
                    + 1;

                // Auto-log every 100 messages or if this is the first one
                if count == 1 || count.is_multiple_of(100) {
                    tracing::info!("{}", self.snapshot.summary(self.inner.name()));
                }

                Ok(Some(new_params))
            }
            Ok(None) => {
                let elapsed = start.elapsed().as_micros() as u64;
                self.snapshot
                    .total_input_bytes
                    .fetch_add(pre_size, Ordering::Relaxed);
                self.snapshot
                    .total_latency_us
                    .fetch_add(elapsed, Ordering::Relaxed);
                self.snapshot
                    .messages_processed
                    .fetch_add(1, Ordering::Relaxed);

                Ok(None)
            }
            Err(e) => {
                self.snapshot.failures.fetch_add(1, Ordering::Relaxed);

                Err(e)
            }
        }
    }
}

/// Dump metrics snapshots via tracing.
///
/// Called periodically or on shutdown to log all metrics.
pub fn log_metrics_summary(snapshots: &[&MetredInterceptor]) {
    for m in snapshots {
        if m.snapshot.messages_processed.load(Ordering::Relaxed) > 0 {
            tracing::info!("{}", m.snapshot.summary(m.name()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct TestInterceptor;

    #[async_trait::async_trait]
    impl Interceptor for TestInterceptor {
        fn name(&self) -> &str {
            "test"
        }
        fn applies_to(&self, method: &str, _direction: Direction) -> bool {
            !method.is_empty()
        }
        async fn intercept(
            &self,
            _method: &str,
            _params: serde_json::Value,
            _direction: Direction,
        ) -> Result<Option<serde_json::Value>, LspzError> {
            Ok(Some(json!({"compressed": true})))
        }
    }

    #[tokio::test]
    async fn test_metred_records_sizes() {
        let interceptor = MetredInterceptor::new(Box::new(TestInterceptor)).enable();
        let params = json!({"key": "value", "data": "hello world"});

        let _ = interceptor
            .intercept("test/method", params, Direction::ServerToClient)
            .await
            .unwrap();

        assert_eq!(
            interceptor
                .snapshot
                .messages_processed
                .load(Ordering::Relaxed),
            1
        );
        assert!(
            interceptor
                .snapshot
                .total_input_bytes
                .load(Ordering::Relaxed)
                > 0
        );
        assert!(
            interceptor
                .snapshot
                .total_output_bytes
                .load(Ordering::Relaxed)
                > 0
        );
        assert!(
            interceptor
                .snapshot
                .total_latency_us
                .load(Ordering::Relaxed)
                > 0
        );
    }

    #[tokio::test]
    async fn test_disabled_no_recording() {
        let interceptor = MetredInterceptor::new(Box::new(TestInterceptor)); // not enabled
        let params = json!({"key": "value"});

        let _ = interceptor
            .intercept("test/method", params, Direction::ServerToClient)
            .await
            .unwrap();

        assert_eq!(
            interceptor
                .snapshot
                .messages_processed
                .load(Ordering::Relaxed),
            0
        );
    }

    #[tokio::test]
    async fn test_failure_recorded() {
        struct FailInterceptor;
        #[async_trait::async_trait]
        impl Interceptor for FailInterceptor {
            fn name(&self) -> &str {
                "fail"
            }
            fn applies_to(&self, _m: &str, _d: Direction) -> bool {
                true
            }
            async fn intercept(
                &self,
                _m: &str,
                _p: serde_json::Value,
                _d: Direction,
            ) -> Result<Option<serde_json::Value>, LspzError> {
                Err(LspzError::Protocol("fail".into()))
            }
        }

        let interceptor = MetredInterceptor::new(Box::new(FailInterceptor)).enable();
        let params = json!({"x": 1});

        let _ = interceptor
            .intercept("test/method", params, Direction::ServerToClient)
            .await;

        assert_eq!(interceptor.snapshot.failures.load(Ordering::Relaxed), 1);
        assert_eq!(
            interceptor
                .snapshot
                .messages_processed
                .load(Ordering::Relaxed),
            0 // failure is not counted as processed
        );
    }

    #[test]
    fn test_compression_ratio() {
        let snap = MetricsSnapshot::new();
        snap.total_input_bytes.store(200, Ordering::Relaxed);
        snap.total_output_bytes.store(50, Ordering::Relaxed);

        let ratio = snap.compression_ratio();
        assert!((ratio - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_summary_format() {
        let snap = MetricsSnapshot::new();
        snap.total_input_bytes.store(100, Ordering::Relaxed);
        snap.total_output_bytes.store(30, Ordering::Relaxed);
        snap.total_latency_us.store(500, Ordering::Relaxed);
        snap.messages_processed.store(10, Ordering::Relaxed);

        let s = snap.summary("test");
        assert!(s.contains("Metrics[test]"));
        assert!(s.contains("compression_ratio=70.0%") || s.contains("compression_ratio=70%"));
        assert!(s.contains("avg_latency_us=50"));
    }
}
