#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.10"
# ///
# ruff: noqa: T201
"""Run cargo clippy with strict warnings and pretty-print results.

Usage:
    uv run scripts/check-rust-clippy.py
"""
from __future__ import annotations

import os
import subprocess
import sys


def main() -> int:
    env = os.environ.copy()
    env["CLIPPY_CONF_DIR"] = os.getcwd()

    cmd = [
        "cargo",
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
        "-W",
        "clippy::cognitive_complexity",
        "-W",
        "clippy::too_many_lines",
        "-W",
        "clippy::type_complexity",
        "-W",
        "clippy::too_many_arguments",
        "-W",
        "clippy::fn_params_excessive_bools",
        "-W",
        "clippy::large_enum_variant",
    ]

    result = subprocess.run(cmd, env=env)  # noqa: S603
    if result.returncode == 0:
        print("cargo clippy: OK")
        return 0
    print("cargo clippy: FAILED", file=sys.stderr)
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
