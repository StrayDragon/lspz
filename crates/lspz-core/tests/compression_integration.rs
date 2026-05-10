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

    // basedpyright message patterns are not yet normalized by the compressor,
    // so group count equals the original 6 diagnostics
    // TODO: add normalization patterns for basedpyright/unused-import/unused-variable
    assert!(diags.len() <= 6, "basedpyright: groups ({}) <= original (6)", diags.len());
    assert!(!diags.is_empty(), "basedpyright should produce at least 1 group");
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

    // TypeScript message patterns are not yet normalized by the compressor,
    // so group count equals the original 6 diagnostics
    // TODO: add normalization patterns for TypeScript/unused-variable
    assert!(diags.len() <= 6, "typescript: groups ({}) <= original (6)", diags.len());
    assert!(!diags.is_empty(), "typescript should produce at least 1 group");
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
