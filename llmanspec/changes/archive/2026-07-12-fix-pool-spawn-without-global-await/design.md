# Design: fix-pool-spawn-without-global-await

```text
lock → lookup hit? return → unlock
spawn+initialize (no pool lock)
lock → insert if absent (drop loser) → unlock
```
