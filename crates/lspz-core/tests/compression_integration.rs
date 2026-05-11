//! Integration tests for the diagnostic compression pipeline.
//!
//! Tests use realistic diagnostic data from multiple LSP servers:
//! - gopls (Go): unused variables, unused imports, type mismatches
//! - rust-analyzer (Rust): unused variables, type errors, unresolved references
//! - basedpyright (Python): type errors, unused imports, unused variables
//! - typescript-language-server (TypeScript): type mismatches, unused locals
//!
//! [MermaidChart:./docs/mmd/compression-pipeline.mmd]

use lspz_core::codec::compact::{CompactDiagnostics, compress, decompress};
use lspz_core::codec::toon;
use lspz_core::interceptors::diagnostics::DiagnosticsCompressor;
use lspz_core::interceptors::{Direction, Interceptor};

/// Generate gopls-style diagnostics with 9 unused variables (same code, different names),
/// 2 unused imports, 1 type mismatch, and 1 unused parameter.
fn gopls_diagnostics() -> serde_json::Value {
    let mut diags = Vec::new();
    let unused_code = "UnusedVar";
    let import_code = "UnusedImport";

    // 9 unused variables — same message pattern, different variable names
    let var_names = [
        "unusedVar",
        "anotherVar",
        "x",
        "temp",
        "result",
        "count",
        "index",
        "value",
        "data",
    ];
    for (i, name) in var_names.iter().enumerate() {
        diags.push(serde_json::json!({
            "range": {
                "start": {"line": i * 2, "character": 5},
                "end": {"line": i * 2, "character": 5 + name.len() as u64}
            },
            "severity": 2,
            "message": format!("declared and not used: {}", name),
            "code": unused_code,
            "source": "gopls",
        }));
    }

    // 2 unused imports
    let imports = ["os", "fmt"];
    for (i, pkg) in imports.iter().enumerate() {
        diags.push(serde_json::json!({
            "range": {
                "start": {"line": 20 + i, "character": 0},
                "end": {"line": 20 + i, "character": 10}
            },
            "severity": 2,
            "message": format!("\"{}\" imported and not used", pkg),
            "code": import_code,
            "source": "gopls",
        }));
    }

    // 1 type mismatch
    diags.push(serde_json::json!({
        "range": {
            "start": {"line": 30, "character": 5},
            "end": {"line": 30, "character": 15}
        },
        "severity": 1,
        "message": "cannot use str (type string) as type int in assignment",
        "code": "Assignment",
        "source": "gopls",
    }));

    // 1 unused parameter
    diags.push(serde_json::json!({
        "range": {
            "start": {"line": 35, "character": 10},
            "end": {"line": 35, "character": 20}
        },
        "severity": 2,
        "message": "param paramName is unused",
        "code": "unusedparam",
        "source": "gopls",
    }));

    serde_json::json!({
        "uri": "file:///test/main.go",
        "diagnostics": diags,
    })
}

/// Generate rust-analyzer-style diagnostics.
fn rust_analyzer_diagnostics() -> serde_json::Value {
    serde_json::json!({
        "uri": "file:///test/src/main.rs",
        "diagnostics": [
            {
                "range": {"start": {"line": 2, "character": 8}, "end": {"line": 2, "character": 13}},
                "severity": 2,
                "message": "unused variable: `x`",
                "code": "unused_variables",
                "source": "rust-analyzer"
            },
            {
                "range": {"start": {"line": 3, "character": 8}, "end": {"line": 3, "character": 13}},
                "severity": 2,
                "message": "unused variable: `y`",
                "code": "unused_variables",
                "source": "rust-analyzer"
            },
            {
                "range": {"start": {"line": 5, "character": 13}, "end": {"line": 5, "character": 22}},
                "severity": 1,
                "message": "cannot find value `unknown_fn` in this scope",
                "code": "E0425",
                "source": "rust-analyzer"
            }
        ]
    })
}

// ─── Integration Tests ────────────────────────────────────────────────────

