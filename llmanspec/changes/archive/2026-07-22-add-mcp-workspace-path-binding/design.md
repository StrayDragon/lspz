# Design: add-mcp-workspace-path-binding

## Workspace 锚点优先级

```text
explicit tool `workspace` param
  → session bind (set_workspace / prior explicit)
  → MCP roots/list (best-effort)
  → daemon/process cwd ONLY if plausible project root
  → else actionable error (no scan)
```

**Trusted sources** (`explicit`, `session`, `mcp_roots`): always accepted for scan / relative resolve.

**Untrusted sources** (`fallback`, `process_cwd`): reject when path is `$HOME`, `/`, or lacks project markers (ROOT_MARKERS + `.git`).

## Path resolution

| Input | Behavior |
|-------|----------|
| `file://...` | use as-is |
| absolute path | normalize → `file://` |
| relative path | require workspace anchor; `join(root, path)` → `file://` |

`uri` / `path` / `paths` 可同时出现，均加入目标列表。目标为空 → workspace scan（仅可信锚点）。单目标 → 单文件 TOON；多目标 → scan 概览 TOON。

## set_workspace

- 参数：`workspace`（绝对路径或 `file://`）
- 规范化后写入 `Arc<Mutex<Option<String>>>` 会话缓存
- 返回 TOON：`workspace` + `workspace_source: session`

## Unknown fields

`#[serde(deny_unknown_fields)]` on all MCP tool input structs.
