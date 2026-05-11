#!/usr/bin/env bash
# One-click compression benchmark: build + report + save
set -euo pipefail

cd "$(dirname "$0")/.."

echo "=== lspz Compression Benchmark Report ==="
echo ""

mkdir -p docs/reports

cargo run --example bench-report -p lspz-core 2>/dev/null | tee docs/reports/latest.md

echo ""
echo "Report saved to docs/reports/latest.md"
