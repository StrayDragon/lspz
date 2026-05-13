//! LSP 诊断的紧凑格式。
//!
//! [MermaidChart:docs/src/diagrams/compression-pipeline.mmd]

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::LspzError;

/// 编码为单个字符的严重程度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CompactSeverity {
    E, // 错误   (LSP severity 1)
    W, // 警告 (LSP severity 2)
    I, // 信息    (LSP severity 3)
    H, // 提示    (LSP severity 4)
}

impl CompactSeverity {
    /// 从 LSP 严重程度数字（1–4）转换。
    pub fn from_lsp(severity: u64) -> Option<Self> {
        match severity {
            1 => Some(Self::E),
            2 => Some(Self::W),
            3 => Some(Self::I),
            4 => Some(Self::H),
            _ => None,
        }
    }

    /// 转换回 LSP 严重程度数字（1–4）。
    pub fn to_lsp(self) -> u64 {
        match self {
            Self::E => 1,
            Self::W => 2,
            Self::I => 3,
            Self::H => 4,
        }
    }
}

/// 单个紧凑诊断条目。
///
/// 多个具有相同（消息、严重程度、代码）的原始诊断
/// 被合并为一个具有多个范围的条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactDiagnostic {
    /// 标准化的消息文本。
    pub m: String,
    /// 单个字符的严重程度。
    pub s: CompactSeverity,
    /// 范围：第一个元素是绝对的，后续的是增量编码的。
    pub r: Vec<[i64; 4]>,
    /// 可选的诊断代码（帮助 AI 分类错误）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<String>,
    /// 可选的标签（逗号分隔："U" / "D" / "U,D"）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<String>,
    /// 合并的诊断数量（默认为 1）。
    #[serde(default, skip_serializing_if = "is_one")]
    pub n: u32,
}

fn is_one(n: &u32) -> bool {
    *n == 1
}

/// 顶层紧凑诊断消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactDiagnostics {
    /// 格式版本（当前为 1）。
    pub version: u32,
    /// 文档 URI。
    pub uri: String,
    /// 紧凑诊断条目。
    pub diagnostics: Vec<CompactDiagnostic>,
}

// ─── Range encoding / decoding ─────────────────────────────────────────────

/// Encode a single LSP range into `[line_start, char_start, line_end, char_end]`.
///
/// Input is the LSP `range` value: `{"start": {"line": L, "character": C}, "end": {"line": L, "character": C}}`.
pub fn encode_range(range: &Value) -> Option<[i64; 4]> {
    let start = range.get("start")?;
    let end = range.get("end")?;
    Some([
        start.get("line")?.as_i64()?,
        start.get("character")?.as_i64()?,
        end.get("line")?.as_i64()?,
        end.get("character")?.as_i64()?,
    ])
}

/// Encode multiple ranges with delta compression.
///
/// First range is absolute, subsequent ranges are deltas from the previous one.
pub fn encode_ranges_with_delta(ranges: Vec<Value>) -> Vec<[i64; 4]> {
    let mut result: Vec<[i64; 4]> = Vec::with_capacity(ranges.len());
    let mut prev = [0i64; 4];

    for (i, range_val) in ranges.into_iter().enumerate() {
        if let Some(abs) = encode_range(&range_val) {
            if i == 0 {
                result.push(abs);
                prev = abs;
            } else {
                let delta = [
                    abs[0] - prev[0],
                    abs[1] - prev[1],
                    abs[2] - prev[2],
                    abs[3] - prev[3],
                ];
                result.push(delta);
                prev = abs;
            }
        }
    }

    result
}

/// Decode a delta-encoded range back to absolute values.
///
/// `prev` is the previous absolute range (used for delta accumulation).
pub fn decode_delta_range(delta: &[i64; 4], prev: &[i64; 4]) -> [i64; 4] {
    [
        prev[0] + delta[0],
        prev[1] + delta[1],
        prev[2] + delta[2],
        prev[3] + delta[3],
    ]
}

