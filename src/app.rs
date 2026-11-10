pub mod pacing;

use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::{Stream, StreamExt};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use crate::error::Result;
use crate::input::{handle_wm_key_event, handle_wm_paste_event, handle_wm_mouse_event, AppAction};
use crate::pane::CompositePane;
use crate::ui::WorkspaceWidget;
use crate::wm::WindowManager;

pub use pacing::{FramePacer, LoopStats, ECHO_GRACE, FRAME_FLOOR};

/// Cadence of timers, RPC waiters and STT draining (PTY output no longer waits for it).
pub const TICK: Duration = Duration::from_millis(16);

/// The always-on foreground RPC socket: request stream plus its handler.
pub type LiveRpcPair = (crate::api::live::LiveRpcRx, crate::api::live::LiveRpcHandler);

/// Resolves on SIGINT (Ctrl+C), SIGTERM or SIGHUP.
async fn host_shutdown_signal() {
    #[cfg(unix)]
    let (mut sigterm, mut sighup) = {
        use tokio::signal::unix::{signal, SignalKind};
        (signal(SignalKind::terminate()).ok(), signal(SignalKind::hangup()).ok())
    };
    #[cfg(not(unix))]
    let (mut sigterm, mut sighup) = ((), ());
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = next_unix_signal(&mut sigterm) => {}
        _ = next_unix_signal(&mut sighup) => {}
    }
}

static HOST_OUT_TAP: std::sync::OnceLock<crate::traffic::TrafficTap> = std::sync::OnceLock::new();

/// `io::Stdout` wrapper that counts every byte written to the host terminal (hook H29,
/// `TMON-01`). Payload bytes are never inspected.
pub struct CountingStdout {
    inner: io::Stdout,
    tap: crate::traffic::TrafficTap,
}

/// Host-terminal writer type used by the ratatui backend.
pub type HostOut = CountingStdout;

impl CountingStdout {
    /// Opens the `HostTty` output channel on `meter` (also used for outbound OSC 52).
    pub fn new(meter: &crate::traffic::TrafficMeter) -> Self {
        let tap = meter.open_channel(
            crate::traffic::Origin::HostTty,
            "host-tty",
            None,
            crate::traffic::Role::Wire,
        );
        let _ = HOST_OUT_TAP.set(tap.clone());
        Self { inner: io::stdout(), tap }
    }
}

impl Write for CountingStdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.tap.tx(n);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Next request on the always-on foreground RPC socket; pends forever when it is unavailable.
async fn next_live_request(
    rx: &mut Option<crate::api::live::LiveRpcRx>,
) -> Option<crate::api::RpcRequest> {
    match rx.as_mut() {
        Some(r) => r.recv().await,
        None => std::future::pending().await,
    }
}

/// Consecutive host input stream read errors after which the app quits.
pub const MAX_INPUT_STREAM_ERRORS: u8 = 8;

/// Next delivery of a long-lived Unix signal stream (pending forever when unavailable).
#[cfg(unix)]
async fn next_unix_signal(sig: &mut Option<tokio::signal::unix::Signal>) {
    match sig.as_mut() {
        Some(s) => {
            if s.recv().await.is_none() {
                std::future::pending::<()>().await;
            }
        }
        None => std::future::pending().await,
    }
}

#[cfg(not(unix))]
async fn next_unix_signal(_sig: &mut ()) {
    std::future::pending().await
}

/// Top-level Bilterm TUI application state and async multi-pane event loop coordinator.
pub struct App {
    pub wm: WindowManager,
    pub should_quit: bool,
    pub last_viewport: Rect,
    pub needs_redraw: bool,
}

impl App {
    pub fn new(pane: CompositePane) -> Self {
        let (rows, cols) = pane.vt().size();
        let headless = pane.is_headless();
        let wm = WindowManager::from_initial_pane(pane, headless);
        Self {
            wm,
            should_quit: false,
            last_viewport: Rect::new(0, 0, cols, rows + 1),
            needs_redraw: true,
        }
    }

    pub fn spawn_default(rows: u16, cols: u16) -> Result<Self> {
        let wm = WindowManager::spawn_default(rows, cols)?;
        Ok(Self {
            wm,
            should_quit: false,
            last_viewport: Rect::new(0, 0, cols, rows),
            needs_redraw: true,
        })
    }

