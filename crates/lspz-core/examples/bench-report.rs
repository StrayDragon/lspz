//! lspz Compression Benchmark Report
//!
//! Loads JSON fixture files from `fixtures/bench/`, runs all 7 compressors,
//! measures bytes and token savings for both Compact JSON and TOON output formats,
//! and outputs a Markdown report.
//! Also loads per-LSP-server diagnostics from `fixtures/bench/lsp/*.json`
//! to show how DiagnosticsCompressor performs for each supported LSP server.
//!
//! Run: cargo run --example bench-report -p lspz-core

use std::path::Path;

use serde_json::Value;
use tiktoken_rs::cl100k_base;

use lspz_core::codec::compact::CompactDiagnostics;
use lspz_core::codec::toon;
use lspz_core::interceptors::workspace_diagnostics::workspace_diagnostics_to_toon;
use lspz_core::interceptors::workspace_symbols::workspace_symbols_to_toon;
use lspz_core::interceptors::{Direction, Interceptor};
use lspz_core::{
    CompletionCompressor, DiagnosticsCompressor, DocumentSymbolCompressor, HoverCompressor,
    LocationCompressor, WorkspaceDiagnosticCompressor, WorkspaceSymbolCompressor,
};

/// Get today's date as a formatted string.
fn get_today() -> String {
    let result = std::process::Command::new("date")
        .arg("+%F")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|c| c.wait_with_output());
    match result {
        Ok(output) if output.status.success() => {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if s.len() == 10 {
                s
            } else {
                "2026-05-11".into()
            }
        }
        _ => "2026-05-11".into(),
    }
}

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
        "textDocument/references"
        | "textDocument/definition"
        | "textDocument/implementation"
        | "textDocument/typeDefinition" => Some(Box::new(LocationCompressor)),
        "workspace/symbol" => Some(Box::new(WorkspaceSymbolCompressor)),
        "workspace/diagnostic" => Some(Box::new(WorkspaceDiagnosticCompressor)),
        _ => None,
    }
}

