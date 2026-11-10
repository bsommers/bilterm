//! Workspace resilience: debounced background checkpoints, unclean-shutdown recovery, a
//! persisted session tree, and daemon start/stop jobs that never block the UI thread.
//!
//! The UI thread only builds an in-memory snapshot; serialization and all file I/O happen on a
//! worker thread. A `<session>.running` marker is written at startup and removed on a clean
//! exit, so finding one whose process is gone means the previous run ended uncleanly.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::session::SavedWorkspaceSession;
use super::session_tree::SessionTree;
use super::WindowManager;
use crate::secure_fs::{read_with_backup, write_private_file_durable, DurableSource};

/// How often the cheap change probe runs.
const PROBE_EVERY: Duration = Duration::from_millis(500);
/// Quiet time after a layout/pane/cwd change before a checkpoint is written.
const DEBOUNCE: Duration = Duration::from_millis(750);
/// Upper bound between checkpoints while the workspace is otherwise unchanged.
const PERIODIC: Duration = Duration::from_secs(30);

/// A daemon lifecycle operation run off the UI thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DaemonJob {
    Start { name: String },
    Stop { name: String, force: bool },
}

enum CheckpointJob {
    Session { path: PathBuf, snap: Box<SavedWorkspaceSession> },
    Tree { path: PathBuf, tree: SessionTree },
}

/// Per-workspace resilience state (one field on [`WindowManager`]).
#[derive(Default)]
pub struct ResilienceState {
    daemon_jobs: Vec<Receiver<String>>,
    restore_note: Option<String>,
    worker: Option<Sender<CheckpointJob>>,
    worker_errors: Option<Receiver<String>>,
    last_probe: Option<Instant>,
    last_sig: Option<u64>,
    last_tree_sig: Option<u64>,
    dirty_since: Option<Instant>,
    last_write: Option<Instant>,
    run_marker: Option<PathBuf>,
    last_ui_cost: Duration,
    written: Arc<AtomicU64>,
}

impl std::fmt::Debug for ResilienceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResilienceState")
            .field("run_marker", &self.run_marker)
            .field("checkpoints_written", &self.checkpoints_written())
            .finish_non_exhaustive()
    }
}

impl ResilienceState {
    /// Checkpoints completed by the worker so far.
    pub fn checkpoints_written(&self) -> u64 {
        self.written.load(Ordering::Relaxed)
    }

    /// Time the last checkpoint cost on the calling (UI) thread.
    pub fn last_ui_cost(&self) -> Duration {
        self.last_ui_cost
    }

