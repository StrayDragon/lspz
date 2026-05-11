//! lspz Compression Demo (v0.7.0)
//!
//! Run: cargo run --example compress-demo -p lspz-core
//!
//! Demonstrates all 4 LSP compressors with sample data,
//! comparing Raw JSON vs Compact JSON vs TOON format.
//! Token counts (cl100k_base) are the primary metric.
//! Bytes shown for reference.

use std::time::Instant;

use lspz_core::codec::compact::CompactDiagnostics;
use lspz_core::codec::toon;
use lspz_core::interceptors::{Direction, Interceptor};
use lspz_core::{
    CompletionCompressor, DiagnosticsCompressor, DocumentSymbolCompressor, HoverCompressor,
};
use serde_json::{Value, json};
use tiktoken_rs::CoreBPE;
use tiktoken_rs::cl100k_base;

struct DemoResult {
    name: String,
    orig_bytes: usize,
    orig_tokens: usize,
    comp_bytes: usize,
    comp_tokens: usize,
    toon_bytes: usize,
    toon_tokens: usize,
    elapsed_us: u128,
}

#[tokio::main]
async fn main() {
    let bpe = cl100k_base().expect("Failed to initialize tiktoken");

    println!();
    println!("  ╔══════════════════════════════════════════════════╗");
    println!("  ║        lspz Compression Demo  (v0.7.0)          ║");
    println!("  ║  Token-optimized LSP for AI coding agents       ║");
    println!("  ╚══════════════════════════════════════════════════╝");
    println!();

    let toon_methods = [
        "textDocument/publishDiagnostics",
        "textDocument/completion",
        "textDocument/hover",
        "textDocument/documentSymbol",
    ];

    let mut results: Vec<DemoResult> = Vec::new();

    for (name, method, params, compressor) in [
        (
            "DiagnosticsCompressor",
            "textDocument/publishDiagnostics",
            json!({
                "uri": "file:///src/main.rs",
                "diagnostics": [
                    {
                        "range": { "start": { "line": 10, "character": 5 }, "end": { "line": 10, "character": 15 } },
                        "severity": 2,
                        "message": "unused variable: `x`",
                        "code": "unused_variables",
                        "source": "rust-analyzer",
                        "tags": [1]
                    },
                    {
                        "range": { "start": { "line": 11, "character": 5 }, "end": { "line": 11, "character": 15 } },
                        "severity": 2,
                        "message": "unused variable: `x`",
                        "code": "unused_variables",
                        "source": "rust-analyzer",
                        "tags": [1]
                    },
                    {
                        "range": { "start": { "line": 20, "character": 0 }, "end": { "line": 20, "character": 10 } },
                        "severity": 1,
                        "message": "cannot find value `y` in this scope",
                        "code": "E0425",
                        "source": "rust-analyzer"
                    }
                ]
            }),
            Box::new(DiagnosticsCompressor::default()) as Box<dyn Interceptor>,
        ),
        (
            "CompletionCompressor",
            "textDocument/completion",
            json!({
                "isIncomplete": false,
                "items": [
                    { "label": "push", "kind": 3, "detail": "fn push(&mut self, value: T)", "documentation": "Appends an element to the back of a collection." },
                    { "label": "pop", "kind": 3, "detail": "fn pop(&mut self) -> Option<T>", "documentation": "Removes the last element from a collection." },
                    { "label": "len", "kind": 16, "detail": "fn len(&self) -> usize", "documentation": "Returns the number of elements." },
                    { "label": "is_empty", "kind": 16, "detail": "fn is_empty(&self) -> bool", "documentation": "Returns true if the collection contains no elements.", "deprecated": true }
                ]
            }),
            Box::new(CompletionCompressor::default()),
        ),
        (
            "HoverCompressor",
            "textDocument/hover",
            json!({
                "contents": {
                    "kind": "markdown",
                    "value": "```rust\nfn push(&mut self, value: T)\n```\n\n\nAppends an element to the back of a collection."
                },
                "range": {
                    "start": { "line": 42, "character": 4 },
                    "end": { "line": 42, "character": 8 }
                }
            }),
            Box::new(HoverCompressor::default()),
        ),
        (
            "DocumentSymbolCompressor",
            "textDocument/documentSymbol",
            json!([
                {
                    "name": "MyStruct",
                    "kind": 23,
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 10, "character": 1 } },
                    "selectionRange": { "start": { "line": 0, "character": 0 }, "end": { "line": 10, "character": 1 } },
                    "children": [
                        {
                            "name": "field1",
                            "kind": 8,
                            "range": { "start": { "line": 1, "character": 4 }, "end": { "line": 1, "character": 11 } },
                            "selectionRange": { "start": { "line": 1, "character": 4 }, "end": { "line": 1, "character": 11 } }
                        }
                    ]
                }
            ]),
            Box::new(DocumentSymbolCompressor),
        ),
    ] {
        let result = run_demo(name, method, params, &*compressor, &toon_methods, &bpe).await;
        results.push(result);
    }

    // ─── Summary table ─────────────────────────────────────
    println!("  ┌──────────────────────────────┬──────────┬──────────┬──────────┬──────────┐");
    println!("  │ Compressor                   │ Raw      │ Compact  │ TOON     │ Best     │");
    println!("  ├──────────────────────────────┼──────────┼──────────┼──────────┼──────────┤");

    for r in &results {
        let best = if r.toon_tokens <= r.comp_tokens {
            "TOON"
        } else {
            "Compact"
        };
        println!(
            "  │ {:<28} │ {:>6}t   │ {:>6}t   │ {:>6}t   │ {:<8} │",
            r.name, r.orig_tokens, r.comp_tokens, r.toon_tokens, best
        );
    }
    println!("  ├──────────────────────────────┼──────────┼──────────┼──────────┼──────────┤");

    let t_orig: usize = results.iter().map(|r| r.orig_tokens).sum();
    let t_comp: usize = results.iter().map(|r| r.comp_tokens).sum();
    let t_toon: usize = results.iter().map(|r| r.toon_tokens).sum();
    println!(
        "  │ {:<28} │ {:>6}t   │ {:>6}t   │ {:>6}t   │ TOON     │",
        "TOTAL", t_orig, t_comp, t_toon
    );
    println!("  └──────────────────────────────┴──────────┴──────────┴──────────┴──────────┘");
    println!();

    // ─── Savings summary ───────────────────────────────────
    let t_comp_pct = if t_orig > 0 {
        (t_orig.saturating_sub(t_comp)) as f64 / t_orig as f64 * 100.0
    } else {
        0.0
    };
    let t_toon_pct = if t_orig > 0 {
        (t_orig.saturating_sub(t_toon)) as f64 / t_orig as f64 * 100.0
    } else {
        0.0
    };

    let b_orig: usize = results.iter().map(|r| r.orig_bytes).sum();
    let b_comp: usize = results.iter().map(|r| r.comp_bytes).sum();
    let b_toon: usize = results.iter().map(|r| r.toon_bytes).sum();
    let b_comp_pct = if b_orig > 0 {
        (b_orig.saturating_sub(b_comp)) as f64 / b_orig as f64 * 100.0
    } else {
        0.0
    };
    let b_toon_pct = if b_orig > 0 {
        (b_orig.saturating_sub(b_toon)) as f64 / b_orig as f64 * 100.0
    } else {
        0.0
    };

    println!(
        "  Tokens: {} → Compact: {} (↓{:.1}%)  TOON: {} (↓{:.1}%)",
        t_orig, t_comp, t_comp_pct, t_toon, t_toon_pct
    );
    println!(
        "  Bytes:  {} → Compact: {} (↓{:.1}%)  TOON: {} (↓{:.1}%)",
        b_orig, b_comp, b_comp_pct, b_toon, b_toon_pct
    );
    println!();

    // ─── Per-compressor detail ─────────────────────────────
    println!("  ── Per-compressor breakdown ──");
    println!();
    for r in &results {
        let comp_savings = if r.orig_tokens > 0 {
            (r.orig_tokens as f64 - r.comp_tokens as f64) / r.orig_tokens as f64 * 100.0
        } else {
            0.0
        };
        let toon_savings = if r.orig_tokens > 0 {
            (r.orig_tokens as f64 - r.toon_tokens as f64) / r.orig_tokens as f64 * 100.0
        } else {
            0.0
        };
        let best = if r.toon_tokens <= r.comp_tokens {
            "TOON"
        } else {
            "Compact"
        };

        println!(
            "  {:<28}  Raw {:>4}t  Compact {:>4}t (↓{:>5.1}%)  TOON {:>4}t (↓{:>5.1}%)  ✓{}  {:>6.1}μs",
            r.name,
            r.orig_tokens,
            r.comp_tokens,
            comp_savings,
            r.toon_tokens,
            toon_savings,
            best,
            r.elapsed_us as f64,
        );
        println!(
            "  {:>28}  {:>7}B  {:>10}B (↓{:>5.1}%)  {:>9}B (↓{:>5.1}%)",
            "",
            r.orig_bytes,
            r.comp_bytes,
            if r.orig_bytes > 0 {
                (r.orig_bytes.saturating_sub(r.comp_bytes)) as f64 / r.orig_bytes as f64 * 100.0
            } else {
                0.0
            },
            r.toon_bytes,
            if r.orig_bytes > 0 {
                (r.orig_bytes.saturating_sub(r.toon_bytes)) as f64 / r.orig_bytes as f64 * 100.0
            } else {
                0.0
            },
        );
        println!();
    }

    println!("  Note: Real-world savings are higher with repeated data.");
    println!("  E.g., 30 \"unused variable\" diagnostics → 1 entry + ranges.");
    println!();
}

