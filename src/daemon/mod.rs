//! Persistent Background Session Daemon (`bilterm daemon`, `DAEM-01`, `DAEM-02`).
//!
//! Decouples PTY and [`SessionTree`] lifecycles from individual TUI or `--gui` client windows:
//! - Safe stale Unix domain socket probing (`probe_or_clean_stale_socket`) that never unlinks
//!   on `ErrorKind::PermissionDenied`.
//! - `Ord::clamp`-safe multi-client viewport negotiation (`negotiate_session_viewport`) across
//!   simultaneous TUI and native `--gui` attachments.
//! - Subtree freeze/thaw, broadcast fan-out, derez termination, and atomic journal checkpointing.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, ErrorKind};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::secure_fs::private_runtime_dir;
use crate::wm::journal::{
    PaneJournalRecord, SessionJournalRecord, WorkspaceJournal,
};
use crate::wm::session_tree::{SessionNodeKind, SessionTree};

/// Metadata for a live TUI or `--gui` client attached to a daemon session (`DAEM-02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachedClient {
    pub client_id: u64,
    pub session_name: String,
    pub is_gui: bool,
    pub cols: u16,
    pub rows: u16,
    pub font_scale_pct: u16,
}

/// Summary of a daemon-hosted session returned by `ListSessions` (`DAEM-01`, `DAEM-02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonSessionSummary {
    pub name: String,
    pub kind: SessionNodeKind,
    pub parent: Option<String>,
    pub pane_count: usize,
    pub attached_clients: usize,
    pub frozen: bool,
    pub effective_cols: u16,
    pub effective_rows: u16,
}

/// Wire protocol requests accepted by the background session daemon (`DAEM-01`, `DAEM-02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaemonRequest {
    Ping,
    ListSessions,
    Attach {
        client_id: u64,
        session_name: String,
        is_gui: bool,
        cols: u16,
        rows: u16,
        font_scale_pct: u16,
    },
    Detach {
        client_id: u64,
    },
    ResizeClient {
        client_id: u64,
        cols: u16,
        rows: u16,
        font_scale_pct: u16,
    },
    BranchSession {
        parent: String,
        child: String,
    },
    GraftSession {
        src: String,
        dst: String,
    },
    FreezeSubtree {
        target: String,
    },
    ThawSubtree {
        target: String,
    },
    BroadcastSubtree {
        target: String,
        command: String,
    },
    Derez {
        target: String,
        confirmed: bool,
    },
    Checkpoint,
    Shutdown {
        confirmed: bool,
    },
}

/// Wire protocol responses emitted by the background session daemon (`DAEM-01`, `DAEM-02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaemonResponse {
    Pong {
        pid: u32,
        uptime_secs: u64,
        session_count: usize,
        client_count: usize,
    },
    SessionList {
        sessions: Vec<DaemonSessionSummary>,
        tree_lines: Vec<String>,
    },
    Attached {
        session_name: String,
        effective_cols: u16,
        effective_rows: u16,
        client_count: usize,
    },
    Detached {
        client_id: u64,
    },
    Ok {
        message: String,
    },
    Error {
        message: String,
    },
}

/// Computes `Ord::clamp`-safe effective `(cols, rows)` across all clients attached to a session (`DAEM-02`).
///
/// Never panics on `0×0` client dimensions or extreme `font_scale_pct` values.
pub fn negotiate_session_viewport(clients: &[&AttachedClient]) -> (u16, u16) {
    if clients.is_empty() {
        return (80, 24);
    }
    let mut min_cols = u16::MAX;
    let mut min_rows = u16::MAX;
    for client in clients {
        let scale = u32::from(client.font_scale_pct.clamp(50, 300));
        let raw_cols = u32::from(client.cols.max(1));
        let raw_rows = u32::from(client.rows.max(1));
        let eff_cols = ((raw_cols * 100) / scale).clamp(20, 500) as u16;
        let eff_rows = ((raw_rows * 100) / scale).clamp(6, 200) as u16;
        min_cols = min_cols.min(eff_cols);
        min_rows = min_rows.min(eff_rows);
    }
    (min_cols.clamp(20, 500), min_rows.clamp(6, 200))
}

