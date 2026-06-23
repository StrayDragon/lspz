//! Daemon server — long-lived LSP session manager.
//!
//! Accepts connections on a Unix domain socket, multiplexes LSP requests
//! over the internal [`LspPool`], and returns results as JSON-RPC responses.

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::Context;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use super::protocol::DaemonResponse;
use super::status::DaemonStatus;
use crate::mcp::LspPool;

/// The daemon server.
pub struct DaemonServer {
    socket_path: PathBuf,
    pool: Arc<Mutex<LspPool>>,
    status: Arc<Mutex<DaemonStatus>>,
    /// Count of currently-connected clients. The idle reaper consults this to
    /// avoid exiting while a client (e.g. a live `lspz mcp` process) is still
    /// attached, even if the pool is momentarily empty.
    active_connections: Arc<AtomicU64>,
}

impl DaemonServer {
    /// Create a new daemon server.
    ///
    /// `workspace_root` should be a canonicalized absolute path uniquely
    /// identifying the project workspace.
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            pool: Arc::new(Mutex::new(LspPool::new())),
            status: Arc::new(Mutex::new(DaemonStatus::default())),
            active_connections: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Start the daemon, listening on the Unix socket.
    ///
    /// This spawns a cleanup task on SIGTERM/SIGINT.
    /// Returns immediately (the daemon runs until signalled).
    pub async fn start(self) -> Result<(), anyhow::Error> {
        // Remove stale socket file
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path).with_context(|| {
                format!("Failed to remove stale socket: {:?}", self.socket_path)
            })?;
        }

        // Ensure parent directory exists
        if let Some(parent) = self.socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(&self.socket_path)
            .with_context(|| format!("Failed to bind to {:?}", self.socket_path))?;

        let socket_path = self.socket_path.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            info!("Daemon received shutdown signal");
            let _ = std::fs::remove_file(&socket_path);
            std::process::exit(0);
        });

        info!(path = %self.socket_path.display(), "Daemon listening");

        let pool = self.pool.clone();
        let status = self.status.clone();
        let active_connections = self.active_connections.clone();

        spawn_idle_reaper(
            pool.clone(),
            status.clone(),
            active_connections.clone(),
            self.socket_path.clone(),
        );

        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    active_connections.fetch_add(1, Ordering::Relaxed);
                    debug!(
                        active = active_connections.load(Ordering::Relaxed),
                        "Daemon: new client connection"
                    );
                    let pool = pool.clone();
                    let status = status.clone();
                    let conns = active_connections.clone();
                    tokio::spawn(async move {
                        // Decrement on task exit — including panic/abort, via Drop.
                        let _guard = ConnectionGuard(conns);
                        if let Err(e) = handle_client(stream, pool, status).await {
                            warn!(error = %e, "Client handler exited with error");
                        }
                    });
                }
                Err(e) => {
                    error!(error = %e, "Daemon accept error");
                }
            }
        }
    }
}

/// RAII guard that decrements the daemon's active-connection counter when
/// dropped. Ensures the count stays accurate even if a handler task panics or
/// is aborted, which the idle reaper relies on to decide when the daemon is
/// truly unused.
struct ConnectionGuard(Arc<AtomicU64>);

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Reclaim an idle LSP session after this long without I/O.
///
/// Dropping the session kills its child language-server process, freeing the
/// bulk of the memory.
const SESSION_IDLE_TTL: Duration = Duration::from_secs(10 * 60);

/// Once the pool is empty **and** no client is connected, wait this long before
/// the daemon shuts itself down. This is the only safety net for daemons that
/// short-lived MCP clients detached via `setsid()` and then crashed: there is
/// no one left to send `daemon/shutdown`.
const DAEMON_IDLE_TTL: Duration = Duration::from_secs(10 * 60);

/// How often the reaper rechecks idle state.
const REAPER_CHECK_INTERVAL: Duration = Duration::from_secs(60);

