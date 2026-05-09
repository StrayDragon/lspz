# 拦截器参考

> 自动从 `Interceptor` trait 定义及实现生成。编辑源码后运行 `just gen-api-docs` 刷新。

Direction of an LSP message.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
```

Client → Server

```rust
ClientToServer,
```

Server → Client

```rust
ServerToClient,
```

A single interceptor in the chain.

[MermaidChart:./docs/mmd/interceptor-chain.mmd]

```rust
#[async_trait::async_trait]
```

Unique name for logging / configuration.

```rust
fn name(&self) -> &str;
```

Whether this interceptor should process the given message.

```rust
fn applies_to(&self, method: &str, direction: Direction) -> bool;
```

Transform the message params.

Returns:
- `Ok(Some(params))` — modified params to use
- `Ok(None)` — drop the message
- `Err(_)` — fail open; caller should forward original

```rust
async fn intercept(
```

A chain of interceptors executed in order.

```rust
pub struct InterceptorChain {
```

Create a new chain with the given interceptors.

```rust
pub fn new(interceptors: Vec<Box<dyn Interceptor>>) -> Self {
```

Process a message through all matching interceptors.

```rust
pub async fn process(
```
