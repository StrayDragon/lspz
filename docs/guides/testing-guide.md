# lspz 测试指南

**版本**: v0.1.0
**最后更新**: 2026-05-09

## 测试策略

lspz 采用分层测试策略，确保代码质量和 LSP 兼容性。

```
┌─────────────────────────────────────┐
│         测试金字塔                    │
├─────────────────────────────────────┤
│                                     │
│        /\                            │
│       /  \                           │
│      / E2E \        少量端到端测试     │
│     /────────\                      │
│    /          \                     │
│   /  集成测试   \    中等集成测试      │
│  /──────────────\                  │
│ /                \                 │
│/   单元测试         \   大量单元测试   │
└─────────────────────────────────────┘
```

---

## 单元测试

### 目标

测试独立函数和模块的逻辑正确性。

### 位置

- 与源码同目录的 `tests` 模块
- 或 `src/**/tests/` 子目录

### 示例

```rust
// src/codec/compress.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_encoding() {
        assert_eq!(encode_severity(Severity::Error), 'E');
        assert_eq!(encode_severity(Severity::Warning), 'W');
        assert_eq!(encode_severity(Severity::Information), 'I');
        assert_eq!(encode_severity(Severity::Hint), 'H');
    }

    #[test]
    fn test_dedup_diagnostics() {
        let input = vec![
            Diagnostic {
                message: "unused".to_string(),
                severity: Severity::Warning,
                range: Range::new(1, 0, 1, 5),
            },
            Diagnostic {
                message: "unused".to_string(),
                severity: Severity::Warning,
                range: Range::new(2, 0, 2, 5),
            },
        ];

        let output = dedup_diagnostics(input);
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].ranges.len(), 2);
    }
}
```

### 运行单元测试

```bash
# 运行所有单元测试
cargo test

# 运行特定测试
cargo test test_severity_encoding

# 显示输出
cargo test -- --nocapture

# 运行并显示覆盖率（需要安装 tarpaulin）
cargo tarpaulin --out Html
```

---

## 集成测试

### 目标

测试 lspz 与真实 LSP 服务器的交互。

### 位置

- `tests/` 目录（项目根目录）
- 或 `tests/integration/` 子目录

### LSP 服务器集成测试

```rust
// tests/integration/rust_analyzer.rs

use lspz_core::Proxy;
use lspz_core::Config;

#[tokio::test]
async fn test_rust_analyzer_diagnostics() {
    // 创建临时测试文件
    let test_file = create_test_file(r#"
fn main() {
    let x = 42;
    let y = 100;
}
"#);

    // 启动代理
    let config = Config::builder()
        .backend_cmd("rust-analyzer")
        .enable_diag_compress(true)
        .build();

    let proxy = Proxy::new(config).await.unwrap();
    proxy.initialize().await.unwrap();

    // 打开文件
    proxy.did_open(&test_file).await.unwrap();

    // 等待诊断
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 获取诊断（压缩后）
    let diagnostics = proxy.get_diagnostics(&test_file.uri()).await.unwrap();

    // 验证压缩效果
    assert!(!diagnostics.is_empty());
    assert!(diagnostics[0].m.contains("unused"));

    // 验证 Token 节省
    let original_tokens = estimate_tokens(&original_diagnostics);
    let compressed_tokens = estimate_tokens(&diagnostics);
    let savings = 1.0 - (compressed_tokens as f64 / original_tokens as f64);
    assert!(savings >= 0.4, "Token savings should be >= 40%, got {:.0}%", savings * 100.0);
}
```

### 运行集成测试

```bash
# 运行所有集成测试
cargo test --test '*'

# 运行特定集成测试
cargo test --test rust_analyzer

# 需要真实 LSP 服务器的测试（标记为 ignore）
cargo test -- --ignored

# 仅运行文档测试
cargo test --doc
```

---

## 端到端测试

### 目标

测试完整的 lspz CLI 工作流。

### 示例

```bash
#!/bin/bash
# tests/e2e/basic_proxy.sh

set -e

# 启动 lspz 代理
lspz --backend rust-analyzer --stdio &
LSPZ_PID=$!

# 等待启动
sleep 1

# 发送 initialize 请求
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{...}}' >&3

# 读取响应
read response <&4

# 验证响应
echo "$response" | jq '.result.capabilities.textDocumentSync'

# 清理
kill $LSPZ_PID
```

---

## 性能测试

### Token 节省测试

```rust
use tiktoken_rust::count_tokens;

fn test_token_savings(original: &str, compressed: &str) {
    let original_tokens = count_tokens(original);
    let compressed_tokens = count_tokens(compressed);
    let savings = 1.0 - (compressed_tokens as f64 / original_tokens as f64);

    println!("Original tokens: {}", original_tokens);
    println!("Compressed tokens: {}", compressed_tokens);
    println!("Savings: {:.1}%", savings * 100.0);

    assert!(savings >= 0.4, "Token savings should be >= 40%");
}
```

### 延迟测试

```rust
#[tokio::test]
async fn test_compression_latency() {
    let start = Instant::now();
    let compressed = compress_diagnostics(large_diagnostic_set()).await.unwrap();
    let latency = start.elapsed();

    assert!(latency < Duration::from_millis(10), "Compression should be < 10ms, got {:?}", latency);
}
```

