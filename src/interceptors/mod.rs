//! Interceptor trait and chain.
//!
//! All Server→Client message transformations go through the interceptor chain.

pub mod capping;
pub mod completions;
pub mod diagnostics;
pub mod hover;
pub mod locations;
pub mod symbols;
pub mod workspace_diagnostics;
pub mod workspace_symbols;

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::config::Config;
use crate::error::LspzError;

/// Direction of an LSP message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Client → Server
    ClientToServer,
    /// Server → Client
    ServerToClient,
}

/// A single interceptor in the chain.
///
/// [MermaidChart:docs/src/diagrams/interceptor-chain.mmd]
#[async_trait::async_trait]
pub trait Interceptor: Send + Sync {
    /// Unique name for logging / configuration.
    fn name(&self) -> &str;

    /// Whether this interceptor should process the given message.
    fn applies_to(&self, method: &str, direction: Direction) -> bool;

    /// Transform the message params.
    ///
    /// Returns:
    /// - `Ok(Some(params))` — modified params to use
    /// - `Ok(None)` — drop the message
    /// - `Err(_)` — fail open; caller should forward original
    async fn intercept(
        &self,
        method: &str,
        params: serde_json::Value,
        direction: Direction,
    ) -> Result<Option<serde_json::Value>, LspzError>;
}

/// A chain of interceptors executed in order.
///
/// Holds a shared config reference for runtime enable/disable checks.
pub struct InterceptorChain {
    interceptors: Vec<Box<dyn Interceptor>>,
    config: Arc<RwLock<Config>>,
}

impl InterceptorChain {
    /// Create a new chain with the given interceptors and shared config.
    pub fn new(interceptors: Vec<Box<dyn Interceptor>>, config: Arc<RwLock<Config>>) -> Self {
        Self {
            interceptors,
            config,
        }
    }

    /// Process a message through all matching interceptors.
    ///
    /// Skips interceptors that are disabled in the current config.
    /// On interceptor failure, logs a WARN and returns the original params (fail-open).
    pub async fn process(
        &self,
        method: &str,
        params: serde_json::Value,
        direction: Direction,
    ) -> Result<Option<serde_json::Value>, LspzError> {
        let config = self.config.read().await;
        let mut params = Some(params);
        for interceptor in &self.interceptors {
            if interceptor.applies_to(method, direction)
                && config.is_interceptor_enabled(interceptor.name())
                && let Some(p) = params.take()
            {
                let original = Some(p.clone());
                match interceptor.intercept(method, p, direction).await {
                    Ok(Some(new_params)) => params = Some(new_params),
                    Ok(None) => return Ok(None),
                    Err(e) => {
                        tracing::warn!(interceptor = %interceptor.name(), error = %e, "Interceptor failed, forwarding original");
                        return Ok(original);
                    }
                }
            }
        }
        Ok(params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// An interceptor that always fails.
    struct AlwaysFailInterceptor;

    #[async_trait::async_trait]
    impl Interceptor for AlwaysFailInterceptor {
        fn name(&self) -> &str {
            "always_fail"
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
            Err(LspzError::Protocol("simulated failure".into()))
        }
    }

    fn make_chain(interceptors: Vec<Box<dyn Interceptor>>) -> InterceptorChain {
        InterceptorChain::new(interceptors, Arc::new(RwLock::new(Config::default())))
    }

    #[tokio::test]
    async fn test_fail_open_returns_original() {
        let chain = make_chain(vec![Box::new(AlwaysFailInterceptor)]);
        let params = json!({"key": "value"});

        let result = chain
            .process("someMethod", params.clone(), Direction::ServerToClient)
            .await
            .expect("fail-open should not propagate error");

        assert_eq!(
            result,
            Some(params),
            "fail-open should return original params"
        );
    }

    #[tokio::test]
    async fn test_empty_chain_passthrough() {
        let chain = make_chain(vec![]);
        let params = json!({"key": "value"});

        let result = chain
            .process("someMethod", params.clone(), Direction::ServerToClient)
            .await
            .expect("empty chain should succeed");

        assert_eq!(result, Some(params));
    }
}
