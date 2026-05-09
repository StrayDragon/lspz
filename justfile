set shell := ["bash", "-euo", "pipefail", "-c"]

_default:
    @just --list

# Install prek hooks and build the project.
setup:
    prek install
    cargo build --workspace

# Run cargo fmt (write).
fmt:
    cargo fmt

# Check formatting without writing.
fmt-check:
    cargo fmt -- --check

# Run cargo clippy with strict lints.
lint:
    #!/usr/bin/env bash
    set -euo pipefail
    CLIPPY_CONF_DIR="$(pwd)" cargo clippy --workspace --all-targets -- \
        -D warnings \
        -W clippy::cognitive_complexity \
        -W clippy::too_many_lines \
        -W clippy::type_complexity \
        -W clippy::too_many_arguments \
        -W clippy::fn_params_excessive_bools \
        -W clippy::large_enum_variant

# Run cargo test --workspace.
test:
    cargo test --workspace

# Run all checks (fmt check + clippy + test).
check: fmt-check lint test

# Run prek + all checks (full CI simulation).
ci:
    prek run --all-files
    just check

# Install prek git hooks.
prek-install:
    prek install

# Run prek on staged files.
prek-run:
    prek run

# Run prek on all files.
prek-run-all:
    prek run --all-files

# Update prek hook revisions.
prek-update:
    prek auto-update
