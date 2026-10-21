pub mod colon_command;
pub mod fuzzy_history;
pub mod highlight;

use ratatui::layout::Rect;

use crate::error::Result;
use crate::pane::fuzzy_history::FuzzyHistorySearchState;
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct EditorSnapshot {
    lines: Vec<String>,
    cursor_row: usize,
    cursor_col: usize,
}

/// Interactive state for the pane's `FloatingCommandPrompt` overlay (`PANE-01..04`).
#[derive(Clone, Debug)]
pub struct FloatingPromptState {
    /// Flattened view of `lines.join("\n")` kept synchronized for single-string inspection.
    pub input: String,
    /// Multi-line buffer (`PANE-04`). Always has at least 1 line.
    pub lines: Vec<String>,
    /// Active cursor row index (`0..lines.len()`).
    pub cursor_row: usize,
    /// Active cursor character column within `lines[cursor_row]`.
    pub cursor_col: usize,
    /// Global character offset across `input` (`lines.join("\n")`).
    pub cursor_char_idx: usize,
    pub anchor: PromptAnchor,
    /// Minimum/base prompt box height in rows (default `3`, range `3..=12`).
    pub height: u16,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    /// Last killed text (`Ctrl+K`, `Ctrl+U`, `Ctrl+W`) available for `Ctrl+Y` yank.
    pub yank_buffer: String,
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,
    /// Interactive `Ctrl+R` fuzzy history search state (`nucleo-matcher`).
    pub fuzzy_search: FuzzyHistorySearchState,
    /// Manual toggle (`Ctrl+G` / `F12`) allowing the user to hide the floating prompt and
    /// route keystrokes directly to the underlying PTY at any time (`PTY-02`).
    pub manual_raw_override: bool,
}

impl Default for FloatingPromptState {
    fn default() -> Self {
        Self {
            input: String::new(),
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            cursor_char_idx: 0,
            anchor: PromptAnchor::Top,
            height: 3,
            history: Vec::new(),
            history_index: None,
            yank_buffer: String::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            fuzzy_search: FuzzyHistorySearchState::default(),
            manual_raw_override: false,
        }
    }
}

