# Design: fix-file-uri-percent-decode

```rust
path_from_file_uri("file:///tmp/a%20b.rs") -> Ok("/tmp/a b.rs")
```
