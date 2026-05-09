#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.10"
# ///
# ruff: noqa: T201
"""Run cargo fmt --check and pretty-print results.

Usage:
    uv run scripts/check-rust-fmt.py
"""
from __future__ import annotations

import subprocess
import sys


def main() -> int:
    result = subprocess.run(
        ["cargo", "fmt", "--", "--check"],  # noqa: S603
    )
    if result.returncode == 0:
        print("cargo fmt: OK")
        return 0
    print("cargo fmt: FAILED (run `just fmt` to fix)", file=sys.stderr)
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
