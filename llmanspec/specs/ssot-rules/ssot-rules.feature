# language: zh-CN
# capability: ssot-rules
# purpose: SSOT（单一事实源）规则，定义代码即 SSOT 的文档生成和维护规范。
# scope: src/

功能: ssot-rules

  @req:r22
  规则: 文档 SSOT
    系统 MUST 以源码注释（cargo doc）与 llmanspec/specs 为规范 SSOT；用户面向文档为 README.md 与 AGENTS.md，不得引用已删除的 docs/src、CHANGELOG、ROADMAP 或 mdbook 命令。
    场景: no-dead-links
      假如 README 与 AGENTS 文档链接
      当 读者打开文档
      那么 链接指向存在的路径或 docs.rs，不含 CHANGELOG/ROADMAP/docs/src

  @req:r32
  规则: API 文档生成
    系统 MUST 使用 cargo doc 从代码注释自动生成 API 文档。
    场景: cargo-doc-generate
      假如 src/ 中的文档注释
      当 cargo doc 运行
      那么 target/doc/ 中生成文档

  @req:r41
  规则: 规范文档位置
    系统 MUST 将规范文档放在 llmanspec/specs/ 目录。
    场景: specs-in-llmanspec
      假如 规范文档
      当 创建规范
      那么 放在 llmanspec/specs/ 目录

  @req:r50
  规则: 架构图
    系统 MUST NOT 将已删除的 docs/src/diagrams 作为文档构建依赖；架构说明 MUST 以源码模块文档或 llmanspec 为准。
    场景: no-required-mdbook
      假如 仓库不含 docs/src
      当 构建文档
      那么 cargo doc 与 llman specs 仍可用

  @req:r57
  规则: 文档漂移检测
    系统 MUST 在 CI 中检测文档漂移（cargo doc + cargo test --doc）。
    场景: ci-doc-check
      假如 CI 运行
      当 cargo doc --no-deps --all-features
      那么 文档构建成功无错误
    场景: ci-doc-test
      假如 CI 运行
      当 cargo test --doc --all-features
      那么 文档示例编译通过

  @req:r64
  规则: 配置与文档默认值一致
    系统文档与注释 MUST 将默认 OutputFormat 描述为 toon；CI/配置上下文 MUST NOT 依赖已删除的 mdbook/docs/src 构建路径。
    场景: default-toon-docs
      假如 阅读 OutputFormat::Json 文档注释与 llmanspec/config.yaml
      当 核对默认输出格式与文档工具
      那么 默认标明为 toon，且无 mdbook/docs/src 依赖表述
