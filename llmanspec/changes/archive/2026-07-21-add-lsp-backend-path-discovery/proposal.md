# Proposal: add-lsp-backend-path-discovery

## Why

Agent / CI / 精简 PATH 环境下，`uv tool install`、`cargo install`、`go install` 等装到默认布局的语言服务器常不在进程 `PATH` 中。当前 `languages.rs` 仅用 `which`，MCP 后端解析也只得到命令名，导致「已安装却 not installed / spawn 失败」，只能靠用户改环境变量——脆弱且与 dapz 对齐目标冲突。

## What Changes

- 新增共享 `resolve_tool(name) -> Option<PathBuf>`（或等价 API），按固定顺序搜索：PATH → `~/.local/bin` → uv tools / cargo / go（可选 npm/bun）默认布局
- `languages` 可用性检查与 MCP/daemon 后端解析/spawn 改用该解析器
- README 安装节写明推荐 `uv tool install …`，默认布局下无需 export PATH
- 移除备忘文档 `docs/tips/00-tool-path-discovery.md`（需求迁入本 change / 后续主 specs）

## Capabilities

- `tool-path-discovery`（新建，主）
- 间接触及 `mcp` / `languages` 实现落点（行为合约以本 capability delta 为准）

## Impact

- **架构**：在 `src/languages.rs` 旁或新模块抽出路径发现；MCP spawn 可能改为绝对路径。不改变 Transport / Interceptor / fail-open 不变量
- **兼容性**：显式传入的 backend 字符串仍可用；PATH 命中优先，行为对「已在 PATH 上」用户透明。不抽跨仓 crate（与 dapz 暂复制搜索顺序表）
- **公共 API**：库面若导出 `resolve_tool`，属新增能力，不破坏既有签名

## 非目标

- 不实现 shebang → `python -m …` 的 wrapper 展开（可后续变更）
- 不强制与 dapz 共享 crate；不改 daemon 认证 / allowlist
- 不把「任意用户目录递归搜索」或修改用户 shell rc / 全局 PATH
- 不在本变更启用 BDD-on

## Ethics

- `ethics.risk_level`: low
- `ethics.prohibited_actions`: 不得执行不可信路径上的下载/安装；不得静默执行非预期二进制（仅解析存在性与路径）
- `ethics.required_evidence`: 精简 PATH 单测覆盖默认布局；`llman sdd validate add-lsp-backend-path-discovery --strict`；实现阶段 `just qa`
- `ethics.refusal_contract`: 若要求扫描任意用户指定目录执行未知二进制，拒绝并限于文档化的默认布局表
- `ethics.escalation_policy`: 是否纳入 npm/bun 全局 bin、以及 spawn 是否一律改绝对路径有歧义时升级用户确认
