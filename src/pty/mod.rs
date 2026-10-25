pub mod line_discipline;
pub mod shell_integration;

use std::collections::BTreeMap;
use std::io::{Read, Write};
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
    writer: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    output_rx: mpsc::Receiver<PtyEvent>,
    current_size: Mutex<(u16, u16)>,
    sigwinch_count: Arc<AtomicUsize>,
    child_pid: Option<u32>,
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
                            if tx
                                .blocking_send(PtyEvent::Output(buf[..n].to_vec()))
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(_) => {
                            let _ = tx.blocking_send(PtyEvent::Exited(None));
                            break;
                        }
                    }
                }
            })
            .map_err(BiltermError::Io)?;

        Ok(Self {
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            child: Mutex::new(child),
            output_rx: rx,
            current_size: Mutex::new((initial_rows, initial_cols)),
            sigwinch_count: Arc::new(AtomicUsize::new(0)),
            child_pid,
            #[cfg(unix)]
            master_fd,
        })
    }

    /// Writes raw bytes to the PTY master input stream and flushes immediately.
    pub fn write_all(&self, bytes: &[u8]) -> Result<()> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| BiltermError::Pty("PTY writer mutex poisoned".to_string()))?;
        writer.write_all(bytes)?;
        writer.flush()?;
        Ok(())
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

        let master = self
            .master
            .lock()
            .map_err(|_| BiltermError::Pty("PTY master mutex poisoned".to_string()))?;
        master
            .resize(PtySize {
                rows: clamped_rows,
                cols: clamped_cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| BiltermError::Pty(format!("PTY resize failed: {e}")))?;

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
        events
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