#[test]
fn test_gopls_compression_roundtrip() {
    let params = gopls_diagnostics();
    let original_count = params["diagnostics"].as_array().unwrap().len();

    // Compress
    let compact_val = compress(&params).expect("gopls compression should succeed");
    let compact: CompactDiagnostics =
        serde_json::from_value(compact_val.clone()).expect("should deserialize");

    // Verify all original diagnostics are preserved (decompressed count ≥ original)
    let decompressed = decompress(&compact_val).expect("decompression should succeed");
    let decompressed_count = decompressed["diagnostics"].as_array().unwrap().len();
    assert!(
        decompressed_count >= original_count,
        "decompressed count {} should be >= original {}",
        decompressed_count,
        original_count
    );

    // Verify URI preserved
    assert_eq!(compact.uri, "file:///test/main.go");
    assert_eq!(compact.version, 1);
}

#[tokio::test]
async fn test_gopls_dedup_with_normalization() {
    let compressor = DiagnosticsCompressor::default();
    let params = gopls_diagnostics();

    let result = compressor
        .intercept(
            "textDocument/publishDiagnostics",
            params,
            Direction::ServerToClient,
        )
        .await
        .expect("compression should succeed");

    let compact = result.expect("should produce output");
    let diags = compact["diagnostics"].as_array().unwrap();

    // 9 unused vars → 1 group ("unused variable"), 2 unused imports → 1 group,
    // 1 type mismatch, 1 unused param = 4 groups total
    assert_eq!(
        diags.len(),
        4,
        "gopls 13 diags → 4 groups with normalization"
    );

    // Verify the "unused variable" group has count 9
    let unused_var = diags.iter().find(|d| d["m"] == "unused variable").unwrap();
    assert_eq!(unused_var["n"], 9, "unused variable count should be 9");
    assert_eq!(unused_var["s"], "W");
}

#[test]
fn test_rust_analyzer_compression_roundtrip() {
    let params = rust_analyzer_diagnostics();
    let original_count = params["diagnostics"].as_array().unwrap().len();

    let compact_val = compress(&params).expect("rust-analyzer compression should succeed");
    let decompressed = decompress(&compact_val).expect("decompression should succeed");

    let decompressed_count = decompressed["diagnostics"].as_array().unwrap().len();
    assert!(
        decompressed_count >= original_count,
        "rust-analyzer: decompressed {} >= original {}",
        decompressed_count,
        original_count
    );

    assert_eq!(decompressed["uri"], "file:///test/src/main.rs");
}

#[tokio::test]
async fn test_rust_analyzer_dedup() {
    let compressor = DiagnosticsCompressor::default();
    let params = rust_analyzer_diagnostics();

    let result = compressor
        .intercept(
            "textDocument/publishDiagnostics",
            params,
            Direction::ServerToClient,
        )
        .await
        .expect("compression should succeed");

    let compact = result.expect("should produce output");
    let diags = compact["diagnostics"].as_array().unwrap();

    // 2 unused vars → 1 group, 1 type error → 1 group = 2 groups
    assert_eq!(diags.len(), 2, "rust-analyzer 3 diags → 2 groups");
}

// ─── basedpyright (Python) ──────────────────────────────────────────────