    /// Runs the main async `tokio::select!` loop multiplexing Crossterm input events,
    /// multi-PTY output chunks, and line-discipline telemetry ticks.
    pub async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<HostOut>>,
    ) -> Result<()> {
        self.wm.set_tui_cell_grid_mode(true);
        let _ = terminal.hide_cursor();
        let reader = EventStream::new();
        let input_tap = self.wm.traffic_meter().open_channel(
            crate::traffic::Origin::HostTty,
            "host-input",
            None,
            crate::traffic::Role::Wire,
        );
        let live = crate::api::live::LiveRpc::start(self.wm.traffic_meter());
        if live.is_none() {
            crate::pty::set_live_socket(None);
        }
        let draw = |app: &mut App| -> Result<()> {
            terminal.draw(|frame| {
                let area = frame.area();
                app.last_viewport = area;
                let _ = app.wm.resize_all(area);
                frame.render_widget(WorkspaceWidget::new(&app.wm), area);
                crate::ui::gui_font::geometric::sanitize_tui_pua_buffer(
                    frame.buffer_mut(),
                    app.wm.config().glyphs,
                );
            })?;
            Ok(())
        };
        let events = reader.inspect(|item| {
            if item.is_ok() {
                input_tap.event(1);
            }
        });
        self.run_loop(events, draw, live, host_shutdown_signal(), &LoopStats::default()).await
    }

    /// The event loop proper, generic over the host so it can be driven headlessly: `events` is
    /// the host input stream, `draw` paints one frame, `shutdown` resolves when the process is
    /// asked to stop.
    ///
    /// PTY output wakes the loop through [`crate::pty::wake`] and is drained at once; redraws
    /// are paced by a [`FramePacer`]. The `select!` is `biased`, in an order chosen so that no
    /// arm can starve another:
    /// 1. the 16 ms tick (rate-limited by its timer, so it cannot starve anything);
    /// 2. shutdown signals;
    /// 3. host input, ahead of everything a flooding pane can keep ready;
    /// 4. RPC requests;
    /// 5. the frame deadline;
    /// 6. PTY output, last: it may be ready on every turn during a flood, and each drain is
    ///    bounded (32 x 8 KiB per pane), so input is consumed at most two turns after it arrives.
    pub async fn run_loop<S, D, Q>(
        &mut self,
        mut events: S,
        mut draw: D,
        live: Option<LiveRpcPair>,
        shutdown: Q,
        stats: &LoopStats,
    ) -> Result<()>
    where
        S: Stream<Item = io::Result<Event>> + Unpin,
        D: FnMut(&mut App) -> Result<()>,
        Q: std::future::Future<Output = ()>,
    {
        let (mut live_rx, mut live) = match live {
            Some((rx, handler)) => (Some(rx), Some(handler)),
            None => (None, None),
        };
        let mut tick = tokio::time::interval(TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut input_errors: u8 = 0;
        let mut pacer = FramePacer::default();
        let mut output_seen = crate::pty::wake::output_seq();
        tokio::pin!(shutdown);

        loop {
            LoopStats::bump(&stats.turns);
            let now = Instant::now();
            if self.needs_redraw && pacer.can_draw(now) {
                draw(self)?;
                self.needs_redraw = false;
                pacer.drew(now);
                LoopStats::bump(&stats.draws);
            }

            self.flush_osc52_to_host()?;

            if self.should_quit || self.all_panes_exited() {
                break;
            }

            let frame_due = tokio::time::Instant::from_std(pacer.due(now));
            tokio::select! {
                biased;
                _ = tick.tick() => {
                    LoopStats::bump(&stats.ticks);
                    if let Some(handler) = live.as_mut() {
                        handler.poll_waiters(&mut self.wm);
                    }
                    self.wm.drain_stt_events();
                    if self.poll_all_ptys() {
                        pacer.output_arrived();
                    }
                }
                _ = &mut shutdown => {
                    self.should_quit = true;
                }
                maybe_event = events.next() => {
                    let redraw_was_pending = self.needs_redraw;
                    let sent_before = crate::pty::wake::input_seq();
                    self.on_input_stream_item(maybe_event, &mut input_errors);
                    if crate::pty::wake::input_seq() != sent_before {
                        pacer.input_sent_to_pty(Instant::now(), redraw_was_pending);
                    }
                    LoopStats::bump(&stats.inputs);
                    stats.last_input_turn.set(stats.turns.get());
                }
                Some((line, reply)) = next_live_request(&mut live_rx) => {
                    if let Some(handler) = live.as_mut() {
                        handler.handle(&mut self.wm, &line, reply);
                    }
                    self.needs_redraw = true;
                }
                _ = tokio::time::sleep_until(frame_due), if self.needs_redraw => {}
                _ = crate::pty::wake::output_ready(&mut output_seen) => {
                    LoopStats::bump(&stats.pty_wakes);
                    let drained = self.wm.drain_pty_output();
                    if drained.exited {
                        // Reap or hold the pane now, exactly as the tick would.
                        self.poll_all_ptys();
                    }
                    if drained.changed || drained.exited {
                        self.needs_redraw = true;
                        pacer.output_arrived();
                    }
                    // Let the runtime turn its timer and I/O drivers even if output keeps this
                    // arm ready on every turn (matters on a current-thread runtime).
                    tokio::task::yield_now().await;
                }
            }
        }

        Ok(())
    }

    pub fn poll_all_ptys(&mut self) -> bool {
        let changed = self.wm.poll_all_ptys();
        if changed {
            self.needs_redraw = true;
        }
        changed
    }

    pub fn all_panes_exited(&self) -> bool {
        self.wm.all_panes_exited()
    }

    pub fn flush_osc52_to_host(&mut self) -> Result<()> {
        let mut out = io::stdout();
        let mut wrote = false;
        let inside_tmux = crate::tmux::TmuxEnv::detect().is_some();
        for seq in self.wm.drain_outbound_osc52() {
            let wire = if inside_tmux {
                crate::tmux::wrap_tmux_dcs_passthrough(&seq)
            } else {
                seq
            };
            out.write_all(wire.as_bytes())?;
            if let Some(t) = HOST_OUT_TAP.get() {
                t.tx(wire.len());
            }
            wrote = true;
        }
        if wrote {
            out.flush()?;
        }
        Ok(())
    }

    /// Consumes one item of the host input stream. A closed stream (`None`) or
    /// [`MAX_INPUT_STREAM_ERRORS`] consecutive read errors quit instead of spinning; an error from
    /// handling one event (e.g. a PTY write on one pane) is shown in the status line, not fatal.
    pub fn on_input_stream_item(&mut self, item: Option<io::Result<Event>>, consecutive_errors: &mut u8) {
        match item {
            Some(Ok(event)) => {
                *consecutive_errors = 0;
                let _ = self.handle_crossterm_event(event);
                self.needs_redraw = true;
            }
            Some(Err(_)) => {
                *consecutive_errors = consecutive_errors.saturating_add(1);
                if *consecutive_errors >= MAX_INPUT_STREAM_ERRORS {
                    self.should_quit = true;
                }
            }
            None => self.should_quit = true,
        }
    }

    /// Shows a non-fatal input-handling error in the focused pane's status line.
    pub fn report_event_error(&mut self, err: &crate::error::BiltermError) {
        self.wm.active_pane_mut().set_status_message(Some(format!("Input error: {err}")));
        self.needs_redraw = true;
    }

    /// Handles one host event. Always `Ok`: an error from handling it (a refused or failed PTY
    /// write on one pane, a rejected command) is shown in the status line and must never end
    /// the event loop of any front end (the GUI loops propagate this result with `?`).
    pub fn handle_crossterm_event(&mut self, event: Event) -> Result<()> {
        if let Err(err) = self.dispatch_crossterm_event(event) {
            self.report_event_error(&err);
        }
        Ok(())
    }

    fn dispatch_crossterm_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                if handle_wm_key_event(&mut self.wm, key)? == AppAction::Quit {
                    self.should_quit = true;
                }
            }
            Event::Mouse(mouse) => {
                let _ = handle_wm_mouse_event(&mut self.wm, mouse, self.last_viewport)?;
            }
            Event::Paste(text) => {
                let _ = handle_wm_paste_event(&mut self.wm, &text)?;
            }
            Event::Resize(cols, rows) => {
                crate::ui::hires_subcell::refresh_terminal_pixel_size();
                self.last_viewport = Rect::new(0, 0, cols, rows);
                let _ = self.wm.resize_all(self.last_viewport);
                self.wm.rebalance_scrollback(std::time::Instant::now());
            }
            _ => {}
        }
        Ok(())
    }
}
