//! 拦截器 trait 和链。
//!
//! 所有服务端→客户端的消息转换都通过拦截器链进行。

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

/// LSP 消息的方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// 客户端 → 服务端
    ClientToServer,
    /// 服务端 → 客户端
    ServerToClient,
}

/// 链中的单个拦截器。
///
/// [MermaidChart:docs/src/diagrams/interceptor-chain.mmd]
#[async_trait::async_trait]
pub trait Interceptor: Send + Sync {
    /// 用于日志/配置的唯一名称。
    fn name(&self) -> &str;

    /// 此拦截器是否应该处理给定的消息。
    fn applies_to(&self, method: &str, direction: Direction) -> bool;

    /// 转换消息参数。
    ///
    /// 返回：
    /// - `Ok(Some(params))` — 使用修改后的参数
    /// - `Ok(None)` — 丢弃消息
    /// - `Err(_)` — 失败开放；调用者应转发原始消息
    async fn intercept(
        &self,
        method: &str,
        params: serde_json::Value,
        direction: Direction,
    ) -> Result<Option<serde_json::Value>, LspzError>;
}

/// 按顺序执行的拦截器链。
///
/// 保存共享配置引用，用于运行时启用/禁用检查。
pub struct InterceptorChain {
    interceptors: Vec<Box<dyn Interceptor>>,
    config: Arc<RwLock<Config>>,
}

impl InterceptorChain {
    /// 使用给定的拦截器和共享配置创建新链。
    pub fn new(interceptors: Vec<Box<dyn Interceptor>>, config: Arc<RwLock<Config>>) -> Self {
        Self {
            interceptors,
            config,
        }
    }

    /// 通过所有匹配的拦截器处理消息。
    ///
    /// 跳过在当前配置中禁用的拦截器。
    /// 拦截器失败时，记录 WARN 并返回原始参数（失败开放）。
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
