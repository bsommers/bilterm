//! Always-on foreground JSON-RPC socket (`LIVE-01`, `LIVE-02`, `LIVE-03`).
//!
//! The foreground TUI owns the live [`WindowManager`]. [`LiveRpc::start`] registers this process
//! in the instance registry, binds a private `0600` Unix socket and returns a receiver half plus a
//! handler half. The event loop selects on the receiver and passes each request line to
//! [`LiveRpcHandler::handle`], which temporarily lends the live `WindowManager` to an
//! [`AutomationControlPlane`] (a pointer-sized swap, no copy), so `bilterm ctl` / MCP commands run
//! against exactly what the user sees. The caller redraws after every handled request.

use std::path::PathBuf;

use tokio::sync::{mpsc, oneshot};

use super::agent_wait::{AgentWaiters, WAIT_TICK};
use super::http::RpcRequest;
use super::instance_registry::InstanceGuard;
use super::rpc::AutomationControlPlane;
use crate::wm::WindowManager;

/// Receiving half: await [`LiveRpcRx::recv`] in the event loop.
pub struct LiveRpcRx {
    rx: mpsc::Receiver<RpcRequest>,
}

impl LiveRpcRx {
    pub async fn recv(&mut self) -> Option<RpcRequest> {
        self.rx.recv().await
    }
}

/// Handling half: owns the lent-out control plane, parked waiters and the registry guard.
pub struct LiveRpcHandler {
    plane: AutomationControlPlane,
    waiters: AgentWaiters,
    socket: PathBuf,
    _guard: InstanceGuard,
    #[cfg(unix)]
    _acceptor: tokio::task::JoinHandle<()>,
}

/// Interval at which the event loop should call [`LiveRpcHandler::poll_waiters`].
pub const LIVE_WAIT_TICK: std::time::Duration = WAIT_TICK;

/// Entry point.
pub struct LiveRpc;

impl LiveRpc {
    /// Computes this process's instance socket path without binding it, so child PTYs spawned
    /// before the listener is up can already receive `BILTERM_SOCKET`.
    pub fn announce() -> Option<PathBuf> {
        let runtime = crate::secure_fs::private_runtime_dir().ok()?;
        let path = super::instance_registry::instance_socket_path(&runtime, std::process::id());
        crate::pty::set_live_socket(Some(path.clone()));
        Some(path)
    }

    /// Registers, binds and starts accepting. `None` when sockets are unavailable (non-Unix) or
    /// the runtime directory cannot be prepared; the foreground app then runs without it.
    #[cfg(unix)]
    pub fn start(meter: crate::traffic::TrafficMeter) -> Option<(LiveRpcRx, LiveRpcHandler)> {
        use std::os::unix::fs::PermissionsExt;
        let runtime = crate::secure_fs::private_runtime_dir().ok()?;
        let guard = InstanceGuard::register(&runtime, std::process::id()).ok()?;
        let path = guard.socket_path().to_path_buf();
        // A stale socket from a recycled pid is ours to replace; refuse anything else.
        if let Ok(meta) = std::fs::symlink_metadata(&path) {
            use std::os::unix::fs::FileTypeExt;
            if !meta.file_type().is_socket() {
                return None;
            }
            let _ = std::fs::remove_file(&path);
        }
        let listener = tokio::net::UnixListener::bind(&path).ok()?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).ok()?;
        let _ = guard.publish_default_link();
        crate::pty::set_live_socket(Some(path.clone()));
        let (tx, rx) = mpsc::channel::<RpcRequest>(64);
        let acceptor = super::server::spawn_unix_acceptor(listener, tx, meter);
        let plane = AutomationControlPlane::new(
            WindowManager::new_headless(24, 80),
            ratatui::layout::Rect::new(0, 0, 80, 24),
        );
        Some((
            LiveRpcRx { rx },
            LiveRpcHandler { plane, waiters: AgentWaiters::default(), socket: path, _guard: guard, _acceptor: acceptor },
        ))
    }

    #[cfg(not(unix))]
    pub fn start(_meter: crate::traffic::TrafficMeter) -> Option<(LiveRpcRx, LiveRpcHandler)> {
        None
    }
}

impl LiveRpcHandler {
    pub fn socket_path(&self) -> &std::path::Path {
        &self.socket
    }

    /// True while `agent.wait` requests are parked and the caller should tick [`Self::poll_waiters`].
    pub fn has_waiters(&self) -> bool {
        !self.waiters.is_empty()
    }

    /// Runs one request line against the live `wm` and replies.
    pub fn handle(&mut self, wm: &mut WindowManager, line: &str, reply: oneshot::Sender<String>) {
        std::mem::swap(&mut self.plane.wm, wm);
        self.waiters.handle(&mut self.plane, line, reply);
        std::mem::swap(&mut self.plane.wm, wm);
    }

    /// Re-checks parked waiters (deadline / new inbox mail) against the live `wm`.
    pub fn poll_waiters(&mut self, wm: &mut WindowManager) {
        if self.waiters.is_empty() {
            return;
        }
        std::mem::swap(&mut self.plane.wm, wm);
        self.waiters.poll(&self.plane);
        std::mem::swap(&mut self.plane.wm, wm);
    }
}
