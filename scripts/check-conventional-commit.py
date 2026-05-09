#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.10"
# ///
# ruff: noqa: T201
"""Validate commit messages follow Conventional Commits format.

Usage:
    uv run scripts/check-conventional-commit.py .git/COMMIT_EDITMSG

Conventional Commits format:
    type(scope)!: description

    Types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert
    Scope: optional
    !: optional breaking change marker
"""
from __future__ import annotations

import re
import sys

CONVENTIONAL_COMMIT_RE = re.compile(
    r"^(?P<type>feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)"
    r"(?:\((?P<scope>[a-z0-9\-]+)\))?"
    r"(?P<breaking>!)?"
    r":\s(?P<description>.+)$",
)

MAX_SUBJECT_LENGTH = 72


def check_commit_msg(msg_path: str) -> int:
    with open(msg_path) as f:
        lines = f.readlines()

    content_lines = [line for line in lines if not line.startswith("#")]

    if not content_lines:
        print("ERROR: empty commit message", file=sys.stderr)
        return 1

    subject = content_lines[0].rstrip("\n")

    match = CONVENTIONAL_COMMIT_RE.match(subject)
    if not match:
        print(
            f"ERROR: commit message does not follow Conventional Commits format:\n"
            f"  {subject}\n\n"
            f"Expected format: type(scope)!: description\n"
            f"Types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert\n"
            f"Example: feat(proxy): add diagnostic compression interceptor",
            file=sys.stderr,
        )
        return 1

    if len(subject) > MAX_SUBJECT_LENGTH:
        print(
            f"ERROR: subject line too long ({len(subject)} > {MAX_SUBJECT_LENGTH}):\n"
            f"  {subject}",
            file=sys.stderr,
        )
        return 1

    print(f"commit message OK: {subject}")
    return 0


def main() -> int:
    if len(sys.argv) < 2:
        print("Usage: check-conventional-commit.py <commit-msg-file>", file=sys.stderr)
        return 1
    return check_commit_msg(sys.argv[1])


if __name__ == "__main__":
    raise SystemExit(main())