/// Reconstruct all absolute ranges from a delta-encoded array.
pub fn decode_ranges(encoded: &[[i64; 4]]) -> Vec<[i64; 4]> {
    let mut result: Vec<[i64; 4]> = Vec::with_capacity(encoded.len());

    for (i, range) in encoded.iter().enumerate() {
        if i == 0 {
            result.push(*range);
        } else {
            let prev = result[i - 1];
            result.push(decode_delta_range(range, &prev));
        }
    }

    result
}

// ─── Tag encoding / decoding ───────────────────────────────────────────────

/// Encode LSP diagnostic tags to a compact single-character format.
///
/// - `Unnecessary` (1) → `"U"`
/// - `Deprecated` (2) → `"D"`
/// - Multiple tags → `"U,D"`
pub fn encode_tags(tags: &[Value]) -> Option<String> {
    if tags.is_empty() {
        return None;
    }
    let encoded: Vec<&str> = tags
        .iter()
        .filter_map(|t| match t.as_u64() {
            Some(1) => Some("U"),
            Some(2) => Some("D"),
            _ => None,
        })
        .collect();
    if encoded.is_empty() {
        None
    } else {
        Some(encoded.join(","))
    }
}

// ─── Compress ───────────────────────────────────────────────────────────────

/// Result of a single compression step.
#[derive(Debug)]
pub struct CompressedEntry {
    pub message: String,
    pub severity: CompactSeverity,
    pub code: Option<String>,
    pub tags: Option<String>,
    pub ranges: Vec<[i64; 4]>,
    pub count: u32,
}

/// Compress LSP diagnostic params into the compact format.
///
/// `params` is the value of a `textDocument/publishDiagnostics` notification:
/// `{"uri": "...", "diagnostics": [...]}`.
pub fn compress(params: &Value) -> Result<Value, LspzError> {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| LspzError::Protocol("missing 'uri' in publishDiagnostics".into()))?
        .to_string();

    let diagnostics = params
        .get("diagnostics")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LspzError::Protocol("missing 'diagnostics' array".into()))?;

    // Step 1: Collect all diagnostics into entries
    let mut entries: Vec<CompressedEntry> = Vec::new();

    for diag in diagnostics {
        let message = diag
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let severity = match diag.get("severity").and_then(|v| v.as_u64()) {
            Some(s) => CompactSeverity::from_lsp(s).unwrap_or(CompactSeverity::W),
            None => CompactSeverity::W, // Default to Warning
        };

        let code = diag.get("code").and_then(|v| v.as_str()).map(String::from);

        let tags = diag
            .get("tags")
            .and_then(|v| v.as_array())
            .and_then(|t| encode_tags(t));

        // Encode range
        let range = diag
            .get("range")
            .and_then(encode_range)
            .unwrap_or([0, 0, 0, 0]);

        entries.push(CompressedEntry {
            message,
            severity,
            code,
            tags,
            ranges: vec![range],
            count: 1,
        });
    }

    // Step 2: Build compact diagnostics
    let compact_diags: Vec<CompactDiagnostic> = entries
        .into_iter()
        .map(|e| {
            let r = if e.ranges.len() > 1 {
                encode_ranges_with_delta(
                    e.ranges
                        .into_iter()
                        .map(|a| {
                            serde_json::json!({
                                "start": {"line": a[0], "character": a[1]},
                                "end": {"line": a[2], "character": a[3]}
                            })
                        })
                        .collect(),
                )
            } else {
                e.ranges
            };

            CompactDiagnostic {
                m: e.message,
                s: e.severity,
                r,
                c: e.code,
                t: e.tags,
                n: e.count,
            }
        })
        .collect();

    let compact = CompactDiagnostics {
        version: 1,
        uri,
        diagnostics: compact_diags,
    };

    Ok(serde_json::to_value(compact)?)
}

// ─── Decompress ─────────────────────────────────────────────────────────────