fn basedpyright_diagnostics() -> serde_json::Value {
    serde_json::json!({
        "uri": "file:///test/src/main.py",
        "diagnostics": [
            {
                "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 10}},
                "severity": 1,
                "message": "Argument of type 'int' is not assignable to parameter of type 'str'",
                "code": "reportGeneralTypeIssues",
                "source": "basedpyright"
            },
            {
                "range": {"start": {"line": 3, "character": 0}, "end": {"line": 3, "character": 14}},
                "severity": 2,
                "message": "\"os\" is not accessed",
                "code": "reportUnusedImport",
                "source": "basedpyright"
            },
            {
                "range": {"start": {"line": 4, "character": 0}, "end": {"line": 4, "character": 16}},
                "severity": 2,
                "message": "\"typing\" is not accessed",
                "code": "reportUnusedImport",
                "source": "basedpyright"
            },
            {
                "range": {"start": {"line": 6, "character": 4}, "end": {"line": 6, "character": 10}},
                "severity": 2,
                "message": "Variable \"result\" is not used",
                "code": "reportUnusedVariable",
                "source": "basedpyright"
            },
            {
                "range": {"start": {"line": 7, "character": 4}, "end": {"line": 7, "character": 8}},
                "severity": 2,
                "message": "Variable \"temp\" is not used",
                "code": "reportUnusedVariable",
                "source": "basedpyright"
            },
            {
                "range": {"start": {"line": 9, "character": 0}, "end": {"line": 9, "character": 1}},
                "severity": 1,
                "message": "Name \"undefined_var\" is not defined",
                "code": "reportUndefinedVariable",
                "source": "basedpyright"
            },
        ]
    })
}

#[test]
fn test_basedpyright_compression_roundtrip() {
    let params = basedpyright_diagnostics();
    let original_count = params["diagnostics"].as_array().unwrap().len();

    let compact_val = compress(&params).expect("basedpyright compression should succeed");
    let compact: CompactDiagnostics =
        serde_json::from_value(compact_val.clone()).expect("should deserialize");

    let decompressed = decompress(&compact_val).expect("decompression should succeed");
    let decompressed_count = decompressed["diagnostics"].as_array().unwrap().len();
    assert!(
        decompressed_count >= original_count,
        "basedpyright: decompressed {} >= original {}",
        decompressed_count,
        original_count
    );

    assert_eq!(compact.uri, "file:///test/src/main.py");
    assert_eq!(compact.version, 1);
}

#[tokio::test]
async fn test_basedpyright_dedup() {
    let compressor = DiagnosticsCompressor::default();
    let params = basedpyright_diagnostics();

    let result = compressor
        .intercept(
            "textDocument/publishDiagnostics",
            params,
            Direction::ServerToClient,
        )
        .await
        .expect("compression should succeed");

    let compact = result.expect("should produce output");
    let diags = compact["diagnostics"].as_array().unwrap();

    // basedpyright: 6 diags → 4 groups
    //   (type issue + 2× unused import + 2× unused variable + undefined variable)
    assert_eq!(
        diags.len(),
        4,
        "basedpyright: expected 4 groups, got {}",
        diags.len()
    );
}

// ─── typescript-language-server (TypeScript) ────────────────────────────

fn typescript_diagnostics() -> serde_json::Value {
    serde_json::json!({
        "uri": "file:///test/src/app.ts",
        "diagnostics": [
            {
                "range": {"start": {"line": 1, "character": 7}, "end": {"line": 1, "character": 12}},
                "severity": 1,
                "message": "Type 'number' is not assignable to type 'string'.",
                "code": 2322,
                "source": "ts"
            },
            {
                "range": {"start": {"line": 3, "character": 7}, "end": {"line": 3, "character": 16}},
                "severity": 1,
                "message": "Type 'null' is not assignable to type 'string'.",
                "code": 2322,
                "source": "ts"
            },
            {
                "range": {"start": {"line": 5, "character": 10}, "end": {"line": 5, "character": 14}},
                "severity": 2,
                "message": "'temp' is declared but its value is never read.",
                "code": 6133,
                "source": "ts"
            },
            {
                "range": {"start": {"line": 6, "character": 10}, "end": {"line": 6, "character": 15}},
                "severity": 2,
                "message": "'count' is declared but its value is never read.",
                "code": 6133,
                "source": "ts"
            },
            {
                "range": {"start": {"line": 8, "character": 1}, "end": {"line": 8, "character": 6}},
                "severity": 1,
                "message": "Property 'nonExistent' does not exist on type 'MyType'.",
                "code": 2339,
                "source": "ts"
            },
            {
                "range": {"start": {"line": 10, "character": 0}, "end": {"line": 10, "character": 17}},
                "severity": 1,
                "message": "Cannot find name 'unknownFn'. Did you mean 'knownFn'?",
                "code": 2552,
                "source": "ts"
            },
        ]
    })
}