/// In-memory coordinator state for the persistent background session daemon (`DAEM-01`, `DAEM-02`).
#[derive(Clone, Debug)]
pub struct DaemonServerState {
    pub session_tree: SessionTree,
    pub clients: Vec<AttachedClient>,
    pub session_panes: BTreeMap<String, usize>,
    pub broadcast_log: Vec<(String, String)>,
    pub journal_dir: Option<PathBuf>,
    pub running: bool,
    pub started_at: Instant,
}

impl Default for DaemonServerState {
    fn default() -> Self {
        Self::new()
    }
}

impl DaemonServerState {
    /// Creates a new running [`DaemonServerState`] with the default `"main"` rooted session.
    pub fn new() -> Self {
        let mut session_panes = BTreeMap::new();
        session_panes.insert("main".to_string(), 1);
        Self {
            session_tree: SessionTree::new(),
            clients: Vec::new(),
            session_panes,
            broadcast_log: Vec::new(),
            journal_dir: None,
            running: true,
            started_at: Instant::now(),
        }
    }

    /// Creates a [`DaemonServerState`] configured to persist checkpoints into `journal_dir`.
    pub fn with_journal_dir(journal_dir: PathBuf) -> Self {
        let mut state = Self::new();
        state.journal_dir = Some(journal_dir);
        state
    }

    /// Returns the default Unix domain socket path inside the `0700` private runtime directory (`DAEM-01`).
    pub fn default_socket_path() -> PathBuf {
        private_runtime_dir()
            .unwrap_or_else(|_| std::env::temp_dir())
            .join("daemon.sock")
    }

    /// Probes `sock_path` to check if a live daemon is listening (`Ok(true)`), or cleans up a
    /// stale socket (`Ok(false)`) when connection fails with `ConnectionRefused` or `NotFound` (`DAEM-01`).
    ///
    /// **Security Invariant:** NEVER unlinks `sock_path` on `ErrorKind::PermissionDenied`.
    pub fn probe_or_clean_stale_socket(sock_path: &Path) -> io::Result<bool> {
        match fs::symlink_metadata(sock_path) {
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e),
        }

