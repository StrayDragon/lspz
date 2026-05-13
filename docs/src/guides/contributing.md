# 贡献指南

感谢你对 lspz 项目的关注！本文档指导如何为 lspz 做出贡献。

---

## 开始之前

### 必读文档

在开始贡献前，请按顺序阅读以下文档：

1. **[ROADMAP.md](https://github.com/straydragon/lspz/blob/main/ROADMAP.md)** - 了解项目整体规划
2. **[docs/README.md](../introduction.md)** - 开发者前导文档
3. **[docs/specs/001-tri-modal-architecture.md](../architecture.md)** - 理解三模态架构
4. **[docs/guides/coding-conventions.md](coding-conventions.md)** - 熟悉编码规范
5. **[docs/guides/testing-guide.md](testing-guide.md)** - 了解测试要求

### 适合你的贡献方式

| 你的背景 | 推荐贡献方式 |
|---------|-------------|
| 熟悉 Rust，了解 LSP | 代码开发、Bug 修复 |
| 了解 AI/LLM，熟悉 Agent | 压缩算法优化、集成测试 |
| 熟悉文档写作 | 文档完善、示例编写 |
| 熟悉测试 | 测试用例编写、CI/CD |

---

## 开发流程

### 1. 设置开发环境

```bash
# 克隆仓库
git clone https://github.com/your-org/lspz.git
cd lspz

# 安装 Rust 工具链
rustup install stable
rustup component add rust-analyzer clippy rustfmt

# 安装 LSP 服务器（用于测试）
rustup component add rust-analyzer
go install golang.org/x/tools/gopls@latest
npm install -g typescript-language-server
```

### 2. 创建功能分支

```bash
# 从 main 分支创建
git checkout -b feature/your-feature-name

# 或修复 bug
git checkout -b fix/issue-123
```

### 3. 开发和测试

```bash
# 格式化代码
cargo fmt

# 运行 clippy
cargo clippy -- -D warnings

# 运行测试
cargo test

# 运行特定测试
cargo test test_name

# 运行集成测试（需要 LSP 服务器）
cargo test --test '*'
```

### 4. 提交代码

```bash
# 添加变更
git add .

# 提交（使用规范的提交信息）
git commit -m "feat: add TCP transport support

- Implement TcpTransport for TCP connections
- Add configuration for TCP transport
- Add integration tests

Closes #123"
```

### 5. 推送和创建 PR

```bash
# 推送到远程
git push origin feature/your-feature-name

# 在 GitHub 上创建 Pull Request
```

---

## Pull Request 指南

### PR 标题格式

使用 [Conventional Commits](https://www.conventionalcommits.org/) 格式：

| 类型 | 说明 | 示例 |
|------|------|------|
| `feat` | 新功能 | `feat: add WebSocket transport` |
| `fix` | Bug 修复 | `fix: handle compression errors gracefully` |
| `docs` | 文档更新 | `docs: update API documentation` |
| `test` | 测试相关 | `test: add integration tests for gopls` |
| `refactor` | 重构 | `refactor: simplify interceptor chain` |
| `perf` | 性能优化 | `perf: reduce allocation in compression` |
| `ci` | CI/CD | `ci: add clippy check to CI` |

### PR 描述模板

```markdown
## 变更类型
- [ ] Bug 修复
- [ ] 新功能
- [ ] 破坏性变更
- [ ] 文档更新

## 描述
<!-- 简要描述这个 PR 做了什么 -->

## 相关 Issue
<!-- 关联的 Issue，如 Closes #123 -->

## 变更内容
- [ ] 列表项 1
- [ ] 列表项 2

## 测试
- [ ] 单元测试通过
- [ ] 集成测试通过
- [ ] 手动测试通过

## 检查清单
- [ ] 代码符合 [编码规范](coding-conventions.md)
- [ ] 已添加必要的测试
- [ ] 已更新相关文档
- [ ] PR 描述清晰完整
```

### PR 审查流程

1. **自动检查**:
   - CI 构建必须通过
   - 代码覆盖率不能降低
   - Clippy 无警告

2. **代码审查**:
   - 至少一位维护者批准
   - 所有审查意见已解决

3. **合并**:
   - 使用 Squash and Merge
   - 更新 CHANGELOG.md

---

## 代码规范

### Rust 代码风格

- 遵循 [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- 使用 `cargo fmt` 格式化
- 使用 `cargo clippy` 检查
- 公共 API 必须有文档注释

详见: [docs/guides/coding-conventions.md](coding-conventions.md)

### 测试要求

- 单元测试覆盖率 ≥ 80%
- 新功能必须有集成测试
- 性能敏感代码需要基准测试

详见: [docs/guides/testing-guide.md](testing-guide.md)

---

## 开发指南

### 添加新拦截器

1. 实现 `Interceptor` trait:

```rust
use async_trait::async_trait;
use lspz::interceptors::{Interceptor, Direction};
use lspz::error::LspzError;

pub struct MyInterceptor;

#[async_trait]
impl Interceptor for MyInterceptor {
    async fn intercept(
        &self,
        message: &JsonRpcMessage,
        direction: Direction,
    ) -> Result<Option<JsonRpcMessage>, LspzError> {
        // 实现拦截逻辑
        Ok(None)  // 返回 None 表示丢弃消息
    }
}
```

2. 注册拦截器:

```rust
proxy.register_interceptor(Box::new(MyInterceptor))?;
```

3. 添加测试:

```rust
#[tokio::test]
async fn test_my_interceptor() {
    // 测试逻辑
}
```

### 添加新传输层

1. 实现 `Transport` trait:

```rust
use async_trait::async_trait;
use lspz::transport::Transport;
use lspz::error::LspzError;

pub struct MyTransport;

#[async_trait]
impl Transport for MyTransport {
    async fn send(&self, msg: JsonRpcMessage) -> Result<(), LspzError> {
        // 实现发送逻辑
        Ok(())
    }

    async fn receive(&self) -> Result<JsonRpcMessage, LspzError> {
        // 实现接收逻辑
        Ok(message)
    }
}
```

2. 添加配置:

```rust
pub enum TransportType {
    Stdio,
    MyCustom,  // 新增
}
```

3. 添加测试和文档。

---

## 报告 Bug

### Bug 报告模板

```markdown
## Bug 描述
<!-- 清晰简洁地描述 bug -->

## 复现步骤
1.
2.
3.

## 期望行为
<!-- 描述你期望发生什么 -->

## 实际行为
<!-- 描述实际发生了什么 -->

## 环境
- lspz 版本:
- Rust 版本:
- 操作系统:
- LSP 服务器:
- LSP 客户端:

## 日志输出
<!-- 如果有，请粘贴相关日志 -->

## 额外信息
<!-- 任何其他相关信息 -->
```

---

## 功能请求

### 功能请求模板

```markdown
## 功能描述
<!-- 清晰描述你想要的功能 -->

## 使用场景
<!-- 描述这个功能的使用场景 -->

## 可能的实现方案
<!-- 如果你有想法，请描述可能的实现方式 -->

## 替代方案
<!-- 描述你考虑过的替代方案 -->

## 额外信息
<!-- 任何其他相关信息 -->
```

---

## 社区准则

### 行为准则

- **尊重**: 尊重不同观点和经验
- **建设性**: 专注于对项目最有用的改进
- **协作**: 协作优于竞争
- **同理心**: 理解他人的处境

### 冲突解决

1. 首先尝试私下解决
2. 如无法解决，联系维护者
3. 维护者保留最终决定权

---

## 获取帮助

### 沟通渠道

- **GitHub Issues**: Bug 报告和功能请求
- **GitHub Discussions**: 技术讨论
- **Discord/Slack**: 实时交流（如有）

### 适合的问题

| 问题类型 | 合适 | 不合适 |
|---------|------|--------|
| Bug 报告 | ✅ | ❌ |
| 功能请求 | ✅ | ❌ |
| 使用帮助 | ✅ | ❌ |
| 代码审查 | ✅ | ❌ |
| 职位咨询 | ❌ | ✅ |

---

## 许可证

贡献的代码将根据项目的开源许可证发布。提交 PR 即表示你同意将代码贡献给 lspz 项目。

---

## 致谢

感谢所有贡献者！你的贡献让 lspz 变得更好。

---

## 参考文档

- [ROADMAP.md](https://github.com/straydragon/lspz/blob/main/ROADMAP.md) - 项目路线图
- [docs/README.md](../introduction.md) - 开发者文档导航
- [docs/guides/coding-conventions.md](coding-conventions.md) - 编码规范
- [docs/guides/testing-guide.md](testing-guide.md) - 测试指南
