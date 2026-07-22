set shell := ["bash", "-euo", "pipefail", "-c"]

_default:
    @just --list

# Install prek hooks and build the project.
setup:
    prek install

# Run cargo fmt (write).
fmt:
    cargo fmt

# Run cargo clippy with strict lints.
lint:
    cargo clippy

# Run cargo test.
test:
    cargo test

# Run all checks (qa = fmt-check + lint + test + doc-check).
qa: fmt-check lint test doc-check
    @echo "All checks passed!"
    prek run --all-files

alias check := qa
alias ci := qa

# Full verification harness (fmt + clippy + tests + docs + SDD + prek).
verify:
    bash scripts/verify-all.sh

# fmt-check only
fmt-check:
    cargo fmt -- --check

# --- Documentation ---

# Build API docs (cargo doc) and open in browser.
doc:
    cargo doc --no-deps --all-features --open

# Check API docs build without errors.
doc-check:
    cargo doc --no-deps --all-features

# Run doc tests (verify /// examples compile).
doc-test:
    cargo test --doc --all-features

# --- Benchmarks ---

# Run Criterion throughput benchmarks.
bench:
    cargo bench

# Run compression ratio report (fixture-based benchmark).
bench-report:
    cargo run --example bench-report

# Run compression demo with sample data (quick verification).
compress-demo:
    cargo run --example compress-demo

# Run full benchmark suite.
bench-all:
    cargo bench

# --- MCP / Inspector ---

# Release binary used by MCP Inspector recipes.
lspz_mcp_bin := "./target/release/lspz"

# Build release `lspz` with MCP feature (stdio tools).
build-mcp:
    cargo build --release --features mcp,cli

# MCP Inspector UI against in-process `lspz mcp --no-daemon` (http://localhost:6274).
# Logs: ~/.cache/lspz-mcp.log — look for `workspace resolved` / `workspace_source`.
mcp-inspector: build-mcp
    npx -y @modelcontextprotocol/inspector \
        -e LSPZ_LOG_LEVEL=info \
        -- {{lspz_mcp_bin}} mcp --no-daemon

# CLI: list MCP tools.
mcp-inspector-list: build-mcp
    npx -y @modelcontextprotocol/inspector --cli \
        -- {{lspz_mcp_bin}} mcp --no-daemon \
        --method tools/list

# CLI: no-uri `get_diagnostics` (workspace scan; prints `workspace_source`).
mcp-inspector-scan: build-mcp
    npx -y @modelcontextprotocol/inspector --cli \
        -- {{lspz_mcp_bin}} mcp --no-daemon \
        --method tools/call \
        --tool-name get_diagnostics \
        --tool-arg '{}'
