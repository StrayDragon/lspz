#!/usr/bin/env bash
# Regenerate benchmark report from fixtures.
# Output: docs/src/benchmarks.md (auto-generated, do not edit manually)
set -euo pipefail

cd "$(dirname "$0")/.."

cargo run --example bench-report 2>/dev/null > docs/src/benchmarks.md

echo "Generated docs/src/benchmarks.md"
