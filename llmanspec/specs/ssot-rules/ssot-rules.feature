# language: en
# capability: ssot-rules
# purpose: SSOT（单一事实源）规则，定义代码即 SSOT 的文档生成和维护规范。
# scope: src/

Feature: ssot-rules

  @req:r22 @human
  Scenario: 文档 SSOT
    - 系统 MUST 以源码注释（cargo doc）与 llmanspec/specs 为规范 SSOT；用户面向文档为 README.md 与 AGENTS.md，不得引用已删除的 docs/src、CHANGELOG、ROADMAP 或 mdbook 命令。

  @req:r32 @human
  Scenario: API 文档生成
    - 系统 MUST 使用 cargo doc 从代码注释自动生成 API 文档。

  @req:r41 @human
  Scenario: 规范文档位置
    - 系统 MUST 将规范文档放在 llmanspec/specs/ 目录。

  @req:r50 @human
  Scenario: 架构图
    - 系统 MUST NOT 将已删除的 docs/src/diagrams 作为文档构建依赖；架构说明 MUST 以源码模块文档或 llmanspec 为准。

  @req:r57 @human
  Scenario: 文档漂移检测
    - 系统 MUST 在 CI 中检测文档漂移（cargo doc + cargo test --doc）。

  @req:r64 @human
  Scenario: 配置与文档默认值一致
    - 系统文档与注释 MUST 将默认 OutputFormat 描述为 toon；CI/配置上下文 MUST NOT 依赖已删除的 mdbook/docs/src 构建路径。

  @req:r22 @human
  Scenario: no-dead-links
    - MUST hold: Given README 与 AGENTS 文档链接; When 读者打开文档; Then 链接指向存在的路径或 docs.rs，不含 CHANGELOG/ROADMAP/docs/src.
    Given README 与 AGENTS 文档链接
    When 读者打开文档
    Then 链接指向存在的路径或 docs.rs，不含 CHANGELOG/ROADMAP/docs/src

  @req:r32 @human
  Scenario: cargo-doc-generate
    - MUST hold: Given src/ 中的文档注释; When cargo doc 运行; Then target/doc/ 中生成文档.
    Given src/ 中的文档注释
    When cargo doc 运行
    Then target/doc/ 中生成文档

  @req:r41 @human
  Scenario: specs-in-llmanspec
    - MUST hold: Given 规范文档; When 创建规范; Then 放在 llmanspec/specs/ 目录.
    Given 规范文档
    When 创建规范
    Then 放在 llmanspec/specs/ 目录

  @req:r50 @human
  Scenario: no-required-mdbook
    - MUST hold: Given 仓库不含 docs/src; When 构建文档; Then cargo doc 与 llman specs 仍可用.
    Given 仓库不含 docs/src
    When 构建文档
    Then cargo doc 与 llman specs 仍可用

  @req:r57 @human
  Scenario: ci-doc-check
    - MUST hold: Given CI 运行; When cargo doc --no-deps --all-features; Then 文档构建成功无错误.
    Given CI 运行
    When cargo doc --no-deps --all-features
    Then 文档构建成功无错误

  @req:r57 @human
  Scenario: ci-doc-test
    - MUST hold: Given CI 运行; When cargo test --doc --all-features; Then 文档示例编译通过.
    Given CI 运行
    When cargo test --doc --all-features
    Then 文档示例编译通过

  @req:r64 @human
  Scenario: default-toon-docs
    - MUST hold: Given 阅读 OutputFormat::Json 文档注释与 llmanspec/config.yaml; When 核对默认输出格式与文档工具; Then 默认标明为 toon，且无 mdbook/docs/src 依赖表述.
    Given 阅读 OutputFormat::Json 文档注释与 llmanspec/config.yaml
    When 核对默认输出格式与文档工具
    Then 默认标明为 toon，且无 mdbook/docs/src 依赖表述
