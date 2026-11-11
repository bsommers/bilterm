mod exit_report;
pub mod line_discipline;
pub mod shell_integration;
pub mod wake;
pub mod writer;

pub use line_discipline::LineDisciplineState;

use std::collections::BTreeMap;
use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use tokio::sync::mpsc;

use crate::error::{BiltermError, Result};

/// Configuration for spawning a PTY child process inside a `CompositePane`.
#[derive(Clone, Debug)]
pub struct PtyConfig {
    pub rows: u16,
    pub cols: u16,
    pub program: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<std::path::PathBuf>,
}

impl Default for PtyConfig {
    fn default() -> Self {
        let program = default_shell_for_platform(
            std::env::consts::OS,
            std::env::var("SHELL").ok().as_deref(),
            std::env::var("COMSPEC").ok().as_deref(),
        );
        let mut env = BTreeMap::new();
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        env.insert("COLORTERM".to_string(), "truecolor".to_string());
        if std::env::var("LANG")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .is_none()
            && std::env::var("LC_ALL")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .is_none()
        {
            env.insert("LANG".to_string(), "en_US.UTF-8".to_string());
        }
        Self {
            rows: 24,
            cols: 80,
            program,
            args: Vec::new(),
            env,
            cwd: None,
        }
    }
}

/// Process-wide path of this foreground instance's live RPC socket (`LIVE-02`).
static LIVE_SOCKET: std::sync::RwLock<Option<std::path::PathBuf>> = std::sync::RwLock::new(None);

/// Records (or clears) the live RPC socket path injected into child PTY environments.
pub fn set_live_socket(path: Option<std::path::PathBuf>) {
    if let Ok(mut g) = LIVE_SOCKET.write() {
        *g = path;
    }
}

/// The live RPC socket path children should receive as `BILTERM_SOCKET`, when one is announced.
pub fn live_socket() -> Option<std::path::PathBuf> {
    LIVE_SOCKET.read().ok().and_then(|g| g.clone())
}

/// Workspace id handed to children: a foreground instance has exactly one live workspace.
pub const LIVE_WORKSPACE_ID: &str = "w1";

/// Pure builder of the `BILTERM_*` child environment (`LIVE-02`).
pub fn live_child_env(
    socket: Option<&std::path::Path>,
    instance_pid: u32,
    workspace_id: &str,
    pane_id: usize,
) -> Vec<(String, String)> {
    let mut v = Vec::with_capacity(4);
    if let Some(s) = socket {
        v.push(("BILTERM_SOCKET".to_string(), s.to_string_lossy().to_string()));
    }
    v.push(("BILTERM_INSTANCE_PID".to_string(), instance_pid.to_string()));
    v.push(("BILTERM_WORKSPACE_ID".to_string(), workspace_id.to_string()));
    v.push(("BILTERM_PANE_ID".to_string(), pane_id.to_string()));
    v
}

/// Injects `BILTERM_SOCKET`, `BILTERM_INSTANCE_PID`, `BILTERM_WORKSPACE_ID` and
/// `BILTERM_PANE_ID` into a child's environment map (overriding inherited stale values).
pub fn inject_live_env(env: &mut BTreeMap<String, String>, pane_id: usize) {
    let sock = live_socket();
    if sock.is_none() {
        env.remove("BILTERM_SOCKET");
    }
    for (k, v) in live_child_env(sock.as_deref(), std::process::id(), LIVE_WORKSPACE_ID, pane_id) {
        env.insert(k, v);
    }
}

/// Resolves the default interactive shell across Linux, macOS, and Windows (`PLAT-01`).
pub fn default_shell_for_platform(
    os: &str,
    shell_env: Option<&str>,
    comspec_env: Option<&str>,
) -> String {
    if let Some(sh) = shell_env.filter(|s| !s.trim().is_empty()) {
        return sh.to_string();
    }
    if os.eq_ignore_ascii_case("windows") {
        if let Some(cs) = comspec_env.filter(|s| !s.trim().is_empty()) {
            return cs.to_string();
        }
        "powershell.exe".to_string()
    } else if os.eq_ignore_ascii_case("macos") {
        "/bin/zsh".to_string()
    } else {
        "/bin/sh".to_string()
    }
}

/// Asynchronous events emitted by a `PtySession` reader loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PtyEvent {
    /// Raw VT100/ANSI byte chunk produced by the child PTY.
    Output(Vec<u8>),
    /// Child process exited or closed its PTY stream.
    Exited(Option<u32>),
}

