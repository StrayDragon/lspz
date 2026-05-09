# lspz 压缩格式规范

**版本**: v0.1.0
**状态**: 草案
**最后更新**: 2026-05-09

## 概述

本文档定义 lspz 使用的紧凑格式（Compact Format），用于压缩 LSP 诊断消息，减少 Token 消耗。

### 目标

- **Token 节省**: 相比原始 JSON 格式节省 ≥ 40% Token
- **语义完整**: 保留所有关键信息（文件、行号、严重级别、消息）
- **可还原**: Agent 端可以还原为标准 LSP Diagnostic 格式
- **可扩展**: 支持版本控制和向后兼容

---

## 压缩策略

### 1. 去重合并 (Deduplication)

**规则**: 同一文件中相同 `message` 且相同 `severity` 的诊断合并为一个条目，`ranges` 平铺。

**示例**:

```json
// 原始: 3 个重复诊断
[
  {"message": "unused variable", "severity": 2, "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 5}}},
  {"message": "unused variable", "severity": 2, "range": {"start": {"line": 2, "character": 0}, "end": {"line": 2, "character": 5}}},
  {"message": "unused variable", "severity": 2, "range": {"start": {"line": 3, "character": 0}, "end": {"line": 3, "character": 5}}}
]

// 压缩后: 1 个诊断，3 个 range
{
  "message": "unused variable",
  "severity": 2,
  "ranges": [[1, 0, 1, 5], [2, 0, 2, 5], [3, 0, 3, 5]]
}
```

### 2. 字段裁剪 (Field Pruning)

**默认保留字段**:
- `message`: 诊断消息
- `severity`: 严重级别
- `range`: 位置范围（压缩为 `[line, char, line, char]`）
- `code`: 诊断代码（可选）

**默认移除字段**:
- `data`: 额外数据
- `codeDescription`: 代码描述
- `relatedInformation`: 相关信息
- `tags`: 标签（压缩后编码）
- `source`: 来源

### 3. 枚举缩减 (Enum Compression)

**Severity 编码**:

| 原始值 | 压缩值 | 说明 |
|--------|--------|------|
| `1` (Error) | `'E'` | 错误 |
| `2` (Warning) | `'W'` | 警告 |
| `3` (Information) | `'I'` | 信息 |
| `4` (Hint) | `'H'` | 提示 |

**Tags 编码** (如有):
- `Unnecessary`: `'U'`
- `Deprecated`: `'D'`

### 4. Range 编码

**原始格式**:
```json
{
  "start": {"line": 10, "character": 5},
  "end": {"line": 10, "character": 15}
}
```

**压缩格式**: `[line_start, char_start, line_end, char_end]`

**多个 Range**: 使用 delta 编码
```json
// 原始: 3 个 range
{
  "ranges": [
    {"start": {"line": 10, "character": 5}, "end": {"line": 10, "character": 15}},
    {"start": {"line": 11, "character": 5}, "end": {"line": 11, "character": 15}},
    {"start": {"line": 12, "character": 5}, "end": {"line": 12, "character": 15}}
  ]
}

// 压缩: delta 编码
// 第一个 range: [10, 5, 10, 15]
// 后续 range: [delta_line, delta_char_start, delta_line_end, delta_char_end]
{
  "ranges": [
    [10, 5, 10, 15],
    [1, 0, 1, 0],   // line +1, char 相同
    [1, 0, 1, 0]    // line +1, char 相同
  ]
}
```

---

## 紧凑格式 Schema

### 顶层结构

```typescript
interface CompactDiagnosticsParams {
  version: 1;           // 格式版本
  uri: string;          // 文档 URI
  diagnostics: CompactDiagnostic[];
}
```

### 单个诊断

```typescript
interface CompactDiagnostic {
  m: string;            // message
  s: 'E' | 'W' | 'I' | 'H';  // severity
  r: number[];          // ranges (delta 编码)

  // 可选字段
  c?: string | number;  // code
  t?: string;           // tags (逗号分隔)
}
```

### Range 格式

```typescript
// 单个 range: [line_start, char_start, line_end, char_end]
type Range = [number, number, number, number];

// 多个 range: delta 编码数组
type Ranges = Range[];
```

---

## 完整示例

### 原始 LSP Diagnostics

