#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.10"
# ///
# ruff: noqa: T201
"""Run cargo test and pretty-print results.

Usage:
    uv run scripts/check-rust-test.py
"""
from __future__ import annotations

import subprocess
import sys


def main() -> int:
    result = subprocess.run(
        ["cargo", "test"],  # noqa: S603
    )
    if result.returncode == 0:
        print("cargo test: OK")
        return 0
    print("cargo test: FAILED", file=sys.stderr)
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