/// Spawn the background idle reaper.
///
/// Periodically reaps quiet LSP sessions and, once the daemon has had no
/// sessions and no connections for [`DAEMON_IDLE_TTL`], removes the socket
/// file and exits the process.
fn spawn_idle_reaper(
    pool: Arc<Mutex<LspPool>>,
    status: Arc<Mutex<DaemonStatus>>,
    active_connections: Arc<AtomicU64>,
    socket_path: PathBuf,
) {
    tokio::spawn(async move {
        let mut daemon_idle_since: Option<Instant> = None;
        loop {
            tokio::time::sleep(REAPER_CHECK_INTERVAL).await;

            // Reap idle sessions first — this may empty the pool.
            let reaped = pool.lock().await.reap_idle(SESSION_IDLE_TTL);
            if reaped > 0 {
                info!(reaped, "Reaped idle LSP sessions");
                // Drop status entries whose backing session no longer exists,
                // so `daemon list` does not show ghost sessions.
                let live_keys = pool.lock().await.session_keys();
                status
                    .lock()
                    .await
                    .sessions
                    .retain(|info| live_keys.contains(&info.key));
            }

            let busy = {
                let pool_guard = pool.lock().await;
                !pool_guard.is_empty() || active_connections.load(Ordering::Relaxed) > 0
            };

            if busy {
                if daemon_idle_since.is_some() {
                    debug!("Daemon active again, cancelling pending self-exit");
                }
                daemon_idle_since = None;
            } else if daemon_idle_since.is_none() {
                daemon_idle_since = Some(Instant::now());
                info!(
                    ttl_secs = DAEMON_IDLE_TTL.as_secs(),
                    "Daemon is idle; will self-exit if it stays unused"
                );
            } else if daemon_idle_since.unwrap().elapsed() >= DAEMON_IDLE_TTL {
                info!(
                    socket = %socket_path.display(),
                    "Daemon idle timeout reached, shutting down"
                );
                let _ = std::fs::remove_file(&socket_path);
                std::process::exit(0);
            }
        }
    });
}

/// Handle a single client connection.
async fn handle_client(
    stream: UnixStream,
    pool: Arc<Mutex<LspPool>>,
    status: Arc<Mutex<DaemonStatus>>,
) -> Result<(), anyhow::Error> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let n = buf_reader.read_line(&mut line).await?;
        if n == 0 {
            break; // EOF
        }

        let request: super::protocol::DaemonRequest = match serde_json::from_str(line.trim()) {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "Failed to parse client request");
                let resp = DaemonResponse::err(0, format!("Parse error: {e}"));
                let json = serde_json::to_string(&resp)?;
                writer.write_all(format!("{json}\n").as_bytes()).await?;
                continue;
            }
        };

        let response = dispatch(request, &pool, &status).await;

        let json = serde_json::to_string(&response)?;
        writer.write_all(format!("{json}\n").as_bytes()).await?;
    }

    Ok(())
}

/// Dispatch a client request to the appropriate handler.
async fn dispatch(
    req: super::protocol::DaemonRequest,
    pool: &Arc<Mutex<LspPool>>,
    status: &Arc<Mutex<DaemonStatus>>,
) -> DaemonResponse {
    let id = req.id;

    match req.method.as_str() {
        "lsp/spawn" => handle_spawn(id, &req.params, pool, status).await,
        "lsp/request" => handle_lsp_request(id, &req.params, pool, status).await,
        "lsp/notify" => handle_lsp_notify(id, &req.params, pool, status).await,
        "lsp/wait_notify" => handle_wait_notify(id, &req.params, pool, status).await,
        "daemon/status" => handle_status(id, status).await,
        "daemon/shutdown" => {
            info!("Daemon shutdown requested by client");
            let _ = std::fs::remove_file(std::env::var("LSPZ_SOCKET").unwrap_or_default());
            // Spawn so response is sent before exit
            tokio::spawn(async {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                std::process::exit(0);
            });
            DaemonResponse::ok(id, serde_json::json!({"ok": true}))
        }
        _ => DaemonResponse::err(id, format!("Unknown method: {}", req.method)),
    }
}