#[test]
fn test_typescript_compression_roundtrip() {
    let params = typescript_diagnostics();
    let original_count = params["diagnostics"].as_array().unwrap().len();

    let compact_val = compress(&params).expect("typescript compression should succeed");
    let compact: CompactDiagnostics =
        serde_json::from_value(compact_val.clone()).expect("should deserialize");

    let decompressed = decompress(&compact_val).expect("decompression should succeed");
    let decompressed_count = decompressed["diagnostics"].as_array().unwrap().len();
    assert!(
        decompressed_count >= original_count,
        "typescript: decompressed {} >= original {}",
        decompressed_count,
        original_count
    );

    assert_eq!(compact.uri, "file:///test/src/app.ts");
    assert_eq!(compact.version, 1);
}

#[tokio::test]
async fn test_typescript_dedup() {
    let compressor = DiagnosticsCompressor::default();
    let params = typescript_diagnostics();

    let result = compressor
        .intercept(
            "textDocument/publishDiagnostics",
            params,
            Direction::ServerToClient,
        )
        .await
        .expect("compression should succeed");

    let compact = result.expect("should produce output");
    let diags = compact["diagnostics"].as_array().unwrap();

    // TypeScript: 6 diags → 4 groups
    //   (2× type mismatch + 2× unused variable + missing property + unresolved reference)
    assert_eq!(
        diags.len(),
        4,
        "typescript: expected 4 groups, got {}",
        diags.len()
    );
}

// ─── Token Savings ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_token_savings_gopls() {
    use tiktoken_rs::cl100k_base;

    let bpe = cl100k_base().expect("should load tokenizer");

    let params = gopls_diagnostics();
    let original_json = serde_json::to_string(&params).expect("should serialize");

    // Baseline compression (no normalization)
    let compact_basic = compress(&params).expect("should compress");
    let compact_basic_json = serde_json::to_string(&compact_basic).expect("should serialize");

    // With normalization + dedup via interceptor
    let compressor = DiagnosticsCompressor::default();
    let compact_norm = compressor
        .intercept(
            "textDocument/publishDiagnostics",
            params,
            Direction::ServerToClient,
        )
        .await
        .expect("should compress");
    let compact_norm_json = serde_json::to_string(&compact_norm.expect("should produce output"))
        .expect("should serialize");

    let original_tokens = bpe.encode_with_special_tokens(&original_json).len();
    let basic_tokens = bpe.encode_with_special_tokens(&compact_basic_json).len();
    let norm_tokens = bpe.encode_with_special_tokens(&compact_norm_json).len();

    let basic_savings = 1.0 - (basic_tokens as f64 / original_tokens as f64);
    let norm_savings = 1.0 - (norm_tokens as f64 / original_tokens as f64);

    println!("─── gopls Token Savings ───");
    println!("Original tokens:        {}", original_tokens);
    println!(
        "Basic compression:      {} ({:.1}% savings)",
        basic_tokens,
        basic_savings * 100.0
    );
    println!(
        "With normalization:     {} ({:.1}% savings)",
        norm_tokens,
        norm_savings * 100.0
    );

    // In CI with various Rust editions, these thresholds are generous
    // The real gopls capture showed 65.7% basic, 85.0% with normalization
    assert!(
        basic_savings >= 0.30,
        "Basic compression should save ≥30%, got {:.1}%",
        basic_savings * 100.0
    );
    assert!(
        norm_savings >= 0.60,
        "Normalization + dedup should save ≥60%, got {:.1}%",
        norm_savings * 100.0
    );
}