    fn sender(&mut self) -> Option<Sender<CheckpointJob>> {
        if self.worker.is_none() {
            let (tx, rx) = mpsc::channel::<CheckpointJob>();
            let (err_tx, err_rx) = mpsc::channel::<String>();
            let written = Arc::clone(&self.written);
            std::thread::Builder::new()
                .name("checkpoint-writer".to_string())
                .spawn(move || {
                    for job in rx {
                        let res = match &job {
                            CheckpointJob::Session { path, snap } => toml::to_string_pretty(snap.as_ref())
                                .map_err(|e| e.to_string())
                                .and_then(|t| write_private_file_durable(path, t.as_bytes()).map_err(|e| e.to_string())),
                            CheckpointJob::Tree { path, tree } => toml::to_string_pretty(tree)
                                .map_err(|e| e.to_string())
                                .and_then(|t| write_private_file_durable(path, t.as_bytes()).map_err(|e| e.to_string())),
                        };
                        match res {
                            Ok(()) => {
                                if matches!(job, CheckpointJob::Session { .. }) {
                                    written.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            Err(e) => {
                                let _ = err_tx.send(e);
                            }
                        }
                    }
                })
                .ok()?;
            self.worker = Some(tx);
            self.worker_errors = Some(err_rx);
        }
        self.worker.clone()
    }
}

impl Drop for ResilienceState {
    fn drop(&mut self) {
        // A clean exit clears the marker; a panic or kill leaves it for the next start.
        if !std::thread::panicking()
            && let Some(marker) = self.run_marker.take()
        {
            let _ = std::fs::remove_file(marker);
        }
    }
}

fn sanitize(name: &str) -> String {
    let s: String = name.trim().chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')).collect();
    if s.is_empty() { "default".to_string() } else { s }
}

fn marker_owner_alive(pid: u32) -> bool {
    if pid == std::process::id() || !crate::daemon::supervisor::check_pid_alive(pid) {
        return false;
    }
    // Guard against PID reuse where the platform lets us look.
    match std::fs::read_to_string(format!("/proc/{pid}/comm")) {
        Ok(comm) => comm.trim().contains("bilterm"),
        Err(_) => !cfg!(target_os = "linux"),
    }
}

fn parse_session(text: &str) -> Option<SavedWorkspaceSession> {
    toml::from_str::<SavedWorkspaceSession>(text).ok().filter(|s| !s.panes.is_empty())
}

impl WindowManager {
    /// Resilience state (checkpoint counters, timings).
    pub fn resilience(&self) -> &ResilienceState {
        &self.resilience
    }

    /// Note left by the last restore (backup fallback, crash recovery), consumed by the caller.
    pub fn take_restore_note(&mut self) -> Option<String> {
        self.resilience.restore_note.take()
    }

    pub(crate) fn set_restore_note(&mut self, note: String) {
        self.resilience.restore_note = Some(note);
    }

    /// `<session_dir>/<session>.ckpt`: the crash-recovery checkpoint of the active session.
    pub fn checkpoint_path(&self) -> Option<PathBuf> {
        let dir = self.resolve_session_dir().ok()?;
        Some(dir.join(format!("{}.ckpt", sanitize(&self.config.history_session))))
    }

    fn session_tree_path(&self) -> Option<PathBuf> {
        Some(self.resolve_session_dir().ok()?.join("session_tree.state"))
    }

    fn workspace_signature(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.config.history_session.hash(&mut h);
        format!("{:?}{:?}{:?}{:?}", self.layout_mode, self.split_tree, self.z_stack.order(), self.zoomed_pane).hash(&mut h);
        for p in self.panes.values() {
            (p.id(), p.title(), p.resolve_cwd()).hash(&mut h);
        }
        h.finish()
    }

    fn tree_signature(&self) -> u64 {
        let mut h = DefaultHasher::new();
        for n in &self.daemon_state.session_tree.nodes {
            (&n.name, &n.parent, &n.children, n.collapsed, n.frozen).hash(&mut h);
            format!("{:?}", n.kind).hash(&mut h);
        }
        h.finish()
    }

    /// Queues a checkpoint of the active session now. The UI thread only clones the snapshot;
    /// the worker serializes and writes it. Returns `false` when no session dir is available.
    pub fn checkpoint_now(&mut self) -> bool {
        let started = Instant::now();
        let Some(path) = self.checkpoint_path() else { return false };
        if let Some(dir) = path.parent() {
            let _ = crate::secure_fs::create_private_dir_all(dir);
        }
        let snap = Box::new(self.snapshot_session(&sanitize(&self.config.history_session)));
        let Some(tx) = self.resilience.sender() else { return false };
        let sent = tx.send(CheckpointJob::Session { path, snap }).is_ok();
        self.resilience.last_ui_cost = started.elapsed();
        self.resilience.dirty_since = None;
        self.resilience.last_write = Some(Instant::now());
        sent
    }

    fn persist_session_tree(&mut self) {
        let Some(path) = self.session_tree_path() else { return };
        if let Some(dir) = path.parent() {
            let _ = crate::secure_fs::create_private_dir_all(dir);
        }
        let tree = self.daemon_state.session_tree.clone();
        if let Some(tx) = self.resilience.sender() {
            let _ = tx.send(CheckpointJob::Tree { path, tree });
        }
    }

    /// Loads the persisted session tree (parents, order, fold/freeze) saved by an earlier run.
    pub fn load_persisted_session_tree(&mut self) -> bool {
        let Some(path) = self.session_tree_path() else { return false };
        let valid = |t: &str| toml::from_str::<SessionTree>(t).is_ok_and(|tr| !tr.nodes.is_empty());
        let Ok((text, source)) = read_with_backup(&path, &valid) else { return false };
        let Ok(mut tree) = toml::from_str::<SessionTree>(&text) else { return false };
        // Drop links to sessions that no longer exist so a damaged file cannot create cycles.
        let names: Vec<String> = tree.nodes.iter().map(|n| n.name.clone()).collect();
        for n in &mut tree.nodes {
            if n.parent.as_ref().is_some_and(|p| !names.contains(p) || *p == n.name) {
                n.parent = None;
            }
            n.children.retain(|c| names.contains(c));
        }
        self.daemon_state.session_tree = tree;
        self.resilience.last_tree_sig = Some(self.tree_signature());
        if source == DurableSource::Backup {
            self.set_restore_note("session tree file was corrupt; restored from backup".to_string());
        }
        true
    }

    /// Per-tick resilience work: finishes daemon jobs, reports checkpoint write failures, and
    /// checkpoints after a debounced change (layout, pane open/close, cwd) or periodically.
    /// Returns `true` when something user-visible changed.
    pub fn resilience_tick(&mut self, now: Instant) -> bool {
        let mut changed = self.poll_daemon_jobs();
        if let Some(err) = self.resilience.worker_errors.as_ref().and_then(|rx| rx.try_iter().last()) {
            self.active_pane_mut().set_status_message(Some(format!("Checkpoint write failed: {err}")));
            changed = true;
        }
        if self.resilience.run_marker.is_none() {
            return changed;
        }
        if self.resilience.last_probe.is_some_and(|t| now.duration_since(t) < PROBE_EVERY) {
            return changed;
        }
        self.resilience.last_probe = Some(now);
        let sig = self.workspace_signature();
        if self.resilience.last_sig != Some(sig) {
            self.resilience.last_sig = Some(sig);
            self.resilience.dirty_since = Some(now);
        }
        let tree_sig = self.tree_signature();
        if self.resilience.last_tree_sig != Some(tree_sig) {
            self.resilience.last_tree_sig = Some(tree_sig);
            self.persist_session_tree();
        }
        let debounced = self.resilience.dirty_since.is_some_and(|t| now.duration_since(t) >= DEBOUNCE);
        let periodic = self.resilience.last_write.is_none_or(|t| now.duration_since(t) >= PERIODIC);
        if debounced || (periodic && self.resilience.dirty_since.is_none()) {
            self.checkpoint_now();
        }
        changed
    }

    /// Marks this process as running the active session and, when `recover` is set and the
    /// previous run ended uncleanly, restores its last checkpoint. Returns a status message
    /// when an unclean shutdown was detected.
    pub fn begin_run(&mut self, recover: bool) -> Option<String> {
        let dir = self.resolve_session_dir().ok()?;
        crate::secure_fs::create_private_dir_all(&dir).ok()?;
        let sess = sanitize(&self.config.history_session);
        let marker = dir.join(format!("{sess}.running"));
        let previous = std::fs::read_to_string(&marker).ok();
        if let Some(pid) = previous.as_deref().and_then(|s| s.trim().parse::<u32>().ok())
            && marker_owner_alive(pid)
        {
            return None; // another live instance owns this session; leave its marker alone
        }
        let note = previous.filter(|_| recover).map(|_| self.recover_from_checkpoint(&sess));
        if crate::secure_fs::write_private_file(&marker, format!("{}\n", std::process::id()).as_bytes()).is_ok() {
            self.resilience.run_marker = Some(marker);
        }
        self.resilience.last_sig = Some(self.workspace_signature());
        note
    }

    fn recover_from_checkpoint(&mut self, sess: &str) -> String {
        let Some(path) = self.checkpoint_path() else {
            return format!("Session '{sess}' ended uncleanly; no checkpoint location available");
        };
        let age = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .map(|d| format!(" from {}s ago", d.as_secs()))
            .unwrap_or_default();
        match read_with_backup(&path, &|t| parse_session(t).is_some()) {
            Ok((text, source)) => match parse_session(&text) {
                Some(snap) => {
                    // The backup is one generation older: never let it replace a richer layout
                    // that the normal startup restore already brought back.
                    if source == DurableSource::Backup && snap.panes.len() < self.panes.len() {
                        return format!(
                            "Session '{sess}' ended uncleanly; its newest checkpoint was corrupt, kept the last saved layout ({} pane(s))",
                            self.panes.len()
                        );
                    }
                    let panes = self.apply_saved_session_snapshot(&snap);
                    let from = if source == DurableSource::Backup { " (backup copy; newest was corrupt)" } else { "" };
                    format!(
                        "Recovered session '{sess}' after an unclean shutdown: {panes} pane(s) restored from the checkpoint{age}{from}; programs were restarted, not kept running"
                    )
                }
                None => format!("Session '{sess}' ended uncleanly; its checkpoint could not be read"),
            },
            Err(e) => format!("Session '{sess}' ended uncleanly; no usable checkpoint ({e})"),
        }
    }

    /// Clears the running marker (clean exit). Also happens when the workspace is dropped.
    pub fn end_run(&mut self) {
        if let Some(marker) = self.resilience.run_marker.take() {
            let _ = std::fs::remove_file(marker);
        }
    }

    /// Runs a daemon start/stop on a worker thread; the outcome is reported by the next
    /// [`Self::resilience_tick`] as a status message. Never blocks the caller.
    pub fn spawn_daemon_job(&mut self, runtime_dir: &Path, job: DaemonJob) {
        let (tx, rx) = mpsc::channel();
        let dir = runtime_dir.to_path_buf();
        let spawned = std::thread::Builder::new().name("daemon-job".to_string()).spawn(move || {
            let _ = tx.send(run_daemon_job(&dir, &job));
        });
        if spawned.is_ok() {
            self.resilience.daemon_jobs.push(rx);
        }
    }

    /// `true` while a daemon start/stop is still running in the background.
    pub fn daemon_job_pending(&self) -> bool {
        !self.resilience.daemon_jobs.is_empty()
    }

    fn poll_daemon_jobs(&mut self) -> bool {
        let mut done = Vec::new();
        self.resilience.daemon_jobs.retain(|rx| match rx.try_recv() {
            Ok(msg) => {
                done.push(msg);
                false
            }
            Err(mpsc::TryRecvError::Empty) => true,
            Err(mpsc::TryRecvError::Disconnected) => false,
        });
        let Some(msg) = done.pop() else { return false };
        self.active_pane_mut().set_status_message(Some(msg));
        if self.daemon_window().active {
            self.refresh_daemon_window();
        }
        true
    }
}

/// Executes `job` synchronously and returns the status message for it.
pub fn run_daemon_job(runtime_dir: &Path, job: &DaemonJob) -> String {
    use crate::daemon::supervisor::{ensure_started, stop_daemon, DaemonStartResult};
    match job {
        DaemonJob::Start { name } => match ensure_started(runtime_dir, name) {
            Ok(DaemonStartResult::Started { pid, .. }) => format!("Daemon '{name}' started (PID {pid})"),
            Ok(DaemonStartResult::AlreadyRunning { pid, .. }) => {
                format!("Daemon '{name}' is already running (PID {pid})")
            }
            Err(e) => format!("Failed to start daemon '{name}': {e}"),
        },
        DaemonJob::Stop { name, force } => match stop_daemon(runtime_dir, name, *force) {
            Ok(true) => format!("Daemon '{name}' stopped"),
            Ok(false) => format!("Daemon '{name}' was not running"),
            Err(e) => format!("Failed to stop daemon '{name}': {e}"),
        },
    }
}