/// Handle `lsp/spawn` — get or create an LSP session.
async fn handle_spawn(
    id: u64,
    params: &serde_json::Value,
    pool: &Arc<Mutex<LspPool>>,
    status: &Arc<Mutex<DaemonStatus>>,
) -> DaemonResponse {
    let spawn: super::protocol::SpawnParams = match serde_json::from_value(params.clone()) {
        Ok(s) => s,
        Err(e) => return DaemonResponse::err(id, format!("Invalid params: {e}")),
    };

    let key = match &spawn.root_path {
        Some(root) => format!("{}:{}:{}", spawn.language, spawn.backend, root),
        None => format!("{}:{}", spawn.language, spawn.backend),
    };

    // Check if already spawned
    {
        let mut pool_guard = pool.lock().await;
        match pool_guard
            .get_or_spawn(
                &spawn.language,
                &spawn.backend,
                spawn.root_path.as_deref(),
                &spawn.extra_args,
            )
            .await
        {
            Ok(_session) => {
                // Record stats
                let mut s = status.lock().await;
                s.touch_session(
                    &key,
                    &spawn.language,
                    &spawn.backend,
                    spawn.root_path.clone(),
                );
                DaemonResponse::ok(
                    id,
                    serde_json::json!({
                        "session_key": key,
                        "status": "ready",
                    }),
                )
            }
            Err(e) => DaemonResponse::err(id, format!("Failed to spawn LSP session: {e}")),
        }
    }
}

/// Handle `lsp/request` — send an LSP request through a session.
///
/// Lock the pool mutex here to get mutable access to the session.
async fn handle_lsp_request(
    id: u64,
    params: &serde_json::Value,
    pool: &Arc<Mutex<LspPool>>,
    status: &Arc<Mutex<DaemonStatus>>,
) -> DaemonResponse {
    let req: super::protocol::LspRequestParams = match serde_json::from_value(params.clone()) {
        Ok(r) => r,
        Err(e) => return DaemonResponse::err(id, format!("Invalid params: {e}")),
    };
    status.lock().await.touch_by_key(&req.session_key);

    let mut pool_guard = pool.lock().await;
    let session = match pool_guard.get_mut_by_key(&req.session_key) {
        Ok(s) => s,
        Err(e) => return DaemonResponse::err(id, e.to_string()),
    };

    match session.send_request(&req.method, req.params).await {
        Ok(result) => DaemonResponse::ok(id, result),
        Err(e) => DaemonResponse::err(id, format!("LSP request failed: {e}")),
    }
}

/// Handle `lsp/notify` — send an LSP notification through a session.
async fn handle_lsp_notify(
    id: u64,
    params: &serde_json::Value,
    pool: &Arc<Mutex<LspPool>>,
    status: &Arc<Mutex<DaemonStatus>>,
) -> DaemonResponse {
    let req: super::protocol::LspNotifyParams = match serde_json::from_value(params.clone()) {
        Ok(r) => r,
        Err(e) => return DaemonResponse::err(id, format!("Invalid params: {e}")),
    };
    status.lock().await.touch_by_key(&req.session_key);

    let mut pool_guard = pool.lock().await;
    let session = match pool_guard.get_mut_by_key(&req.session_key) {
        Ok(s) => s,
        Err(e) => return DaemonResponse::err(id, e.to_string()),
    };

    match session.send_notification(&req.method, req.params).await {
        Ok(()) => DaemonResponse::ok(id, serde_json::json!({"ok": true})),
        Err(e) => DaemonResponse::err(id, format!("LSP notify failed: {e}")),
    }
}

