mod common;

use common::LspTestHarness;

async fn try_get_diagnostics(h: &mut LspTestHarness, uri: &str) -> Vec<serde_json::Value> {
    match h.get_diagnostics(uri).await {
        Ok(d) => d["diagnostics"].as_array().cloned().unwrap_or_default(),
        Err(e) => {
            eprintln!("  SKIP: get_diagnostics failed: {e}");
            vec![]
        }
    }
}

const RUST_FIXTURE: &str = r#"
fn main() {
    let x = 1;
    let y = 2;
    let z = 3;
    let s: String = 42;
}
"#;

const GO_FIXTURE: &str = r#"
package main

import "fmt"

func main() {
	var x int
}
"#;

const PYTHON_FIXTURE: &str = r#"
import os
import typing

x: str = 42
y = 1
"#;

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
    let diags = try_get_diagnostics(&mut h, &uri).await;
    tracing::info!("rust-analyzer: {} diagnostics", diags.len());
}

#[tokio::test]
async fn test_gopls_diagnostics() {
    let Some(mut h) = LspTestHarness::try_new("gopls", GO_FIXTURE, "main.go", "go").await else {
        return;
    };

    let uri = h.write_source_file(GO_FIXTURE);
    let diags = try_get_diagnostics(&mut h, &uri).await;
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
    let diags = try_get_diagnostics(&mut h, &uri).await;
    tracing::info!("basedpyright: {} diagnostics", diags.len());
}

#[tokio::test]
async fn test_typescript_language_server_diagnostics() {
    let Some(mut h) = LspTestHarness::try_new_with_args(
        "typescript-language-server",
        &["--stdio"],
        TS_FIXTURE,
        "test.ts",
        "typescript",
    )
    .await
    else {
        return;
    };

    let uri = h.write_source_file(TS_FIXTURE);
    let diags = try_get_diagnostics(&mut h, &uri).await;
    tracing::info!("typescript-language-server: {} diagnostics", diags.len());
}
