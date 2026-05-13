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

# Build the mdbook.
book:
    cd docs && mdbook build

# Serve mdbook with live reload.
book-serve:
    cd docs && mdbook serve --open

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

# Run full benchmark suite + save report.
bench-all:
    cargo bench
    bash scripts/run-bench.sh
