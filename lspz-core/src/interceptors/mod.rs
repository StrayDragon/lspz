//! Interceptor trait and chain.
//!
//! All Server→Client message transformations go through the interceptor chain.

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
                match interceptor.intercept(method, p, direction).await {
                    Ok(Some(new_params)) => params = Some(new_params),
                    Ok(None) => return Ok(None),
                    Err(e) => {
                        tracing::warn!(interceptor = %interceptor.name(), error = %e, "Interceptor failed, forwarding original");
                        return Ok(params);
                    }
                }
            }
        }
        Ok(params)
    }
}
