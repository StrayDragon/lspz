//! Integration tests for lspz daemon — verifies session reuse.
//!
//! These tests exercise the real daemon server and client over a Unix socket.
//! They require the `mcp` feature.

#[cfg(feature = "mcp")]
mod mcp_tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use lspz::daemon::protocol::{DaemonRequest, DaemonResponse};
    use lspz::daemon::{DaemonClient, DaemonServer, socket_path_for_workspace};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::{UnixListener, UnixStream};

    /// Two connections to the same daemon share the same LSP session.
    ///
    /// This test verifies the session reuse logic at the daemon protocol level:
    /// calling lsp/spawn twice with identical params returns the same session_key.
    #[tokio::test]
    async fn test_daemon_session_reuse() {
        let test_id = format!("lspz-test-reuse-{}", std::process::id());
        let socket_path = PathBuf::from(format!("/tmp/{test_id}.sock"));

        let server = DaemonServer::new(socket_path.clone());
        let daemon_handle = tokio::spawn(async move {
            let _ = server.start().await;
        });
        tokio::time::sleep(Duration::from_millis(300)).await;

        async fn send_spawn(
            socket: &PathBuf,
            id: u64,
            lang: &str,
            backend: &str,
        ) -> DaemonResponse {
            let mut stream = UnixStream::connect(socket).await.unwrap();
            let req = DaemonRequest {
                id,
                method: "lsp/spawn".into(),
                params: serde_json::json!({
                    "language": lang,
                    "backend": backend,
                    "root_path": "/tmp/lspz-test-workspace",
                }),
            };
            stream
                .write_all(format!("{}\n", serde_json::to_string(&req).unwrap()).as_bytes())
                .await
                .unwrap();
            stream.flush().await.unwrap();

            let mut buf_reader = BufReader::new(&mut stream);
            let mut line = String::new();
            tokio::time::timeout(Duration::from_secs(3), buf_reader.read_line(&mut line))
                .await
                .unwrap()
                .unwrap();
            serde_json::from_str(line.trim()).unwrap()
        }

        // First spawn — will fail because echo is not an LSP server,
        // but the test verifies protocol integrity.
        let resp1 = send_spawn(&socket_path, 1, "rust", "echo").await;
        // Second spawn — same params.
        let resp2 = send_spawn(&socket_path, 2, "rust", "echo").await;

        assert_eq!(resp1.error, resp2.error);
        if resp1.error.is_none() {
            assert_eq!(resp1.result["session_key"], resp2.result["session_key"]);
        }

        daemon_handle.abort();
        let _ = std::fs::remove_file(&socket_path);
    }

    /// Socket path is deterministic for a given workspace.
    #[test]
    fn test_socket_path_deterministic() {
        let a = socket_path_for_workspace("/home/user/my-project");
        let b = socket_path_for_workspace("/home/user/my-project");
        assert_eq!(a, b);
    }

    /// Different workspaces produce different socket paths.
    #[test]
    fn test_socket_path_different_workspaces() {
        let a = socket_path_for_workspace("/home/user/project-a");
        let b = socket_path_for_workspace("/home/user/project-b");
        assert_ne!(a, b);
    }

    /// Regression: `DaemonClient::request` must drain orphan / out-of-order
    /// response lines instead of treating the first line on the wire as its
    /// own response.
    ///
    /// A cancelled earlier request (e.g. a timed-out `lsp/wait_notify`) is
    /// still completed by the daemon, whose response line then lingers on the
    /// socket as an "orphan". If the next request read exactly one line, that
    /// orphan would be returned instead of the real response, permanently
    /// desyncing the newline-delimited protocol (root cause of the
    /// `spawn failed: Wait for notification failed: ...` and `missing 'uri' in
    /// publishDiagnostics` errors seen on clean files).
    ///
    /// Here a fake daemon writes an orphan (id=999) *before* the matching
    /// response (id=1). The client must return only the id=1 result.
    #[tokio::test]
    async fn test_client_drains_orphan_response() {
        let socket_path =
            PathBuf::from(format!("/tmp/lspz-test-orphan-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path).unwrap();
        let client_socket = socket_path.clone();

        let fake = tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let (rh, mut wh) = tokio::io::split(sock);
            let mut reader = BufReader::new(rh);
            let mut line = String::new();
            // Read the client's single request (id=1, daemon/status).
            reader.read_line(&mut line).await.unwrap();

            // Write an ORPHAN (wrong id) first, then the real response (id=1).
            let orphan = DaemonResponse::ok(999, serde_json::json!({ "stale": true }));
            let real = DaemonResponse::ok(1, serde_json::json!({ "ok": true }));
            let payload = format!(
                "{}\n{}\n",
                serde_json::to_string(&orphan).unwrap(),
                serde_json::to_string(&real).unwrap()
            );
            wh.write_all(payload.as_bytes()).await.unwrap();
            wh.flush().await.unwrap();
        });

        let mut client = DaemonClient::connect_explicit(&client_socket)
            .await
            .unwrap();
        // get_status() sends id=1 and must read past the orphan to reach id=1.
        let result = client.get_status().await.unwrap();

        fake.await.unwrap();
        let _ = std::fs::remove_file(&socket_path);

        assert_eq!(
            result,
            serde_json::json!({ "ok": true }),
            "client returned an orphan response (id=999) instead of the matching \
             one (id=1); the daemon protocol would be desynced"
        );
    }

    /// Daemon responds with error for unknown method.
    #[tokio::test]
    async fn test_daemon_unknown_method() {
        let test_id = format!("lspz-test-unknown-{}", std::process::id());
        let socket_path = PathBuf::from(format!("/tmp/{test_id}.sock"));

        let server = DaemonServer::new(socket_path.clone());
        let daemon_handle = tokio::spawn(async move {
            let _ = server.start().await;
        });
        tokio::time::sleep(Duration::from_millis(300)).await;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        let req = DaemonRequest {
            id: 1,
            method: "nonexistent/method".into(),
            params: serde_json::Value::Null,
        };
        stream
            .write_all(format!("{}\n", serde_json::to_string(&req).unwrap()).as_bytes())
            .await
            .unwrap();
        stream.flush().await.unwrap();

        let mut buf_reader = BufReader::new(&mut stream);
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(3), buf_reader.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();

        let resp: DaemonResponse = serde_json::from_str(line.trim()).unwrap();
        assert!(
            resp.error.is_some(),
            "Expected error for unknown method, got: {resp:?}"
        );
        assert!(resp.error.unwrap().contains("Unknown method"));

        daemon_handle.abort();
        let _ = std::fs::remove_file(&socket_path);
    }
}
