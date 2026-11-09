//! Floating `DAEMON SUPERVISOR` manager window (TUI + GUI).
//!
//! CONTRACT (frozen by the orchestrator; agents fill bodies, do not rename or change signatures):
//! state lives in `WindowManager.daemon_window` (private field), UI reads it via `wm.daemon_window()`
//! and `wm.daemon_window_rect(ws)`; input routes through `wm.daemon_window_key(key)` and
//! `wm.daemon_window_click(ws, col, row)`.

use ratatui::layout::Rect;

use crate::wm::WindowManager;

/// One row of the floating daemons window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DaemonMenuEntry {
    pub name: String,
    pub running: bool,
    pub pid: Option<u32>,
    pub sessions: usize,
    pub panes: usize,
    pub socket_path: String,
    /// True when this is the daemon the client is currently attached to / switched to.
    pub current: bool,
}

/// Buttons in the action footer, in left-to-right render order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DaemonWindowButton {
    Start,
    Stop,
    Switch,
    Prune,
    Refresh,
    Close,
}

impl DaemonWindowButton {
    pub const ALL: [DaemonWindowButton; 6] = [
        Self::Start,
        Self::Stop,
        Self::Switch,
        Self::Prune,
        Self::Refresh,
        Self::Close,
    ];

    /// Footer label; UI and hit-testing must both use this so they cannot drift.
    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "[s/Enter] Start",
            Self::Stop => "[x] Stop",
            Self::Switch => "[w] Switch",
            Self::Prune => "[p] Prune",
            Self::Refresh => "[r] Refresh",
            Self::Close => "[Esc] Close",
        }
    }
}

/// Mouse hit-test result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DaemonWindowHit {
    Outside,
    Frame,
    CloseBadge,
    Row(usize),
    Button(DaemonWindowButton),
}

/// Layout shared by renderer and hit-testing (single source of truth).
/// Inside `outer`: row 0 = title bar (with `[X]` at the right edge), row 1 = column header,
/// rows 2..h-3 = daemon rows, row h-3 = status banner, row h-2 = button footer, row h-1 = bottom border.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DaemonWindowLayout {
    pub outer: Rect,
    pub header_y: u16,
    pub rows: Rect,
    pub status_y: u16,
    pub footer_y: u16,
    /// Absolute x of the `[X]` close badge (3 cells wide) on the title row.
    pub close_x: u16,
}

impl DaemonWindowLayout {
    pub fn new(outer: Rect) -> Self {
        let h = outer.height;
        let rows_y = outer.y + 2;
        let status_y = outer.y + h.saturating_sub(3);
        Self {
            outer,
            header_y: outer.y + 1,
            rows: Rect::new(outer.x + 1, rows_y, outer.width.saturating_sub(2), status_y.saturating_sub(rows_y)),
            status_y,
            footer_y: outer.y + h.saturating_sub(2),
            close_x: outer.x + outer.width.saturating_sub(4),
        }
    }

    /// `(button, x_start, width)` for each footer button, separated by two spaces, starting at `outer.x + 2`.
    pub fn button_spans(&self) -> Vec<(DaemonWindowButton, u16, u16)> {
        let mut x = self.outer.x + 2;
        let mut out = Vec::new();
        for b in DaemonWindowButton::ALL {
            let w = b.label().chars().count() as u16;
            out.push((b, x, w));
            x += w + 2;
        }
        out
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DaemonWindowState {
    pub active: bool,
    pub focused: bool,
    pub selected: usize,
    pub scroll: usize,
    pub entries: Vec<DaemonMenuEntry>,
    pub status_line: String,
    /// Last rendered outer rect (set by the renderer/rect helper; informational).
    pub rect: Option<Rect>,
}

impl DaemonWindowState {
    /// True when key input should be routed to the window.
    pub fn captures_input(&self) -> bool {
        self.active && self.focused
    }

    pub fn selected_entry(&self) -> Option<&DaemonMenuEntry> {
        self.entries.get(self.selected)
    }
}

impl WindowManager {
    pub fn daemon_window(&self) -> &DaemonWindowState {
        &self.daemon_window
    }

    pub fn daemon_window_mut(&mut self) -> &mut DaemonWindowState {
        &mut self.daemon_window
    }

    /// Centered floating rect (about 76x16, clamped to the workspace); `None` when inactive or ws too small.
    pub fn daemon_window_rect(&self, ws: Rect) -> Option<Rect> {
        if !self.daemon_window.active || ws.width < 40 || ws.height < 10 {
            return None;
        }
        let w = 76.min(ws.width.saturating_sub(2));
        let h = 16.min(ws.height.saturating_sub(2));
        Some(Rect::new(
            ws.x + ws.width.saturating_sub(w) / 2,
            ws.y + ws.height.saturating_sub(h) / 2,
            w,
            h,
        ))
    }
}