/// Live cross-platform PTY session wrapping `portable-pty` with deduplicated `SIGWINCH` resizing.
pub struct PtySession {
    /// Input queue drained by this PTY's writer thread; writes never block the caller.
    writer: Arc<writer::PtyWriter>,
    /// `drain_events` has handed out a `PtyEvent::Exited` (the reader's or a synthesised one).
    exit_reported: bool,
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    child: Mutex<Option<Box<dyn Child + Send + Sync>>>,
    output_rx: mpsc::Receiver<PtyEvent>,
    current_size: Mutex<(u16, u16)>,
    sigwinch_count: Arc<AtomicUsize>,
    child_pid: Option<u32>,
    traffic: crate::traffic::TrafficTap,
    #[cfg(unix)]
    master_fd: Option<std::os::fd::RawFd>,
}

impl PtySession {
    /// Spawns a new child process inside a native OS pseudoterminal.
    pub fn spawn(mut config: PtyConfig) -> Result<Self> {
        let pty_system = native_pty_system();
        let initial_rows = config.rows.max(1);
        let initial_cols = config.cols.max(1);
        let pair = pty_system
            .openpty(PtySize {
                rows: initial_rows,
                cols: initial_cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| BiltermError::Pty(format!("openpty failed: {e}")))?;

        let _ = shell_integration::prepare_shell_integration(
            &config.program,
            &mut config.args,
            &mut config.env,
        );

        let mut cmd = CommandBuilder::new(&config.program);
        for arg in &config.args {
            cmd.arg(arg);
        }
        for (k, v) in &config.env {
            cmd.env(k, v);
        }
        if let Some(cwd) = &config.cwd {
            cmd.cwd(cwd);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| BiltermError::Pty(format!("spawn_command failed: {e}")))?;
        let child_pid = child.process_id();
        drop(pair.slave);

        #[cfg(unix)]
        let master_fd = pair.master.as_raw_fd();

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| BiltermError::Pty(format!("try_clone_reader failed: {e}")))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| BiltermError::Pty(format!("take_writer failed: {e}")))?;

