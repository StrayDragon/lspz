//! Criterion throughput benchmarks for all 4 LSP compressors.
//!
//! Measures μs/op for small/medium/large fixture data.
//! Fixtures loaded once per benchmark group (outside measured loop).

use std::path::Path;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use serde_json::Value;

use lspz_core::interceptors::{Direction, Interceptor};
use lspz_core::{
    CompletionCompressor, DiagnosticsCompressor, DocumentSymbolCompressor, HoverCompressor,
};

/// Root of the `fixtures/bench/` directory, resolved at compile time.
/// `CARGO_MANIFEST_DIR` = `crates/lspz-core/`, so go up 2 levels to workspace root.
const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/bench");

fn load_fixtures(name: &str) -> Vec<(String, String, Value)> {
    let path = Path::new(FIXTURE_DIR).join(name);
    let content = std::fs::read_to_string(&path).unwrap();
    let cases: Vec<Value> = serde_json::from_str(&content).unwrap();
    cases
        .iter()
        .map(|c| {
            let name = c["name"].as_str().unwrap().to_string();
            let method = c["method"].as_str().unwrap().to_string();
            let params = c["params"].clone();
            (name, method, params)
        })
        .collect()
}

fn bench_diagnostics(c: &mut Criterion) {
    let compressor = DiagnosticsCompressor::default();
    let cases = load_fixtures("diagnostics.json");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("diagnostics");
    for (name, method, params) in &cases {
        group.bench_function(name, |b| {
            b.iter(|| {
                rt.block_on(compressor.intercept(
                    method,
                    black_box(params.clone()),
                    Direction::ServerToClient,
                ))
            })
        });
    }
    group.finish();
}

fn bench_completions(c: &mut Criterion) {
    let compressor = CompletionCompressor::default();
    let cases = load_fixtures("completions.json");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("completions");
    for (name, method, params) in &cases {
        group.bench_function(name, |b| {
            b.iter(|| {
                rt.block_on(compressor.intercept(
                    method,
                    black_box(params.clone()),
                    Direction::ServerToClient,
                ))
            })
        });
    }
    group.finish();
}

fn bench_hover(c: &mut Criterion) {
    let compressor = HoverCompressor::default();
    let cases = load_fixtures("hover.json");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("hover");
    for (name, method, params) in &cases {
        group.bench_function(name, |b| {
            b.iter(|| {
                rt.block_on(compressor.intercept(
                    method,
                    black_box(params.clone()),
                    Direction::ServerToClient,
                ))
            })
        });
    }
    group.finish();
}

fn bench_symbols(c: &mut Criterion) {
    let compressor = DocumentSymbolCompressor;
    let cases = load_fixtures("symbols.json");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("symbols");
    for (name, method, params) in &cases {
        group.bench_function(name, |b| {
            b.iter(|| {
                rt.block_on(compressor.intercept(
                    method,
                    black_box(params.clone()),
                    Direction::ServerToClient,
                ))
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_diagnostics,
    bench_completions,
    bench_hover,
    bench_symbols
);
criterion_main!(benches);