#[tokio::test]
async fn test_token_savings_rust_analyzer() {
    use tiktoken_rs::cl100k_base;

    let bpe = cl100k_base().expect("should load tokenizer");

    let params = rust_analyzer_diagnostics();
    let original_json = serde_json::to_string(&params).expect("should serialize");

    let compressor = DiagnosticsCompressor::default();
    let compact = compressor
        .intercept(
            "textDocument/publishDiagnostics",
            params,
            Direction::ServerToClient,
        )
        .await
        .expect("should compress");
    let compact_json =
        serde_json::to_string(&compact.expect("should produce output")).expect("should serialize");

    let original_tokens = bpe.encode_with_special_tokens(&original_json).len();
    let compressed_tokens = bpe.encode_with_special_tokens(&compact_json).len();
    let savings = 1.0 - (compressed_tokens as f64 / original_tokens as f64);

    println!("─── rust-analyzer Token Savings ───");
    println!("Original tokens:    {}", original_tokens);
    println!("Compressed tokens:  {}", compressed_tokens);
    println!("Savings:           {:.1}%", savings * 100.0);

    assert!(
        savings >= 0.40,
        "rust-analyzer compression should save ≥40%, got {:.1}%",
        savings * 100.0
    );
}

/// Verify that disabling normalization reduces dedup effectiveness.
#[tokio::test]
async fn test_normalization_increases_dedup() {
    let with_norm = DiagnosticsCompressor::default();
    let without_norm = DiagnosticsCompressor {
        enable_normalisation: false,
    };

    let params = gopls_diagnostics();

    let with_result = with_norm
        .intercept(
            "textDocument/publishDiagnostics",
            params.clone(),
            Direction::ServerToClient,
        )
        .await
        .expect("should compress");
    let without_result = without_norm
        .intercept(
            "textDocument/publishDiagnostics",
            params,
            Direction::ServerToClient,
        )
        .await
        .expect("should compress");

    let with_groups = with_result.unwrap()["diagnostics"]
        .as_array()
        .unwrap()
        .len();
    let without_groups = without_result.unwrap()["diagnostics"]
        .as_array()
        .unwrap()
        .len();

    assert!(
        with_groups < without_groups,
        "Normalization should reduce groups: {} < {}",
        with_groups,
        without_groups
    );
}

#[tokio::test]
async fn test_token_savings_basedpyright() {
    use tiktoken_rs::cl100k_base;

    let bpe = cl100k_base().expect("should load tokenizer");
    let params = basedpyright_diagnostics();
    let original_json = serde_json::to_string(&params).expect("should serialize");

    let compressor = DiagnosticsCompressor::default();
    let compact = compressor
        .intercept(
            "textDocument/publishDiagnostics",
            params,
            Direction::ServerToClient,
        )
        .await
        .expect("should compress");
    let compact_json =
        serde_json::to_string(&compact.expect("should produce output")).expect("should serialize");

    let original_tokens = bpe.encode_with_special_tokens(&original_json).len();
    let compressed_tokens = bpe.encode_with_special_tokens(&compact_json).len();
    let savings = 1.0 - (compressed_tokens as f64 / original_tokens as f64);

    println!("─── basedpyright Token Savings ───");
    println!("Original tokens:      {}", original_tokens);
    println!("Compressed tokens:    {}", compressed_tokens);
    println!("Savings:             {:.1}%", savings * 100.0);

    assert!(
        savings >= 0.30,
        "basedpyright compression should save ≥30%, got {:.1}%",
        savings * 100.0
    );
}

#[tokio::test]
async fn test_token_savings_typescript() {
    use tiktoken_rs::cl100k_base;

    let bpe = cl100k_base().expect("should load tokenizer");
    let params = typescript_diagnostics();
    let original_json = serde_json::to_string(&params).expect("should serialize");

    let compressor = DiagnosticsCompressor::default();
    let compact = compressor
        .intercept(
            "textDocument/publishDiagnostics",
            params,
            Direction::ServerToClient,
        )
        .await
        .expect("should compress");
    let compact_json =
        serde_json::to_string(&compact.expect("should produce output")).expect("should serialize");

    let original_tokens = bpe.encode_with_special_tokens(&original_json).len();
    let compressed_tokens = bpe.encode_with_special_tokens(&compact_json).len();
    let savings = 1.0 - (compressed_tokens as f64 / original_tokens as f64);

    println!("─── typescript Token Savings ───");
    println!("Original tokens:      {}", original_tokens);
    println!("Compressed tokens:    {}", compressed_tokens);
    println!("Savings:             {:.1}%", savings * 100.0);

    assert!(
        savings >= 0.30,
        "typescript compression should save ≥30%, got {:.1}%",
        savings * 100.0
    );
}

