//! TOON (Token-Oriented Object Notation) output format.
//!
//! Converts compact LSP message types to TOON — a token-efficient format for LLM consumption.
//! Field names use full self-explanatory words (not abbreviations) to avoid confusing LLM agents.
//!
//! ## Format overview
//!
//! ```toon
//! uri: file:///src/main.rs
//! diagnostics[2]{severity,message,code,range,count}:
//!   warning,unused variable: `x`,unused_variables,10:5-10:15,2
//!   error,cannot find value `y`,E0425,20:0-20:10,1
//! ```
//!
//! See <https://github.com/toon-format/toon> for the TOON specification.

use serde_json::Value;

use super::compact::{CompactDiagnostics, CompactSeverity, decode_ranges};
use crate::error::LspzError;

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Convert compact severity to a full, self-explanatory word.
fn severity_to_str(s: CompactSeverity) -> &'static str {
    match s {
        CompactSeverity::E => "error",
        CompactSeverity::W => "warning",
        CompactSeverity::I => "info",
        CompactSeverity::H => "hint",
    }
}

/// Format a single `[line, char, line, char]` range as `L:C-L:C`.
fn format_single_range(r: &[i64; 4]) -> String {
    format!("{}:{}-{}:{}", r[0], r[1], r[2], r[3])
}

/// Format delta-encoded ranges as space-separated absolute positions.
fn format_ranges(ranges: &[[i64; 4]]) -> String {
    let abs = decode_ranges(ranges);
    abs.iter()
        .map(format_single_range)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Escape a string value for CSV output in TOON tabular format.
///
/// Replaces newlines with `\n` to keep rows single-line.
/// Wraps in double quotes if the value contains comma or double-quote.
fn escape_csv(s: &str) -> String {
    // Replace newlines so each table row stays single-line
    let s = s.replace('\n', "\\n");
    if s.contains(',') || s.contains('"') {
        let escaped = s.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        s
    }
}

// ─── Diagnostics ────────────────────────────────────────────────────────────

/// Convert [`CompactDiagnostics`] to TOON tabular format.
///
/// ```toon
/// uri: file:///src/main.rs
/// diagnostics[2]{severity,message,code,range,count}:
///   warning,unused variable: `x`,unused_variables,10:5-10:15,2
///   error,cannot find value `y`,E0425,20:0-20:10,1
/// ```
pub fn diagnostics_to_toon(diags: &CompactDiagnostics) -> String {
    let mut out = String::new();

    // URI line
    out.push_str(&format!("uri: {}\n", diags.uri));

    // Table header
    let fields = ["severity", "message", "code", "range", "count"];
    out.push_str(&format!(
        "diagnostics[{}]{{{}}}:\n",
        diags.diagnostics.len(),
        fields.join(",")
    ));

    // Table rows
    for d in &diags.diagnostics {
        let severity = severity_to_str(d.s);
        let message = escape_csv(&d.m);
        let code = escape_csv(d.c.as_deref().unwrap_or(""));
        let range = format_ranges(&d.r);
        let count = d.n.max(1);

        out.push_str(&format!(
            "  {},{},{},{},{}\n",
            severity, message, code, range, count
        ));
    }

    out
}

// ─── Completions ────────────────────────────────────────────────────────────

/// Convert compact completion response to TOON tabular format.
///
/// Input is the compact JSON `Value` produced by `CompletionCompressor`.
/// ```toon
/// completions[2]{label,kind,detail,documentation,deprecated}:
///   push,function,fn push(&mut self, value: T),Adds element to back,false
///   pop,function,fn pop(&mut self) -> Option<T>,Removes last element,false
/// ```
pub fn completions_to_toon(value: &Value) -> Result<String, LspzError> {
    let items = value
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LspzError::Protocol("completions missing 'items' array".into()))?;

    let incomplete = value
        .get("incomplete")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut out = String::new();

    // Doc pool lookup for dedup references
    let docs_pool: Vec<String> = value
        .get("docs")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Metadata
    out.push_str(&format!("incomplete: {}\n", incomplete));

    // Table header
    let fields = ["label", "kind", "detail", "documentation", "deprecated"];
    out.push_str(&format!(
        "completions[{}]{{{}}}:\n",
        items.len(),
        fields.join(",")
    ));

    // Table rows
    for item in items {
        let label = item.get("l").and_then(|v| v.as_str()).unwrap_or("");
        let label = if label.contains(',') {
            escape_csv(label)
        } else {
            label.to_string()
        };

        let kind = item
            .get("k")
            .and_then(|v| v.as_str())
            .map(|k| completion_kind_from_char(k))
            .unwrap_or("");

        let detail = item.get("d").and_then(|v| v.as_str()).unwrap_or("");
        let detail = if detail.is_empty() {
            String::new()
        } else {
            escape_csv(detail)
        };

        let doc = resolve_doc(item, &docs_pool);
        let doc = if doc.is_empty() {
            String::new()
        } else {
            escape_csv(&doc)
        };

        let deprecated = item.get("dep").and_then(|v| v.as_bool()).unwrap_or(false);

        out.push_str(&format!(
            "  {},{},{},{},{}\n",
            label, kind, detail, doc, deprecated
        ));
    }

    Ok(out)
}

