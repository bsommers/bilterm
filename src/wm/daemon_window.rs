//! Floating `DAEMON SUPERVISOR` manager window (TUI + GUI).
//!
//! CONTRACT (frozen by the orchestrator; agents fill bodies, do not rename or change signatures):
//! state lives in `WindowManager.daemon_window` (private field), UI reads it via `wm.daemon_window()`
//! and `wm.daemon_window_rect(ws)`; input routes through `wm.daemon_window_key(key)` and
//! `wm.daemon_window_click(ws, col, row)`.

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;

use crate::action::{Action, DaemonSupervisorOp};
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

fn default_entry(runtime_dir: &Path) -> DaemonMenuEntry {
    use crate::daemon::supervisor::{DaemonLockRecord, check_pid_alive, daemon_lock_path_in};
    let rec = daemon_lock_path_in(runtime_dir, "default")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| DaemonLockRecord::from_json(&s).ok());
    match rec {
        Some(r) => {
            let alive = check_pid_alive(r.pid);
            DaemonMenuEntry {
                name: "default".into(),
                running: alive,
                pid: alive.then_some(r.pid),
                socket_path: r.socket_path.to_string_lossy().into_owned(),
                ..Default::default()
            }
        }
        None => DaemonMenuEntry {
            name: "default".into(),
            socket_path: runtime_dir.join("daemon.sock").to_string_lossy().into_owned(),
            ..Default::default()
        },
    }
}

impl WindowManager {
    pub fn open_daemon_window(&mut self) {
        self.daemon_window.active = true;
        self.daemon_window.focused = true;
        self.refresh_daemon_window();
    }

    /// Opens the window and refreshes it from an explicit runtime dir.
    pub fn open_daemon_window_from(&mut self, runtime_dir: &Path) {
        self.daemon_window.active = true;
        self.daemon_window.focused = true;
        self.refresh_daemon_window_from(runtime_dir);
    }

    pub fn close_daemon_window(&mut self) {
        self.daemon_window.active = false;
        self.daemon_window.focused = false;
    }

    pub fn refresh_daemon_window(&mut self) {
        match crate::daemon::supervisor::resolve_runtime_dir() {
            Ok(dir) => self.refresh_daemon_window_from(&dir),
            Err(e) => self.daemon_window.status_line = format!("runtime dir error: {e}"),
        }
    }

    pub fn refresh_daemon_window_from(&mut self, runtime_dir: &Path) {
        let mut entries: Vec<DaemonMenuEntry> =
            crate::daemon::registry::DaemonRegistry::list_active(runtime_dir)
                .into_iter()
                .map(|d| DaemonMenuEntry {
                    current: d.is_current(),
                    name: d.name,
                    running: true,
                    pid: Some(d.pid),
                    sessions: d.sessions_count,
                    panes: d.panes_count,
                    socket_path: d.socket_path.to_string_lossy().into_owned(),
                })
                .collect();
        if !entries.iter().any(|e| e.name == "default") {
            entries.push(default_entry(runtime_dir));
        }
        entries.sort_by(|a, b| {
            (a.name != "default", a.name.as_str()).cmp(&(b.name != "default", b.name.as_str()))
        });
        let running = entries.iter().filter(|e| e.running).count();
        let w = &mut self.daemon_window;
        w.status_line = format!(
            "{} daemon{}, {} running",
            entries.len(),
            if entries.len() == 1 { "" } else { "s" },
            running
        );
        w.entries = entries;
        w.selected = w.selected.min(w.entries.len().saturating_sub(1));
        w.scroll = w.scroll.min(w.selected);
    }

    fn daemon_visible_rows(&self) -> usize {
        // Same geometry the renderer uses: rows = outer height - 5 (title, header, status, footer, border).
        let (rows, cols) = self.default_size;
        let ws = Rect::new(0, 0, cols.max(40), rows.max(10));
        let h = self.daemon_window_rect(ws).map_or(16, |r| r.height);
        usize::from(h.saturating_sub(5)).max(1)
    }

    fn daemon_select(&mut self, idx: usize) {
        let vis = self.daemon_visible_rows();
        let w = &mut self.daemon_window;
        w.selected = idx.min(w.entries.len().saturating_sub(1));
        if w.selected < w.scroll {
            w.scroll = w.selected;
        } else if w.selected >= w.scroll + vis {
            w.scroll = w.selected + 1 - vis;
        }
    }

    /// Runs a footer/key operation via the Action path, then refreshes.
    fn daemon_window_run(&mut self, button: DaemonWindowButton) {
        let name = self.daemon_window.selected_entry().map(|e| e.name.clone());
        let op = match (button, name) {
            (DaemonWindowButton::Start, Some(name)) => {
                Some(DaemonSupervisorOp::Start { name: Some(name) })
            }
            (DaemonWindowButton::Stop, Some(name)) => {
                Some(DaemonSupervisorOp::Stop { name: Some(name), force: false })
            }
            (DaemonWindowButton::Switch, Some(name)) => Some(DaemonSupervisorOp::Switch { name }),
            (DaemonWindowButton::Prune, _) => Some(DaemonSupervisorOp::Prune),
            (DaemonWindowButton::Close, _) => {
                self.close_daemon_window();
                return;
            }
            _ => None,
        };
        if let Some(op) = op {
            let _ = self.apply(Action::Daemon(op));
        }
        if self.daemon_window.active {
            self.refresh_daemon_window();
        }
    }

