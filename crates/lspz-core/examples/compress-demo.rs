//! lspz Compression Demo (v0.7.0)
//!
//! Run: cargo run --example compress-demo -p lspz-core
//!
//! Demonstrates all 4 LSP compressors with sample data
//! and shows byte / token savings for both Compact JSON and TOON.

use std::time::Instant;

use lspz_core::codec::compact::CompactDiagnostics;
use lspz_core::codec::toon;
use lspz_core::interceptors::Direction;
use lspz_core::interceptors::Interceptor;
use lspz_core::{
    CompletionCompressor, DiagnosticsCompressor, DocumentSymbolCompressor, HoverCompressor,
};
use serde_json::{Value, json};

#[tokio::main]
async fn main() {
    println!();
    println!("  ╔══════════════════════════════════════════════════╗");
    println!("  ║        lspz Compression Demo  (v0.7.0)          ║");
    println!("  ║  LSP compression proxy for AI coding agents     ║");
    println!("  ╚══════════════════════════════════════════════════╝");
    println!();

    let mut total_orig = 0usize;
    let mut total_comp = 0usize;
    let mut total_toon_total = 0usize;

    let toon_methods = [
        "textDocument/publishDiagnostics",
        "textDocument/completion",
        "textDocument/hover",
        "textDocument/documentSymbol",
    ];

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
        let (orig, comp, toon_bytes) =
            run_demo(name, method, params, &*compressor, &toon_methods).await;
        total_orig += orig;
        total_comp += comp;
        total_toon_total += toon_bytes;
    }

    let comp_pct = if total_orig > 0 {
        (total_orig.saturating_sub(total_comp)) as f64 / total_orig as f64 * 100.0
    } else {
        0.0
    };
    let toon_pct = if total_orig > 0 {
        (total_orig.saturating_sub(total_toon_total)) as f64 / total_orig as f64 * 100.0
    } else {
        0.0
    };

    println!("  ────────────────────────────────────────────────────");
    println!(
        "  TOTAL:      {:>6} bytes → Compact {:<6} bytes ({:>5.1}% saved)",
        total_orig, total_comp, comp_pct
    );
    println!(
        "  {:>28} → TOON    {:<6} bytes ({:>5.1}% saved)",
        "", total_toon_total, toon_pct
    );
    println!("  ────────────────────────────────────────────────────");
    println!();
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
) -> (usize, usize, usize) {
    let original_bytes = serde_json::to_string(&params).unwrap().len();

    let start = Instant::now();
    let result = match compressor
        .intercept(method, params, Direction::ServerToClient)
        .await
    {
        Ok(Some(v)) => v,
        Ok(None) => {
            println!("  ⚠️  {:<30}   (dropped)", name);
            return (original_bytes, 0, 0);
        }
        Err(e) => {
            println!("  ⚠️  {:<30}   failed: {}", name, e);
            return (original_bytes, original_bytes, original_bytes);
        }
    };
    let elapsed = start.elapsed();

    let compressed_bytes = serde_json::to_string(&result).unwrap().len();
    let savings = if original_bytes > 0 {
        (original_bytes.saturating_sub(compressed_bytes)) as f64 / original_bytes as f64 * 100.0
    } else {
        0.0
    };

    // Convert to TOON if supported
    let (toon_text, toon_bytes) = if toon_methods.contains(&method) {
        match convert_compact_to_toon(method, &result) {
            Some(text) => {
                let bytes = text.len();
                (Some(text), bytes)
            }
            None => (None, compressed_bytes),
        }
    } else {
        (None, compressed_bytes)
    };
    let toon_savings = if original_bytes > 0 {
        (original_bytes.saturating_sub(toon_bytes)) as f64 / original_bytes as f64 * 100.0
    } else {
        0.0
    };

    let pad = format!("{:<30}", name);
    println!(
        "  📦 {} {:>6}B → Compact: {:<6}B ({:>5.1}%, {:.1}μs)  TOON: {:<6}B ({:>5.1}%)",
        pad,
        original_bytes,
        compressed_bytes,
        savings,
        elapsed.as_micros(),
        toon_bytes,
        toon_savings,
    );

    // Print compact JSON
    let pretty = serde_json::to_string_pretty(&result).unwrap();
    println!("  │ compact:");
    for line in pretty.lines() {
        println!("  │   {}", line);
    }
    println!();

    // Print TOON if available
    if let Some(text) = toon_text {
        println!("  │ toon:");
        for line in text.lines() {
            println!("  │   {}", line);
        }
        println!();
    }

    (original_bytes, compressed_bytes, toon_bytes)
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
