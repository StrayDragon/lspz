# llmanspec AGENTS.md

此文件由根目录的 `AGENTS.md` 托管块引用。承载对 SDD change 的额外要求与基本认知;
通用项目规范(技术栈、架构、测试、代码风格等)见根 `AGENTS.md` 正文。

项目: lspz - AI-friendly LSP compression proxy。

## Artifact 规则

- proposal: 提案保持在 800 字以内
- proposal: 必须包含"非目标"章节
- proposal: 必须说明对现有架构的影响
- proposal: 涉及公共 API 变更时需说明兼容性
- tasks: 每个任务不超过 2 小时
- tasks: 任务描述需明确涉及的文件和模块
- tasks: 测试任务需说明覆盖率目标
- spec: 技术规格需包含代码示例
- spec: 涉及架构变更需更新 AGENTS.md
- spec: API 变更需同步更新 cargo doc 注释