/// Map compact completion kind char back to full string.
fn completion_kind_from_char(k: &str) -> &'static str {
    match k {
        "T" => "text",
        "M" => "method",
        "F" => "function",
        "C" => "constructor",
        "f" => "field",
        "V" => "variable",
        "c" => "class",
        "I" => "interface",
        "m" => "module",
        "P" => "property",
        "U" => "unit",
        "v" => "value",
        "e" => "enum",
        "K" => "keyword",
        "s" => "snippet",
        "o" => "color",
        "r" => "reference",
        "D" => "folder",
        "n" => "enum_member",
        "q" => "constant",
        "S" => "struct",
        "W" => "event",
        "O" => "operator",
        "t" => "type_parameter",
        _ => "unknown",
    }
}

/// Resolve documentation from inline doc or doc_pool reference.
fn resolve_doc(item: &Value, pool: &[String]) -> String {
    // Direct inline doc
    if let Some(doc) = item.get("doc").and_then(|v| v.as_str()) {
        return doc.to_string();
    }
    // Pool reference
    if let Some(idx) = item.get("doc_id").and_then(|v| v.as_u64())
        && let Some(doc) = pool.get(idx as usize)
    {
        return doc.clone();
    }
    String::new()
}

// ─── Hover ──────────────────────────────────────────────────────────────────

/// Convert compact hover response to TOON object format.
///
/// Input is the compact JSON `Value` produced by `HoverCompressor`.
/// ```toon
/// kind: markdown
/// value: Some description\nwith escaped\nnewlines
/// ```
pub fn hover_to_toon(value: &Value) -> Result<String, LspzError> {
    let mut out = String::new();

    // Extract compact contents
    let contents = value
        .get("c")
        .ok_or_else(|| LspzError::Protocol("hover missing 'c' (contents)".into()))?;

    let kind = contents
        .get("k")
        .and_then(|v| v.as_str())
        .map(hover_kind_to_str)
        .unwrap_or("markdown");

    let value_text = contents.get("v").and_then(|v| v.as_str()).unwrap_or("");
    // Escape newlines for single-line display in TOON
    let value_text_escaped = value_text.replace('\n', "\\n");

    out.push_str(&format!("kind: {}\n", kind));
    out.push_str(&format!("value: {}\n", value_text_escaped));

    // Optional range
    if let Some(range) = value.get("r") {
        let r_str = format_hover_range(range);
        if !r_str.is_empty() {
            out.push_str(&format!("range: {}\n", r_str));
        }
    }

    Ok(out)
}

/// Map compact markup kind char back to full string.
fn hover_kind_to_str(k: &str) -> &'static str {
    match k {
        "m" => "markdown",
        "p" => "plaintext",
        _ => "markdown", // default to markdown for unknown kinds
    }
}