    /// Handles a key while the window captures input. Returns true when consumed.
    pub fn daemon_window_key(&mut self, key: KeyEvent) -> bool {
        let sel = self.daemon_window.selected;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.daemon_select(sel.saturating_sub(1)),
            KeyCode::Down | KeyCode::Char('j') => self.daemon_select(sel + 1),
            KeyCode::Enter | KeyCode::Char('s') => self.daemon_window_run(DaemonWindowButton::Start),
            KeyCode::Char('x' | 'q') | KeyCode::Delete => {
                self.daemon_window_run(DaemonWindowButton::Stop)
            }
            KeyCode::Char('w') => self.daemon_window_run(DaemonWindowButton::Switch),
            KeyCode::Char('p') => self.daemon_window_run(DaemonWindowButton::Prune),
            KeyCode::Char('r') => self.refresh_daemon_window(),
            KeyCode::Esc => self.close_daemon_window(),
            _ => {}
        }
        true
    }

    /// Pure hit test (no side effects).
    pub fn daemon_window_hit(&self, ws: Rect, col: u16, row: u16) -> DaemonWindowHit {
        let Some(outer) = self.daemon_window_rect(ws) else {
            return DaemonWindowHit::Outside;
        };
        let inside = col >= outer.x
            && col < outer.x + outer.width
            && row >= outer.y
            && row < outer.y + outer.height;
        if !inside {
            return DaemonWindowHit::Outside;
        }
        let lay = DaemonWindowLayout::new(outer);
        if row == outer.y && col >= lay.close_x && col < lay.close_x + 3 {
            return DaemonWindowHit::CloseBadge;
        }
        if row == lay.footer_y {
            for (b, x, w) in lay.button_spans() {
                if col >= x && col < x + w {
                    return DaemonWindowHit::Button(b);
                }
            }
        }
        if row >= lay.rows.y
            && row < lay.rows.y + lay.rows.height
            && col >= lay.rows.x
            && col < lay.rows.x + lay.rows.width
        {
            let idx = self.daemon_window.scroll + usize::from(row - lay.rows.y);
            if idx < self.daemon_window.entries.len() {
                return DaemonWindowHit::Row(idx);
            }
        }
        DaemonWindowHit::Frame
    }

    /// Handles a left-button press at (col,row).
    pub fn daemon_window_click(&mut self, ws: Rect, col: u16, row: u16) -> DaemonWindowHit {
        let hit = self.daemon_window_hit(ws, col, row);
        match hit {
            DaemonWindowHit::Outside => self.daemon_window.focused = false,
            DaemonWindowHit::Frame => self.daemon_window.focused = true,
            DaemonWindowHit::CloseBadge => self.close_daemon_window(),
            DaemonWindowHit::Row(i) => {
                self.daemon_window.focused = true;
                self.daemon_select(i);
            }
            DaemonWindowHit::Button(b) => {
                self.daemon_window.focused = true;
                self.daemon_window_run(b);
            }
        }
        hit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn wm_with_dir() -> (WindowManager, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let mut wm = WindowManager::new_headless(100, 40);
        wm.daemon_window_mut().active = true;
        wm.daemon_window_mut().focused = true;
        wm.refresh_daemon_window_from(dir.path());
        (wm, dir)
    }

    #[test]
    fn refresh_lists_default_stopped() {
        let (wm, _d) = wm_with_dir();
        let w = wm.daemon_window();
        assert_eq!(w.entries.len(), 1);
        assert_eq!(w.entries[0].name, "default");
        assert!(!w.entries[0].running);
        assert_eq!(w.entries[0].pid, None);
        assert_eq!(w.status_line, "1 daemon, 0 running");
    }

    #[test]
    fn selection_clamps_and_esc_closes() {
        let (mut wm, _d) = wm_with_dir();
        wm.daemon_window_mut().entries.push(DaemonMenuEntry { name: "b".into(), ..Default::default() });
        assert!(wm.daemon_window_key(key(KeyCode::Char('j'))));
        wm.daemon_window_key(key(KeyCode::Char('j')));
        assert_eq!(wm.daemon_window().selected, 1);
        wm.daemon_window_key(key(KeyCode::Char('k')));
        wm.daemon_window_key(key(KeyCode::Char('k')));
        assert_eq!(wm.daemon_window().selected, 0);
        wm.daemon_window_key(key(KeyCode::Esc));
        assert!(!wm.daemon_window().active);
    }

    #[test]
    fn hit_tests() {
        let (mut wm, _d) = wm_with_dir();
        let ws = Rect::new(0, 0, 100, 40);
        let outer = wm.daemon_window_rect(ws).unwrap();
        let lay = DaemonWindowLayout::new(outer);
        assert_eq!(wm.daemon_window_hit(ws, 0, 0), DaemonWindowHit::Outside);
        assert_eq!(wm.daemon_window_hit(ws, lay.close_x + 1, outer.y), DaemonWindowHit::CloseBadge);
        assert_eq!(wm.daemon_window_hit(ws, lay.rows.x + 1, lay.rows.y), DaemonWindowHit::Row(0));
        for (b, x, w) in lay.button_spans() {
            assert_eq!(wm.daemon_window_hit(ws, x + w - 1, lay.footer_y), DaemonWindowHit::Button(b));
        }
        wm.daemon_window_click(ws, 0, 0);
        assert!(!wm.daemon_window().focused);
        wm.daemon_window_click(ws, lay.close_x, outer.y);
        assert!(!wm.daemon_window().active);
    }
}