async fn run_demo(
    name: &str,
    method: &str,
    params: Value,
    compressor: &dyn Interceptor,
    toon_methods: &[&str],
    bpe: &CoreBPE,
) -> DemoResult {
    let orig_json = serde_json::to_string(&params).unwrap();
    let orig_bytes = orig_json.len();
    let orig_tokens = bpe.encode_with_special_tokens(&orig_json).len();

    let start = Instant::now();
    let result = match compressor
        .intercept(method, params, Direction::ServerToClient)
        .await
    {
        Ok(Some(v)) => v,
        Ok(None) => {
            println!("  ⚠️  {:<30}   (dropped)", name);
            return DemoResult {
                name: name.into(),
                orig_bytes,
                orig_tokens,
                comp_bytes: 0,
                comp_tokens: 0,
                toon_bytes: 0,
                toon_tokens: 0,
                elapsed_us: 0,
            };
        }
        Err(e) => {
            println!("  ⚠️  {:<30}   failed: {}", name, e);
            return DemoResult {
                name: name.into(),
                orig_bytes,
                orig_tokens,
                comp_bytes: orig_bytes,
                comp_tokens: orig_tokens,
                toon_bytes: orig_bytes,
                toon_tokens: orig_tokens,
                elapsed_us: 0,
            };
        }
    };
    let elapsed = start.elapsed();

    let comp_json = serde_json::to_string(&result).unwrap();
    let comp_bytes = comp_json.len();
    let comp_tokens = bpe.encode_with_special_tokens(&comp_json).len();

    // Convert to TOON if supported
    let (toon_bytes, toon_tokens) = if toon_methods.contains(&method) {
        match convert_compact_to_toon(method, &result) {
            Some(text) => {
                let bytes = text.len();
                let tokens = bpe.encode_with_special_tokens(&text).len();
                (bytes, tokens)
            }
            None => (comp_bytes, comp_tokens),
        }
    } else {
        (comp_bytes, comp_tokens)
    };

    DemoResult {
        name: name.into(),
        orig_bytes,
        orig_tokens,
        comp_bytes,
        comp_tokens,
        toon_bytes,
        toon_tokens,
        elapsed_us: elapsed.as_micros(),
    }
}

/// Convert compact JSON output to TOON format text.
fn convert_compact_to_toon(method: &str, compact: &Value) -> Option<String> {
    match method {
        "textDocument/publishDiagnostics" => {
            let typed: CompactDiagnostics = serde_json::from_value(compact.clone()).ok()?;
            Some(toon::diagnostics_to_toon(&typed))
        }
        "textDocument/completion" => toon::completions_to_toon(compact).ok(),
        "textDocument/hover" => toon::hover_to_toon(compact).ok(),
        "textDocument/documentSymbol" => toon::symbols_to_toon(compact).ok(),
        _ => None,
    }
}