// ─── TOON Integration Tests ──────────────────────────────────────────────

/// Helper: compress diagnostics via the real pipeline, then convert to TOON.
async fn compress_and_toon(params: serde_json::Value) -> String {
    let compressor = DiagnosticsCompressor::default();
    let compact = compressor
        .intercept(
            "textDocument/publishDiagnostics",
            params,
            Direction::ServerToClient,
        )
        .await
        .expect("compression should succeed")
        .expect("should produce output");

    let typed: CompactDiagnostics =
        serde_json::from_value(compact).expect("should deserialize to CompactDiagnostics");
    toon::diagnostics_to_toon(&typed)
}

/// Verify TOON output uses self-explanatory full field names (no abbreviations).
#[tokio::test]
async fn test_toon_uses_self_explanatory_field_names() {
    let toon = compress_and_toon(gopls_diagnostics()).await;

    // Must use full severity words, not single chars
    assert!(
        toon.contains("warning") || toon.contains("error"),
        "TOON should contain 'warning' or 'error', got: {}",
        toon
    );
    // Must NOT use compact severity chars as field values
    // (they can appear in range numbers, so be specific)
    let lines: Vec<&str> = toon.lines().collect();
    for line in &lines {
        if line.starts_with("  ") {
            // Table row: first field is severity
            let severity = line.split(',').next().unwrap_or("").trim();
            assert!(
                severity == "warning"
                    || severity == "error"
                    || severity == "info"
                    || severity == "hint",
                "TOON severity should be full word, got: '{:?}'",
                severity
            );
        }
    }

    // Must have table header with full field names
    assert!(
        toon.contains("severity,message,code,range,count"),
        "TOON table header should use full field names"
    );
}

/// Verify TOON output does NOT contain compact abbreviations (m:, s:, r:, c:).
#[tokio::test]
async fn test_toon_no_compact_abbreviations() {
    let toon = compress_and_toon(gopls_diagnostics()).await;

    // All JSON compact keys start new objects; TOON is line-based.
    // These patterns should NOT appear in TOON output.
    assert!(
        !toon.contains("\"m\":"),
        "TOON should not contain compact 'm:' field"
    );
    assert!(
        !toon.contains("\"s\":"),
        "TOON should not contain compact 's:' field"
    );
    assert!(
        !toon.contains("\"r\":"),
        "TOON should not contain compact 'r:' field"
    );
    assert!(
        !toon.contains("\"c\":"),
        "TOON should not contain compact 'c:' field"
    );
}

/// TOON conversion with gopls data.
#[tokio::test]
async fn test_toon_gopls() {
    let toon = compress_and_toon(gopls_diagnostics()).await;

    // URI present
    assert!(toon.contains("uri: file:///test/main.go"));

    // Count in header indicates dedup worked
    assert!(
        toon.contains("diagnostics[4]{"),
        "gopls 13 diags → 4 groups, header: {:?}",
        toon.lines().find(|l| l.starts_with("diagnostics"))
    );

    // Full severity words
    assert!(toon.contains("warning"), "should have warning entries");

    eprintln!("\n=== gopls TOON ===\n{}", toon);
}

/// TOON conversion with rust-analyzer data.
#[tokio::test]
async fn test_toon_rust_analyzer() {
    let toon = compress_and_toon(rust_analyzer_diagnostics()).await;

    assert!(toon.contains("uri: file:///test/src/main.rs"));
    assert!(
        toon.contains("diagnostics[2]{"),
        "rust-analyzer 3 diags → 2 groups, header: {:?}",
        toon.lines().find(|l| l.starts_with("diagnostics"))
    );
    assert!(toon.contains("unused variable"));
    assert!(toon.contains("cannot find value"));

    eprintln!("\n=== rust-analyzer TOON ===\n{}", toon);
}

