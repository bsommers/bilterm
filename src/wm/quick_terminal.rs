use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::error::Result;
use crate::pane::CompositePane;
use crate::pty::PtyConfig;

/// Insets a window rectangle by 1 cell on all sides for the Borland Turbo Vision window frame (`╔═╗` / `┌─┐`).
pub fn inset_window_frame(outer: Rect) -> Rect {
    if outer.width <= 2 || outer.height <= 2 {
        return outer;
    }
    Rect {
        x: outer.x + 1,
        y: outer.y + 1,
        width: outer.width.saturating_sub(2),
        height: outer.height.saturating_sub(2),
    }
}

pub fn rect_contains(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}

/// Translates a global `MouseEvent` into pane-local coordinates and encodes an SGR (1006)
/// mouse sequence (`\x1b[<btn;local_col;local_rowM` or `m`) (`WM-03`).
pub fn encode_sgr_mouse_local(mouse: MouseEvent, pane_inner: Rect) -> Option<Vec<u8>> {
    if !rect_contains(pane_inner, mouse.column, mouse.row) {
        return None;
    }
    let local_col = mouse.column.saturating_sub(pane_inner.x) + 1;
    let local_row = mouse.row.saturating_sub(pane_inner.y) + 1;

    let (btn_code, suffix) = match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => (0, 'M'),
        MouseEventKind::Down(MouseButton::Middle) => (1, 'M'),
        MouseEventKind::Down(MouseButton::Right) => (2, 'M'),
        MouseEventKind::Up(MouseButton::Left) => (0, 'm'),
        MouseEventKind::Up(MouseButton::Middle) => (1, 'm'),
        MouseEventKind::Up(MouseButton::Right) => (2, 'm'),
        MouseEventKind::Drag(MouseButton::Left) => (32, 'M'),
        MouseEventKind::ScrollUp => (64, 'M'),
        MouseEventKind::ScrollDown => (65, 'M'),
        _ => return None,
    };

    Some(format!("\x1b[<{btn_code};{local_col};{local_row}{suffix}").into_bytes())
}

/// Persistent Guake/Quake-style top-anchored drop-down quick terminal (`WM-05`).
///
/// Toggles over the top of the workspace via `Alt+\`` / `Ctrl+Space` / `:quake` for rapid
/// "by-the-way" shell commands while keeping its underlying PTY process alive when hidden.
pub struct QuickTerminal {
    pub pane: CompositePane,
    pub visible: bool,
    /// Height of the drop-down overlay as a percentage of total screen height (`20..=80`, default `40`).
    pub height_pct: u16,
}

impl QuickTerminal {
    pub fn new_headless(rows: u16, cols: u16) -> Self {
        let overlay_rows = ((u32::from(rows) * 40) / 100).max(5) as u16;
        let mut pane = CompositePane::new_headless(0, overlay_rows, cols);
        pane.set_title("QUAKE QUICK TERM [~]");
        Self {
            pane,
            visible: false,
            height_pct: 40,
        }
    }

    pub fn spawn(mut cfg: PtyConfig) -> Result<Self> {
        let overlay_rows = ((u32::from(cfg.rows) * 40) / 100).max(5) as u16;
        cfg.rows = overlay_rows;
        let mut pane = CompositePane::spawn(0, cfg)?;
        pane.set_title("QUAKE QUICK TERM [~]");
        Ok(Self {
            pane,
            visible: false,
            height_pct: 40,
        })
    }

    /// Toggles drop-down visibility without terminating or resetting the underlying PTY session.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Computes the top-anchored overlay rectangle for the Guake drop-down terminal.
    pub fn overlay_rect(&self, screen: Rect) -> Rect {
        let pct = self.height_pct.clamp(20, 80);
        let h = ((u32::from(screen.height) * u32::from(pct)) / 100)
            .max(5)
            .min(u32::from(screen.height)) as u16;
        Rect {
            x: screen.x,
            y: screen.y,
            width: screen.width,
            height: h,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_terminal_preserves_state_across_hide_and_show_toggles() {
        let mut qt = QuickTerminal::new_headless(25, 80);
        assert!(!qt.visible);
        qt.toggle();
        assert!(qt.visible);

        qt.pane.process_pty_output(b"PERSISTENT_QUAKE_OUTPUT\r\n");
        qt.pane.prompt_mut().insert_str("htop");
        qt.toggle();
        assert!(!qt.visible);

        // Toggle back on: PTY canvas and prompt state remain intact
        qt.toggle();
        assert!(qt.visible);
        assert!(qt.pane.vt().contents().contains("PERSISTENT_QUAKE_OUTPUT"));
        assert_eq!(qt.pane.prompt().text(), "htop");
    }
}