impl FloatingPromptState {
    fn snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            lines: self.lines.clone(),
            cursor_row: self.cursor_row,
            cursor_col: self.cursor_col,
        }
    }

    fn record_undo(&mut self) {
        let snap = self.snapshot();
        if self.undo_stack.last() != Some(&snap) {
            self.undo_stack.push(snap);
            if self.undo_stack.len() > 100 {
                self.undo_stack.remove(0);
            }
        }
        self.redo_stack.clear();
    }

    fn sync_from_lines(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor_row = self.cursor_row.min(self.lines.len() - 1);
        let row_len = self.lines[self.cursor_row].chars().count();
        self.cursor_col = self.cursor_col.min(row_len);
        self.input = self.lines.join("\n");

        let mut flat_idx = 0;
        for r in 0..self.cursor_row {
            flat_idx += self.lines[r].chars().count() + 1;
        }
        flat_idx += self.cursor_col;
        self.cursor_char_idx = flat_idx;
    }

    /// Replaces the entire editor contents with `text` (splitting on `\n`).
    pub fn set_text(&mut self, text: &str) {
        self.record_undo();
        self.lines = text.split('\n').map(ToOwned::to_owned).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor_row = self.lines.len() - 1;
        self.cursor_col = self.lines[self.cursor_row].chars().count();
        self.sync_from_lines();
    }

    /// Returns the full multi-line command text.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn insert_char(&mut self, ch: char) {
        if ch == '\n' {
            self.insert_newline();
            return;
        }
        self.record_undo();
        let byte_idx = Self::char_to_byte(&self.lines[self.cursor_row], self.cursor_col);
        self.lines[self.cursor_row].insert(byte_idx, ch);
        self.cursor_col += 1;
        self.sync_from_lines();
    }

    /// Inserts a multi-character string (such as a bracketed paste payload) at the cursor.
    pub fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.record_undo();
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        for ch in normalized.chars() {
            if ch == '\n' {
                let byte_idx = Self::char_to_byte(&self.lines[self.cursor_row], self.cursor_col);
                let tail = self.lines[self.cursor_row].split_off(byte_idx);
                self.cursor_row += 1;
                self.lines.insert(self.cursor_row, tail);
                self.cursor_col = 0;
            } else {
                let byte_idx = Self::char_to_byte(&self.lines[self.cursor_row], self.cursor_col);
                self.lines[self.cursor_row].insert(byte_idx, ch);
                self.cursor_col += 1;
            }
        }
        self.sync_from_lines();
    }

    /// Inserts a newline (`Shift+Enter` or `Alt+Enter`) splitting the current line at `cursor_col`.
    pub fn insert_newline(&mut self) {
        self.record_undo();
        let byte_idx = Self::char_to_byte(&self.lines[self.cursor_row], self.cursor_col);
        let tail = self.lines[self.cursor_row].split_off(byte_idx);
        self.cursor_row += 1;
        self.lines.insert(self.cursor_row, tail);
        self.cursor_col = 0;
        self.sync_from_lines();
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            self.record_undo();
            let prev_col = self.cursor_col - 1;
            let byte_idx = Self::char_to_byte(&self.lines[self.cursor_row], prev_col);
            self.lines[self.cursor_row].remove(byte_idx);
            self.cursor_col = prev_col;
            self.sync_from_lines();
        } else if self.cursor_row > 0 {
            self.record_undo();
            let current_line = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            let prev_len = self.lines[self.cursor_row].chars().count();
            self.lines[self.cursor_row].push_str(&current_line);
            self.cursor_col = prev_len;
            self.sync_from_lines();
        }
    }

    pub fn delete_char(&mut self) {
        let row_len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col < row_len {
            self.record_undo();
            let byte_idx = Self::char_to_byte(&self.lines[self.cursor_row], self.cursor_col);
            self.lines[self.cursor_row].remove(byte_idx);
            self.sync_from_lines();
        } else if self.cursor_row + 1 < self.lines.len() {
            self.record_undo();
            let next_line = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next_line);
            self.sync_from_lines();
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
        }
        self.sync_from_lines();
    }

    pub fn move_cursor_right(&mut self) {
        let row_len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col < row_len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
        self.sync_from_lines();
    }

    /// Moves cursor up one row in multi-line input, or returns `false` if already at row 0.
    pub fn move_cursor_up(&mut self) -> bool {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            let len = self.lines[self.cursor_row].chars().count();
            self.cursor_col = self.cursor_col.min(len);
            self.sync_from_lines();
            true
        } else {
            false
        }
    }

    /// Moves cursor down one row in multi-line input, or returns `false` if already at the last row.
    pub fn move_cursor_down(&mut self) -> bool {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            let len = self.lines[self.cursor_row].chars().count();
            self.cursor_col = self.cursor_col.min(len);
            self.sync_from_lines();
            true
        } else {
            false
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_col = 0;
        self.sync_from_lines();
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_row].chars().count();
        self.sync_from_lines();
    }

    pub fn move_word_left(&mut self) {
        let chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
        let mut col = self.cursor_col.min(chars.len());
        while col > 0 && chars[col - 1].is_whitespace() {
            col -= 1;
        }
        while col > 0 && !chars[col - 1].is_whitespace() {
            col -= 1;
        }
        self.cursor_col = col;
        self.sync_from_lines();
    }

    pub fn move_word_right(&mut self) {
        let chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
        let n = chars.len();
        let mut col = self.cursor_col.min(n);
        while col < n && !chars[col].is_whitespace() {
            col += 1;
        }
        while col < n && chars[col].is_whitespace() {
            col += 1;
        }
        self.cursor_col = col;
        self.sync_from_lines();
    }

    /// `Ctrl+K`: kills from cursor to end of current line and stores in `yank_buffer`.
    pub fn kill_to_end_of_line(&mut self) {
        self.record_undo();
        let byte_idx = Self::char_to_byte(&self.lines[self.cursor_row], self.cursor_col);
        let killed = self.lines[self.cursor_row].split_off(byte_idx);
        if !killed.is_empty() {
            self.yank_buffer = killed;
        }
        self.sync_from_lines();
    }

    /// `Ctrl+W`: deletes the word preceding the cursor and stores it in `yank_buffer`.
    pub fn kill_prev_word(&mut self) {
        let orig_col = self.cursor_col;
        if orig_col == 0 {
            return;
        }
        self.record_undo();
        let chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
        let mut start_col = orig_col.min(chars.len());
        while start_col > 0 && chars[start_col - 1].is_whitespace() {
            start_col -= 1;
        }
        while start_col > 0 && !chars[start_col - 1].is_whitespace() {
            start_col -= 1;
        }
        let start_byte = Self::char_to_byte(&self.lines[self.cursor_row], start_col);
        let end_byte = Self::char_to_byte(&self.lines[self.cursor_row], orig_col);
        self.yank_buffer = self.lines[self.cursor_row][start_byte..end_byte].to_string();
        self.lines[self.cursor_row].replace_range(start_byte..end_byte, "");
        self.cursor_col = start_col;
        self.sync_from_lines();
    }

    /// `Ctrl+Y`: yanks the last killed text at the cursor.
    pub fn yank(&mut self) {
        let text = self.yank_buffer.clone();
        self.insert_str(&text);
    }

    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo_stack.pop() else {
            return false;
        };
        self.redo_stack.push(self.snapshot());
        self.lines = prev.lines;
        self.cursor_row = prev.cursor_row;
        self.cursor_col = prev.cursor_col;
        self.sync_from_lines();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        self.undo_stack.push(self.snapshot());
        self.lines = next.lines;
        self.cursor_row = next.cursor_row;
        self.cursor_col = next.cursor_col;
        self.sync_from_lines();
        true
    }

    pub fn clear_line(&mut self) {
        if !self.input.is_empty() {
            self.record_undo();
            self.yank_buffer = self.input.clone();
        }
        self.lines = vec![String::new()];
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.sync_from_lines();
    }

    pub fn take_submission(&mut self) -> String {
        let full = self.text();
        self.lines = vec![String::new()];
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.history_index = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.sync_from_lines();
        if !full.trim().is_empty() && self.history.last() != Some(&full) {
            self.history.push(full.clone());
        }
        full
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
        let cmd = self.history[next_idx].clone();
        self.set_text(&cmd);
    }

    pub fn history_next(&mut self) {
        let Some(curr) = self.history_index else {
            return;
        };
        if curr + 1 < self.history.len() {
            let next_idx = curr + 1;
            self.history_index = Some(next_idx);
            let cmd = self.history[next_idx].clone();
            self.set_text(&cmd);
        } else {
            self.history_index = None;
            self.clear_line();
        }
    }

    /// Opens interactive `Ctrl+R` fuzzy history search (`PANE-04`).
    pub fn open_fuzzy_search(&mut self) {
        self.fuzzy_search.active = true;
        self.fuzzy_search.query.clear();
        self.fuzzy_search.selected = 0;
        self.fuzzy_search.recompute(&self.history);
    }

    pub fn push_fuzzy_query_char(&mut self, ch: char) {
        self.fuzzy_search.query.push(ch);
        self.fuzzy_search.recompute(&self.history);
    }

    pub fn pop_fuzzy_query_char(&mut self) {
        self.fuzzy_search.query.pop();
        self.fuzzy_search.recompute(&self.history);
    }

    pub fn accept_fuzzy_match(&mut self) -> bool {
        let chosen = self.fuzzy_search.current_match().map(|m| m.command.clone());
        self.fuzzy_search.active = false;
        if let Some(cmd) = chosen {
            self.set_text(&cmd);
            true
        } else {
            false
        }
    }

    pub fn cancel_fuzzy_search(&mut self) {
        self.fuzzy_search.active = false;
    }

    /// Computes the effective prompt height, growing automatically for multi-line commands or active
    /// `Ctrl+R` fuzzy search while respecting `canvas_height`.
    pub fn effective_height(&self, canvas_height: u16) -> u16 {
        let content_rows = if self.fuzzy_search.active {
            // Query line + up to 4 match lines + 2 border rows
            (self.fuzzy_search.matches.len().min(4) as u16 + 3).max(self.height)
        } else {
            (self.lines.len() as u16 + 2).clamp(self.height, 12)
        };
        content_rows.clamp(3, canvas_height.max(1)).min(canvas_height)
    }

    fn char_to_byte(s: &str, char_idx: usize) -> usize {
        s.char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(s.len())
    }
}

