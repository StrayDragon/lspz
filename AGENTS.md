# AGENTS.md

Project conventions for lspz (LSP compression proxy).

## Rust

- Edition 2024, stable toolchain (pinned in `rust-toolchain.toml`)
- All cargo commands use `--workspace` flag (workspace split planned per PRD)
- Clippy thresholds configured in `.clippy.toml`; lint flags in `just lint` and `scripts/check-rust-clippy.py` must stay in sync
- Format config in `rustfmt.toml`; nightly-only options are not allowed

## Scripts (`scripts/`)

- **Naming**: kebab-case, `check-` prefix for all validation scripts
- **Template**: `#!/usr/bin/env -S uv run` + PEP 723 `# /// script` block + `raise SystemExit(main())`
- **No underscores** in filenames — prevents accidental Python module import
- Scripts are the single source of truth; justfile and prek hooks delegate to them

## Git Hooks (`prek.toml`)

- Only `repo = "builtin"` + `repo = "local"` — zero network dependency at runtime
- Local hooks: `pass_filenames = false` + `require_serial = true` (avoid cargo concurrent conflicts)
- Stages:
  - **pre-commit**: cargo-fmt, cargo-clippy (+ builtin whitespace/toml/yaml checks)
  - **commit-msg**: conventional commits validation
  - **pre-push**: cargo-test (full tests, not on every commit)

## Commits

- Conventional Commits format: `type(scope)!: description`
- Types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert
- Subject line max 72 characters
- Validated by `scripts/check-conventional-commit.py`

## Task Runner (`justfile`)

- `just check` = fmt-check + lint + test
- `just ci` = prek run --all-files + check (full CI simulation)
- Shell: `bash -euo pipefail`
