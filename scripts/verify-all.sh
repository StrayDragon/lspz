#!/usr/bin/env bash
# Full local verification harness for lspz.
# Runs fmt-check, clippy, tests, doc checks, SDD validate, and prek.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

step() {
  echo ""
  echo "==> $*"
}

step "cargo fmt --check"
cargo fmt -- --check

step "cargo clippy"
CLIPPY_CONF_DIR="$ROOT" cargo clippy --all-targets --all-features -- \
  -D warnings \
  -W clippy::cognitive_complexity \
  -W clippy::too_many_lines \
  -W clippy::type_complexity \
  -W clippy::too_many_arguments \
  -W clippy::fn_params_excessive_bools \
  -W clippy::large_enum_variant

step "cargo test --all-features"
cargo test --all-features

step "cargo doc --no-deps --all-features"
cargo doc --no-deps --all-features

step "cargo test --doc --all-features"
cargo test --doc --all-features

step "llman sdd validate --all"
llman sdd validate --all

step "prek run --all-files"
prek run --all-files

echo ""
echo "All verification steps passed."