/// Convert compact JSON output to TOON format text.
fn convert_to_toon(method: &str, compact: &Value) -> Result<String, String> {
    match method {
        "textDocument/publishDiagnostics" => {
            let typed: CompactDiagnostics =
                serde_json::from_value(compact.clone()).map_err(|e| e.to_string())?;
            Ok(toon::diagnostics_to_toon(&typed))
        }
        "textDocument/completion" => toon::completions_to_toon(compact).map_err(|e| e.to_string()),
        "textDocument/hover" => toon::hover_to_toon(compact).map_err(|e| e.to_string()),
        "textDocument/documentSymbol" => toon::symbols_to_toon(compact).map_err(|e| e.to_string()),
        "textDocument/references" => toon::locations_to_toon(compact).map_err(|e| e.to_string()),
        "workspace/symbol" => workspace_symbols_to_toon(compact).map_err(|e| e.to_string()),
        "workspace/diagnostic" => workspace_diagnostics_to_toon(compact).map_err(|e| e.to_string()),
        _ => Err("no TOON converter".into()),
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
        ("locations.json", "LocationCompressor"),
        ("workspace_symbols.json", "WorkspaceSymbolCompressor"),
        (
            "workspace_diagnostics.json",
            "WorkspaceDiagnosticCompressor",
        ),
    ];

    println!("# lspz Compression Benchmark Report\n");
    println!("**Version**: v0.7.0\n");
    println!("**Date**: {}  \n", get_today());

    let mut all_results: Vec<(&str, f64, f64)> = Vec::new();

    for (filename, compressor_name) in &fixture_files {
        let path = fixture_dir.join(filename);
        let cases = load_fixtures(&path.to_string_lossy());

        let toon_methods = [
            "textDocument/publishDiagnostics",
            "textDocument/completion",
            "textDocument/hover",
            "textDocument/documentSymbol",
            "textDocument/references",
            "workspace/symbol",
            "workspace/diagnostic",
        ];

        println!("## {}\n", compressor_name);
        println!(
            "| Scenario | Raw (T) | Compact (T) | TOON (T) | Δ% (Compact vs Raw) | Δ% (TOON vs Raw) | Δ% (TOON vs Compact) |"
        );
        println!(
            "|----------|---------|-------------|----------|---------------------|-------------------|----------------------|"
        );

        let mut total_orig_tokens = 0usize;
        let mut total_comp_tokens = 0usize;
        let mut total_toon_tokens = 0usize;

        for case in &cases {
            let orig_json = serde_json::to_string(&case.params).unwrap();
            let orig_tokens = bpe.encode_with_special_tokens(&orig_json).len();

            let result = if let Some(compressor) = make_compressor(&case.method) {
                match compressor
                    .intercept(&case.method, case.params.clone(), Direction::ServerToClient)
                    .await
                {
                    Ok(Some(v)) => v,
                    Ok(None) => {
                        println!(
                            "| {} | {} | dropped | — | — | — | — |",
                            case.name, orig_tokens
                        );
                        continue;
                    }
                    Err(e) => {
                        eprintln!("  WARN: {} failed: {}", case.name, e);
                        println!(
                            "| {} | {} | error | — | — | — | — |",
                            case.name, orig_tokens
                        );
                        continue;
                    }
                }
            } else {
                println!("| {} | {} | — | — | — | — | — |", case.name, orig_tokens);
                continue;
            };

            let comp_json = serde_json::to_string(&result).unwrap();
            let comp_tokens = bpe.encode_with_special_tokens(&comp_json).len();

            // Convert to TOON if supported
            let (toon_text, toon_token_count) = if toon_methods.contains(&case.method.as_str()) {
                match convert_to_toon(&case.method, &result) {
                    Ok(text) => {
                        let t = bpe.encode_with_special_tokens(&text).len();
                        (Some(text), t)
                    }
                    Err(_) => (None, comp_tokens), // fallback: no TOON data
                }
            } else {
                (None, comp_tokens)
            };

            let compact_delta = if orig_tokens > 0 {
                (orig_tokens as f64 - comp_tokens as f64) / orig_tokens as f64 * 100.0
            } else {
                0.0
            };
            let toon_delta = if orig_tokens > 0 {
                (orig_tokens as f64 - toon_token_count as f64) / orig_tokens as f64 * 100.0
            } else {
                0.0
            };
            let toon_vs_compact = if comp_tokens > 0 {
                (comp_tokens as f64 - toon_token_count as f64) / comp_tokens as f64 * 100.0
            } else {
                0.0
            };

            let toon_token_str = if toon_text.is_some() {
                format!("{}", toon_token_count)
            } else {
                "—".to_string()
            };
            let toon_delta_str = if toon_text.is_some() {
                format!("{:.1}%", toon_delta)
            } else {
                "—".to_string()
            };
            let toon_vs_compact_str = if toon_text.is_some() {
                format!("{:.1}%", toon_vs_compact)
            } else {
                "—".to_string()
            };

            println!(
                "| {} | {} | {} | {} | {:.1}% | {} | {} |",
                case.name,
                orig_tokens,
                comp_tokens,
                toon_token_str,
                compact_delta,
                toon_delta_str,
                toon_vs_compact_str,
            );

            total_orig_tokens += orig_tokens;
            total_comp_tokens += comp_tokens;
            if toon_text.is_some() {
                total_toon_tokens += toon_token_count;
            } else {
                total_toon_tokens += comp_tokens;
            }
        }

        let avg_compact = if total_orig_tokens > 0 {
            (total_orig_tokens as f64 - total_comp_tokens as f64) / total_orig_tokens as f64 * 100.0
        } else {
            0.0
        };
        let avg_toon = if total_orig_tokens > 0 {
            (total_orig_tokens as f64 - total_toon_tokens as f64) / total_orig_tokens as f64 * 100.0
        } else {
            0.0
        };
        let avg_toon_vs_compact = if total_comp_tokens > 0 {
            (total_comp_tokens as f64 - total_toon_tokens as f64) / total_comp_tokens as f64 * 100.0
        } else {
            0.0
        };

        println!(
            "| **Total** | **{}** | **{}** | **{}** | **{:.1}%** | **{:.1}%** | **{:.1}%** |",
            total_orig_tokens,
            total_comp_tokens,
            total_toon_tokens,
            avg_compact,
            avg_toon,
            avg_toon_vs_compact,
        );
        println!();

        all_results.push((compressor_name, avg_compact, avg_toon));
    }

    // Summary table
    println!("## Summary\n");
    println!("| Compressor | Avg Δ% (Compact vs Raw) | Avg Δ% (TOON vs Raw) |");
    println!("|------------|------------------------|----------------------|");

    let overall_compact =
        all_results.iter().map(|(_, c, _)| c).sum::<f64>() / all_results.len() as f64;
    let overall_toon =
        all_results.iter().map(|(_, _, t)| t).sum::<f64>() / all_results.len() as f64;

    for (name, compact, toon) in &all_results {
        println!("| {} | {:.1}% | {:.1}% |", name, compact, toon);
    }
    println!(
        "| **Overall** | **{:.1}%** | **{:.1}%** |",
        overall_compact, overall_toon
    );
    println!();

    // ─── Per-LSP-Server Diagnostics Benchmarks ───────────
    run_lsp_benchmarks(fixture_dir, &bpe, &mut all_results).await;

    // Extended summary including LSP benchmarks
    println!("## Full Summary\n");
    println!("| Benchmark | Avg Δ% (Compact vs Raw) | Avg Δ% (TOON vs Raw) |");
    println!("|----------|------------------------|----------------------|");

    let overall_compact =
        all_results.iter().map(|(_, c, _)| c).sum::<f64>() / all_results.len() as f64;
    let overall_toon =
        all_results.iter().map(|(_, _, t)| t).sum::<f64>() / all_results.len() as f64;

    for (name, compact, toon) in &all_results {
        println!("| {} | {:.1}% | {:.1}% |", name, compact, toon);
    }
    println!(
        "| **Overall** | **{:.1}%** | **{:.1}%** |",
        overall_compact, overall_toon
    );
    println!();
}