/// Decompress a `CompactDiagnostics` value back to LSP `publishDiagnostics` params.
pub fn decompress(compact_val: &Value) -> Result<Value, LspzError> {
    let compact: CompactDiagnostics = serde_json::from_value(compact_val.clone())?;

    let mut diagnostics = Vec::new();

    for diag in &compact.diagnostics {
        let absolute_ranges = decode_ranges(&diag.r);
        let count = diag.n.max(1) as usize;

        // If there are more diagnostics (n > ranges.len), distribute ranges round-robin
        for i in 0..count {
            let abs_range = &absolute_ranges[i % absolute_ranges.len()];

            let range_val = serde_json::json!({
                "start": {"line": abs_range[0], "character": abs_range[1]},
                "end": {"line": abs_range[2], "character": abs_range[3]}
            });

            let mut diagnostic = serde_json::json!({
                "range": range_val,
                "severity": diag.s.to_lsp(),
                "message": diag.m,
            });

            if let Some(ref code) = diag.c {
                diagnostic["code"] = serde_json::json!(code);
            }

            diagnostics.push(diagnostic);
        }
    }

    Ok(serde_json::json!({
        "uri": compact.uri,
        "diagnostics": diagnostics,
    }))
}

// ─── Normalised entry (for dedup) ──────────────────────────────────────────

/// A dedup key used during compression grouping.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct NormalisedKey {
    pub message: String,
    pub severity: CompactSeverity,
    pub code: Option<String>,
}

