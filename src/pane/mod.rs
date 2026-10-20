use ratatui::layout::Rect;

use crate::error::Result;
use crate::pty::line_discipline::{inspect_pty_fd, InputRoutingMode, LineDisciplineState};
use crate::pty::{PtyConfig, PtyEvent, PtySession};
use crate::vt::VtParser;

/// Unique identifier for a `CompositePane`.
pub type PaneId = usize;

/// Vertical docking/floating mode for a pane's `FloatingCommandPrompt`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptAnchor {
    /// Pinned to the top of the pane's output canvas (classic IBM 3270 mainframe default).
    Top,
    /// Pinned to the bottom of the pane's output canvas (above the 1-row OIA status bar).
    Bottom,
    /// Free-floating at a vertical row offset relative to the top of the output canvas.
    Floating { y_offset: u16 },
}

/// Interactive state for the pane's `FloatingCommandPrompt` overlay.
#[derive(Clone, Debug)]
pub struct FloatingPromptState {
    pub input: String,
    pub cursor_char_idx: usize,
    pub anchor: PromptAnchor,
    pub height: u16,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    /// Manual toggle (`Ctrl+G` / `F12`) allowing the user to hide the floating prompt and
    /// route keystrokes directly to the underlying PTY at any time (`PTY-02`).
    pub manual_raw_override: bool,
}

impl Default for FloatingPromptState {
    fn default() -> Self {
        Self {
            input: String::new(),
            cursor_char_idx: 0,
            anchor: PromptAnchor::Top,
            height: 3,
            history: Vec::new(),
            history_index: None,
            manual_raw_override: false,
        }
    }
}

impl FloatingPromptState {
    pub fn insert_char(&mut self, ch: char) {
        let byte_idx = self.char_to_byte_idx(self.cursor_char_idx);
        self.input.insert(byte_idx, ch);
        self.cursor_char_idx += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor_char_idx > 0 {
            let prev_char_idx = self.cursor_char_idx - 1;
            let byte_idx = self.char_to_byte_idx(prev_char_idx);
            self.input.remove(byte_idx);
            self.cursor_char_idx = prev_char_idx;
        }
    }

    pub fn delete_char(&mut self) {
        let len_chars = self.input.chars().count();
        if self.cursor_char_idx < len_chars {
            let byte_idx = self.char_to_byte_idx(self.cursor_char_idx);
            self.input.remove(byte_idx);
        }
    }

    pub fn move_cursor_left(&mut self) {
        self.cursor_char_idx = self.cursor_char_idx.saturating_sub(1);
    }

    pub fn move_cursor_right(&mut self) {
        let len_chars = self.input.chars().count();
        if self.cursor_char_idx < len_chars {
            self.cursor_char_idx += 1;
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_char_idx = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_char_idx = self.input.chars().count();
    }

    pub fn clear_line(&mut self) {
        self.input.clear();
        self.cursor_char_idx = 0;
    }

    pub fn take_submission(&mut self) -> String {
        let line = std::mem::take(&mut self.input);
        self.cursor_char_idx = 0;
        self.history_index = None;
        if !line.trim().is_empty() && self.history.last() != Some(&line) {
            self.history.push(line.clone());
        }
        line
    }

    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let next_idx = match self.history_index {
            None => self.history.len() - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        };
        self.history_index = Some(next_idx);
        self.input = self.history[next_idx].clone();
        self.cursor_char_idx = self.input.chars().count();
    }

    pub fn history_next(&mut self) {
        let Some(curr) = self.history_index else {
            return;
        };
        if curr + 1 < self.history.len() {
            let next_idx = curr + 1;
            self.history_index = Some(next_idx);
            self.input = self.history[next_idx].clone();
            self.cursor_char_idx = self.input.chars().count();
        } else {
            self.history_index = None;
            self.clear_line();
        }
    }

    fn char_to_byte_idx(&self, char_idx: usize) -> usize {
        self.input
            .char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(self.input.len())
    }
}

/// A single composite terminal pane combining:
/// 1. Full-height `OutputCanvas` (`VtParser` + `PtySession`)
/// 2. Overlaid `FloatingCommandPrompt` (`FloatingPromptState`)
/// 3. Bottom 1-row Operator Information Area (`OiaStatusBar`)
pub struct CompositePane {
    pub id: PaneId,
    pub title: String,
    pub vt: VtParser,
    pub pty: Option<PtySession>,
    pub prompt: FloatingPromptState,
    pub line_discipline: LineDisciplineState,
    last_canvas_size: (u16, u16),
    exited: bool,
}

