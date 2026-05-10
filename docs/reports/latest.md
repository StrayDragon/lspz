# lspz Compression Benchmark Report

**Version**: v0.7.0

**Date**: 2026-05-11

## DiagnosticsCompressor

| Scenario | Raw (T) | Compact (T) | TOON (T) | Δ% (Compact vs Raw) | Δ% (TOON vs Raw) | Δ% (TOON vs Compact) |
|----------|---------|-------------|----------|---------------------|-------------------|----------------------|
| small (3 diagnostics, one duplicate pair) | 158 | 89 | 75 | 43.7% | 52.5% | 15.7% |
| medium (15 diagnostics, mixed duplicates and unique) | 746 | 316 | 260 | 57.6% | 65.1% | 17.7% |
| large (60 diagnostics, many duplicates — TypeScript lint flood) | 2983 | 625 | 575 | 79.0% | 80.7% | 8.0% |
| **Total** | **3887** | **1030** | **910** | **73.5%** | **76.6%** | **11.7%** |

## CompletionCompressor

| Scenario | Raw (T) | Compact (T) | TOON (T) | Δ% (Compact vs Raw) | Δ% (TOON vs Raw) | Δ% (TOON vs Compact) |
|----------|---------|-------------|----------|---------------------|-------------------|----------------------|
| small (5 items) | 174 | 178 | 157 | -2.3% | 9.8% | 11.8% |
| medium (15 items, some shared docs) | 562 | 566 | 475 | -0.7% | 15.5% | 16.1% |
| large (30 items, varied kinds) | 889 | 893 | 719 | -0.4% | 19.1% | 19.5% |
| **Total** | **1625** | **1637** | **1351** | **-0.7%** | **16.9%** | **17.5%** |

## HoverCompressor

| Scenario | Raw (T) | Compact (T) | TOON (T) | Δ% (Compact vs Raw) | Δ% (TOON vs Raw) | Δ% (TOON vs Compact) |
|----------|---------|-------------|----------|---------------------|-------------------|----------------------|
| plaintext (simple) | 60 | 59 | 44 | 1.7% | 26.7% | 25.4% |
| markdown (rich) | 159 | 156 | 141 | 1.9% | 11.3% | 9.6% |
| object MarkedString (language + value form) | 33 | 33 | 7 | 0.0% | 78.8% | 78.8% |
| **Total** | **252** | **248** | **192** | **1.6%** | **23.8%** | **22.6%** |

## DocumentSymbolCompressor

| Scenario | Raw (T) | Compact (T) | TOON (T) | Δ% (Compact vs Raw) | Δ% (TOON vs Raw) | Δ% (TOON vs Compact) |
|----------|---------|-------------|----------|---------------------|-------------------|----------------------|
| small (3 DocumentSymbols, shallow) | 454 | 267 | 150 | 41.2% | 67.0% | 43.8% |
| medium (SymbolInformation flat, 10 symbols) | 474 | 469 | 84 | 1.1% | 82.3% | 82.1% |
| large (DocumentSymbol hierarchical, 25 symbols nested) | 1169 | 670 | 352 | 42.7% | 69.9% | 47.5% |
| **Total** | **2097** | **1406** | **586** | **33.0%** | **72.1%** | **58.3%** |

## Summary

| Compressor | Avg Δ% (Compact vs Raw) | Avg Δ% (TOON vs Raw) |
|------------|------------------------|----------------------|
| DiagnosticsCompressor | 73.5% | 76.6% |
| CompletionCompressor | -0.7% | 16.9% |
| HoverCompressor | 1.6% | 23.8% |
| DocumentSymbolCompressor | 33.0% | 72.1% |
| **Overall** | **26.8%** | **47.3%** |

Each LSP server produces diagnostics with unique patterns. The table below shows how
the DiagnosticsCompressor handles real-world output for each supported LSP server.

### rust-analyzer

| Scenario | Raw (T) | Compact (T) | TOON (T) | Δ% (Compact vs Raw) | Δ% (TOON vs Raw) | Δ% (TOON vs Compact) |
|----------|---------|-------------|----------|---------------------|-------------------|----------------------|
| small (3 diagnostics, 2 unused vars + unresolved ref) | 159 | 89 | 75 | 44.0% | 52.8% | 15.7% |
| medium (10 diagnostics, mixed duplicates + unique) | 507 | 247 | 203 | 51.3% | 60.0% | 17.8% |
| large (25 diagnostics, many duplicates — real-world flood) | 1256 | 535 | 435 | 57.4% | 65.4% | 18.7% |
| **Total** | **1922** | **871** | **713** | **54.7%** | **62.9%** | **18.1%** |

### gopls

| Scenario | Raw (T) | Compact (T) | TOON (T) | Δ% (Compact vs Raw) | Δ% (TOON vs Raw) | Δ% (TOON vs Compact) |
|----------|---------|-------------|----------|---------------------|-------------------|----------------------|
| small (4 diagnostics, 2 unused vars + import + type) | 210 | 105 | 84 | 50.0% | 60.0% | 20.0% |
| medium (13 diagnostics, high dedup potential) | 639 | 199 | 167 | 68.9% | 73.9% | 16.1% |
| large (20 diagnostics, realistic gopls output) | 980 | 311 | 256 | 68.3% | 73.9% | 17.7% |
| **Total** | **1829** | **615** | **507** | **66.4%** | **72.3%** | **17.6%** |

### basedpyright

| Scenario | Raw (T) | Compact (T) | TOON (T) | Δ% (Compact vs Raw) | Δ% (TOON vs Raw) | Δ% (TOON vs Compact) |
|----------|---------|-------------|----------|---------------------|-------------------|----------------------|
| small (4 diagnostics, mixed types) | 219 | 146 | 122 | 33.3% | 44.3% | 16.4% |
| medium (10 diagnostics, mixed duplicates) | 526 | 265 | 218 | 49.6% | 58.6% | 17.7% |
| large (20 diagnostics, realistic pyright output) | 1035 | 485 | 402 | 53.1% | 61.2% | 17.1% |
| **Total** | **1780** | **896** | **742** | **49.7%** | **58.3%** | **17.2%** |

### typescript

| Scenario | Raw (T) | Compact (T) | TOON (T) | Δ% (Compact vs Raw) | Δ% (TOON vs Raw) | Δ% (TOON vs Compact) |
|----------|---------|-------------|----------|---------------------|-------------------|----------------------|
| small (4 diagnostics, mixed TS errors) | 220 | 102 | 86 | 53.6% | 60.9% | 15.7% |
| medium (12 diagnostics, mixed duplicates and unique) | 615 | 216 | 180 | 64.9% | 70.7% | 16.7% |
| large (25 diagnostics, realistic TS project errors) | 1264 | 449 | 389 | 64.5% | 69.2% | 13.4% |
| **Total** | **2099** | **767** | **655** | **63.5%** | **68.8%** | **14.6%** |

## Full Summary

| Benchmark | Avg Δ% (Compact vs Raw) | Avg Δ% (TOON vs Raw) |
|----------|------------------------|----------------------|
| DiagnosticsCompressor | 73.5% | 76.6% |
| CompletionCompressor | -0.7% | 16.9% |
| HoverCompressor | 1.6% | 23.8% |
| DocumentSymbolCompressor | 33.0% | 72.1% |
| rust-analyzer | 54.7% | 62.9% |
| gopls | 66.4% | 72.3% |
| basedpyright | 49.7% | 58.3% |
| typescript | 63.5% | 68.8% |
| **Overall** | **42.7%** | **56.5%** |