/// Active mouse drag state for moving or resizing a pane's `FloatingCommandPrompt` (`PANE-03`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptMouseDrag {
    /// Dragging the top border of the floating prompt vertically.
    MovePrompt { grab_x_offset: u16 },
    /// Dragging the bottom border of the floating prompt to resize its height (`3..=12`).
    ResizePrompt,
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
    pub mouse_drag: Option<PromptMouseDrag>,
    /// Optional status feedback message displayed in the OIA bar (e.g. `:command` result or yank feedback).
    pub status_message: Option<String>,
    /// Last outbound `OSC 52` clipboard escape sequence emitted by `:yank` or child PTY passthrough.
    pub last_osc52_sequence: Option<String>,
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
            mouse_drag: None,
            status_message: None,
            last_osc52_sequence: None,
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
            mouse_drag: None,
            status_message: None,
            last_osc52_sequence: None,
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
        let height = self.prompt.effective_height(canvas.height);
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

    /// Sets the base height (`3..=12` rows) of the floating command prompt without resizing the PTY (`PANE-03`).
    pub fn set_prompt_height(&mut self, rows: u16) {
        self.prompt.height = rows.clamp(3, 12);
    }

    /// Adjusts the base height of the floating command prompt by `delta_rows` (`Alt+Plus` / `Alt+Minus`).
    pub fn resize_prompt_height(&mut self, delta_rows: i16) {
        let next = (i32::from(self.prompt.height) + i32::from(delta_rows)).clamp(3, 12) as u16;
        self.prompt.height = next;
    }

    /// Moves the floating prompt vertically by `delta_rows` (switching to `PromptAnchor::Floating`).
    pub fn move_prompt_vertical(&mut self, delta_rows: i16, pane_height: u16) {
        let canvas_h = pane_height.saturating_sub(1).max(1);
        let eff_h = self.prompt.effective_height(canvas_h);
        let max_y = canvas_h.saturating_sub(eff_h);
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

    #[test]
    fn multiline_editing_undo_redo_and_word_navigation_work() {
        let mut prompt = FloatingPromptState::default();
        prompt.insert_str("echo first");
        prompt.insert_newline();
        prompt.insert_str("echo second");
        assert_eq!(prompt.lines.len(), 2);
        assert_eq!(prompt.text(), "echo first\necho second");

        // Undo removes "echo second"
        assert!(prompt.undo());
        assert_eq!(prompt.text(), "echo first\n");
        assert!(prompt.redo());
        assert_eq!(prompt.text(), "echo first\necho second");

        // Word navigation and kill/yank
        prompt.move_word_left();
        assert_eq!(prompt.cursor_col, 5);
        prompt.kill_to_end_of_line();
        assert_eq!(prompt.lines[1], "echo ");
        assert_eq!(prompt.yank_buffer, "second");
        prompt.yank();
        assert_eq!(prompt.lines[1], "echo second");
    }
}
