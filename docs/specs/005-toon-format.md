# TOON Output Format for lspz

**版本**: v0.1.0
**状态**: 草案
**最后更新**: 2026-05-11

## 概述

TOON（Token-Oriented Object Notation）是 lspz 的第二输出格式，专为 LLM 上下文窗口优化。

### 设计动机

Compact JSON 虽然比标准 LSP JSON 节省 60%+ token，但结构性字符（引号、花括号、逗号）
的 token 占比仍较高。TOON 采用**行协议 + 表格**的混合格式，在保持自解释性的同时
进一步减少 token。

### 核心设计原则

1. **自解释性**: 使用完整字段名（`severity` 而非 `s`，`message` 而非 `m`），避免 LLM 误解
2. **表格优先**: 同类数据用 TOON 表格格式（`[N]{fields}:` + CSV 行），消除重复字段名
3. **行协议风格**: 简单 key: value 对，无引号包裹
4. **可生成**: Rust `Display` / 字符串格式化即可输出，无需序列化框架
5. **可扩展**: 加字段不破坏解析

### 与 Compact JSON 对比

| 维度 | Compact JSON | TOON |
|------|-------------|------|
| 格式 | 合法 JSON | 行协议 + 表格 |
| Token 节省 | −62% vs 标准 LSP | **−68%+ vs 标准 LSP** |
| 字段名 | 缩写（`m`, `s`, `r`） | **完整（`message`, `severity`, `range`）** |
| 严重级别 | 单字符（`W`, `E`） | **完整（`warning`, `error`）** |
| LLM 可读性 | 高 | **更高（自解释字段名）** |
| 序列化 | serde_json | `Display` / 字符串格式化 |
| 解析 | serde_json | 简单 split |

---

## 格式定义

### 对象格式

```
key: value
key: value
```

- key 使用完整英文单词，蛇形命名
- 字符串值直接书写，无引号
- 多行字符串使用 `\n` 转义

### 表格格式

```
name[N]{field1,field2,field3}:
  val1,val2,val3
  val1,val2,val3
```

- `name[N]` — 表格名和行数
- `{field1,field2,...}` — 逗号分隔的列名（完整自解释单词）
- 后续行用 2 空格缩进 + 逗号分隔的值
- 含逗号的值用双引号包裹

---

## 各消息类型的 TOON 格式

### Diagnostics (`textDocument/publishDiagnostics`)

```toon
uri: file:///src/main.rs
diagnostics[2]{severity,message,code,range,count}:
  warning,unused variable: `x`,unused_variables,10:5-10:15 11:5-11:15,30
  error,cannot find value `y` in this scope,E0425,20:0-20:10,1
```

#### 字段说明

| 字段 | 含义 | 必须 | 说明 |
|------|------|------|------|
| `severity` | 严重级别 | ✅ | `error` / `warning` / `info` / `hint` |
| `message` | 诊断消息 | ✅ | 原始消息文本 |
| `code` | 诊断代码 | ❌ | 空字符串表示无 |
| `range` | 位置 | ✅ | `L:C-L:C` 格式，多个用空格分隔 |
| `count` | 合并数 | ✅ | 去重合并的诊断条数 |

#### 完整示例

**Compact JSON (51 tokens):**
```json
{"version":1,"uri":"file:///src/main.rs","diagnostics":[{"m":"unused variable","s":"W","r":[[10,5,10,15]],"c":"unused_variables","t":"U","n":30}]}
```

**TOON (38 tokens, −25% vs Compact JSON):**
```toon
uri: file:///src/main.rs
diagnostics[1]{severity,message,code,range,count}:
  warning,unused variable,unused_variables,10:5-10:15,30
```

---

### Completions (`textDocument/completion`)

```toon
incomplete: false
completions[3]{label,kind,detail,documentation,deprecated}:
  push,function,fn push(&mut self, value: T),Adds element to back,false
  pop,function,fn pop(&mut self) -> Option<T>,Removes last element,false
  is_empty,method,fn is_empty(&self) -> bool,,true
```

