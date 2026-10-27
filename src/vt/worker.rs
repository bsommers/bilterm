//! **[EXPERIMENTAL]** Dedicated background worker thread pipeline (`ARCH-02`, `ARCH-R05`):
//!
//! > **Subsystem Status (`ADR-007` / `ARCH-R05`):** `VtWorkerHandle` is an **experimental, opt-in**
//! > asynchronous VT worker seam exercised by `tests/vt_worker_and_snapshot_seam.rs`. Production
//! > `CompositePane` instances use synchronous `PtySession` + `VtParser` (`TerminalBackend`) polling.
//!
//! ```text
//! ┌────────────────────────────────────────────────────────┐
//! │          UI & Window Layer (Ratatui / GPUI / egui)     │
//! └───────────────────────────┬────────────────────────────┘
//!                             │ (Reads Point-in-Time GridSnapshot)
//! ┌───────────────────────────▼────────────────────────────┐
//! │    Dedicated Worker Thread: libghostty-vt / VtParser   │
//! └───────────────────────────▲────────────────────────────┘
//!                             │ (Pipes Raw VT/ANSI Bytes)
//! ┌───────────────────────────┴────────────────────────────┐
//! │         OS Shell Pipe: portable-pty (ConPTY / Unix)    │
//! └────────────────────────────────────────────────────────┘
//! ```

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::error::{BiltermError, Result};
use crate::pty::{PtyConfig, shell_integration};
use crate::vt::scrollback::PaneScrollMode;
use crate::vt::snapshot::GridSnapshot;
use crate::vt::VtParser;

/// Commands sent from the UI/Window layer to the dedicated terminal emulation worker thread (`ARCH-02`).
#[derive(Clone, Debug)]
pub enum VtWorkerCommand {
    /// Writes raw input bytes (encoded keystrokes, mouse events, or submitted command lines) to the OS PTY master.
    WritePty(Vec<u8>),
    /// Feeds raw VT/ANSI output bytes directly into the terminal emulator (useful for `tmux -CC` `%output` or tests).
    FeedOutput(Vec<u8>),
    /// Resizes both the virtual terminal grid (with soft-wrap scrollback reflow) and the OS PTY (`TIOCSWINSZ`).
    Resize { rows: u16, cols: u16 },
    /// Scrolls the viewport by `delta` lines (negative scrolls up into history; positive scrolls down toward bottom).
    Scroll(isize),
    /// Resets the scrollback viewport to the live bottom (`0`).
    ResetScrollback,
    /// Configures the worker's scroll discipline (`Smooth`, `Instant`, or `BlockPage` page-gate).
    SetScrollMode(PaneScrollMode),
    /// Advances one full viewport page in `BlockPage` mode (`Enter`, `Space`, `PageDown`, `PF8`/`PA2`).
    PageAdvance,
    /// Steps backward one full viewport page in `BlockPage` mode (`b`, `PageUp`, `PF7`).
    PageBack,
    /// Updates the default theme foreground, background, and cursor RGB colors for `OSC 10/11/12` queries.
    SetThemeColors {
        fg: (u8, u8, u8),
        bg: (u8, u8, u8),
        cursor: (u8, u8, u8),
    },
    /// Gracefully terminates the worker thread and child PTY session.
    Shutdown,
}

/// Asynchronous lifecycle & telemetry events emitted by the worker thread to the UI layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VtWorkerEvent {
    /// Window title updated via `OSC 0` / `OSC 2`.
    TitleChanged(String),
    /// Working directory updated via `OSC 7`.
    CwdChanged(PathBuf),
    /// Outbound `OSC 52` clipboard sequence emitted by child process.
    Osc52Clipboard(String),
    /// Child process exited.
    Exited,
}

/// Thread-safe shared snapshot buffer updated by the worker thread and read by any UI renderer (`Ratatui`, `GPUI`, `egui`).
pub struct SharedGridSnapshot {
    snapshot: Mutex<GridSnapshot>,
    dirty: AtomicBool,
    generation: AtomicU64,
    rows: AtomicU16,
    cols: AtomicU16,
    page_holding: AtomicBool,
    holding_inspected: AtomicBool,
    pending_page_lines: AtomicU64,
}