```json
{
  "uri": "file:///path/to/file.rs",
  "diagnostics": [
    {
      "range": {
        "start": {"line": 10, "character": 5},
        "end": {"line": 10, "character": 15}
      },
      "severity": 2,
      "message": "unused variable: `x`",
      "code": "unused_variables",
      "source": "rust-analyzer",
      "tags": [1]
    },
    {
      "range": {
        "start": {"line": 11, "character": 5},
        "end": {"line": 11, "character": 15}
      },
      "severity": 2,
      "message": "unused variable: `x`",
      "code": "unused_variables",
      "source": "rust-analyzer",
      "tags": [1]
    },
    {
      "range": {
        "start": {"line": 20, "character": 0},
        "end": {"line": 20, "character": 10}
      },
      "severity": 1,
      "message": "cannot find value `y` in this scope",
      "code": "E0425",
      "source": "rustc"
    }
  ]
}
```

### 紧凑格式

```json
{
  "version": 1,
  "uri": "file:///path/to/file.rs",
  "diagnostics": [
    {
      "m": "unused variable: `x`",
      "s": "W",
      "r": [
        [10, 5, 10, 15],
        [1, 0, 1, 0]
      ],
      "c": "unused_variables",
      "t": "U"
    },
    {
      "m": "cannot find value `y` in this scope",
      "s": "E",
      "r": [
        [20, 0, 20, 10]
      ],
      "c": "E0425"
    }
  ]
}
```

### Token 对比

| 格式 | 估算 Token (GPT-4) | 节省 |
|------|-------------------|------|
| 原始 JSON | ~350 | - |
| 紧凑格式 | ~180 | **48%** |

---

## 版本控制

### 格式版本字段

所有紧凑格式消息必须包含 `version` 字段：

```json
{
  "version": 1,
  // ... 其他字段
}
```

### 版本兼容性

| Agent 端版本 | 服务端版本 | 兼容性 |
|-------------|-----------|--------|
| v1 | v1 | ✅ 完全兼容 |
| v2 | v1 | ⚠️ 可降级 |
| v1 | v2 | ❌ 需升级 |

### 升级策略

当格式更新时：
1. 增加版本号
2. Agent 端和 Proxy 端同时更新
3. 提供兼容层支持旧版本（临时）

---

## Agent 端解压

### Rust 解压库示例

```rust
use lspz_decompress::{decompress_diagnostics, Diagnostic};

fn main() {
    let compact = r#"{
      "version": 1,
      "uri": "file:///path/to/file.rs",
      "diagnostics": [...]
    }"#;

    let diagnostics: Vec<Diagnostic> = decompress_diagnostics(compact)?;

    for diag in diagnostics {
        println!("{}: {}", diag.severity, diag.message);
    }
}
```

### JavaScript 解压库示例

```javascript
import { decompressDiagnostics } from 'lspz-decompress';

const compact = {
  version: 1,
  uri: "file:///path/to/file.rs",
  diagnostics: [...]
};

const diagnostics = decompressDiagnostics(compact);

diagnostics.forEach(diag => {
  console.log(`${diag.severity}: ${diag.message}`);
});
```

---

## 配置选项

### 压缩策略配置

```rust
pub struct CompressionConfig {
    /// 是否启用去重合并
    pub enable_dedup: bool,

    /// 是否启用字段裁剪
    pub enable_field_pruning: bool,

    /// 是否启用枚举缩减
    pub enable_enum_compression: bool,

    /// 是否启用 Range delta 编码
    pub enable_range_delta: bool,

    /// 保留字段列表（当 enable_field_pruning=true 时）
    pub keep_fields: Vec<String>,
}
```

### 默认配置

```rust
impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enable_dedup: true,
            enable_field_pruning: true,
            enable_enum_compression: true,
            enable_range_delta: true,
            keep_fields: vec!["message".into(), "severity".into(), "range".into()],
        }
    }
}
```

---

## 实现参考

### Codec 层接口

```rust
pub trait CompactCodec: Send + Sync {
    /// 压缩诊断
    fn compress_diagnostics(
        &self,
        diagnostics: Vec<lsp_types::Diagnostic>,
    ) -> Result<CompactDiagnostics, LspzError>;

    /// 解压诊断
    fn decompress_diagnostics(
        &self,
        compact: CompactDiagnostics,
    ) -> Result<Vec<lsp_types::Diagnostic>, LspzError>;
}
```

---

## 参考文档

- [LSP Diagnostic 类型](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#diagnostic)
- [plan/01-mvp-phase.md](../plan/01-mvp-phase.md) - MVP 实施计划
- [specs/001-tri-modal-architecture.md](001-tri-modal-architecture.md) - 三模态架构
