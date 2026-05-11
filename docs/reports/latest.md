# lspz Compression Benchmark Report

**Version**: v0.5.0

**Date**: 2026-05-11

## DiagnosticsCompressor

| Scenario | Original (B) | Compact (B) | Byte Δ% | Original (T) | Compact (T) | Token Δ% |
|----------|-------------|------------|---------|-------------|------------|---------|
| small (3 diagnostics, one duplicate pair) | 577 | 234 | 59.4% | 158 | 89 | 43.7% |
| medium (15 diagnostics, mixed duplicates and unique) | 2748 | 756 | 72.5% | 746 | 316 | 57.6% |
| large (60 diagnostics, many duplicates — TypeScript lint flood) | 10553 | 1034 | 90.2% | 2983 | 625 | 79.0% |
| **Total** | **13878** | **2024** | **85.4%** | **3887** | **1030** | **73.5%** |

## CompletionCompressor

| Scenario | Original (B) | Compact (B) | Byte Δ% | Original (T) | Compact (T) | Token Δ% |
|----------|-------------|------------|---------|-------------|------------|---------|
| small (5 items) | 739 | 647 | 12.4% | 174 | 178 | -2.3% |
| medium (15 items, some shared docs) | 2156 | 1863 | 13.6% | 562 | 566 | -0.7% |
| large (30 items, varied kinds) | 3381 | 2791 | 17.5% | 889 | 893 | -0.4% |
| **Total** | **6276** | **5301** | **15.5%** | **1625** | **1637** | **-0.7%** |

## HoverCompressor

| Scenario | Original (B) | Compact (B) | Byte Δ% | Original (T) | Compact (T) | Token Δ% |
|----------|-------------|------------|---------|-------------|------------|---------|
| plaintext (simple) | 232 | 166 | 28.4% | 60 | 59 | 1.7% |
| markdown (rich) | 456 | 389 | 14.7% | 159 | 156 | 1.9% |
| object MarkedString (language + value form) | 127 | 109 | 14.2% | 33 | 33 | 0.0% |
| **Total** | **815** | **664** | **18.5%** | **252** | **248** | **1.6%** |

## DocumentSymbolCompressor

| Scenario | Original (B) | Compact (B) | Byte Δ% | Original (T) | Compact (T) | Token Δ% |
|----------|-------------|------------|---------|-------------|------------|---------|
| small (3 DocumentSymbols, shallow) | 1549 | 583 | 62.4% | 454 | 267 | 41.2% |
| medium (SymbolInformation flat, 10 symbols) | 1755 | 1213 | 30.9% | 474 | 469 | 1.1% |
| large (DocumentSymbol hierarchical, 25 symbols nested) | 4016 | 1452 | 63.8% | 1169 | 670 | 42.7% |
| **Total** | **7320** | **3248** | **55.6%** | **2097** | **1406** | **33.0%** |

## Summary

| Compressor | Avg Byte Savings | Avg Token Savings |
|------------|-----------------|-------------------|
| DiagnosticsCompressor | 85.4% | 73.5% |
| CompletionCompressor | 15.5% | -0.7% |
| HoverCompressor | 18.5% | 1.6% |
| DocumentSymbolCompressor | 55.6% | 33.0% |
| **Overall** | **43.8%** | **26.8%** |
