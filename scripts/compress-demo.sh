#!/usr/bin/env bash
# ─── lspz Compression Demo — one-click verification ─────────────
# Run: bash scripts/compress-demo.sh
#
# Builds lspz and runs all 4 compressors against sample data,
# showing byte-level savings for each compressor.
# ─────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_DIR"

echo ""
echo "  ██╗  ██╗███████╗██████╗ ███████╗"
echo "  ██║  ██║██╔════╝╚════██╗╚══███╔╝"
echo "  ███████║█████╗   █████╔╝  ███╔╝ "
echo "  ██╔══██║██╔══╝  ██╔═══╝  ███╔╝  "
echo "  ██║  ██║███████╗███████╗███████╗"
echo "  ╚═╝  ╚═╝╚══════╝╚══════╝╚══════╝"
echo "  LSP Compression Proxy — v0.7.0"
echo ""

# ─── Build ──────────────────────────────────────────────────────
echo "  [1/2] Building lspz-core..."
cargo build -p lspz-core --quiet 2>/dev/null || cargo build -p lspz-core
echo "  ✓ Build complete"
echo ""

# ─── Run Demo ───────────────────────────────────────────────────
echo "  [2/2] Running compression demo..."
echo ""
# Run with color output if supported
cargo run --example compress-demo -p lspz-core 2>&1

echo ""
echo "  ────────────────────────────────────────────────────────"
echo "  To run against a real LSP server:"
echo "    cargo run -- proxy --backend rust-analyzer"
echo "  ────────────────────────────────────────────────────────"
echo ""