/// Handle `lsp/wait_notify` — wait for a notification from the LSP server.
async fn handle_wait_notify(
    id: u64,
    params: &serde_json::Value,
    pool: &Arc<Mutex<LspPool>>,
    status: &Arc<Mutex<DaemonStatus>>,
) -> DaemonResponse {
    let req: super::protocol::WaitNotifyParams = match serde_json::from_value(params.clone()) {
        Ok(r) => r,
        Err(e) => return DaemonResponse::err(id, format!("Invalid params: {e}")),
    };
    status.lock().await.touch_by_key(&req.session_key);

    let mut pool_guard = pool.lock().await;
    let session = match pool_guard.get_mut_by_key(&req.session_key) {
        Ok(s) => s,
        Err(e) => return DaemonResponse::err(id, e.to_string()),
    };

    // The client may pass a deadline (`timeout_ms`). The daemon enforces it so
    // it always writes a response *before* the client gives up. Without this,
    // a client that cancels its `lsp_wait_notify` future at its own (shorter)
    // deadline would leave an orphan response on the wire and desync the
    // newline-delimited protocol — see the analysis of the spawn/desync bug.
    let deadline = req.timeout_ms.map(Duration::from_millis);

    let result = if let Some(uri) = &req.filter_uri {
        let uri_clone = uri.clone();
        let fut = session.wait_for_notification_where(&req.method, move |p| {
            p.get("uri").and_then(serde_json::Value::as_str) == Some(&uri_clone)
        });
        apply_wait_deadline(fut, deadline, &req.method).await
    } else {
        let fut = session.wait_for_notification(&req.method);
        apply_wait_deadline(fut, deadline, &req.method).await
    };

    match result {
        Ok(params) => DaemonResponse::ok(id, params),
        Err(e) => DaemonResponse::err(id, format!("Wait for notification failed: {e}")),
    }
}

/// Run a notification-wait future, optionally bounded by a client-supplied
/// deadline. When the deadline elapses, return a timeout error instead of
/// continuing to wait on the LSP transport.
async fn apply_wait_deadline<F>(
    fut: F,
    deadline: Option<Duration>,
    method: &str,
) -> Result<serde_json::Value, anyhow::Error>
where
    F: Future<Output = Result<serde_json::Value, anyhow::Error>>,
{
    match deadline {
        Some(d) => match tokio::time::timeout(d, fut).await {
            Ok(inner) => inner,
            Err(_) => Err(anyhow::anyhow!(
                "timeout waiting for '{method}' notification"
            )),
        },
        None => fut.await,
    }
}

/// Handle `daemon/status` — return current daemon state.
async fn handle_status(id: u64, status: &Arc<Mutex<DaemonStatus>>) -> DaemonResponse {
    let s = status.lock().await;
    let json = serde_json::to_value(&*s).unwrap_or(serde_json::Value::Null);
    DaemonResponse::ok(id, json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Transport;
    use crate::error::LspzError;
    use crate::mcp::{LspPool, LspSession};

    /// A transport whose `receive()` never completes, simulating an LSP server
    /// that never publishes the waited-for notification.
    struct PendingTransport;

    #[async_trait::async_trait]
    impl Transport for PendingTransport {
        async fn receive(&mut self) -> Result<Vec<u8>, LspzError> {
            std::future::pending().await
        }
        async fn send(&mut self, _data: &[u8]) -> Result<(), LspzError> {
            Ok(())
        }
    }

    /// `handle_wait_notify` must honour `timeout_ms` instead of falling back to
    /// the LSP session's internal ~30s wait. Enforcing the deadline on the
    /// daemon side is what lets the client await the response directly (no
    /// cancellation, no orphan response line). A timeout here must therefore
    /// arrive at ~`timeout_ms`, not 30s.
    #[tokio::test]
    async fn test_wait_notify_respects_client_timeout_ms() {
        let pool = Arc::new(Mutex::new(LspPool::new()));
        pool.lock().await.insert_session_for_test(
            "rust:fake:/tmp",
            LspSession::with_transport(Box::new(PendingTransport)),
        );

        let params = serde_json::json!({
            "session_key": "rust:fake:/tmp",
            "method": "textDocument/publishDiagnostics",
            "timeout_ms": 200,
        });

        let start = std::time::Instant::now();
        let status = Arc::new(Mutex::new(DaemonStatus::default()));
        let resp = handle_wait_notify(1, &params, &pool, &status).await;
        let elapsed = start.elapsed();

        assert!(
            resp.error.is_some(),
            "expected a timeout error, got success: {:?}",
            resp.result
        );
        assert!(
            elapsed >= Duration::from_millis(150),
            "waited only {elapsed:?}, expected ~200ms"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "waited {elapsed:?}; the daemon did NOT honour timeout_ms and fell \
             back to the long default wait (would orphan a cancelled client)"
        );
    }
}
