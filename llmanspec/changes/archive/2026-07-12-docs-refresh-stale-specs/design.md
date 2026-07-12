# Design: docs-refresh-stale-specs

STALE 判定：相对 baseRef（origin/main），valid_scope 内路径变更且 spec 文件未更新。

策略：用 delta 把已落地行为写入主规格并归档合并；顺带修正 mcp/product 与代码/compression 的文字漂移。
