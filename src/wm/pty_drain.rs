//! Wake-driven PTY drain (`RESP-01`): the part of `poll_all_ptys` that must not wait for the
//! 16 ms tick. Timers, the tmux link, traffic sampling and pane reaping stay on the tick.

use crate::wm::WindowManager;

/// Outcome of [`WindowManager::drain_pty_output`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PtyDrain {
    /// Some pane ingested output (a redraw is needed).
    pub changed: bool,
    /// Some pane's child exited during this drain; the caller must run `poll_all_ptys` so the
    /// pane is reaped or held before anything asks `all_panes_exited`.
    pub exited: bool,
}

impl WindowManager {
    /// Feeds queued PTY output into every pane (and the quick terminal), with the same bounded
    /// per-pane budget as the tick (`PtySession::drain_events`: 32 chunks of 8 KiB).
    pub fn drain_pty_output(&mut self) -> PtyDrain {
        let mut out = PtyDrain::default();
        for pane in self.panes.values_mut().chain(std::iter::once(&mut self.quick_terminal.pane)) {
            let was_exited = pane.has_exited();
            if pane.poll_pty_events() {
                out.changed = true;
            }
            if !was_exited && pane.has_exited() {
                out.exited = true;
            }
        }
        out
    }
}
