//! lspz Compression Benchmark Report (v0.5.0)
//!
//! Loads JSON fixture files from `fixtures/bench/`, runs all 4 compressors,
//! measures bytes and token savings, and outputs a Markdown report.
//! Also loads per-LSP-server diagnostics from `fixtures/bench/lsp/*.json`
//! to show how DiagnosticsCompressor performs for each supported LSP server.
//!
//! Run: cargo run --example bench-report -p lspz-core

use std::path::Path;

use serde_json::Value;
use tiktoken_rs::cl100k_base;

use lspz_core::interceptors::{Direction, Interceptor};
use lspz_core::{
    CompletionCompressor, DiagnosticsCompressor, DocumentSymbolCompressor, HoverCompressor,
};

struct FixtureCase {
    name: String,
    method: String,
    params: Value,
}

fn load_fixtures(path: &str) -> Vec<FixtureCase> {
    let content = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Error reading {}: {}", path, e);
        std::process::exit(1);
    });
    let cases: Vec<Value> = serde_json::from_str(&content).unwrap_or_else(|e| {
        eprintln!("Error parsing {}: {}", path, e);
        std::process::exit(1);
    });
    cases
        .into_iter()
        .map(|c| {
            let name = c["name"].as_str().unwrap().to_string();
            let method = c["method"].as_str().unwrap().to_string();
            let params = c["params"].clone();
            FixtureCase {
                name,
                method,
                params,
            }
        })
        .collect()
}

fn make_compressor(method: &str) -> Option<Box<dyn Interceptor>> {
    match method {
        "textDocument/publishDiagnostics" => Some(Box::new(DiagnosticsCompressor::default())),
        "textDocument/completion" => Some(Box::new(CompletionCompressor::default())),
        "textDocument/hover" => Some(Box::new(HoverCompressor::default())),
        "textDocument/documentSymbol" => Some(Box::new(DocumentSymbolCompressor)),
        _ => None,
    }
}