/// TOON conversion with basedpyright data.
#[tokio::test]
async fn test_toon_basedpyright() {
    let toon = compress_and_toon(basedpyright_diagnostics()).await;

    assert!(toon.contains("uri: file:///test/src/main.py"));
    assert!(toon.contains("diagnostics[4]{"));
    // Normalized messages (code-based normalization matches)
    assert!(toon.contains("warning,unused import") || toon.contains("warning,unused variable"));
    // Non-normalized messages (codes not matched by normalizer)
    assert!(toon.contains("not assignable"));

    eprintln!("\n=== basedpyright TOON ===\n{}", toon);
}

/// TOON conversion with TypeScript data.
#[tokio::test]
async fn test_toon_typescript() {
    let toon = compress_and_toon(typescript_diagnostics()).await;

    assert!(toon.contains("uri: file:///test/src/app.ts"));
    assert!(toon.contains("diagnostics[4]{"));
    // All TypeScript messages are code-normalized (2322→"type mismatch", 6133→"unused variable",
    // 2339→"missing property", 2552→"unresolved reference")
    assert!(toon.contains("type mismatch"));
    assert!(toon.contains("unused variable"));
    assert!(toon.contains("missing property"));
    assert!(toon.contains("unresolved reference"));

    eprintln!("\n=== TypeScript TOON ===\n{}", toon);
}

/// TOON format uses L:C-L:C range syntax.
#[tokio::test]
async fn test_toon_range_format() {
    let toon = compress_and_toon(rust_analyzer_diagnostics()).await;

    // Ranges should use L:C-L:C format (not JSON arrays or [l,c,l,c])
    for line in toon.lines() {
        if line.contains("warning") || line.contains("error") {
            // Each row: severity,message,code,range,count
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 4 {
                let range = parts[3].trim();
                // Range should match pattern like "2:8-2:13" or "5:13-5:22"
                assert!(
                    range.contains(':') && range.contains('-'),
                    "Range should be L:C-L:C, got: '{}'",
                    range
                );
            }
        }
    }
}

/// Empty diagnostics produce valid TOON with zero entries.
#[tokio::test]
async fn test_toon_empty_diagnostics() {
    let params = serde_json::json!({
        "uri": "file:///empty.rs",
        "diagnostics": []
    });
    let toon = compress_and_toon(params).await;
    assert!(toon.contains("diagnostics[0]{"));
    assert!(toon.contains("uri: file:///empty.rs"));
}

/// TOON token savings compared to Compact JSON.
#[tokio::test]
async fn test_toon_token_savings() {
    use tiktoken_rs::cl100k_base;

    let bpe = cl100k_base().expect("should load tokenizer");
    let params = gopls_diagnostics();

    // Compact JSON (baseline)
    let compressor = DiagnosticsCompressor::default();
    let compact = compressor
        .intercept(
            "textDocument/publishDiagnostics",
            params.clone(),
            Direction::ServerToClient,
        )
        .await
        .expect("should compress")
        .expect("should produce output");
    let compact_json = serde_json::to_string(&compact).expect("should serialize");

    // TOON output
    let toon = compress_and_toon(params).await;

    let compact_tokens = bpe.encode_with_special_tokens(&compact_json).len();
    let toon_tokens = bpe.encode_with_special_tokens(&toon).len();
    let savings = 1.0 - (toon_tokens as f64 / compact_tokens as f64);

    println!("─── gopls TOON vs Compact JSON Token Savings ───");
    println!("Compact JSON tokens: {}", compact_tokens);
    println!("TOON tokens:          {}", toon_tokens);
    println!("Savings:             {:.1}%", savings * 100.0);

    // TOON should save tokens vs Compact JSON (self-explanatory field names
    // but zero structural overhead from JSON quoting/braces)
    assert!(
        savings >= 0.0,
        "TOON should not be worse than Compact JSON, got {:.1}%",
        savings * 100.0
    );
}
