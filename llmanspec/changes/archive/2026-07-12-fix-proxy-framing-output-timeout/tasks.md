# Tasks: fix-proxy-framing-output-timeout

- [x] 1. Export shared MAX_BODY_SIZE; enforce in framing::read_frame and stdin path
- [x] 2. Refactor Proxy message_loop to cancel-safe reader tasks + mpsc channels
- [x] 3. Handshake/shutdown: match response by JSON-RPC id; forward interleaved notifications; wrap receive with 30s timeout
- [x] 4. Honor OutputFormat::Passthrough (skip intercept/rewrite)
- [x] 5. Apply TOON to responses while preserving id (result.format/text)
- [x] 6. Fix build_interceptor_chain blocking_read → async or try_read
- [x] 7. WsTransport: accept Text frames as LSP payloads
- [x] 8. Add/update unit tests for framing size limit, passthrough, toon response id, handshake id match helpers
- [x] 9. Run `just qa` and `llman sdd validate fix-proxy-framing-output-timeout --strict --no-interactive`