impl CompositePane {
    /// Creates a headless `CompositePane` (without spawning an OS process) for deterministic unit/snapshot testing.
    pub fn new_headless(id: PaneId, rows: u16, cols: u16) -> Self {
        let canvas_rows = rows.saturating_sub(1).max(1);
        let canvas_cols = cols.max(1);
        Self {
            id,
            title: format!("PANE #{id}"),
            vt: VtParser::new(canvas_rows, canvas_cols, 2000),
            pty: None,
            prompt: FloatingPromptState::default(),
            line_discipline: LineDisciplineState::default(),
            last_canvas_size: (canvas_rows, canvas_cols),
            exited: false,
        }
    }

    /// Spawns a live `CompositePane` backed by a `portable-pty` shell process.
    pub fn spawn(id: PaneId, mut pty_config: PtyConfig) -> Result<Self> {
        let canvas_rows = pty_config.rows.saturating_sub(1).max(1);
        let canvas_cols = pty_config.cols.max(1);
        pty_config.rows = canvas_rows;
        pty_config.cols = canvas_cols;

        let title = format!("{} [#{id}]", pty_config.program);
        let session = PtySession::spawn(pty_config)?;
        Ok(Self {
            id,
            title,
            vt: VtParser::new(canvas_rows, canvas_cols, 5000),
            pty: Some(session),
            prompt: FloatingPromptState::default(),
            line_discipline: LineDisciplineState::default(),
            last_canvas_size: (canvas_rows, canvas_cols),
            exited: false,
        })
    }

    /// Computes the full-height `OutputCanvas` rectangle (`100%` of pane height minus 1 row for the OIA status bar).
    pub fn canvas_rect(&self, pane_area: Rect) -> Rect {
        Rect {
            x: pane_area.x,
            y: pane_area.y,
            width: pane_area.width,
            height: pane_area.height.saturating_sub(1).max(1),
        }
    }

    /// Computes the bottom 1-row Operator Information Area (OIA) status bar rectangle.
    pub fn status_bar_rect(&self, pane_area: Rect) -> Rect {
        let y = pane_area.y + pane_area.height.saturating_sub(1);
        Rect {
            x: pane_area.x,
            y,
            width: pane_area.width,
            height: 1.min(pane_area.height),
        }
    }

    /// Computes the clamped overlay rectangle of the `FloatingCommandPrompt` within `canvas_rect`.
    pub fn prompt_rect(&self, pane_area: Rect) -> Rect {
        let canvas = self.canvas_rect(pane_area);
        let height = self.prompt.height.clamp(3, canvas.height.max(1)).min(canvas.height);
        let max_y_offset = canvas.height.saturating_sub(height);
        let y_offset = match self.prompt.anchor {
            PromptAnchor::Top => 0,
            PromptAnchor::Bottom => max_y_offset,
            PromptAnchor::Floating { y_offset } => y_offset.min(max_y_offset),
        };
        let margin_x = if canvas.width > 6 { 1 } else { 0 };
        Rect {
            x: canvas.x + margin_x,
            y: canvas.y + y_offset,
            width: canvas.width.saturating_sub(margin_x * 2).max(1),
            height,
        }
    }

    /// Resizes the pane's outer viewport.
    ///
    /// Crucial `PANE-01` invariant: Only resizes the underlying `VtParser` and `PtySession`
    /// (`SIGWINCH`) if the `OutputCanvas` `(rows, cols)` actually changed. Moving, pinning,
    /// resizing, or hiding the `FloatingCommandPrompt` never triggers `SIGWINCH`.
    pub fn resize_viewport(&mut self, pane_area: Rect) -> Result<bool> {
        let canvas = self.canvas_rect(pane_area);
        let new_size = (canvas.height.max(1), canvas.width.max(1));
        if new_size == self.last_canvas_size {
            return Ok(false);
        }
        self.last_canvas_size = new_size;
        self.vt.set_size(new_size.0, new_size.1);
        if let Some(pty) = &self.pty {
            return pty.resize(new_size.0, new_size.1);
        }
        Ok(false)
    }

    /// Changes the floating prompt's anchor without resizing the underlying PTY (`zero SIGWINCH`).
    pub fn set_prompt_anchor(&mut self, anchor: PromptAnchor) {
        self.prompt.anchor = anchor;
    }

    /// Moves the floating prompt vertically by `delta_rows` (switching to `PromptAnchor::Floating`).
    pub fn move_prompt_vertical(&mut self, delta_rows: i16, pane_height: u16) {
        let canvas_h = pane_height.saturating_sub(1).max(1);
        let max_y = canvas_h.saturating_sub(self.prompt.height);
        let current_y = match self.prompt.anchor {
            PromptAnchor::Top => 0,
            PromptAnchor::Bottom => max_y,
            PromptAnchor::Floating { y_offset } => y_offset.min(max_y),
        };
        let next_y = (i32::from(current_y) + i32::from(delta_rows)).clamp(0, i32::from(max_y)) as u16;
        self.prompt.anchor = PromptAnchor::Floating { y_offset: next_y };
    }

