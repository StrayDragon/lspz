//! LSP proxy state machine and message loop.

use crate::config::Config;
use crate::error::LspzError;
use crate::interceptors::InterceptorChain;
use crate::transport::Transport;

/// The lspz proxy.
///
/// [MermaidChart:./docs/mmd/proxy-state-machine.mmd]
#[expect(dead_code)]
pub struct Proxy {
    config: Config,
    transport: Box<dyn Transport>,
    interceptor_chain: InterceptorChain,
}

impl Proxy {
    /// Create a new [`Proxy`].
    pub fn new(
        config: Config,
        transport: Box<dyn Transport>,
        interceptor_chain: InterceptorChain,
    ) -> Self {
        Self {
            config,
            transport,
            interceptor_chain,
        }
    }

    /// Start the proxy (placeholder — will be implemented in Task B).
    pub async fn start(&mut self) -> Result<(), LspzError> {
        tracing::info!("Proxy started");
        Ok(())
    }
}