/// Format a compact hover range as `L:C-L:C`.
fn format_hover_range(range: &Value) -> String {
    let s = match range.get("s") {
        Some(s) => s,
        None => return String::new(),
    };
    let e = match range.get("e") {
        Some(e) => e,
        None => return String::new(),
    };
    let sl = s.get("l").and_then(|v| v.as_i64()).unwrap_or(0);
    let sc = s.get("c").and_then(|v| v.as_i64()).unwrap_or(0);
    let el = e.get("l").and_then(|v| v.as_i64()).unwrap_or(0);
    let ec = e.get("c").and_then(|v| v.as_i64()).unwrap_or(0);
    format!("{}:{}-{}:{}", sl, sc, el, ec)
}

// ─── Document Symbols ──────────────────────────────────────────────────────

/// Convert compact document symbol response to TOON tabular format.
///
/// Input is the compact JSON `Value` produced by `DocumentSymbolCompressor`.
///
/// For hierarchical `DocumentSymbol`, children are flattened with
/// parent-prefixed names. For flat `SymbolInformation`, the `container` field
/// is included.
///
/// ```toon
/// symbols[3]{name,kind,range,detail,container}:
///   MyStruct,struct,0:0-10:1,A test struct,
///   MyStruct.field1,field,1:4-1:11,,
///   main,function,12:0-20:1,,
/// ```
pub fn symbols_to_toon(value: &Value) -> Result<String, LspzError> {
    let items = value
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LspzError::Protocol("symbols missing 'items' array".into()))?;

    // Flatten hierarchical symbols into a list with parent-prefixed names
    let flat = flatten_symbols(items, "");

    let mut out = String::new();

    let fields = ["name", "kind", "range", "detail", "container"];
    out.push_str(&format!(
        "symbols[{}]{{{}}}:\n",
        flat.len(),
        fields.join(",")
    ));

    for s in &flat {
        let name = if s.name.contains(',') {
            escape_csv(&s.name)
        } else {
            s.name.clone()
        };
        out.push_str(&format!(
            "  {},{},{},{},{}\n",
            name, s.kind, s.range, s.detail, s.container,
        ));
    }

    Ok(out)
}

/// A flattened symbol entry for tabular output.
struct FlatSymbol {
    name: String,
    kind: String,
    range: String,
    detail: String,
    container: String,
}

/// Recursively flatten hierarchical DocumentSymbol items.
///
/// Children get parent-prefixed names: `"parent.child"`.
fn flatten_symbols(items: &[Value], prefix: &str) -> Vec<FlatSymbol> {
    let mut result = Vec::new();

    for item in items {
        let name = item.get("n").and_then(|v| v.as_str()).unwrap_or("");

        let full_name = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}.{}", prefix, name)
        };

        let kind = item
            .get("k")
            .and_then(|v| v.as_str())
            .map(symbol_kind_from_char)
            .unwrap_or("unknown")
            .to_string();

        let range = item.get("r").map(format_symbol_range).unwrap_or_default();

        let detail = item
            .get("d")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Container (SymbolInformation) or empty
        let container = item
            .get("cn")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        result.push(FlatSymbol {
            name: full_name.clone(),
            kind,
            range,
            detail,
            container,
        });

        // Recurse into children
        if let Some(children) = item.get("c").and_then(|v| v.as_array()) {
            result.extend(flatten_symbols(children, &full_name));
        }
    }

    result
}

/// Map compact symbol kind char back to full string.
fn symbol_kind_from_char(k: &str) -> &'static str {
    match k {
        "F" => "file",
        "M" => "module",
        "N" => "namespace",
        "P" => "package",
        "s" => "class",
        "m" => "method",
        "p" => "property",
        "f" => "field",
        "c" => "constructor",
        "E" => "enum",
        "I" => "interface",
        "u" => "function",
        "V" => "variable",
        "C" => "constant",
        "S" => "string",
        "b" => "number",
        "B" => "boolean",
        "A" => "array",
        "O" => "object",
        "K" => "key",
        "Z" => "null",
        "e" => "enum_member",
        "t" => "struct",
        "v" => "event",
        "r" => "operator",
        "T" => "type_parameter",
        _ => "unknown",
    }
}