    /// Toggles manual Raw PTY passthrough (`Ctrl+G` / `F12`).
    pub fn toggle_manual_raw_override(&mut self) {
        self.prompt.manual_raw_override = !self.prompt.manual_raw_override;
    }

    /// Refreshes three-signal line-discipline telemetry (`OSC 133` + `tcgetpgrp` + `termios !ECHO`).
    pub fn refresh_line_discipline(&mut self) {
        let osc = self.vt.osc133_phase();
        #[cfg(unix)]
        if let Some(pty) = &self.pty {
            if let Some(fd) = pty.master_raw_fd() {
                self.line_discipline = inspect_pty_fd(fd, pty.child_pid(), osc);
                return;
            }
        }
        self.line_discipline.osc133_executing =
            matches!(osc, Some(crate::vt::Osc133Phase::CommandExecuted));
    }

    /// Determines the active keyboard routing mode (`PTY-02`, `PTY-03`).
    pub fn effective_routing_mode(&self) -> InputRoutingMode {
        if self.vt.is_alternate_screen()
            || self.line_discipline.requires_raw_passthrough()
            || self.prompt.manual_raw_override
        {
            InputRoutingMode::RawPtyPassthrough
        } else {
            InputRoutingMode::CommandPrompt
        }
    }

    /// Returns `true` when the `FloatingCommandPrompt` overlay should be rendered over the `OutputCanvas`.
    ///
    /// Automatically hides during `DECSET 47/1047/1049` alternate screen (`vim`, `htop`, `less`),
    /// `!ECHO` password prompts (`sudo`, `ssh`), foreground sub-REPLs, or manual raw override.
    pub fn is_prompt_visible(&self) -> bool {
        self.effective_routing_mode() == InputRoutingMode::CommandPrompt
    }

    /// Feeds raw PTY output bytes into the pane's `VtParser` and updates line-discipline state.
    pub fn process_pty_output(&mut self, bytes: &[u8]) {
        self.vt.process(bytes);
        self.refresh_line_discipline();
    }

    /// Drains any pending events from the live `PtySession`.
    pub fn poll_pty_events(&mut self) {
        let events = if let Some(pty) = &mut self.pty {
            pty.drain_events()
        } else {
            Vec::new()
        };
        for ev in events {
            match ev {
                PtyEvent::Output(bytes) => self.process_pty_output(&bytes),
                PtyEvent::Exited(_) => {
                    self.exited = true;
                }
            }
        }
    }

    pub fn has_exited(&self) -> bool {
        self.exited
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_or_hiding_prompt_never_resizes_canvas_or_emits_sigwinch() {
        let mut pane = CompositePane::new_headless(1, 25, 80);
        let viewport = Rect::new(0, 0, 80, 25);
        assert_eq!(pane.canvas_rect(viewport), Rect::new(0, 0, 80, 24));

        // Move prompt Top -> Bottom -> Floating: canvas size never changes
        pane.set_prompt_anchor(PromptAnchor::Bottom);
        assert_eq!(pane.prompt_rect(viewport).y, 21);
        assert!(!pane.resize_viewport(viewport).unwrap());

        pane.move_prompt_vertical(-5, 25);
        assert_eq!(pane.prompt.anchor, PromptAnchor::Floating { y_offset: 16 });
        assert!(!pane.resize_viewport(viewport).unwrap());

        pane.toggle_manual_raw_override();
        assert!(!pane.is_prompt_visible());
        assert!(!pane.resize_viewport(viewport).unwrap());
    }

    #[test]
    fn alternate_screen_and_password_lock_auto_hide_floating_prompt() {
        let mut pane = CompositePane::new_headless(1, 25, 80);
        assert!(pane.is_prompt_visible());
        assert_eq!(pane.effective_routing_mode(), InputRoutingMode::CommandPrompt);

        // Enter DECSET 1049 alternate screen (e.g., vim / htop)
        pane.process_pty_output(b"\x1b[?1049h");
        assert!(!pane.is_prompt_visible());
        assert_eq!(
            pane.effective_routing_mode(),
            InputRoutingMode::RawPtyPassthrough
        );

        // Exit DECSET 1049 alternate screen -> prompt automatically restores
        pane.process_pty_output(b"\x1b[?1049l");
        assert!(pane.is_prompt_visible());

        // Simulate sudo !ECHO password prompt -> prompt hides and routes to RawPtyPassthrough
        pane.line_discipline.echo_enabled = false;
        assert!(!pane.is_prompt_visible());
        assert_eq!(
            pane.effective_routing_mode(),
            InputRoutingMode::RawPtyPassthrough
        );
    }
}