/// Run per-LSP-server diagnostics benchmarks.
async fn run_lsp_benchmarks(
    fixture_dir: &Path,
    bpe: &tiktoken_rs::CoreBPE,
    all_results: &mut Vec<(&str, f64, f64)>,
) {
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
            "| Scenario | Raw (T) | Compact (T) | TOON (T) | Δ% (Compact vs Raw) | Δ% (TOON vs Raw) | Δ% (TOON vs Compact) |"
        );
        println!(
            "|----------|---------|-------------|----------|---------------------|-------------------|----------------------|"
        );

        let mut t_orig_tokens = 0usize;
        let mut t_comp_tokens = 0usize;
        let mut t_toon_tokens = 0usize;

        for case in &cases {
            let orig_json = serde_json::to_string(&case.params).unwrap();
            let orig_tokens = bpe.encode_with_special_tokens(&orig_json).len();

            let result = if case.method == "textDocument/publishDiagnostics" {
                let compressor = DiagnosticsCompressor::default();
                match compressor
                    .intercept(&case.method, case.params.clone(), Direction::ServerToClient)
                    .await
                {
                    Ok(Some(v)) => v,
                    _ => {
                        println!("| {} | {} | — | — | — | — | — |", case.name, orig_tokens);
                        continue;
                    }
                }
            } else {
                println!("| {} | {} | — | — | — | — | — |", case.name, orig_tokens);
                continue;
            };

            let comp_json = serde_json::to_string(&result).unwrap();
            let comp_tokens = bpe.encode_with_special_tokens(&comp_json).len();

            // Convert to TOON
            let toon_text = convert_to_toon(&case.method, &result).ok();
            let toon_token_count = toon_text
                .as_ref()
                .map(|t| bpe.encode_with_special_tokens(t).len())
                .unwrap_or(comp_tokens);

            let compact_delta = if orig_tokens > 0 {
                (orig_tokens as f64 - comp_tokens as f64) / orig_tokens as f64 * 100.0
            } else {
                0.0
            };
            let toon_delta = if orig_tokens > 0 {
                (orig_tokens as f64 - toon_token_count as f64) / orig_tokens as f64 * 100.0
            } else {
                0.0
            };
            let toon_vs_compact = if comp_tokens > 0 {
                (comp_tokens as f64 - toon_token_count as f64) / comp_tokens as f64 * 100.0
            } else {
                0.0
            };

            println!(
                "| {} | {} | {} | {} | {:.1}% | {:.1}% | {:.1}% |",
                case.name,
                orig_tokens,
                comp_tokens,
                toon_token_count,
                compact_delta,
                toon_delta,
                toon_vs_compact,
            );

            t_orig_tokens += orig_tokens;
            t_comp_tokens += comp_tokens;
            t_toon_tokens += toon_token_count;
        }

        let avg_compact = if t_orig_tokens > 0 {
            (t_orig_tokens as f64 - t_comp_tokens as f64) / t_orig_tokens as f64 * 100.0
        } else {
            0.0
        };
        let avg_toon = if t_orig_tokens > 0 {
            (t_orig_tokens as f64 - t_toon_tokens as f64) / t_orig_tokens as f64 * 100.0
        } else {
            0.0
        };
        let avg_toon_vs_compact = if t_comp_tokens > 0 {
            (t_comp_tokens as f64 - t_toon_tokens as f64) / t_comp_tokens as f64 * 100.0
        } else {
            0.0
        };

        println!(
            "| **Total** | **{}** | **{}** | **{}** | **{:.1}%** | **{:.1}%** | **{:.1}%** |",
            t_orig_tokens, t_comp_tokens, t_toon_tokens, avg_compact, avg_toon, avg_toon_vs_compact,
        );
        println!();

        all_results.push((lsp_name, avg_compact, avg_toon));
    }
}
