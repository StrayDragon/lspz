//! Interceptor trait and chain.
//!
//! All Server→Client message transformations go through the interceptor chain.

pub mod capping;
pub mod completions;
pub mod diagnostics;
pub mod hover;
pub mod locations;
pub mod symbols;

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
/// [MermaidChart:./docs/mmd/interceptor-chain.mmd]
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
pub struct InterceptorChain {
    interceptors: Vec<Box<dyn Interceptor>>,
}

impl InterceptorChain {
    /// Create a new chain with the given interceptors.
    pub fn new(interceptors: Vec<Box<dyn Interceptor>>) -> Self {
        Self { interceptors }
    }

    /// Process a message through all matching interceptors.
    pub async fn process(
        &self,
        method: &str,
        params: serde_json::Value,
        direction: Direction,
    ) -> Result<Option<serde_json::Value>, LspzError> {
        let mut params = Some(params);
        for interceptor in &self.interceptors {
            if interceptor.applies_to(method, direction)
                && let Some(p) = params.take()
            {
                // Capture original before interceptor call for fail-open fallback
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
    use serde_json::json;

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

    #[tokio::test]
    async fn test_fail_open_returns_original() {
        let chain = InterceptorChain::new(vec![Box::new(AlwaysFailInterceptor)]);
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
        let chain = InterceptorChain::new(vec![]);
        let params = json!({"key": "value"});

        let result = chain
            .process("someMethod", params.clone(), Direction::ServerToClient)
            .await
            .expect("empty chain should succeed");

        assert_eq!(result, Some(params));
    }
}