### 运行性能测试

```bash
# 运行 release 模式的性能测试
cargo test --release -- --nocapture performance

# 使用 criterion 进行基准测试
cargo bench
```

---

## LSP 服务器兼容性测试

### 测试环境准备

```bash
# 确保 LSP 服务器已安装
which rust-analyzer
which gopls
which basedpyright
which typescript-language-server
```

### 测试脚本

```rust
// tests/lsp_servers/mod.rs

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    macro_rules! test_lsp_server {
        ($name:expr, $cmd:expr) => {
            #[tokio::test]
            async fn $name() {
                let config = Config::builder()
                    .backend_cmd($cmd)
                    .enable_diag_compress(false)  // 先测试不压缩
                    .build();

                let proxy = Proxy::new(config).await.unwrap();
                proxy.initialize().await.unwrap();

                // 基本功能验证
                assert!(proxy.is_initialized());
            }
        };
    }

    test_lsp_server!(rust_analyzer, "rust-analyzer");
    test_lsp_server!(gopls, "gopls");
    test_lsp_server!(basedpyright, "basedpyright");
    test_lsp_server!(typescript, "typescript-language-server");
}
```

---

## 测试数据

### 测试文件位置

```
tests/
├── fixtures/
│   ├── rust/
│   │   └── test_file.rs
│   ├── go/
│   │   └── test_file.go
│   └── python/
│       └── test_file.py
└── data/
    └── diagnostics/
        ├── rust_diagnostics.json
        └── compressed_diagnostics.json
```

### 示例测试文件

```rust
// tests/fixtures/rust/test_file.rs
fn main() {
    let x = 42;
    let y = x + 1;
    println!("{}", y);
}
```

---

## CI/CD 集成

### GitHub Actions 配置

```yaml
# .github/workflows/test.yml

name: Test

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - uses: actions-rust-lang/setup-rust-toolchain@v1

      # 安装 LSP 服务器
      - run: |
          rustup component add rust-analyzer
          go install golang.org/x/tools/gopls@latest

      # 运行测试
      - run: cargo test --verbose

      # 运行 clippy
      - run: cargo clippy -- -D warnings

      # 检查格式
      - run: cargo fmt -- --check
```

---

## 覆盖率目标

| 层级 | 目标覆盖率 | 说明 |
|------|-----------|------|
| 核心逻辑 (codec/) | ≥ 90% | 关键算法需要高覆盖 |
| Proxy 核心 | ≥ 80% | 主要流程需要覆盖 |
| Transport | ≥ 70% | 基础通信覆盖 |
| 配置/错误 | ≥ 60% | 简单逻辑 |

---

## 测试最佳实践

### 1. 使用参数化测试

```rust
use rstest::rstest;

#[rstest]
#[case(Severity::Error, 'E')]
#[case(Severity::Warning, 'W')]
#[case(Severity::Information, 'I')]
#[case(Severity::Hint, 'H')]
fn test_severity_encoding(#[case] input: Severity, #[case] expected: char) {
    assert_eq!(encode_severity(input), expected);
}
```

### 2. 使用测试构建器

```rust
struct DiagnosticBuilder {
    message: String,
    severity: Severity,
    range: Range,
}

impl DiagnosticBuilder {
    fn new() -> Self {
        Self {
            message: String::new(),
            severity: Severity::Information,
            range: Range::default(),
        }
    }

    fn message(mut self, msg: &str) -> Self {
        self.message = msg.to_string();
        self
    }

    fn severity(mut self, sev: Severity) -> Self {
        self.severity = sev;
        self
    }

    fn build(self) -> Diagnostic {
        // ...
    }
}

#[test]
fn test_with_builder() {
    let diag = DiagnosticBuilder::new()
        .message("test")
        .severity(Severity::Error)
        .build();
}
```

### 3. Mock 外部依赖

```rust
#[cfg(test)]
mockall::mock! {
    Transport {}
    #[async_trait]
    impl Transport for Transport {
        async fn send(&self, msg: JsonRpcMessage) -> Result<()>;
        async fn receive(&self) -> Result<JsonRpcMessage>;
    }
}

#[tokio::test]
async fn test_with_mock() {
    let mut mock_transport = MockTransport::new();
    mock_transport
        .expect_send()
        .returning(|_| Ok(()));

    let proxy = Proxy::with_transport(mock_transport);
    // ...
}
```

---

## 调试测试

### 启用日志

```bash
RUST_LOG=debug cargo test -- --nocapture
```

### 使用 dbg! 宏

```rust
#[test]
fn test_something() {
    let result = complex_function();
    dbg!(&result);  // 打印到 stderr
    assert!(result.is_ok());
}
```

### 有限失败测试

```bash
# 只运行前 3 个失败的测试
cargo test -- --test-threads=1 -- -nocapture --exact
```

---

## 参考文档

- [Rust 测试文档](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [rstest 文档](https://docs.rs/rstest/)
- [tarpaulin 文档](https://github.com/xd009642/tarpaulin)
- [plan/01-mvp-phase.md](../plan/01-mvp-phase.md) - MVP 测试目标
