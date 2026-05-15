//! lspz Compression Benchmark Report
//!
//! Loads JSON fixture files from `fixtures/bench/`, runs all 7 compressors,
//! and outputs a Markdown report to stdout. Also processes LSP-specific
//! fixtures from `fixtures/bench/lsp/`.
//!
//! Run: just gen-bench

use std::path::Path;

use serde_json::Value;
use tiktoken_rs::cl100k_base;

use lspz::codec::compact::CompactDiagnostics;
use lspz::codec::toon;
use lspz::interceptors::workspace_diagnostics::workspace_diagnostics_to_toon;
use lspz::interceptors::workspace_symbols::workspace_symbols_to_toon;
use lspz::interceptors::{Direction, Interceptor};
use lspz::{
    CompletionCompressor, DiagnosticsCompressor, DocumentSymbolCompressor, HoverCompressor,
    LocationCompressor, WorkspaceDiagnosticCompressor, WorkspaceSymbolCompressor,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn get_today() -> String {
    let output = std::process::Command::new("date").arg("+%Y-%m-%d").output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => "unknown".to_string(),
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
        .map(|c| FixtureCase {
            name: c["name"].as_str().unwrap().to_string(),
            method: c["method"].as_str().unwrap().to_string(),
            params: c["params"].clone(),
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

struct BenchResult {
    orig_tokens: usize,
    comp_tokens: usize,
    toon_tokens: usize,
}

async fn bench_case(
    bpe: &tiktoken_rs::CoreBPE,
    case: &FixtureCase,
    toon_methods: &[&str],
) -> Option<BenchResult> {
    let orig_json = serde_json::to_string(&case.params).unwrap();
    let orig_tokens = bpe.encode_with_special_tokens(&orig_json).len();

    let compressor = make_compressor(&case.method)?;
    let result = match compressor
        .intercept(&case.method, case.params.clone(), Direction::ServerToClient)
        .await
    {
        Ok(Some(v)) => v,
        Ok(None) | Err(_) => return None,
    };

    let comp_json = serde_json::to_string(&result).unwrap();
    let comp_tokens = bpe.encode_with_special_tokens(&comp_json).len();

    let toon_tokens = if toon_methods.contains(&case.method.as_str()) {
        match convert_to_toon(&case.method, &result) {
            Ok(text) => bpe.encode_with_special_tokens(&text).len(),
            Err(_) => comp_tokens,
        }
    } else {
        comp_tokens
    };

    Some(BenchResult {
        orig_tokens,
        comp_tokens,
        toon_tokens,
    })
}

fn print_table_row(name: &str, orig: usize, comp: usize, toon: usize) {
    let compact_delta = pct(orig, comp);
    let toon_delta = pct(orig, toon);
    let toon_vs_compact = pct(comp, toon);
    println!(
        "| {} | {} | {} | {} | {:.1}% | {:.1}% | {:.1}% |",
        name, orig, comp, toon, compact_delta, toon_delta, toon_vs_compact,
    );
}

fn print_total_row(cases: &[(&str, BenchResult)]) {
    let orig: usize = cases.iter().map(|(_, r)| r.orig_tokens).sum();
    let comp: usize = cases.iter().map(|(_, r)| r.comp_tokens).sum();
    let toon: usize = cases.iter().map(|(_, r)| r.toon_tokens).sum();
    println!(
        "| **Total** | **{}** | **{}** | **{}** | **{:.1}%** | **{:.1}%** | **{:.1}%** |",
        orig,
        comp,
        toon,
        pct(orig, comp),
        pct(orig, toon),
        pct(comp, toon),
    );
    println!();
}

fn pct(a: usize, b: usize) -> f64 {
    if a == 0 {
        0.0
    } else {
        (a as f64 - b as f64) / a as f64 * 100.0
    }
}

const TOON_METHODS: &[&str] = &[
    "textDocument/publishDiagnostics",
    "textDocument/completion",
    "textDocument/hover",
    "textDocument/documentSymbol",
    "textDocument/references",
    "workspace/symbol",
    "workspace/diagnostic",
];

const COMPRESSOR_CN: &[(&str, &str)] = &[
    ("diagnostics.json", "诊断压缩器"),
    ("completions.json", "补全压缩器"),
    ("hover.json", "悬停压缩器"),
    ("symbols.json", "文档符号压缩器"),
    ("locations.json", "位置压缩器"),
    ("workspace_symbols.json", "工作区符号压缩器"),
    ("workspace_diagnostics.json", "工作区诊断压缩器"),
];

#[tokio::main]
async fn main() {
    let bpe = cl100k_base().expect("Failed to initialize tiktoken");
    let fixture_dir = Path::new("fixtures/bench");

    println!(
        "<!-- AUTO-GENERATED by examples/bench-report.rs. Do not edit manually. Run: just gen-bench -->"
    );
    println!();
    println!("# lspz 压缩基准测试报告\n");
    println!("**版本**: v{}\n", VERSION);
    println!("**日期**: {}  \n", get_today());

    let mut all_results: Vec<(&str, f64, f64)> = Vec::new();

    for (filename, cn_name) in COMPRESSOR_CN {
        let path = fixture_dir.join(filename);
        let cases = load_fixtures(&path.to_string_lossy());

        println!("## {}\n", cn_name);
        println!(
            "| 场景 | 原始 (T) | 紧凑 (T) | TOON (T) | Δ% (紧凑 vs 原始) | Δ% (TOON vs 原始) | Δ% (TOON vs 紧凑) |"
        );
        println!(
            "|----------|---------|-------------|----------|---------------------|-------------------|----------------------|"
        );

        let mut bench_cases: Vec<(&str, BenchResult)> = Vec::new();

        for case in &cases {
            match bench_case(&bpe, case, TOON_METHODS).await {
                Some(result) => {
                    print_table_row(
                        &case.name,
                        result.orig_tokens,
                        result.comp_tokens,
                        result.toon_tokens,
                    );
                    bench_cases.push((&case.name, result));
                }
                None => {
                    println!("| {} | — | — | — | — | — | — |", case.name);
                }
            }
        }

        print_total_row(&bench_cases);

        if !bench_cases.is_empty() {
            let orig: usize = bench_cases.iter().map(|(_, r)| r.orig_tokens).sum();
            let comp: usize = bench_cases.iter().map(|(_, r)| r.comp_tokens).sum();
            let toon: usize = bench_cases.iter().map(|(_, r)| r.toon_tokens).sum();
            all_results.push((cn_name, pct(orig, comp), pct(orig, toon)));
        }
    }

    // Summary
    println!("## 总结\n");
    println!("| 压缩器 | 平均 Δ% (紧凑 vs 原始) | 平均 Δ% (TOON vs 原始) |");
    println!("|------------|------------------------|----------------------|");
    for (name, compact, toon) in &all_results {
        println!("| {} | {:.1}% | {:.1}% |", name, compact, toon);
    }
    if !all_results.is_empty() {
        let overall_compact: f64 =
            all_results.iter().map(|(_, c, _)| *c).sum::<f64>() / all_results.len() as f64;
        let overall_toon: f64 =
            all_results.iter().map(|(_, _, t)| *t).sum::<f64>() / all_results.len() as f64;
        println!(
            "| **总体** | **{:.1}%** | **{:.1}%** |",
            overall_compact, overall_toon
        );
    }
    println!();

    // LSP-specific diagnostics section
    let lsp_dir = fixture_dir.join("lsp");
    if lsp_dir.exists() {
        println!(
            "每个 LSP 服务器产生的诊断都有独特的模式。下表显示了诊断压缩器如何处理每个支持的 LSP 服务器的真实世界输出。"
        );

        let lsp_fixtures = [
            ("rust-analyzer.json", "rust-analyzer"),
            ("gopls.json", "gopls"),
            ("basedpyright.json", "basedpyright"),
            ("typescript.json", "typescript-language-server"),
        ];

        for (filename, server_name) in &lsp_fixtures {
            let path = lsp_dir.join(filename);
            if !path.exists() {
                continue;
            }
            let cases = load_fixtures(&path.to_string_lossy());

            println!("\n### {}\n", server_name);
            println!(
                "| 场景 | 原始 (T) | 紧凑 (T) | TOON (T) | Δ% (紧凑 vs 原始) | Δ% (TOON vs 原始) | Δ% (TOON vs 紧凑) |"
            );
            println!(
                "|----------|---------|-------------|----------|---------------------|-------------------|----------------------|"
            );

            let mut bench_cases: Vec<(&str, BenchResult)> = Vec::new();

            for case in &cases {
                match bench_case(&bpe, case, TOON_METHODS).await {
                    Some(result) => {
                        print_table_row(
                            &case.name,
                            result.orig_tokens,
                            result.comp_tokens,
                            result.toon_tokens,
                        );
                        bench_cases.push((&case.name, result));
                    }
                    None => {
                        println!("| {} | — | — | — | — | — | — |", case.name);
                    }
                }
            }

            print_total_row(&bench_cases);
        }
    }
}