        #[cfg(unix)]
        {
            match UnixStream::connect(sock_path) {
                Ok(_stream) => Ok(true),
                Err(e)
                    if matches!(
                        e.kind(),
                        ErrorKind::ConnectionRefused | ErrorKind::NotFound
                    ) =>
                {
                    match fs::remove_file(sock_path) {
                        Ok(()) => Ok(false),
                        Err(rm_err) if rm_err.kind() == ErrorKind::NotFound => Ok(false),
                        Err(rm_err) => Err(rm_err),
                    }
                }
                Err(e) => Err(e),
            }
        }
        #[cfg(not(unix))]
        {
            Ok(false)
        }
    }

    /// Computes the negotiated `(cols, rows)` viewport for `session_name`.
    pub fn effective_viewport_for_session(&self, session_name: &str) -> (u16, u16) {
        let attached: Vec<&AttachedClient> = self
            .clients
            .iter()
            .filter(|c| c.session_name == session_name)
            .collect();
        negotiate_session_viewport(&attached)
    }

    /// Builds [`DaemonSessionSummary`] entries for all sessions in [`Self::session_tree`].
    pub fn session_summaries(&self) -> Vec<DaemonSessionSummary> {
        self.session_tree
            .nodes
            .iter()
            .map(|node| {
                let attached_clients = self
                    .clients
                    .iter()
                    .filter(|c| c.session_name == node.name)
                    .count();
                let (effective_cols, effective_rows) =
                    self.effective_viewport_for_session(&node.name);
                let pane_count = self
                    .session_panes
                    .get(&node.name)
                    .copied()
                    .unwrap_or(node.pane_count.max(1));
                DaemonSessionSummary {
                    name: node.name.clone(),
                    kind: node.kind,
                    parent: node.parent.clone(),
                    pane_count,
                    attached_clients,
                    frozen: node.frozen,
                    effective_cols,
                    effective_rows,
                }
            })
            .collect()
    }

    /// Processes a single [`DaemonRequest`] and mutates daemon state atomically (`DAEM-01`, `DAEM-02`).
    pub fn handle_request(&mut self, req: DaemonRequest) -> DaemonResponse {
        match req {
            DaemonRequest::Ping => DaemonResponse::Pong {
                pid: std::process::id(),
                uptime_secs: self.started_at.elapsed().as_secs(),
                session_count: self.session_tree.nodes.len(),
                client_count: self.clients.len(),
            },
            DaemonRequest::ListSessions => DaemonResponse::SessionList {
                sessions: self.session_summaries(),
                tree_lines: self.session_tree.render_herdr_tree_lines("main"),
            },
            DaemonRequest::Attach {
                client_id,
                session_name,
                is_gui,
                cols,
                rows,
                font_scale_pct,
            } => {
                let target = if session_name.trim().is_empty() {
                    "main".to_string()
                } else {
                    session_name.trim().to_string()
                };
                if let Some((parent, child)) = target.split_once('/') {
                    let _ = self.session_tree.branch(parent, child);
                    self.session_panes.entry(parent.to_string()).or_insert(1);
                    self.session_panes.entry(child.to_string()).or_insert(1);
                } else if self.session_tree.node(&target).is_none() {
                    self.session_tree
                        .upsert_session(&target, SessionNodeKind::Rooted, None);
                    self.session_panes.entry(target.clone()).or_insert(1);
                }

                let effective_target = target
                    .split_once('/')
                    .map(|(_, c)| c.to_string())
                    .unwrap_or(target);

                self.clients.retain(|c| c.client_id != client_id);
                self.clients.push(AttachedClient {
                    client_id,
                    session_name: effective_target.clone(),
                    is_gui,
                    cols,
                    rows,
                    font_scale_pct: font_scale_pct.clamp(50, 300),
                });

                let (effective_cols, effective_rows) =
                    self.effective_viewport_for_session(&effective_target);
                let client_count = self
                    .clients
                    .iter()
                    .filter(|c| c.session_name == effective_target)
                    .count();
                DaemonResponse::Attached {
                    session_name: effective_target,
                    effective_cols,
                    effective_rows,
                    client_count,
                }
            }
            DaemonRequest::Detach { client_id } => {
                self.clients.retain(|c| c.client_id != client_id);
                DaemonResponse::Detached { client_id }
            }
            DaemonRequest::ResizeClient {
                client_id,
                cols,
                rows,
                font_scale_pct,
            } => {
                if let Some(client) = self.clients.iter_mut().find(|c| c.client_id == client_id) {
                    client.cols = cols;
                    client.rows = rows;
                    client.font_scale_pct = font_scale_pct.clamp(50, 300);
                    let session_name = client.session_name.clone();
                    let (effective_cols, effective_rows) =
                        self.effective_viewport_for_session(&session_name);
                    let client_count = self
                        .clients
                        .iter()
                        .filter(|c| c.session_name == session_name)
                        .count();
                    DaemonResponse::Attached {
                        session_name,
                        effective_cols,
                        effective_rows,
                        client_count,
                    }
                } else {
                    DaemonResponse::Error {
                        message: format!("client {client_id} is not attached"),
                    }
                }
            }
            DaemonRequest::BranchSession { parent, child } => {
                match self.session_tree.branch(&parent, &child) {
                    Ok(()) => {
                        self.session_panes.entry(parent.clone()).or_insert(1);
                        self.session_panes.entry(child.clone()).or_insert(1);
                        DaemonResponse::Ok {
                            message: format!("branched session '{child}' under '{parent}'"),
                        }
                    }
                    Err(message) => DaemonResponse::Error { message },
                }
            }
            DaemonRequest::GraftSession { src, dst } => {
                match self.session_tree.graft(&src, &dst) {
                    Ok(()) => DaemonResponse::Ok {
                        message: format!("grafted subtree '{src}' under '{dst}'"),
                    },
                    Err(message) => DaemonResponse::Error { message },
                }
            }
            DaemonRequest::FreezeSubtree { target } => {
                let affected = self.session_tree.set_subtree_frozen(&target, true);
                if affected.is_empty() {
                    DaemonResponse::Error {
                        message: format!("session '{target}' not found"),
                    }
                } else {
                    DaemonResponse::Ok {
                        message: format!("froze {} session(s): {}", affected.len(), affected.join(", ")),
                    }
                }
            }
            DaemonRequest::ThawSubtree { target } => {
                let affected = self.session_tree.set_subtree_frozen(&target, false);
                if affected.is_empty() {
                    DaemonResponse::Error {
                        message: format!("session '{target}' not found"),
                    }
                } else {
                    DaemonResponse::Ok {
                        message: format!("thawed {} session(s): {}", affected.len(), affected.join(", ")),
                    }
                }
            }
            DaemonRequest::BroadcastSubtree { target, command } => {
                let names = self.session_tree.subtree_session_names(&target);
                if names.is_empty() {
                    return DaemonResponse::Error {
                        message: format!("session '{target}' not found"),
                    };
                }
                let mut delivered = Vec::new();
                for name in names {
                    if let Some(node) = self.session_tree.node(&name)
                        && !node.frozen
                    {
                        self.broadcast_log.push((name.clone(), command.clone()));
                        delivered.push(name);
                    }
                }
                DaemonResponse::Ok {
                    message: format!(
                        "broadcast '{}' to {} session(s): {}",
                        command,
                        delivered.len(),
                        delivered.join(", ")
                    ),
                }
            }
            DaemonRequest::Derez { target, confirmed } => {
                if !confirmed {
                    return DaemonResponse::Error {
                        message: format!(
                            "derez '{target}' requires explicit confirmation (--confirm)"
                        ),
                    };
                }
                let names = self.session_tree.subtree_session_names(&target);
                if names.is_empty() {
                    return DaemonResponse::Error {
                        message: format!("session '{target}' not found"),
                    };
                }
                for name in names.iter().rev() {
                    self.session_tree.remove_session(name);
                    self.session_panes.remove(name);
                    self.clients.retain(|c| &c.session_name != name);
                }
                if self.session_tree.nodes.is_empty() {
                    self.session_tree
                        .upsert_session("main", SessionNodeKind::Rooted, None);
                    self.session_panes.insert("main".to_string(), 1);
                }
                DaemonResponse::Ok {
                    message: format!("derezzed {} session(s) under '{target}'", names.len()),
                }
            }
            DaemonRequest::Checkpoint => match self.save_checkpoint("latest") {
                Ok(path) => DaemonResponse::Ok {
                    message: format!("checkpoint saved to {}", path.display()),
                },
                Err(e) => DaemonResponse::Error {
                    message: format!("checkpoint failed: {e}"),
                },
            },
            DaemonRequest::Shutdown { confirmed } => {
                if !confirmed {
                    DaemonResponse::Error {
                        message: "daemon shutdown requires --confirm".to_string(),
                    }
                } else {
                    let _ = self.save_checkpoint("shutdown");
                    self.running = false;
                    self.clients.clear();
                    DaemonResponse::Ok {
                        message: "MASTER CONTROL PROGRAM SHUTDOWN COMPLETE. END OF LINE.".to_string(),
                    }
                }
            }
        }
    }

    /// Saves a [`WorkspaceJournal`] checkpoint to `self.journal_dir` (or [`WorkspaceJournal::default_journal_dir`]).
    pub fn save_checkpoint(&self, slot: &str) -> io::Result<PathBuf> {
        let dir = match &self.journal_dir {
            Some(d) => d.clone(),
            None => WorkspaceJournal::default_journal_dir()?,
        };
        let sessions: Vec<SessionJournalRecord> = self
            .session_tree
            .nodes
            .iter()
            .map(|n| SessionJournalRecord {
                session_name: n.name.clone(),
                layout_mode: "tiled".to_string(),
                active_pane_id: 1,
                theme_name: "turbo-blue".to_string(),
                split_ratios: vec![50],
                z_stack_rects: Vec::new(),
                panes: vec![PaneJournalRecord {
                    pane_id: 1,
                    title: format!("Session {}", n.name),
                    cwd: None,
                    font_scale_pct: 100,
                    scroll_mode: "smooth".to_string(),
                    prompt_anchor: "floating".to_string(),
                    prompt_offset_y: 0,
                    prompt_height: 3,
                    command_history: Vec::new(),
                    scrollback_tail: Vec::new(),
                }],
            })
            .collect();
        let active = self
            .session_tree
            .nodes
            .first()
            .map(|n| n.name.clone())
            .unwrap_or_else(|| "main".to_string());
        let journal = WorkspaceJournal::new(active, self.session_tree.clone(), sessions);
        journal.save_atomic(&dir, slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::net::UnixListener;

    #[test]
    fn daemon_multi_client_attach_detach_and_zero_size_clamp_safety() {
        let mut daemon = DaemonServerState::new();

        // Attach a TUI client with 0x0 dimensions (must never panic on clamp!)
        let resp1 = daemon.handle_request(DaemonRequest::Attach {
            client_id: 1,
            session_name: "main".to_string(),
            is_gui: false,
            cols: 0,
            rows: 0,
            font_scale_pct: 0,
        });
        assert_eq!(
            resp1,
            DaemonResponse::Attached {
                session_name: "main".to_string(),
                effective_cols: 20,
                effective_rows: 6,
                client_count: 1,
            }
        );

        // Resize client 1 to 120x40 and attach GUI client 2 at 100x30 (100% scale)
        daemon.handle_request(DaemonRequest::ResizeClient {
            client_id: 1,
            cols: 120,
            rows: 40,
            font_scale_pct: 100,
        });
        let resp2 = daemon.handle_request(DaemonRequest::Attach {
            client_id: 2,
            session_name: "main".to_string(),
            is_gui: true,
            cols: 100,
            rows: 30,
            font_scale_pct: 100,
        });
        assert_eq!(
            resp2,
            DaemonResponse::Attached {
                session_name: "main".to_string(),
                effective_cols: 100,
                effective_rows: 30,
                client_count: 2,
            }
        );

        // Detach both clients — session "main" survives in daemon!
        daemon.handle_request(DaemonRequest::Detach { client_id: 1 });
        daemon.handle_request(DaemonRequest::Detach { client_id: 2 });
        assert!(daemon.running);
        assert!(daemon.session_tree.node("main").is_some());
    }

    #[test]
    #[cfg(unix)]
    fn stale_socket_probe_cleans_refused_but_preserves_live_and_permission_denied() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let sock_path = tmp.path().join("daemon.sock");

        // 1. Live listener -> returns Ok(true) and does NOT unlink
        let listener = UnixListener::bind(&sock_path).unwrap();
        assert!(DaemonServerState::probe_or_clean_stale_socket(&sock_path).unwrap());
        assert!(sock_path.exists());
        drop(listener);

        // 2. Dropped listener (ConnectionRefused) -> unlinks stale socket and returns Ok(false)
        assert!(!DaemonServerState::probe_or_clean_stale_socket(&sock_path).unwrap());
        assert!(!sock_path.exists());

        // 3. PermissionDenied on parent/socket -> returns Err(PermissionDenied) and NEVER unlinks
        let locked_dir = tmp.path().join("locked");
        fs::create_dir(&locked_dir).unwrap();
        let protected_sock = locked_dir.join("daemon.sock");
        let l2 = UnixListener::bind(&protected_sock).unwrap();
        drop(l2);
        fs::set_permissions(&protected_sock, fs::Permissions::from_mode(0o000)).unwrap();

        let res = DaemonServerState::probe_or_clean_stale_socket(&protected_sock);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), ErrorKind::PermissionDenied);
        assert!(
            fs::symlink_metadata(&protected_sock).is_ok(),
            "PermissionDenied socket must never be unlinked"
        );
        fs::set_permissions(&protected_sock, fs::Permissions::from_mode(0o600)).unwrap();
    }
}