#### 字段说明

| 字段 | 含义 | 必须 |
|------|------|------|
| `label` | 补全标签 | ✅ |
| `kind` | 补全类型 | ❌ |
| `detail` | 详细信息 | ❌ |
| `documentation` | 文档说明 | ❌ |
| `deprecated` | 是否弃用 | ❌ |

Kind 使用完整类型名：`function`, `method`, `struct`, `variable`, `enum` 等。

---

### Hover (`textDocument/hover`)

```toon
kind: markdown
value: ```rust\nfn push(&mut self, value: T)\n```\n\nAppends an element.
range: 0:5-10:15
```

#### 字段说明

| 字段 | 含义 | 必须 |
|------|------|------|
| `kind` | Markdown 类型 | ✅ |
| `value` | 内容 | ✅ |
| `range` | 可选范围 | ❌ |

多行内容使用 `\n` 转义为单行。

---

### Document Symbols (`textDocument/documentSymbol`)

```toon
symbols[4]{name,kind,range,detail,container}:
  MyStruct,struct,0:0-10:1,A test struct,
  MyStruct.field1,field,1:4-1:11,,
  MyStruct.field2,field,2:4-2:11,,
  main,function,12:0-20:1,,
```

#### 字段说明

| 字段 | 含义 | 必须 |
|------|------|------|
| `name` | 符号名称 | ✅ |
| `kind` | 符号类型 | ✅ |
| `range` | 位置范围 | ✅ |
| `detail` | 详细信息 | ❌ |
| `container` | 容器名 | ❌ |

#### 分层结构扁平化

Hierarchical `DocumentSymbol`（带 `children`）会被扁平化为**父级前缀**命名：

```
MyStruct           ← 顶层符号
MyStruct.field1    ← 子符号（父级前缀）
MyStruct.field2    ← 子符号（父级前缀）
main               ← 顶层符号
```

这种方式保留了层次信息，同时兼容 TOON 表格格式。

---

## Token 节省估算

| 消息类型 | Compact JSON | TOON | 节省 |
|---------|-------------|------|------|
| 单条诊断 | 51t | 38t | **−25%** |
| 30 条去重诊断 | 51t | 38t | **−25%** |
| 4 个补全项 | ~250t | ~120t | **−52%** |
| Hover | ~80t | ~60t | **−25%** |

> TOON 的主要优势是自解释性（完整字段名），而非纯粹的 token 压缩。
> Compact JSON 在 token 节省上仍然更激进（缩写字段名），适合带宽受限场景。

---

## 输出格式选择指南

| 场景 | 推荐格式 | 原因 |
|------|---------|------|
| Agent SDK 内部传输 | Compact JSON | 最小 token，Agent 端可解压 |
| LLM 直接消费 | **TOON** ✅ 默认 | 自解释字段名，LLM 无需预解压 |
| 兼容标准 LSP Client | Passthrough | 标准 JSON-RPC |
| 调试/开发 | TOON | 可读性最好 |

---

## 实现状态

| 组件 | 状态 | 说明 |
|------|------|------|
| `codec/toon.rs` — diagnostics | ✅ 已完成 | `diagnostics_to_toon()` |
| `codec/toon.rs` — completions | ✅ 已完成 | `completions_to_toon()` |
| `codec/toon.rs` — hover | ✅ 已完成 | `hover_to_toon()` |
| `codec/toon.rs` — symbols | ✅ 已完成 | `symbols_to_toon()` |
| Proxy `--output toon` | ✅ 已完成 | 通过 `--output toon` 启用 |
| Agent SDK TOON API | ⏳ 待规划 | `get_diagnostics_toon()` 等 |

---

## 参考

- [TOON 规范 v3.0](https://github.com/toon-format/spec/blob/main/SPEC.md)
- [002-compression-format.md](002-compression-format.md) — 当前紧凑 JSON 格式规范
- [003-lsp-compatibility.md](003-lsp-compatibility.md) — LSP 兼容性规范