/// Group diagnostics by normalised key for dedup.
pub fn group_by_key(entries: Vec<CompressedEntry>) -> Vec<CompressedEntry> {
    use std::collections::HashMap;

    let mut groups: HashMap<NormalisedKey, CompressedEntry> = HashMap::new();

    for entry in entries {
        let key = NormalisedKey {
            message: entry.message.clone(),
            severity: entry.severity,
            code: entry.code.clone(),
        };

        match groups.get_mut(&key) {
            Some(existing) => {
                existing.ranges.extend(entry.ranges);
                existing.count += entry.count;
            }
            None => {
                groups.insert(key, entry);
            }
        }
    }

    groups.into_values().collect()
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_diagnostics_json() -> Value {
        serde_json::json!({
            "uri": "file:///test.rs",
            "diagnostics": [
                {
                    "range": {"start": {"line": 10, "character": 5}, "end": {"line": 10, "character": 15}},
                    "severity": 2,
                    "message": "unused variable: `x`",
                    "code": "unused_variables",
                    "source": "rust-analyzer",
                    "tags": [1]
                },
                {
                    "range": {"start": {"line": 11, "character": 5}, "end": {"line": 11, "character": 15}},
                    "severity": 2,
                    "message": "unused variable: `x`",
                    "code": "unused_variables",
                    "source": "rust-analyzer",
                    "tags": [1]
                },
                {
                    "range": {"start": {"line": 20, "character": 0}, "end": {"line": 20, "character": 10}},
                    "severity": 1,
                    "message": "cannot find value `y` in this scope",
                    "code": "E0425",
                    "source": "rust-analyzer"
                }
            ]
        })
    }

    #[test]
    fn test_encode_range() {
        let range = serde_json::json!({
            "start": {"line": 10, "character": 5},
            "end": {"line": 10, "character": 15}
        });
        assert_eq!(encode_range(&range).unwrap(), [10, 5, 10, 15]);
    }

    #[test]
    fn test_encode_ranges_with_delta() {
        let ranges = vec![
            serde_json::json!({"start": {"line": 10, "character": 5}, "end": {"line": 10, "character": 15}}),
            serde_json::json!({"start": {"line": 11, "character": 5}, "end": {"line": 11, "character": 15}}),
            serde_json::json!({"start": {"line": 12, "character": 5}, "end": {"line": 12, "character": 15}}),
        ];
        let encoded = encode_ranges_with_delta(ranges);
        assert_eq!(encoded[0], [10, 5, 10, 15]); // absolute
        assert_eq!(encoded[1], [1, 0, 1, 0]); // delta
        assert_eq!(encoded[2], [1, 0, 1, 0]); // delta
    }

    #[test]
    fn test_decode_range_roundtrip() {
        let ranges = vec![
            serde_json::json!({"start": {"line": 10, "character": 5}, "end": {"line": 10, "character": 15}}),
            serde_json::json!({"start": {"line": 15, "character": 0}, "end": {"line": 15, "character": 10}}),
        ];
        let encoded = encode_ranges_with_delta(ranges);
        let decoded = decode_ranges(&encoded);
        assert_eq!(decoded[0], [10, 5, 10, 15]);
        assert_eq!(decoded[1], [15, 0, 15, 10]);
    }

    #[test]
    fn test_severity_roundtrip() {
        assert_eq!(CompactSeverity::from_lsp(1), Some(CompactSeverity::E));
        assert_eq!(CompactSeverity::from_lsp(2), Some(CompactSeverity::W));
        assert_eq!(CompactSeverity::from_lsp(3), Some(CompactSeverity::I));
        assert_eq!(CompactSeverity::from_lsp(4), Some(CompactSeverity::H));
        assert_eq!(CompactSeverity::from_lsp(5), None);

        assert_eq!(CompactSeverity::E.to_lsp(), 1);
        assert_eq!(CompactSeverity::W.to_lsp(), 2);
        assert_eq!(CompactSeverity::I.to_lsp(), 3);
        assert_eq!(CompactSeverity::H.to_lsp(), 4);
    }

    #[test]
    fn test_encode_tags() {
        assert_eq!(encode_tags(&[]), None);
        assert_eq!(encode_tags(&[serde_json::json!(1)]), Some("U".to_string()));
        assert_eq!(encode_tags(&[serde_json::json!(2)]), Some("D".to_string()));
        assert_eq!(
            encode_tags(&[serde_json::json!(1), serde_json::json!(2)]),
            Some("U,D".to_string())
        );
    }

    #[test]
    fn test_compress_basic() {
        let params = sample_diagnostics_json();
        let compact_val = compress(&params).unwrap();

        assert_eq!(compact_val["version"], 1);
        assert_eq!(compact_val["uri"], "file:///test.rs");

        let diags = compact_val["diagnostics"].as_array().unwrap();
        assert!(!diags.is_empty());
    }

    #[test]
    fn test_compress_and_decompress_roundtrip() {
        let params = sample_diagnostics_json();
        let compact_val = compress(&params).unwrap();

        // Verify structure
        let compact: CompactDiagnostics = serde_json::from_value(compact_val.clone()).unwrap();
        assert_eq!(compact.version, 1);
        assert_eq!(compact.uri, "file:///test.rs");

        // Decompress back
        let decompressed = decompress(&compact_val).unwrap();

        // Verify URI preserved
        assert_eq!(decompressed["uri"], "file:///test.rs");
        // Verify diagnostics array exists
        assert!(decompressed["diagnostics"].as_array().unwrap().len() >= 3);
    }

    #[test]
    fn test_group_by_key() {
        let entries = vec![
            CompressedEntry {
                message: "unused var".into(),
                severity: CompactSeverity::W,
                code: Some("W001".into()),
                tags: None,
                ranges: vec![[10, 5, 10, 15]],
                count: 1,
            },
            CompressedEntry {
                message: "unused var".into(),
                severity: CompactSeverity::W,
                code: Some("W001".into()),
                tags: None,
                ranges: vec![[20, 5, 20, 15]],
                count: 1,
            },
            CompressedEntry {
                message: "type error".into(),
                severity: CompactSeverity::E,
                code: None,
                tags: None,
                ranges: vec![[30, 0, 30, 10]],
                count: 1,
            },
        ];

        let grouped = group_by_key(entries);
        assert_eq!(grouped.len(), 2);

        for entry in &grouped {
            if entry.message == "unused var" {
                assert_eq!(entry.count, 2);
                assert_eq!(entry.ranges.len(), 2);
            }
        }
    }

    #[test]
    fn test_missing_uri_errors() {
        let params = serde_json::json!({
            "diagnostics": []
        });
        assert!(compress(&params).is_err());
    }

    #[test]
    fn test_missing_diagnostics_errors() {
        let params = serde_json::json!({
            "uri": "file:///test.rs"
        });
        assert!(compress(&params).is_err());
    }
}