#[tokio::main]
async fn main() {
    let bpe = cl100k_base().expect("Failed to initialize tiktoken");
    let fixture_dir = Path::new("fixtures/bench");

    let fixture_files = [
        ("diagnostics.json", "DiagnosticsCompressor"),
        ("completions.json", "CompletionCompressor"),
        ("hover.json", "HoverCompressor"),
        ("symbols.json", "DocumentSymbolCompressor"),
    ];

    println!("# lspz Compression Benchmark Report\n");
    println!("**Version**: v0.5.0  \n");
    println!("**Date**: 2026-05-11\n");

    let mut all_results: Vec<(&str, f64, f64)> = Vec::new();

    for (filename, compressor_name) in &fixture_files {
        let path = fixture_dir.join(filename);
        let cases = load_fixtures(&path.to_string_lossy());

        println!("## {}\n", compressor_name);
        println!(
            "| Scenario | Original (B) | Compact (B) | Byte Δ% | Original (T) | Compact (T) | Token Δ% |"
        );
        println!(
            "|----------|-------------|------------|---------|-------------|------------|---------|"
        );

        let mut total_orig_bytes = 0usize;
        let mut total_comp_bytes = 0usize;
        let mut total_orig_tokens = 0usize;
        let mut total_comp_tokens = 0usize;

        for case in &cases {
            let orig_json = serde_json::to_string(&case.params).unwrap();
            let orig_bytes = orig_json.len();
            let orig_tokens = bpe.encode_with_special_tokens(&orig_json).len();

            let result = if let Some(compressor) = make_compressor(&case.method) {
                match compressor
                    .intercept(&case.method, case.params.clone(), Direction::ServerToClient)
                    .await
                {
                    Ok(Some(v)) => v,
                    Ok(None) => {
                        println!(
                            "| {} | {} | dropped | — | {} | — | — |",
                            case.name, orig_bytes, orig_tokens
                        );
                        continue;
                    }
                    Err(e) => {
                        eprintln!("  WARN: {} failed: {}", case.name, e);
                        println!(
                            "| {} | {} | error | — | {} | — | — |",
                            case.name, orig_bytes, orig_tokens
                        );
                        continue;
                    }
                }
            } else {
                println!(
                    "| {} | {} | — | — | {} | — | — |",
                    case.name, orig_bytes, orig_tokens
                );
                continue;
            };

            let comp_json = serde_json::to_string(&result).unwrap();
            let comp_bytes = comp_json.len();
            let comp_tokens = bpe.encode_with_special_tokens(&comp_json).len();

            let byte_delta = if orig_bytes > 0 {
                (orig_bytes as f64 - comp_bytes as f64) / orig_bytes as f64 * 100.0
            } else {
                0.0
            };
            let token_delta = if orig_tokens > 0 {
                (orig_tokens as f64 - comp_tokens as f64) / orig_tokens as f64 * 100.0
            } else {
                0.0
            };

            println!(
                "| {} | {} | {} | {:.1}% | {} | {} | {:.1}% |",
                case.name,
                orig_bytes,
                comp_bytes,
                byte_delta,
                orig_tokens,
                comp_tokens,
                token_delta
            );

            total_orig_bytes += orig_bytes;
            total_comp_bytes += comp_bytes;
            total_orig_tokens += orig_tokens;
            total_comp_tokens += comp_tokens;
        }

        let avg_byte = if total_orig_bytes > 0 {
            (total_orig_bytes as f64 - total_comp_bytes as f64) / total_orig_bytes as f64 * 100.0
        } else {
            0.0
        };
        let avg_token = if total_orig_tokens > 0 {
            (total_orig_tokens as f64 - total_comp_tokens as f64) / total_orig_tokens as f64 * 100.0
        } else {
            0.0
        };

        println!(
            "| **Total** | **{}** | **{}** | **{:.1}%** | **{}** | **{}** | **{:.1}%** |",
            total_orig_bytes,
            total_comp_bytes,
            avg_byte,
            total_orig_tokens,
            total_comp_tokens,
            avg_token
        );
        println!();

        all_results.push((compressor_name, avg_byte, avg_token));
    }

    // Summary table
    println!("## Summary\n");
    println!("| Compressor | Avg Byte Savings | Avg Token Savings |");
    println!("|------------|-----------------|-------------------|");

    let overall_byte =
        all_results.iter().map(|(_, b, _)| b).sum::<f64>() / all_results.len() as f64;
    let overall_token =
        all_results.iter().map(|(_, _, t)| t).sum::<f64>() / all_results.len() as f64;

    for (name, byte, token) in &all_results {
        println!("| {} | {:.1}% | {:.1}% |", name, byte, token);
    }
    println!(
        "| **Overall** | **{:.1}%** | **{:.1}%** |",
        overall_byte, overall_token
    );
    println!();

    // ─── Per-LSP-Server Diagnostics Benchmarks ───────────
    println!("---\n");
    println!("## Per-LSP Diagnostics Benchmarks\n");
    println!(
        "Each LSP server produces diagnostics with unique patterns. The table below shows how"
    );
    println!(
        "the DiagnosticsCompressor handles real-world output for each supported LSP server.\n"
    );

    let lsp_fixtures: &[(&str, &str)] = &[
        ("rust-analyzer", "lsp/rust-analyzer.json"),
        ("gopls", "lsp/gopls.json"),
        ("basedpyright", "lsp/basedpyright.json"),
        ("typescript", "lsp/typescript.json"),
    ];

    for (lsp_name, fixture_rel) in lsp_fixtures {
        let path = fixture_dir.join(fixture_rel);
        let Ok(content) = std::fs::read_to_string(&path) else {
            eprintln!("  WARN: LSP fixture not found: {}", path.display());
            continue;
        };
        let cases: Vec<FixtureCase> = serde_json::from_str::<Vec<Value>>(&content)
            .unwrap_or_else(|e| {
                eprintln!("  WARN: failed to parse {}: {}", path.display(), e);
                Vec::new()
            })
            .into_iter()
            .map(|c| {
                let name = c["name"].as_str().unwrap().to_string();
                let method = c["method"].as_str().unwrap().to_string();
                let params = c["params"].clone();
                FixtureCase {
                    name,
                    method,
                    params,
                }
            })
            .collect();

        println!("### {}\n", lsp_name);
        println!(
            "| Scenario | Original (B) | Compact (B) | Byte Δ% | Original (T) | Compact (T) | Token Δ% |"
        );
        println!(
            "|----------|-------------|------------|---------|-------------|------------|---------|"
        );

        let mut t_orig_bytes = 0usize;
        let mut t_comp_bytes = 0usize;
        let mut t_orig_tokens = 0usize;
        let mut t_comp_tokens = 0usize;

        for case in &cases {
            let orig_json = serde_json::to_string(&case.params).unwrap();
            let orig_bytes = orig_json.len();
            let orig_tokens = bpe.encode_with_special_tokens(&orig_json).len();

            let result = if case.method == "textDocument/publishDiagnostics" {
                let compressor = DiagnosticsCompressor::default();
                match compressor
                    .intercept(&case.method, case.params.clone(), Direction::ServerToClient)
                    .await
                {
                    Ok(Some(v)) => v,
                    _ => {
                        println!(
                            "| {} | {} | — | — | {} | — | — |",
                            case.name, orig_bytes, orig_tokens
                        );
                        continue;
                    }
                }
            } else {
                println!(
                    "| {} | {} | — | — | {} | — | — |",
                    case.name, orig_bytes, orig_tokens
                );
                continue;
            };

            let comp_json = serde_json::to_string(&result).unwrap();
            let comp_bytes = comp_json.len();
            let comp_tokens = bpe.encode_with_special_tokens(&comp_json).len();

            let byte_delta = if orig_bytes > 0 {
                (orig_bytes as f64 - comp_bytes as f64) / orig_bytes as f64 * 100.0
            } else {
                0.0
            };
            let token_delta = if orig_tokens > 0 {
                (orig_tokens as f64 - comp_tokens as f64) / orig_tokens as f64 * 100.0
            } else {
                0.0
            };

            println!(
                "| {} | {} | {} | {:.1}% | {} | {} | {:.1}% |",
                case.name,
                orig_bytes,
                comp_bytes,
                byte_delta,
                orig_tokens,
                comp_tokens,
                token_delta
            );

            t_orig_bytes += orig_bytes;
            t_comp_bytes += comp_bytes;
            t_orig_tokens += orig_tokens;
            t_comp_tokens += comp_tokens;
        }

        let avg_byte = if t_orig_bytes > 0 {
            (t_orig_bytes as f64 - t_comp_bytes as f64) / t_orig_bytes as f64 * 100.0
        } else {
            0.0
        };
        let avg_token = if t_orig_tokens > 0 {
            (t_orig_tokens as f64 - t_comp_tokens as f64) / t_orig_tokens as f64 * 100.0
        } else {
            0.0
        };

        println!(
            "| **Total** | **{}** | **{}** | **{:.1}%** | **{}** | **{}** | **{:.1}%** |",
            t_orig_bytes, t_comp_bytes, avg_byte, t_orig_tokens, t_comp_tokens, avg_token
        );
        println!();

        all_results.push((lsp_name, avg_byte, avg_token));
    }

    // Extended summary including LSP benchmarks
    println!("## Full Summary\n");
    println!("| Benchmark | Avg Byte Savings | Avg Token Savings |");
    println!("|----------|-----------------|-------------------|");

    let overall_byte =
        all_results.iter().map(|(_, b, _)| b).sum::<f64>() / all_results.len() as f64;
    let overall_token =
        all_results.iter().map(|(_, _, t)| t).sum::<f64>() / all_results.len() as f64;

    for (name, byte, token) in &all_results {
        println!("| {} | {:.1}% | {:.1}% |", name, byte, token);
    }
    println!(
        "| **Overall** | **{:.1}%** | **{:.1}%** |",
        overall_byte, overall_token
    );
    println!();
}