        let (tx, rx) = mpsc::channel(32);
        let traffic = crate::traffic::ChannelCounters::new();
        let reader_cell = traffic.clone();
        thread::Builder::new()
            .name("bilterm-pty-reader".to_string())
            .spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => {
                            let _ = tx.blocking_send(PtyEvent::Exited(None));
                            break;
                        }
                        Ok(n) => {
                            reader_cell.rx(n);
                            if tx
                                .blocking_send(PtyEvent::Output(buf[..n].to_vec()))
                                .is_err()
                            {
                                break;
                            }
                            // Wake the UI loop now; it must not wait for its next timer tick.
                            wake::notify_output();
                        }
                        Err(_) => {
                            let _ = tx.blocking_send(PtyEvent::Exited(None));
                            break;
                        }
                    }
                }
                wake::notify_output();
            })
            .map_err(BiltermError::Io)?;

        let writer = writer::PtyWriter::spawn(writer, traffic.clone()).map_err(BiltermError::Io)?;

        Ok(Self {
            master: Mutex::new(Some(pair.master)),
            writer: Arc::new(writer),
            exit_reported: false,
            child: Mutex::new(Some(child)),
            output_rx: rx,
            current_size: Mutex::new((initial_rows, initial_cols)),
            sigwinch_count: Arc::new(AtomicUsize::new(0)),
            child_pid,
            traffic,
            #[cfg(unix)]
            master_fd,
        })
    }

    /// Shared traffic counters for this PTY (`TMON-01`); adopted lazily by the meter.
    pub fn traffic_cell(&self) -> &crate::traffic::TrafficTap {
        &self.traffic
    }

    /// Queues raw bytes for the PTY master and returns at once: the blocking write happens on
    /// this PTY's writer thread, in call order (see [`writer`]).
    ///
    /// `Err` when the PTY write side has failed, or when the bytes would push the unwritten
    /// backlog past [`writer::MAX_QUEUED_INPUT_BYTES`] (the child is not reading); the write is
    /// then refused whole and the caller is expected to show the error.
    pub fn write_all(&self, bytes: &[u8]) -> Result<()> {
        self.writer.enqueue(bytes)?;
        wake::note_input();
        Ok(())
    }

    /// Bytes accepted by `write_all` that the child has not been given yet.
    pub fn pending_input_bytes(&self) -> usize {
        self.writer.pending_bytes()
    }

    /// Total bytes `write_all` refused because the backlog was full.
    pub fn rejected_input_bytes(&self) -> u64 {
        self.writer.rejected_bytes()
    }

    /// Observer for the writer thread's lifetime (it must end when the session is dropped).
    pub fn writer_probe(&self) -> writer::WriterProbe {
        self.writer.probe()
    }

    /// Resizes the PTY only if `(rows, cols)` changed, preventing `SIGWINCH` storms when
    /// moving or hiding the overlaid floating command prompt (`PANE-01`).
    ///
    /// Returns `true` if a `SIGWINCH` resize was actually sent to the underlying OS PTY.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<bool> {
        let clamped_rows = rows.max(1);
        let clamped_cols = cols.max(1);
        let mut size_guard = self
            .current_size
            .lock()
            .map_err(|_| BiltermError::Pty("PTY size mutex poisoned".to_string()))?;
        if *size_guard == (clamped_rows, clamped_cols) {
            return Ok(false);
        }

        let master_guard = self
            .master
            .lock()
            .map_err(|_| BiltermError::Pty("PTY master mutex poisoned".to_string()))?;
        if let Some(master) = master_guard.as_ref() {
            master
                .resize(PtySize {
                    rows: clamped_rows,
                    cols: clamped_cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| BiltermError::Pty(format!("PTY resize failed: {e}")))?;
        }

        *size_guard = (clamped_rows, clamped_cols);
        self.sigwinch_count.fetch_add(1, Ordering::SeqCst);
        Ok(true)
    }

    /// Current `(rows, cols)` reported to the PTY.
    pub fn size(&self) -> (u16, u16) {
        *self.current_size.lock().expect("size mutex")
    }

    /// Total number of `SIGWINCH` / `MasterPty::resize` calls sent since spawn.
    pub fn sigwinch_count(&self) -> usize {
        self.sigwinch_count.load(Ordering::SeqCst)
    }

    /// Child process ID if available.
    pub fn child_pid(&self) -> Option<u32> {
        self.child_pid
    }

    /// Child exit code, polled (`try_wait` every 2 ms) for at most `patience`.
    ///
    /// `None` while the child is still running after `patience`, or when the child handle is
    /// gone. Never blocks longer than `patience`.
    pub fn exit_code(&self, patience: std::time::Duration) -> Option<i32> {
        let deadline = std::time::Instant::now() + patience;
        loop {
            {
                let mut guard = self.child.lock().ok()?;
                let child = guard.as_mut()?;
                if let Ok(Some(status)) = child.try_wait() {
                    return Some(i32::try_from(status.exit_code()).unwrap_or(i32::MAX));
                }
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return None;
            }
            thread::sleep(std::time::Duration::from_millis(2).min(deadline - now));
        }
    }

    /// Raw file descriptor of the PTY master on Unix platforms (used for `tcgetattr` / `tcgetpgrp`).
    #[cfg(unix)]
    pub fn master_raw_fd(&self) -> Option<std::os::fd::RawFd> {
        self.master_fd
    }

    /// Mutable receiver for consuming `PtyEvent`s in `tokio::select!`.
    pub fn output_rx_mut(&mut self) -> &mut mpsc::Receiver<PtyEvent> {
        &mut self.output_rx
    }

    /// Non-blocking drain of queued `PtyEvent`s, bounded to at most 32 chunks (`256 KB`) per
    /// frame tick so high-throughput streams (`tail -f /dev/random`, `yes`) never starve the
    /// Tokio UI/input event loop.
    pub fn drain_events(&mut self) -> Vec<PtyEvent> {
        const MAX_EVENTS_PER_POLL: usize = 32;
        let mut events = Vec::new();
        while events.len() < MAX_EVENTS_PER_POLL {
            match self.output_rx.try_recv() {
                Ok(ev) => events.push(ev),
                Err(_) => break,
            }
        }
        let drained_all = events.len() < MAX_EVENTS_PER_POLL;
        let failure = if drained_all && !self.exit_reported { self.writer.failure() } else { None };
        if exit_report::settle(&mut events, &mut self.exit_reported, failure.as_deref()) {
            tracing::warn!(error = ?failure, pid = ?self.child_pid, "PTY write failed; reporting the pane as exited");
        }
        events
    }
}

/// How long the reaper lets already-accepted input reach the child before killing it.
const CLOSE_FLUSH_PATIENCE: std::time::Duration = std::time::Duration::from_millis(250);

impl Drop for PtySession {
    fn drop(&mut self) {
        self.output_rx.close();
        while self.output_rx.try_recv().is_ok() {}
        let writer = self.writer.clone();
        let child = self.child.lock().ok().and_then(|mut g| g.take());
        let master = self.master.lock().ok().and_then(|mut g| g.take());
        let _ = thread::Builder::new()
            .name("bilterm-pty-reaper".to_string())
            .spawn(move || {
                // Input written just before the drop (a final command) still reaches a child
                // that is reading; a child that is not reading costs at most this wait.
                writer.wait_drained(CLOSE_FLUSH_PATIENCE);
                writer.close();
                if let Some(mut c) = child {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                drop(master);
            });
    }
}
