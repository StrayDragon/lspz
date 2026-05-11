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

# Run cargo test --workspace.
test:
    cargo test --workspace

# Run all checks (qa = fmt-check + lint + test + gen-check).
qa: gen-check fmt-check lint test
    @echo "All checks passed!"
    prek run --all-files

alias check := qa
alias ci := qa

# fmt-check only
fmt-check:
    cargo fmt -- --check

# --- SSOT Documentation Generation ---

# Generate all documentation from code (SSOT).
gen-docs:
    python3 scripts/gen-docs.py

# Check documentation drift (CI).
gen-check:
    python3 scripts/gen-docs.py --check

# Generate API docs from code comments.
gen-api-docs:
    python3 scripts/gen-docs.py

# Generate config docs from Config struct.
gen-config-docs:
    python3 scripts/gen-docs.py

# Generate error type docs from LspzError enum.
gen-error-docs:
    python3 scripts/gen-docs.py

# Generate metadata docs from Cargo.toml.
gen-meta-docs:
    @echo "TODO: extract workspace members and deps from Cargo.toml"

# Run Criterion throughput benchmarks.
bench:
    cargo bench -p lspz-core

# Run compression ratio report (quick, no Criterion).
bench-report:
    cargo run --example bench-report -p lspz-core

# Run full benchmark suite + save report.
bench-all:
    cargo bench -p lspz-core
    bash scripts/run-bench.sh