impl SharedGridSnapshot {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            snapshot: Mutex::new(GridSnapshot::empty(rows, cols)),
            dirty: AtomicBool::new(true),
            generation: AtomicU64::new(0),
            rows: AtomicU16::new(rows.max(1)),
            cols: AtomicU16::new(cols.max(1)),
            page_holding: AtomicBool::new(false),
            holding_inspected: AtomicBool::new(false),
            pending_page_lines: AtomicU64::new(0),
        }
    }

    pub fn update_page_telemetry(&self, holding: bool, inspected: bool, pending: usize) {
        self.page_holding.store(holding, Ordering::Release);
        self.holding_inspected.store(inspected, Ordering::Release);
        self.pending_page_lines
            .store(pending as u64, Ordering::Release);
    }

    pub fn is_page_holding(&self) -> bool {
        self.page_holding.load(Ordering::Acquire)
    }

    pub fn is_holding_inspected(&self) -> bool {
        self.holding_inspected.load(Ordering::Acquire)
    }

    pub fn pending_page_lines(&self) -> usize {
        self.pending_page_lines.load(Ordering::Acquire) as usize
    }

    /// Returns a cloned point-in-time [`GridSnapshot`] without blocking the PTY reader.
    pub fn read_snapshot(&self) -> GridSnapshot {
        self.snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Publishes a newly materialized [`GridSnapshot`] from the emulation worker thread.
    pub fn publish(&self, next: GridSnapshot) {
        let r = next.rows;
        let c = next.cols;
        let next_gen = next.generation;
        if let Ok(mut guard) = self.snapshot.lock() {
            *guard = next;
        }
        self.rows.store(r, Ordering::Release);
        self.cols.store(c, Ordering::Release);
        self.generation.store(next_gen, Ordering::Release);
        self.dirty.store(true, Ordering::Release);
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Acquire)
    }

    pub fn mark_clean(&self) {
        self.dirty.store(false, Ordering::Release);
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub fn size(&self) -> (u16, u16) {
        (
            self.rows.load(Ordering::Acquire),
            self.cols.load(Ordering::Acquire),
        )
    }
}

/// Handle to a dedicated worker thread that continuously reads bytes from `portable-pty`,
/// feeds them into `libghostty-vt` (or `VtParser`), and publishes [`GridSnapshot`] frames (`ARCH-02`).
pub struct VtWorkerHandle {
    cmd_tx: mpsc::Sender<VtWorkerCommand>,
    event_rx: Mutex<mpsc::Receiver<VtWorkerEvent>>,
    shared: Arc<SharedGridSnapshot>,
    alive: Arc<AtomicBool>,
    child_pid: Option<u32>,
}

