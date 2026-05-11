//! E2E tests with real LSP servers.
//!
//! Each test detects if the target LSP server is installed and gracefully
//! skips if not found. This ensures `cargo test --workspace` always passes
//! regardless of the development environment.

mod common;

use common::LspTestHarness;

/// Rust fixture content — 3 unused vars + 1 type mismatch.
const RUST_FIXTURE: &str = r#"
fn main() {
    let x = 1;
    let y = 2;
    let z = 3;
    let s: String = 42;
}
"#;

/// Go fixture content — unused import + unused var.
const GO_FIXTURE: &str = r#"
package main

import "fmt"

func main() {
	var x int
}
"#;

/// Python fixture content — type error + unused import + unused variable.
const PYTHON_FIXTURE: &str = r#"
import os
import typing

x: str = 42
y = 1
"#;

/// TypeScript fixture content — type mismatch + unused local.
const TS_FIXTURE: &str = r#"
const x: string = 42;
const y = 1;
"#;

#[tokio::test]
async fn test_rust_analyzer_diagnostics() {
    let Some(mut h) =
        LspTestHarness::try_new("rust-analyzer", RUST_FIXTURE, "main.rs", "rust").await
    else {
        return;
    };

    let uri = h.write_source_file(RUST_FIXTURE);
    let diagnostics = h
        .get_diagnostics(&uri)
        .await
        .expect("should get diagnostics");

    let diags = diagnostics["diagnostics"]
        .as_array()
        .expect("should have diagnostics array");

    assert!(
        !diags.is_empty(),
        "rust-analyzer should produce diagnostics for unused vars + type mismatch"
    );
    tracing::info!("rust-analyzer: {} diagnostics", diags.len());
}

#[tokio::test]
async fn test_gopls_diagnostics() {
    let Some(mut h) = LspTestHarness::try_new("gopls", GO_FIXTURE, "main.go", "go").await else {
        return;
    };

    let uri = h.write_source_file(GO_FIXTURE);
    let diagnostics = h
        .get_diagnostics(&uri)
        .await
        .expect("should get diagnostics");

    let diags = diagnostics["diagnostics"]
        .as_array()
        .expect("should have diagnostics array");

    assert!(
        !diags.is_empty(),
        "gopls should produce diagnostics for unused import + unused var"
    );
    tracing::info!("gopls: {} diagnostics", diags.len());
}

#[tokio::test]
async fn test_basedpyright_diagnostics() {
    let Some(mut h) =
        LspTestHarness::try_new("basedpyright", PYTHON_FIXTURE, "main.py", "python").await
    else {
        return;
    };

    let uri = h.write_source_file(PYTHON_FIXTURE);
    let diagnostics = h
        .get_diagnostics(&uri)
        .await
        .expect("should get diagnostics");

    let diags = diagnostics["diagnostics"]
        .as_array()
        .expect("should have diagnostics array");

    assert!(!diags.is_empty(), "basedpyright should produce diagnostics");
    tracing::info!("basedpyright: {} diagnostics", diags.len());
}

#[tokio::test]
async fn test_typescript_language_server_diagnostics() {
    let Some(mut h) = LspTestHarness::try_new(
        "typescript-language-server",
        TS_FIXTURE,
        "test.ts",
        "typescript",
    )
    .await
    else {
        return;
    };

    let uri = h.write_source_file(TS_FIXTURE);
    let diagnostics = h
        .get_diagnostics(&uri)
        .await
        .expect("should get diagnostics");

    let diags = diagnostics["diagnostics"]
        .as_array()
        .expect("should have diagnostics array");

    assert!(
        !diags.is_empty(),
        "typescript-language-server should produce diagnostics"
    );
    tracing::info!("typescript-language-server: {} diagnostics", diags.len());
}
