# API: `src/config_watcher`

> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。

File-system watcher for hot-reloading [`LspzConfig`].

Spawns a background tokio task that monitors the config file.
On every change, re-reads the TOML and updates the shared `Arc<RwLock<LspzConfig>>`.

```rust
pub struct ConfigWatcher {
```

Start watching `path` for changes, writing updates into `shared_config`.

Returns immediately; the watcher runs in the background.

```rust
pub fn spawn(
```

Abort the background watcher task.

```rust
pub fn shutdown(self) {
```

Re-read the TOML file and update the shared config.

```rust
async fn reload(
```