impl VtWorkerHandle {
    /// Spawns a cross-platform `portable-pty` child process and a dedicated VT emulation worker thread.
    pub fn spawn_pty<W>(
        mut config: PtyConfig,
        scrollback_capacity: usize,
        wake: W,
    ) -> Result<Self>
    where
        W: Fn() + Send + Sync + 'static,
    {
        let rows = config.rows.max(1);
        let cols = config.cols.max(1);
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
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

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| BiltermError::Pty(format!("spawn_command failed: {e}")))?;
        let child_pid = child.process_id();
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| BiltermError::Pty(format!("try_clone_reader failed: {e}")))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| BiltermError::Pty(format!("take_writer failed: {e}")))?;
        let master: Box<dyn MasterPty + Send> = pair.master;

        let resizer: Box<dyn FnMut(u16, u16) + Send> = Box::new(move |r, c| {
            let _ = master.resize(PtySize {
                rows: r.max(1),
                cols: c.max(1),
                pixel_width: 0,
                pixel_height: 0,
            });
        });

        let cleanup: Box<dyn FnOnce() + Send> = Box::new(move || {
            let _ = child.kill();
            let _ = child.wait();
        });

        Self::spawn_with_streams(
            rows,
            cols,
            scrollback_capacity,
            Some(reader),
            writer,
            resizer,
            cleanup,
            child_pid,
            wake,
        )
    }

    /// Spawns a headless worker thread without an OS process for deterministic testing of the worker pipeline.
    pub fn spawn_headless(rows: u16, cols: u16, scrollback_capacity: usize) -> Result<Self> {
        let sink: Box<dyn Write + Send> = Box::new(std::io::sink());
        let resizer: Box<dyn FnMut(u16, u16) + Send> = Box::new(|_, _| {});
        let cleanup: Box<dyn FnOnce() + Send> = Box::new(|| {});
        Self::spawn_with_streams(
            rows.max(1),
            cols.max(1),
            scrollback_capacity,
            None,
            sink,
            resizer,
            cleanup,
            None,
            || {},
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_with_streams<W>(
        rows: u16,
        cols: u16,
        scrollback_capacity: usize,
        reader: Option<Box<dyn Read + Send>>,
        writer: Box<dyn Write + Send>,
        mut resizer: Box<dyn FnMut(u16, u16) + Send>,
        cleanup: Box<dyn FnOnce() + Send>,
        child_pid: Option<u32>,
        wake: W,
    ) -> Result<Self>
    where
        W: Fn() + Send + Sync + 'static,
    {
        let shared = Arc::new(SharedGridSnapshot::new(rows, cols));
        let alive = Arc::new(AtomicBool::new(true));
        let (cmd_tx, cmd_rx) = mpsc::channel::<VtWorkerCommand>();
        let (event_tx, event_rx) = mpsc::channel::<VtWorkerEvent>();
        let (pty_tx, pty_rx) = mpsc::sync_channel::< Option<Vec<u8>> >(64);

        if let Some(mut pty_reader) = reader {
            thread::Builder::new()
                .name("bilterm-pty-pipe".to_string())
                .spawn(move || {
                    let mut buf = [0u8; 8192];
                    loop {
                        match pty_reader.read(&mut buf) {
                            Ok(0) | Err(_) => {
                                let _ = pty_tx.send(None);
                                break;
                            }
                            Ok(n) => {
                                if pty_tx.send(Some(buf[..n].to_vec())).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                })
                .map_err(BiltermError::Io)?;
        }

        let shared_clone = Arc::clone(&shared);
        let alive_clone = Arc::clone(&alive);

        thread::Builder::new()
            .name("bilterm-vt-worker".to_string())
            .spawn(move || {
                worker_loop(
                    rows,
                    cols,
                    scrollback_capacity,
                    writer,
                    &mut *resizer,
                    cmd_rx,
                    pty_rx,
                    &shared_clone,
                    &event_tx,
                    &wake,
                );
                alive_clone.store(false, Ordering::Release);
                let _ = event_tx.send(VtWorkerEvent::Exited);
                wake();
                cleanup();
            })
            .map_err(BiltermError::Io)?;

        Ok(Self {
            cmd_tx,
            event_rx: Mutex::new(event_rx),
            shared,
            alive,
            child_pid,
        })
    }

    /// Sends a command to the worker thread.
    pub fn send(&self, cmd: VtWorkerCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    /// Writes raw bytes to the child PTY input stream.
    pub fn write_pty(&self, bytes: impl Into<Vec<u8>>) {
        self.send(VtWorkerCommand::WritePty(bytes.into()));
    }

    /// Feeds raw VT output bytes directly into the worker's terminal core.
    pub fn feed_output(&self, bytes: impl Into<Vec<u8>>) {
        self.send(VtWorkerCommand::FeedOutput(bytes.into()));
    }

    /// Resizes the worker's terminal grid and OS PTY.
    pub fn resize(&self, rows: u16, cols: u16) {
        self.send(VtWorkerCommand::Resize { rows, cols });
    }

    /// Configures the worker's scroll mode (`Smooth`, `Instant`, or `BlockPage` page-gate).
    pub fn set_scroll_mode(&self, mode: PaneScrollMode) {
        self.send(VtWorkerCommand::SetScrollMode(mode));
    }

    /// Advances one full viewport page in `BlockPage` mode.
    pub fn page_advance(&self) {
        self.send(VtWorkerCommand::PageAdvance);
    }

    /// Steps backward one full viewport page in `BlockPage` mode.
    pub fn page_back(&self) {
        self.send(VtWorkerCommand::PageBack);
    }

    pub fn is_page_holding(&self) -> bool {
        self.shared.is_page_holding()
    }

    pub fn is_holding_inspected(&self) -> bool {
        self.shared.is_holding_inspected()
    }

    pub fn pending_page_lines(&self) -> usize {
        self.shared.pending_page_lines()
    }

    /// Reads the latest point-in-time [`GridSnapshot`] produced by the worker thread.
    pub fn snapshot(&self) -> GridSnapshot {
        self.shared.read_snapshot()
    }

    /// Accesses the underlying [`SharedGridSnapshot`] handle.
    pub fn shared_snapshot(&self) -> &Arc<SharedGridSnapshot> {
        &self.shared
    }

    /// Drains any pending [`VtWorkerEvent`] notifications without blocking.
    pub fn drain_events(&self) -> Vec<VtWorkerEvent> {
        let mut out = Vec::new();
        if let Ok(rx) = self.event_rx.lock() {
            while let Ok(ev) = rx.try_recv() {
                out.push(ev);
            }
        }
        out
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    pub fn child_pid(&self) -> Option<u32> {
        self.child_pid
    }
}

impl Drop for VtWorkerHandle {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(VtWorkerCommand::Shutdown);
    }
}

#[allow(clippy::too_many_arguments)]
fn worker_loop(
    initial_rows: u16,
    initial_cols: u16,
    scrollback_capacity: usize,
    mut writer: Box<dyn Write + Send>,
    resizer: &mut (dyn FnMut(u16, u16) + Send),
    cmd_rx: mpsc::Receiver<VtWorkerCommand>,
    pty_rx: mpsc::Receiver<Option<Vec<u8>>>,
    shared: &SharedGridSnapshot,
    event_tx: &mpsc::Sender<VtWorkerEvent>,
    wake: &(dyn Fn() + Send + Sync),
) {
    #[cfg(feature = "ghostty-vt")]
    let mut ghostty_engine =
        crate::vt::ghostty_backend::GhosttyVtEngine::new(initial_rows, initial_cols, scrollback_capacity).ok();

    let mut vt = VtParser::new(initial_rows, initial_cols, scrollback_capacity);
    let mut last_title: Option<String> = None;
    let mut last_cwd: Option<PathBuf> = None;
    let mut scroll_mode = PaneScrollMode::Smooth;
    let mut page_anchor_global_row = 0usize;
    let mut page_hold_active = false;
    let mut holding_inspected = false;

    // Publish initial snapshot
    shared.publish(vt.snapshot());

    loop {
        let mut needs_snapshot = false;
        let mut shutdown = false;

        // 1. Drain all pending UI commands
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                VtWorkerCommand::WritePty(data) => {
                    let _ = writer.write_all(&data);
                    let _ = writer.flush();
                }
                VtWorkerCommand::FeedOutput(bytes) => {
                    #[cfg(feature = "ghostty-vt")]
                    if let Some(eng) = &mut ghostty_engine {
                        eng.process(&bytes);
                    }
                    vt.process(&bytes);
                    needs_snapshot = true;
                }
                VtWorkerCommand::Resize { rows, cols } => {
                    let r = rows.max(1);
                    let c = cols.max(1);
                    #[cfg(feature = "ghostty-vt")]
                    if let Some(eng) = &mut ghostty_engine {
                        eng.set_size(r, c);
                    }
                    vt.set_size(r, c);
                    resizer(r, c);
                    needs_snapshot = true;
                }
                VtWorkerCommand::Scroll(delta) => {
                    #[cfg(feature = "ghostty-vt")]
                    if let Some(eng) = &mut ghostty_engine {
                        eng.scroll_delta(delta);
                    }
                    if delta < 0 {
                        vt.scroll_up(delta.unsigned_abs());
                    } else if delta > 0 {
                        vt.scroll_down(delta as usize);
                    }
                    needs_snapshot = true;
                }
                VtWorkerCommand::ResetScrollback => {
                    #[cfg(feature = "ghostty-vt")]
                    if let Some(eng) = &mut ghostty_engine {
                        eng.scroll_to_bottom();
                    }
                    vt.reset_scrollback();
                    page_anchor_global_row =
                        vt.scrollback_used_rows() + usize::from(vt.cursor_position().0);
                    page_hold_active = false;
                    holding_inspected = false;
                    needs_snapshot = true;
                }
                VtWorkerCommand::SetScrollMode(mode) => {
                    scroll_mode = mode;
                    let used = vt.scrollback_used_rows();
                    let off = vt.scrollback_offset();
                    if mode == PaneScrollMode::BlockPage {
                        if off > 0 {
                            page_anchor_global_row = used.saturating_sub(off);
                            page_hold_active = true;
                            holding_inspected = true;
                        } else {
                            page_anchor_global_row = used;
                            page_hold_active = false;
                            holding_inspected = false;
                        }
                    } else {
                        page_hold_active = false;
                        holding_inspected = false;
                    }
                    needs_snapshot = true;
                }
                VtWorkerCommand::PageAdvance => {
                    let page_rows = usize::from(vt.size().0.max(1));
                    let used = vt.scrollback_used_rows();
                    let cur_r = usize::from(vt.cursor_position().0);
                    if page_hold_active || vt.scrollback_offset() > 0 {
                        let next_anchor = page_anchor_global_row.saturating_add(page_rows);
                        if next_anchor < used {
                            page_anchor_global_row = next_anchor;
                            vt.set_scrollback_offset(used - next_anchor);
                            page_hold_active = true;
                            holding_inspected = false;
                        } else {
                            vt.reset_scrollback();
                            page_anchor_global_row = used + cur_r;
                            page_hold_active = false;
                            holding_inspected = false;
                        }
                        needs_snapshot = true;
                    }
                }
                VtWorkerCommand::PageBack => {
                    let page_rows = usize::from(vt.size().0.max(1));
                    let used = vt.scrollback_used_rows();
                    let new_off = vt.scrollback_offset().saturating_add(page_rows).min(used);
                    vt.set_scrollback_offset(new_off);
                    page_anchor_global_row = used.saturating_sub(new_off);
                    page_hold_active = true;
                    holding_inspected = true;
                    needs_snapshot = true;
                }
                VtWorkerCommand::SetThemeColors { fg, bg, cursor } => {
                    #[cfg(feature = "ghostty-vt")]
                    if let Some(eng) = &mut ghostty_engine {
                        eng.set_theme_colors(fg, bg, cursor);
                    }
                    vt.set_theme_colors(fg, bg, cursor);
                }
                VtWorkerCommand::Shutdown => {
                    shutdown = true;
                    break;
                }
            }
        }
        if shutdown {
            break;
        }

        // 2. Drain up to 32 chunks of PTY output per batch to bound frame latency
        let mut chunks_processed = 0usize;
        while chunks_processed < 32 {
            match pty_rx.try_recv() {
                Ok(Some(bytes)) => {
                    #[cfg(feature = "ghostty-vt")]
                    if let Some(eng) = &mut ghostty_engine {
                        eng.process(&bytes);
                    }
                    vt.process(&bytes);
                    needs_snapshot = true;
                    chunks_processed += 1;
                }
                Ok(None) => {
                    shutdown = true;
                    break;
                }
                Err(_) => break,
            }
        }

        // 3. Flush any terminal query replies (DECRPM, DSR, CPR, DA1, OSC 10/11, XTGETTCAP) back to PTY
        #[cfg(feature = "ghostty-vt")]
        if let Some(eng) = &mut ghostty_engine {
            for resp in eng.take_query_responses() {
                let _ = writer.write_all(&resp);
                let _ = writer.flush();
            }
        }
        for resp in vt.take_query_responses() {
            let _ = writer.write_all(&resp);
            let _ = writer.flush();
        }

        // 4. Emit side-effect events (title, CWD, OSC 52)
        if let Some(title) = vt.window_title().map(str::to_string)
            && last_title.as_ref() != Some(&title)
        {
            last_title = Some(title.clone());
            let _ = event_tx.send(VtWorkerEvent::TitleChanged(title));
        }
        if let Some(cwd) = vt.reported_cwd().map(PathBuf::from)
            && last_cwd.as_ref() != Some(&cwd)
        {
            last_cwd = Some(cwd.clone());
            let _ = event_tx.send(VtWorkerEvent::CwdChanged(cwd));
        }
        for osc52 in vt.take_outbound_osc52() {
            let _ = event_tx.send(VtWorkerEvent::Osc52Clipboard(osc52));
        }

        // 5. Publish updated point-in-time GridSnapshot (with BlockPage MORE.../HOLDING freeze when active)
        if needs_snapshot {
            if scroll_mode == PaneScrollMode::BlockPage && !vt.is_alternate_screen() {
                let page_rows = usize::from(vt.size().0.max(1));
                let used = vt.scrollback_used_rows();
                let cur_r = usize::from(vt.cursor_position().0);
                let total_written = used + cur_r;
                if page_hold_active || total_written.saturating_sub(page_anchor_global_row) >= page_rows {
                    if used > page_anchor_global_row {
                        page_hold_active = true;
                        vt.set_scrollback_offset(used - page_anchor_global_row);
                    } else if cur_r >= page_rows.saturating_sub(1)
                        && total_written.saturating_sub(page_anchor_global_row) >= page_rows.saturating_sub(1)
                    {
                        page_hold_active = true;
                    }
                }
                shared.update_page_telemetry(
                    page_hold_active,
                    holding_inspected,
                    vt.scrollback_offset(),
                );
            } else {
                shared.update_page_telemetry(false, false, 0);
            }

            #[cfg(feature = "ghostty-vt")]
            let snap = if let Some(eng) = &mut ghostty_engine {
                eng.snapshot()
            } else {
                vt.snapshot()
            };
            #[cfg(not(feature = "ghostty-vt"))]
            let snap = vt.snapshot();

            shared.publish(snap);
            wake();
        }

        if shutdown {
            break;
        }

        // Brief yield when idle to avoid spinning while remaining sub-millisecond responsive
        thread::sleep(Duration::from_millis(1));
    }
}