/// Format a compact symbol range `{s:{l,c}, e:{l,c}}` to `L:C-L:C`.
fn format_symbol_range(range: &Value) -> String {
    let start = match range.get("s") {
        Some(s) => s,
        None => return String::new(),
    };
    let end = match range.get("e") {
        Some(e) => e,
        None => return String::new(),
    };
    let sl = start.get("l").and_then(|v| v.as_i64()).unwrap_or(0);
    let sc = start.get("c").and_then(|v| v.as_i64()).unwrap_or(0);
    let el = end.get("l").and_then(|v| v.as_i64()).unwrap_or(0);
    let ec = end.get("c").and_then(|v| v.as_i64()).unwrap_or(0);
    format!("{}:{}-{}:{}", sl, sc, el, ec)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── Diagnostics ──────────────────────────────────────────────────────

    fn sample_compact_diagnostics() -> CompactDiagnostics {
        CompactDiagnostics {
            version: 1,
            uri: "file:///test.rs".into(),
            diagnostics: vec![
                super::super::compact::CompactDiagnostic {
                    m: "unused variable: `x`".into(),
                    s: CompactSeverity::W,
                    r: vec![[10, 5, 10, 15], [1, 0, 1, 0]],
                    c: Some("unused_variables".into()),
                    t: Some("U".into()),
                    n: 2,
                },
                super::super::compact::CompactDiagnostic {
                    m: "cannot find value `y` in this scope".into(),
                    s: CompactSeverity::E,
                    r: vec![[20, 0, 20, 10]],
                    c: Some("E0425".into()),
                    t: None,
                    n: 1,
                },
                super::super::compact::CompactDiagnostic {
                    m: "consider using a type alias".into(),
                    s: CompactSeverity::H,
                    r: vec![[30, 0, 30, 15]],
                    c: None,
                    t: None,
                    n: 1,
                },
            ],
        }
    }

    #[test]
    fn test_diagnostics_to_toon() {
        let diags = sample_compact_diagnostics();
        let toon = diagnostics_to_toon(&diags);

        assert!(toon.contains("uri: file:///test.rs"));
        assert!(toon.contains("diagnostics[3]{severity,message,code,range,count}:"));
        assert!(toon.contains("warning"));
        assert!(toon.contains("error"));
        assert!(toon.contains("hint"));
        assert!(toon.contains("unused variable: `x`"));
        assert!(toon.contains("unused_variables"));
        assert!(toon.contains("10:5-10:15"));
        assert!(toon.contains("11:5-11:15"));
        assert!(toon.contains(",2"));
        assert!(toon.contains(",1"));

        eprintln!("\n=== Diagnostics TOON ===\n{}", toon);
    }

    #[test]
    fn test_diagnostics_empty() {
        let diags = CompactDiagnostics {
            version: 1,
            uri: "file:///empty.rs".into(),
            diagnostics: vec![],
        };
        let toon = diagnostics_to_toon(&diags);
        assert!(toon.contains("diagnostics[0]{"));
    }

    // ── Completions ──────────────────────────────────────────────────────

    fn sample_compact_completions() -> Value {
        json!({
            "version": 1,
            "incomplete": false,
            "items": [
                {"l": "push", "k": "F", "d": "fn push(&mut self, value: T)", "doc": "Adds element to back"},
                {"l": "pop", "k": "F", "d": "fn pop(&mut self) -> Option<T>"},
                {"l": "len", "k": "M", "d": "fn len(&self) -> usize", "dep": true},
                {"l": "is_empty", "k": "M", "d": "fn is_empty(&self) -> bool", "dep": true},
            ]
        })
    }

    #[test]
    fn test_completions_to_toon() {
        let value = sample_compact_completions();
        let toon = completions_to_toon(&value).unwrap();

        assert!(toon.contains("incomplete: false"));
        assert!(toon.contains("completions[4]{"));
        assert!(toon.contains("push,function"));
        assert!(toon.contains("fn push(&mut self, value: T)"));
        assert!(toon.contains(",true"));

        eprintln!("\n=== Completions TOON ===\n{}", toon);
    }

    #[test]
    fn test_completions_missing_items() {
        let value = json!({"version": 1});
        assert!(completions_to_toon(&value).is_err());
    }

    // ── Hover ─────────────────────────────────────────────────────────────

    #[test]
    fn test_hover_to_toon() {
        let value = json!({
            "c": {
                "k": "m",
                "v": "```rust\nfn push(&mut self, value: T)\n```\n\nAppends an element."
            }
        });
        let toon = hover_to_toon(&value).unwrap();

        assert!(toon.contains("kind: markdown"));
        assert!(toon.contains("value:"));
        assert!(toon.contains("```rust\\nfn push"));

        eprintln!("\n=== Hover TOON ===\n{}", toon);
    }

    #[test]
    fn test_hover_with_range() {
        let value = json!({
            "c": {"k": "m", "v": "test"},
            "r": {"s": {"l": 0, "c": 5}, "e": {"l": 10, "c": 15}}
        });
        let toon = hover_to_toon(&value).unwrap();
        assert!(toon.contains("range: 0:5-10:15"));

        eprintln!("\n=== Hover+Range TOON ===\n{}", toon);
    }

    #[test]
    fn test_hover_missing_contents() {
        let value = json!({});
        assert!(hover_to_toon(&value).is_err());
    }

    // ── Symbols ───────────────────────────────────────────────────────────

    fn sample_compact_symbols() -> Value {
        json!({
            "version": 1,
            "items": [
                {
                    "n": "MyStruct",
                    "k": "t",
                    "r": {"s": {"l": 0, "c": 0}, "e": {"l": 10, "c": 1}},
                    "d": "A test struct",
                    "c": [
                        {"n": "field1", "k": "f", "r": {"s": {"l": 1, "c": 4}, "e": {"l": 1, "c": 11}}},
                        {"n": "field2", "k": "f", "r": {"s": {"l": 2, "c": 4}, "e": {"l": 2, "c": 11}}}
                    ]
                },
                {
                    "n": "main",
                    "k": "u",
                    "r": {"s": {"l": 12, "c": 0}, "e": {"l": 20, "c": 1}}
                }
            ]
        })
    }

    #[test]
    fn test_symbols_to_toon() {
        let value = sample_compact_symbols();
        let toon = symbols_to_toon(&value).unwrap();

        assert!(toon.contains("symbols[4]{"));
        assert!(toon.contains("MyStruct,struct"));
        assert!(toon.contains("MyStruct.field1,field"));
        assert!(toon.contains("MyStruct.field2,field"));
        assert!(toon.contains("main,function"));
        assert!(toon.contains("A test struct"));

        eprintln!("\n=== Symbols TOON ===\n{}", toon);
    }

    #[test]
    fn test_symbols_empty() {
        let value = json!({"version": 1, "items": []});
        let toon = symbols_to_toon(&value).unwrap();
        assert!(toon.contains("symbols[0]{"));
    }

    #[test]
    fn test_symbols_missing_items() {
        let value = json!({"version": 1});
        assert!(symbols_to_toon(&value).is_err());
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    #[test]
    fn test_escape_csv() {
        assert_eq!(escape_csv("hello"), "hello");
        assert_eq!(escape_csv("he,llo"), r#""he,llo""#);
        assert_eq!(escape_csv(r#"he"llo"#), r#""he""llo""#);
        assert_eq!(escape_csv("he\nllo"), "he\\nllo");
    }

    #[test]
    fn test_severity_mapping() {
        assert_eq!(severity_to_str(CompactSeverity::E), "error");
        assert_eq!(severity_to_str(CompactSeverity::W), "warning");
        assert_eq!(severity_to_str(CompactSeverity::I), "info");
        assert_eq!(severity_to_str(CompactSeverity::H), "hint");
    }
}
