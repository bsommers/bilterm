use ratatui::layout::Rect;

use crate::error::Result;
use crate::pane::CompositePane;
use crate::pty::PtyConfig;

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
        assert!(qt.pane.vt().screen().contents().contains("PERSISTENT_QUAKE_OUTPUT"));
        assert_eq!(qt.pane.prompt().text(), "htop");
    }
}
