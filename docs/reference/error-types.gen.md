# 错误类型参考

> 自动从 `src/error.rs` 生成。编辑源码后运行 `just gen-error-docs` 刷新。

## `LspzError`

- **`Io(#[from] io::Error)`**
- **`JsonParse(#[from] serde_json::Error)`**
- **`Protocol(String)`**
- **`ServerExited`**
- **`Config(String)`**
