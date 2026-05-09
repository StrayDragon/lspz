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

# Run all checks (lint, test).
qa: lint test
    @echo "All checks passed!"
    prek run --all-files

alias ci := qa

# Generate documentation from code (SSOT).
gen-docs:
    python3 scripts/gen-docs.py

# Check if documentation is up-to-date (for CI).
gen-check:
    python3 scripts/gen-docs.py --check
