# Design: fix-completion-cap-pending-prune

## Completions

```rust
// take(max_items) only when max_items > 0
CompletionCompressor { max_items: 0, .. } // default
```

CappingInterceptor remains the config-driven cap for proxy chains.

## Pending map

```rust
struct PendingEntry { method: String, at: Instant }
const PENDING_TTL: Duration = Duration::from_secs(120);

fn track(...): prune_expired(); insert
fn apply_cancel(raw): if method == "$/cancelRequest" { remove id }
```

## Verify harness

`scripts/verify-all.sh`: fmt-check, clippy, test --all-features, doc-check, doc-test, llman validate --all, prek.
`just verify` delegates to it.
