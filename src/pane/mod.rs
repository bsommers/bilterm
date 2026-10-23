pub mod colon_command;
pub mod completion;
pub mod fuzzy_history;
pub mod highlight;
pub mod history_store;

use std::path::PathBuf;

use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::pane::completion::{compute_completions, longest_common_prefix, TabCompletionState};
use crate::pane::fuzzy_history::FuzzyHistorySearchState;
use crate::pane::history_store::HistoryStore;
use crate::pty::line_discipline::{inspect_pty_fd, InputRoutingMode, LineDisciplineState};
use crate::pty::{PtyConfig, PtyEvent, PtySession};
use crate::vt::{CanvasHint, CanvasSearchMatch, VtParser, encode_osc52_clipboard};

/// Unique identifier for a `CompositePane`.
pub type PaneId = usize;

/// Vertical docking, floating, or non-overlapping split mode for a pane's command prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PromptAnchor {
    /// Pinned to the top of the pane's output canvas (classic IBM 3270 mainframe default, floating over output).
    Top,
    /// Pinned to the bottom of the pane's output canvas (floating over output above the 1-row OIA status bar).
    Bottom,
    /// Free-floating at a vertical row offset relative to the top of the output canvas.
    Floating { y_offset: u16 },
    /// Non-overlapping split: dedicated command pane at the top, dedicated output canvas below.
    SplitTop,
    /// Non-overlapping split: dedicated output canvas at the top, dedicated command pane below.
    SplitBottom,
}

impl PromptAnchor {
    /// Returns `true` when the command pane and output canvas are partitioned with zero overlap.
    pub fn is_split(self) -> bool {
        matches!(self, Self::SplitTop | Self::SplitBottom)
    }
}

/// High-level user-facing terminal/prompt mode for a `CompositePane`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PromptMode {
    /// Floating command prompt overlaid on top of the full-height scrolling `OutputCanvas`.
    Floating,
    /// Fixed (docked/split) command prompt partitioned with zero overlap above or below the `OutputCanvas`.
    Fixed,
    /// Regular terminal mode: command prompt box is hidden and all keystrokes go directly to the shell PTY.
    RegularTerminal,
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
    /// Optional custom width in columns (`None` = full width minus 1-col margin).
    pub width_cols: Option<u16>,
    /// Optional custom horizontal column offset relative to the canvas left edge when `width_cols` is set.
    pub x_offset: Option<u16>,
    /// Vertical line scroll offset inside the command prompt editor (`0..lines.len()`).
    pub scroll_offset: usize,
    /// Horizontal character scroll offset inside the command prompt editor.
    pub h_scroll_offset: usize,
    /// Last submitted command shown on the prompt banner while a foreground command (`tail -f /dev/null`, etc.) is running or pending.
    pub running_command: Option<String>,
    /// Whether the interactive Borland-style Command History Pop-up Window is open.
    pub history_popup_open: bool,
    /// Selected entry index inside the Command History Pop-up Window (`0..history.len()`).
    pub history_popup_selected: usize,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    /// Last killed text (`Ctrl+K`, `Ctrl+U`, `Ctrl+W`) available for `Ctrl+Y` yank.
    pub yank_buffer: String,
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,
    /// Interactive `Ctrl+R` fuzzy history search state (`nucleo-matcher`).
    pub fuzzy_search: FuzzyHistorySearchState,
    /// Interactive `Tab` / `Shift+Tab` completion state for paths, commands, `$ENV`, and `:colon` commands.
    pub completion: TabCompletionState,
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
            width_cols: None,
            x_offset: None,
            scroll_offset: 0,
            h_scroll_offset: 0,
            running_command: None,
            history_popup_open: false,
            history_popup_selected: 0,
            history: Vec::new(),
            history_index: None,
            yank_buffer: String::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            fuzzy_search: FuzzyHistorySearchState::default(),
            completion: TabCompletionState::default(),
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
        self.completion.clear();
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
        self.completion.clear();
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
        }
        self.sync_from_lines();
    }

    pub fn move_cursor_right(&mut self) {
        self.completion.clear();
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
        self.completion.clear();
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
        self.completion.clear();
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
        self.completion.clear();
        self.cursor_col = 0;
        self.sync_from_lines();
    }

    pub fn move_cursor_end(&mut self) {
        self.completion.clear();
        self.cursor_col = self.lines[self.cursor_row].chars().count();
        self.sync_from_lines();
    }

    pub fn move_word_left(&mut self) {
        self.completion.clear();
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
        self.completion.clear();
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

    /// `Alt+D` / `Alt+Delete`: deletes the word following the cursor and stores it in `yank_buffer`.
    pub fn kill_next_word(&mut self) {
        let chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
        let n = chars.len();
        let orig_col = self.cursor_col.min(n);
        if orig_col >= n {
            return;
        }
        self.record_undo();
        let mut end_col = orig_col;
        while end_col < n && chars[end_col].is_whitespace() {
            end_col += 1;
        }
        while end_col < n && !chars[end_col].is_whitespace() {
            end_col += 1;
        }
        let start_byte = Self::char_to_byte(&self.lines[self.cursor_row], orig_col);
        let end_byte = Self::char_to_byte(&self.lines[self.cursor_row], end_col);
        self.yank_buffer = self.lines[self.cursor_row][start_byte..end_byte].to_string();
        self.lines[self.cursor_row].replace_range(start_byte..end_byte, "");
        self.sync_from_lines();
    }

    /// `Ctrl+Y`: yanks the last killed text at the cursor.
    pub fn yank(&mut self) {
        let text = self.yank_buffer.clone();
        self.insert_str(&text);
    }

    pub fn undo(&mut self) -> bool {
        self.completion.clear();
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
        self.completion.clear();
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
        self.completion.clear();
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
        self.completion.clear();
        let full = self.text();
        self.lines = vec![String::new()];
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.history_index = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.sync_from_lines();
        let trimmed = full.trim();
        if !trimmed.is_empty() && !trimmed.starts_with(':') {
            self.running_command = Some(full.replace('\n', " ; "));
        }
        if !trimmed.is_empty() && self.history.last() != Some(&full) {
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
        self.completion.clear();
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

    /// Triggers or cycles `Tab` / `Shift+Tab` completion at the current cursor position.
    pub fn complete_tab(&mut self, cwd: &std::path::Path, reverse: bool) -> bool {
        if self.completion.is_active() {
            let len = self.completion.candidates.len();
            if len == 0 {
                self.completion.clear();
                return false;
            }
            let next_idx = match self.completion.selected {
                None => {
                    if reverse {
                        len - 1
                    } else {
                        0
                    }
                }
                Some(curr) => {
                    if reverse {
                        (curr + len - 1) % len
                    } else {
                        (curr + 1) % len
                    }
                }
            };
            self.completion.selected = Some(next_idx);
            let replacement = self.completion.candidates[next_idx].replacement.clone();
            self.apply_completion_span(replacement);
            return true;
        }

        let current_line = self.lines[self.cursor_row].clone();
        let (token_start, token_end, candidates) =
            compute_completions(&current_line, self.cursor_col, cwd);

        if candidates.is_empty() {
            self.completion.clear();
            return false;
        }

        let snap = self.snapshot();
        if self.undo_stack.last() != Some(&snap) {
            self.undo_stack.push(snap);
            if self.undo_stack.len() > 100 {
                self.undo_stack.remove(0);
            }
        }
        self.redo_stack.clear();

        if candidates.len() == 1 {
            self.completion.token_start = token_start;
            self.completion.token_end = token_end;
            let replacement = candidates[0].replacement.clone();
            self.apply_completion_span(replacement);
            self.completion.clear();
            return true;
        }

        let orig_token: String = current_line
            .chars()
            .skip(token_start)
            .take(token_end.saturating_sub(token_start))
            .collect();
        let lcp = longest_common_prefix(&candidates);

        self.completion.active = true;
        self.completion.token_start = token_start;
        self.completion.token_end = token_end;
        self.completion.original_prefix = orig_token.clone();
        self.completion.candidates = candidates;

        if lcp.chars().count() > orig_token.chars().count() {
            self.completion.selected = None;
            self.apply_completion_span(lcp);
        } else {
            let first_idx = if reverse {
                self.completion.candidates.len() - 1
            } else {
                0
            };
            self.completion.selected = Some(first_idx);
            let replacement = self.completion.candidates[first_idx].replacement.clone();
            self.apply_completion_span(replacement);
        }
        true
    }

    fn apply_completion_span(&mut self, replacement: String) {
        let start_byte =
            Self::char_to_byte(&self.lines[self.cursor_row], self.completion.token_start);
        let end_byte = Self::char_to_byte(&self.lines[self.cursor_row], self.completion.token_end);
        let rep_chars = replacement.chars().count();
        self.lines[self.cursor_row].replace_range(start_byte..end_byte, &replacement);
        let new_end = self.completion.token_start + rep_chars;
        self.completion.token_end = new_end;
        self.cursor_col = new_end;
        self.sync_from_lines();
    }

    /// Computes the effective prompt height, growing automatically for multi-line commands, active
    /// `Ctrl+R` fuzzy search, active `Tab` completion candidate bar, or active `running_command` banner
    /// while respecting `canvas_height`.
    pub fn effective_height(&self, canvas_height: u16) -> u16 {
        let running_extra = u16::from(self.running_command.is_some());
        let content_rows = if self.fuzzy_search.active {
            // Query line + up to 4 match lines + 2 border rows
            (self.fuzzy_search.matches.len().min(4) as u16 + 3).max(self.height)
        } else if self.completion.is_active() {
            // Command lines + 1 completion bar row + running banner + 2 border rows
            (self.lines.len() as u16 + 3 + running_extra).clamp(self.height.max(4), 12)
        } else {
            (self.lines.len() as u16 + 2 + running_extra).clamp(self.height, 12)
        };
        content_rows.clamp(3, canvas_height.max(1)).min(canvas_height)
    }

    /// Scrolls the command prompt's multi-line viewport up by `rows`.
    pub fn scroll_up(&mut self, rows: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(rows);
    }

    /// Scrolls the command prompt's multi-line viewport down by `rows`.
    pub fn scroll_down(&mut self, rows: usize) {
        let max_scroll = self.lines.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + rows).min(max_scroll);
    }

    /// Opens or closes the Borland-style Command History Pop-up Window.
    pub fn toggle_history_popup(&mut self) {
        self.history_popup_open = !self.history_popup_open;
        if self.history_popup_open && !self.history.is_empty() {
            self.history_popup_selected = self.history.len() - 1;
        }
    }

    pub fn history_popup_prev(&mut self) {
        if self.history_popup_selected > 0 {
            self.history_popup_selected -= 1;
        }
    }

    pub fn history_popup_next(&mut self) {
        if self.history_popup_selected + 1 < self.history.len() {
            self.history_popup_selected += 1;
        }
    }

    pub fn accept_history_popup_selection(&mut self) -> bool {
        self.history_popup_open = false;
        if let Some(cmd) = self.history.get(self.history_popup_selected).cloned() {
            self.set_text(&cmd);
            true
        } else {
            false
        }
    }

    fn char_to_byte(s: &str, char_idx: usize) -> usize {
        s.char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(s.len())
    }
}

/// Active mouse drag state for moving or resizing a pane's `FloatingCommandPrompt` (`PANE-03`),
/// or dragging a rectangular/character selection on the `OutputCanvas` (`VAL-04`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptMouseDrag {
    /// Dragging the top border of the floating prompt vertically and/or horizontally.
    MovePrompt { grab_x_offset: u16 },
    /// Dragging the bottom border of the floating prompt to resize its height (`3..=12`).
    ResizePrompt,
    /// Dragging the left border of the floating prompt to resize its width and horizontal offset.
    ResizePromptLeft,
    /// Dragging the right border of the floating prompt to resize its width.
    ResizePromptRight,
    /// Dragging the bottom-right corner (`◢`) to resize both width and height simultaneously.
    ResizePromptCorner,
    /// Dragging a rectangular block selection (`Alt+LeftDrag`) on the `OutputCanvas` (`VAL-04`).
    CanvasRectSelect,
}

/// Interactive scrollback search & match highlighting state for the `OutputCanvas` (`VAL-02`).
#[derive(Clone, Debug, Default)]
pub struct CanvasSearchState {
    pub active: bool,
    pub query: String,
    pub matches: Vec<CanvasSearchMatch>,
    pub selected: usize,
}

impl CanvasSearchState {
    /// Returns `(is_match, is_selected_match)` for a viewport cell `(row, col)`.
    pub fn cell_match_status(&self, row: u16, col: u16) -> (bool, bool) {
        if !self.active || self.matches.is_empty() {
            return (false, false);
        }
        let mut any_match = false;
        let mut selected_match = false;
        for (idx, m) in self.matches.iter().enumerate() {
            if m.row == row && col >= m.start_col && col <= m.end_col {
                any_match = true;
                if idx == self.selected {
                    selected_match = true;
                    break;
                }
            }
        }
        (any_match, selected_match)
    }
}

/// Selection geometry mode inside Vi Copy Mode (`VAL-04`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CopySelectionKind {
    #[default]
    None,
    /// Standard character-wise stream selection (`v`).
    Char,
    /// Full-line selection (`V`).
    Line,
    /// Rectangular column/block selection (`Ctrl+V` / `b` or `Alt+LeftDrag`).
    Block,
}

/// Keyboard-driven Vi Copy Mode & rectangular block selection state for the `OutputCanvas` (`VAL-04`).
#[derive(Clone, Debug, Default)]
pub struct CopyModeState {
    pub active: bool,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub anchor: Option<(u16, u16)>,
    pub selection_kind: CopySelectionKind,
}

impl CopyModeState {
    /// Returns `true` if cell `(row, col)` lies inside the current selection (`Char`, `Line`, or `Block`).
    pub fn is_cell_selected(&self, row: u16, col: u16) -> bool {
        let Some((ar, ac)) = self.anchor else {
            return false;
        };
        let (cr, cc) = (self.cursor_row, self.cursor_col);
        match self.selection_kind {
            CopySelectionKind::None => false,
            CopySelectionKind::Line => {
                let min_r = ar.min(cr);
                let max_r = ar.max(cr);
                row >= min_r && row <= max_r
            }
            CopySelectionKind::Block => {
                let min_r = ar.min(cr);
                let max_r = ar.max(cr);
                let min_c = ac.min(cc);
                let max_c = ac.max(cc);
                row >= min_r && row <= max_r && col >= min_c && col <= max_c
            }
            CopySelectionKind::Char => {
                let ((sr, sc), (er, ec)) = if (ar, ac) <= (cr, cc) {
                    ((ar, ac), (cr, cc))
                } else {
                    ((cr, cc), (ar, ac))
                };
                if row < sr || row > er {
                    false
                } else if sr == er {
                    col >= sc && col <= ec
                } else if row == sr {
                    col >= sc
                } else if row == er {
                    col <= ec
                } else {
                    true
                }
            }
        }
    }

    /// Extracts the selected text from `screen` according to `selection_kind` (`VAL-04`).
    pub fn extract_selected_text(&self, screen: &vt100_ctt::Screen) -> String {
        let (rows, cols) = screen.size();
        if rows == 0 || cols == 0 {
            return String::new();
        }
        let (ar, _ac) = self.anchor.unwrap_or((self.cursor_row, self.cursor_col));
        let (cr, _cc) = (self.cursor_row, self.cursor_col);
        let min_r = ar.min(cr).min(rows.saturating_sub(1));
        let max_r = ar.max(cr).min(rows.saturating_sub(1));

        let mut lines_out = Vec::new();
        for r in min_r..=max_r {
            let mut line = String::new();
            for c in 0..cols {
                let include = if self.anchor.is_some() && self.selection_kind != CopySelectionKind::None {
                    self.is_cell_selected(r, c)
                } else {
                    r == self.cursor_row
                };
                if include {
                    let ch = screen
                        .cell(r, c)
                        .map(|cell| cell.contents())
                        .unwrap_or_default();
                    if ch.is_empty() {
                        line.push(' ');
                    } else {
                        line.push_str(ch);
                    }
                }
            }
            lines_out.push(line.trim_end().to_string());
        }
        lines_out.join("\n")
    }
}

/// Keyboard-driven QuickSelect 3270 Field Hints overlay state (`Alt+F` / `:hints`, `ECO-01`).
#[derive(Clone, Debug, Default)]
pub struct HintsModeState {
    pub active: bool,
    pub hints: Vec<CanvasHint>,
    pub typed_prefix: String,
    pub selected: usize,
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
    pub history_store: HistoryStore,
    pub line_discipline: LineDisciplineState,
    pub mouse_drag: Option<PromptMouseDrag>,
    /// Interactive scrollback search & match highlighting state (`VAL-02`).
    pub canvas_search: CanvasSearchState,
    /// Keyboard-driven Vi Copy Mode & rectangular block selection state (`VAL-04`).
    pub copy_mode: CopyModeState,
    /// Keyboard-driven QuickSelect 3270 Field Hints state (`ECO-01`).
    pub hints_mode: HintsModeState,
    /// Read-Only / Protected 3270 Field Lock (`:lock` / `Alt+L`, `ECO-04`).
    pub read_only: bool,
    /// Whether the Prefix/Leader key (`Ctrl+X` by default) is awaiting the next chord on this pane.
    pub leader_pending: bool,
    /// Optional remote SSH / `tmux -CC` destination badge (`🌐 SSH:<host>`, `v1.4.0`).
    pub remote_target: Option<String>,
    /// Pending multi-line or escape-containing raw PTY paste awaiting user confirmation (`FND-01`).
    pub pending_unsafe_paste: Option<String>,
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
            history_store: HistoryStore::new_headless(id),
            line_discipline: LineDisciplineState::default(),
            mouse_drag: None,
            canvas_search: CanvasSearchState::default(),
            copy_mode: CopyModeState::default(),
            hints_mode: HintsModeState::default(),
            read_only: false,
            leader_pending: false,
            remote_target: None,
            pending_unsafe_paste: None,
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
            history_store: HistoryStore::new_live(id),
            line_discipline: LineDisciplineState::default(),
            mouse_drag: None,
            canvas_search: CanvasSearchState::default(),
            copy_mode: CopyModeState::default(),
            hints_mode: HintsModeState::default(),
            read_only: false,
            leader_pending: false,
            remote_target: None,
            pending_unsafe_paste: None,
            status_message: None,
            last_osc52_sequence: None,
            last_canvas_size: (canvas_rows, canvas_cols),
            exited: false,
        })
    }

    /// Computes the `OutputCanvas` rectangle.
    ///
    /// - In floating modes (`Top`, `Bottom`, `Floating`) or when the prompt is hidden (`ALT_SCREEN` / `!ECHO`),
    ///   occupies `100%` of pane height minus 1 row for the OIA status bar.
    /// - In non-overlapping split modes (`SplitTop`, `SplitBottom`) while the prompt is visible,
    ///   occupies the dedicated remaining height so there is zero overlap with `prompt_rect`.
    pub fn canvas_rect(&self, pane_area: Rect) -> Rect {
        let usable_h = pane_area.height.saturating_sub(1).max(1);
        if self.is_prompt_visible() && self.prompt.anchor.is_split() && usable_h >= 4 {
            let prompt_h = self
                .prompt
                .effective_height(usable_h.saturating_sub(1))
                .min(usable_h.saturating_sub(1));
            let canvas_h = usable_h.saturating_sub(prompt_h).max(1);
            match self.prompt.anchor {
                PromptAnchor::SplitTop => Rect {
                    x: pane_area.x,
                    y: pane_area.y + prompt_h,
                    width: pane_area.width,
                    height: canvas_h,
                },
                PromptAnchor::SplitBottom => Rect {
                    x: pane_area.x,
                    y: pane_area.y,
                    width: pane_area.width,
                    height: canvas_h,
                },
                _ => unreachable!(),
            }
        } else {
            Rect {
                x: pane_area.x,
                y: pane_area.y,
                width: pane_area.width,
                height: usable_h,
            }
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

    /// Computes the rectangle of the command prompt (either overlaid in floating modes or
    /// partitioned with zero overlap in `SplitTop` / `SplitBottom` modes).
    pub fn prompt_rect(&self, pane_area: Rect) -> Rect {
        let usable_h = pane_area.height.saturating_sub(1).max(1);
        if self.prompt.anchor.is_split() && usable_h >= 4 {
            let prompt_h = self
                .prompt
                .effective_height(usable_h.saturating_sub(1))
                .min(usable_h.saturating_sub(1));
            let canvas_h = usable_h.saturating_sub(prompt_h).max(1);
            let y = match self.prompt.anchor {
                PromptAnchor::SplitTop => pane_area.y,
                PromptAnchor::SplitBottom => pane_area.y + canvas_h,
                _ => pane_area.y,
            };
            return Rect {
                x: pane_area.x,
                y,
                width: pane_area.width.max(1),
                height: prompt_h,
            };
        }

        let canvas = self.canvas_rect(pane_area);
        let height = self.prompt.effective_height(canvas.height);
        let max_y_offset = canvas.height.saturating_sub(height);
        let y_offset = match self.prompt.anchor {
            PromptAnchor::Top | PromptAnchor::SplitTop => 0,
            PromptAnchor::Bottom | PromptAnchor::SplitBottom => max_y_offset,
            PromptAnchor::Floating { y_offset } => y_offset.min(max_y_offset),
        };
        let default_margin_x = if canvas.width > 6 { 1 } else { 0 };
        let default_w = canvas.width.saturating_sub(default_margin_x * 2).max(1);
        let width = self
            .prompt
            .width_cols
            .map(|w| w.clamp(16.min(canvas.width), canvas.width.max(1)))
            .unwrap_or(default_w);
        let max_x_off = canvas.width.saturating_sub(width);
        let x_off = self
            .prompt
            .x_offset
            .map(|x| x.min(max_x_off))
            .unwrap_or_else(|| default_margin_x.min(max_x_off));
        Rect {
            x: canvas.x + x_off,
            y: canvas.y + y_offset,
            width,
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

    /// Changes the prompt's anchor (`Top`, `Bottom`, `Floating`, `SplitTop`, or `SplitBottom`)
    /// and ensures the command prompt is visible (exiting Regular Terminal mode if active).
    pub fn set_prompt_anchor(&mut self, anchor: PromptAnchor) {
        self.prompt.manual_raw_override = false;
        self.prompt.anchor = anchor;
    }

    /// Toggles the command prompt between floating mode and non-overlapping fixed/split mode (`Alt+S` / `:prompt split`).
    pub fn toggle_split_prompt(&mut self) {
        self.prompt.manual_raw_override = false;
        self.prompt.anchor = match self.prompt.anchor {
            PromptAnchor::Top | PromptAnchor::Floating { .. } => PromptAnchor::SplitTop,
            PromptAnchor::Bottom => PromptAnchor::SplitBottom,
            PromptAnchor::SplitTop => PromptAnchor::Top,
            PromptAnchor::SplitBottom => PromptAnchor::Bottom,
        };
    }

    /// Returns the active high-level `PromptMode` (`Floating`, `Fixed`, or `RegularTerminal`).
    pub fn prompt_mode(&self) -> PromptMode {
        if self.prompt.manual_raw_override {
            PromptMode::RegularTerminal
        } else if self.prompt.anchor.is_split() {
            PromptMode::Fixed
        } else {
            PromptMode::Floating
        }
    }

    /// Explicitly switches the pane to `Floating` prompt, `Fixed` (docked/split) prompt, or `RegularTerminal` mode.
    pub fn set_prompt_mode(&mut self, mode: PromptMode) {
        match mode {
            PromptMode::Floating => {
                self.prompt.manual_raw_override = false;
                if self.prompt.anchor.is_split() {
                    self.prompt.anchor = match self.prompt.anchor {
                        PromptAnchor::SplitBottom => PromptAnchor::Bottom,
                        _ => PromptAnchor::Top,
                    };
                }
                self.status_message = Some(
                    "MODE: FLOATING PROMPT — overlays terminal canvas (F4/Alt+E to cycle, F12 for Regular Terminal)"
                        .to_string(),
                );
            }
            PromptMode::Fixed => {
                self.prompt.manual_raw_override = false;
                if !self.prompt.anchor.is_split() {
                    self.prompt.anchor = match self.prompt.anchor {
                        PromptAnchor::Bottom => PromptAnchor::SplitBottom,
                        _ => PromptAnchor::SplitTop,
                    };
                }
                self.status_message = Some(
                    "MODE: FIXED (DOCKED) PROMPT — zero overlap with terminal output (F4/Alt+E to cycle)"
                        .to_string(),
                );
            }
            PromptMode::RegularTerminal => {
                self.prompt.manual_raw_override = true;
                self.status_message = Some(
                    "MODE: REGULAR TERMINAL — direct shell input (press F4, Alt+E, or F12/Ctrl+G to restore Command Prompt)"
                        .to_string(),
                );
            }
        }
    }

    /// Cycles through the 3 terminal modes: `Floating Prompt` -> `Fixed Prompt` -> `Regular Terminal` -> `Floating Prompt` (`F4` / `Alt+E` / `:mode`).
    pub fn cycle_prompt_mode(&mut self) -> PromptMode {
        let next = match self.prompt_mode() {
            PromptMode::Floating => PromptMode::Fixed,
            PromptMode::Fixed => PromptMode::RegularTerminal,
            PromptMode::RegularTerminal => PromptMode::Floating,
        };
        self.set_prompt_mode(next);
        next
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

    /// Sets the custom width in columns (`Some(cols)` or `None` for full-width) of the floating command prompt.
    pub fn set_prompt_width(&mut self, cols: Option<u16>) {
        let max_w = self.vt.size().1.max(16);
        self.prompt.width_cols = cols.map(|c| c.clamp(16, max_w));
    }

    /// Adjusts the floating command prompt width by `delta_cols` (`Alt+[` / `Alt+]`).
    pub fn resize_prompt_width(&mut self, delta_cols: i16) {
        let canvas_w = self.vt.size().1.max(16);
        let default_w = canvas_w.saturating_sub(2).max(16);
        let current = self.prompt.width_cols.unwrap_or(default_w);
        let next = (i32::from(current) + i32::from(delta_cols)).clamp(16, i32::from(canvas_w)) as u16;
        self.prompt.width_cols = Some(next);
    }

    /// Moves the floating prompt vertically by `delta_rows` (switching to `PromptAnchor::Floating`).
    pub fn move_prompt_vertical(&mut self, delta_rows: i16, pane_height: u16) {
        let canvas_h = pane_height.saturating_sub(1).max(1);
        let eff_h = self.prompt.effective_height(canvas_h);
        let max_y = canvas_h.saturating_sub(eff_h);
        let current_y = match self.prompt.anchor {
            PromptAnchor::Top | PromptAnchor::SplitTop => 0,
            PromptAnchor::Bottom | PromptAnchor::SplitBottom => max_y,
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
        let osc_finished = matches!(
            osc,
            Some(crate::vt::Osc133Phase::CommandFinished(_) | crate::vt::Osc133Phase::PromptStart)
        );
        #[cfg(unix)]
        if let Some(pty) = &self.pty {
            if let Some(fd) = pty.master_raw_fd() {
                let prev_sub = self.line_discipline.has_foreground_subcommand();
                self.line_discipline = inspect_pty_fd(fd, pty.child_pid(), osc);
                if (prev_sub && !self.line_discipline.has_foreground_subcommand()) || osc_finished {
                    self.prompt.running_command = None;
                }
                // Fallback defense-in-depth: if the cursor line on the OutputCanvas is a password/passphrase
                // prompt while a command is running (even if a subshell didn't change pgrp), enforce !ECHO lock.
                if (self.prompt.running_command.is_some()
                    || self.line_discipline.has_foreground_subcommand())
                    && self.canvas_cursor_line_looks_like_password_prompt()
                {
                    self.line_discipline.echo_enabled = false;
                    self.line_discipline.canonical_mode = true;
                }
                return;
            }
        }
        self.line_discipline.osc133_executing =
            matches!(osc, Some(crate::vt::Osc133Phase::CommandExecuted));
        if osc_finished {
            self.prompt.running_command = None;
        }
        if self.prompt.running_command.is_some() && self.canvas_cursor_line_looks_like_password_prompt() {
            self.line_discipline.echo_enabled = false;
            self.line_discipline.canonical_mode = true;
        }
    }

    /// Checks whether the text on the active `OutputCanvas` cursor row looks like an interactive
    /// password/passphrase prompt (`[sudo] password for ...:`, `[sudo: authenticate] Password:`,
    /// `Password:`, `Enter passphrase for key ...:`).
    pub fn canvas_cursor_line_looks_like_password_prompt(&self) -> bool {
        let screen = self.vt.screen();
        let (cur_r, cur_c) = screen.cursor_position();
        let (_, cols) = screen.size();
        let end_col = (cur_c + 1).min(cols);
        let mut line = String::with_capacity(usize::from(end_col));
        for c in 0..end_col {
            if let Some(cell) = screen.cell(cur_r, c) {
                let s = cell.contents();
                if s.is_empty() {
                    line.push(' ');
                } else {
                    line.push_str(s);
                }
            }
        }
        let lower = line.trim().to_ascii_lowercase();
        (lower.ends_with(':') || lower.ends_with(": "))
            && (lower.contains("password")
                || lower.contains("passphrase")
                || lower.contains("[sudo"))
    }

    /// Determines the active keyboard routing mode (`PTY-02`, `PTY-03`).
    pub fn effective_routing_mode(&self) -> InputRoutingMode {
        if self.vt.is_alternate_screen()
            || self.line_discipline.is_password_prompt()
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

    /// Resolves the live working directory of the child shell process (`OSC 7` reported path first,
    /// then `/proc/<pid>/cwd` on Linux, falling back to `std::env::current_dir()`).
    pub fn resolve_cwd(&self) -> PathBuf {
        if let Some(osc7_cwd) = self.vt.reported_cwd() {
            return osc7_cwd.to_path_buf();
        }
        #[cfg(target_os = "linux")]
        if let Some(pty) = &self.pty {
            if let Some(pid) = pty.child_pid() {
                if let Ok(path) = std::fs::read_link(format!("/proc/{pid}/cwd")) {
                    return path;
                }
            }
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    /// Triggers or cycles `Tab` / `Shift+Tab` completion inside the floating prompt using the pane's live `cwd`.
    pub fn complete_prompt_tab(&mut self, reverse: bool) -> bool {
        let cwd = self.resolve_cwd();
        self.prompt.complete_tab(&cwd, reverse)
    }

    /// Feeds raw PTY output bytes into the pane's `VtParser` and updates line-discipline state.
    pub fn process_pty_output(&mut self, bytes: &[u8]) {
        self.vt.process(bytes);
        if let Some(wt) = self.vt.window_title() {
            self.title = format!("{wt} [#{}]", self.id);
        }
        if let Some(pty) = &self.pty {
            for resp in self.vt.take_query_responses() {
                let _ = pty.write_all(&resp);
            }
        }
        self.refresh_line_discipline();
    }

    /// Drains pending events from the live `PtySession` with a bounded per-frame budget so
    /// high-throughput streams (`tail -f /dev/random`, `yes`) never starve the UI loop.
    pub fn poll_pty_events(&mut self) {
        let events = if let Some(pty) = &mut self.pty {
            pty.drain_events()
        } else {
            Vec::new()
        };
        self.ingest_pty_events(events);
    }

    /// Feeds a batch of drained `PtyEvent`s into the VT state machine in order.
    pub fn ingest_pty_events(&mut self, events: Vec<PtyEvent>) {
        let mut coalesced = Vec::new();
        for ev in events {
            match ev {
                PtyEvent::Output(bytes) => {
                    coalesced.extend_from_slice(&bytes);
                }
                PtyEvent::Exited(_) => {
                    self.exited = true;
                }
            }
        }
        if !coalesced.is_empty() {
            // Every byte must reach the parser in order: truncating the stream splits escape
            // sequences and loses mode changes (DECSET 1049/2004, OSC 133). Per-frame work is
            // already bounded by `PtySession::drain_events` plus the bounded reader channel.
            self.vt.process(&coalesced);
            if let Some(wt) = self.vt.window_title() {
                self.title = format!("{wt} [#{}]", self.id);
            }
            if let Some(pty) = &self.pty {
                for resp in self.vt.take_query_responses() {
                    let _ = pty.write_all(&resp);
                }
            }
            self.refresh_line_discipline();
        }
    }

    /// Writes raw bytes to the underlying `PtySession` if one is attached (`v1.1.0` / `v1.4.0`).
    pub fn write_to_pty(&mut self, bytes: &[u8]) -> Result<()> {
        if let Some(pty) = &self.pty {
            pty.write_all(bytes)?;
        }
        Ok(())
    }

    pub fn has_exited(&self) -> bool {
        self.exited
    }

    /// Executes a case-insensitive search on the visible `OutputCanvas` grid (`VAL-02`).
    pub fn set_canvas_search(&mut self, query: &str) {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            self.clear_canvas_search();
            return;
        }
        let matches = self.vt.search_canvas(trimmed);
        self.canvas_search.active = true;
        self.canvas_search.query = trimmed.to_string();
        self.canvas_search.matches = matches;
        self.canvas_search.selected = 0;
        self.status_message = Some(format!(
            "Search '{}': {} match(es)",
            self.canvas_search.query,
            self.canvas_search.matches.len()
        ));
    }

    /// Advances to the next match in `canvas_search` (`VAL-02`).
    pub fn next_canvas_search_match(&mut self) {
        if !self.canvas_search.matches.is_empty() {
            self.canvas_search.selected =
                (self.canvas_search.selected + 1) % self.canvas_search.matches.len();
            self.status_message = Some(format!(
                "Search '{}': {}/{}",
                self.canvas_search.query,
                self.canvas_search.selected + 1,
                self.canvas_search.matches.len()
            ));
        }
    }

    /// Moves to the previous match in `canvas_search` (`VAL-02`).
    pub fn prev_canvas_search_match(&mut self) {
        if !self.canvas_search.matches.is_empty() {
            let n = self.canvas_search.matches.len();
            self.canvas_search.selected = (self.canvas_search.selected + n - 1) % n;
            self.status_message = Some(format!(
                "Search '{}': {}/{}",
                self.canvas_search.query,
                self.canvas_search.selected + 1,
                self.canvas_search.matches.len()
            ));
        }
    }

    /// Clears active `OutputCanvas` search highlighting (`VAL-02`).
    pub fn clear_canvas_search(&mut self) {
        self.canvas_search.active = false;
        self.canvas_search.query.clear();
        self.canvas_search.matches.clear();
        self.canvas_search.selected = 0;
    }

    /// Enters keyboard-driven Vi Copy Mode on the `OutputCanvas` (`Alt+C` / `:copy`, `VAL-04`).
    pub fn enter_copy_mode(&mut self) {
        let (r, c) = self.vt.screen().cursor_position();
        self.copy_mode.active = true;
        self.copy_mode.cursor_row = r;
        self.copy_mode.cursor_col = c;
        self.copy_mode.anchor = None;
        self.copy_mode.selection_kind = CopySelectionKind::None;
        self.status_message =
            Some("COPY MODE: hjkl move | v char | V line | b block | y yank | Esc exit".to_string());
    }

    /// Exits Vi Copy Mode (`VAL-04`).
    pub fn exit_copy_mode(&mut self) {
        self.copy_mode.active = false;
        self.copy_mode.anchor = None;
        self.copy_mode.selection_kind = CopySelectionKind::None;
        self.status_message = Some("Exited Copy Mode".to_string());
    }

    /// Yanks the active `CopyModeState` selection (or current line) into the prompt yank buffer and `OSC 52` (`VAL-04`).
    pub fn yank_copy_selection(&mut self) -> Option<String> {
        let text = self.copy_mode.extract_selected_text(self.vt.screen());
        self.copy_mode.active = false;
        self.copy_mode.anchor = None;
        self.copy_mode.selection_kind = CopySelectionKind::None;
        if text.is_empty() {
            self.status_message = Some("Copy Mode: empty selection".to_string());
            return None;
        }
        self.prompt.yank_buffer = text.clone();
        let seq = encode_osc52_clipboard(&text);
        self.last_osc52_sequence = Some(seq);
        self.status_message = Some(format!("Yanked {} chars via Copy Mode", text.len()));
        Some(text)
    }

    /// Scans the visible `OutputCanvas` and opens QuickSelect 3270 Field Hints mode (`Alt+F` / `:hints`, `ECO-01`).
    pub fn enter_hints_mode(&mut self) {
        let hints = self.vt.detect_visible_hints();
        if hints.is_empty() {
            self.hints_mode.active = false;
            self.hints_mode.hints.clear();
            self.hints_mode.typed_prefix.clear();
            self.status_message = Some("No actionable field hints found on canvas".to_string());
        } else {
            let count = hints.len();
            self.hints_mode.active = true;
            self.hints_mode.hints = hints;
            self.hints_mode.typed_prefix.clear();
            self.hints_mode.selected = 0;
            self.status_message = Some(format!(
                "3270 FIELD HINTS: {count} token(s) — type [a..z] to insert+yank, Enter for selected, Esc to cancel"
            ));
        }
    }

    /// Exits QuickSelect 3270 Field Hints mode (`ECO-01`).
    pub fn exit_hints_mode(&mut self) {
        self.hints_mode.active = false;
        self.hints_mode.typed_prefix.clear();
        self.status_message = Some("Exited 3270 Field Hints".to_string());
    }

    /// Accepts the hint at `idx`, inserting its text into `prompt` (when not `read_only`) and yanking via `OSC 52` (`ECO-01`).
    pub fn accept_hint_index(&mut self, idx: usize) -> Option<String> {
        let hint = self.hints_mode.hints.get(idx)?.clone();
        self.hints_mode.active = false;
        self.hints_mode.typed_prefix.clear();
        self.prompt.yank_buffer = hint.text.clone();
        self.last_osc52_sequence = Some(encode_osc52_clipboard(&hint.text));
        if !self.read_only {
            self.prompt.insert_str(&hint.text);
        }
        self.status_message = Some(format!("Selected hint [{}]: {}", hint.label, hint.text));
        Some(hint.text)
    }

    /// Processes a typed hint label character (`a`..`z`), immediately accepting on an unambiguous match (`ECO-01`).
    pub fn push_hint_char(&mut self, ch: char) -> Option<String> {
        let lower = ch.to_ascii_lowercase();
        if !lower.is_ascii_lowercase() {
            return None;
        }
        self.hints_mode.typed_prefix.push(lower);
        let prefix = self.hints_mode.typed_prefix.as_str();
        let matching_indices: Vec<usize> = self
            .hints_mode
            .hints
            .iter()
            .enumerate()
            .filter(|(_, h)| h.label.starts_with(prefix))
            .map(|(i, _)| i)
            .collect();

        if matching_indices.is_empty() {
            self.hints_mode.typed_prefix.clear();
            return None;
        }
        self.hints_mode.selected = matching_indices[0];
        if matching_indices.len() == 1 && self.hints_mode.hints[matching_indices[0]].label == prefix
        {
            return self.accept_hint_index(matching_indices[0]);
        }
        None
    }

    /// Sets or clears the Read-Only / Protected 3270 Field Lock (`:lock` / `Alt+L`, `ECO-04`).
    pub fn set_read_only(&mut self, locked: bool) {
        self.read_only = locked;
        self.status_message = Some(format!(
            "Pane #{} Read-Only Protection: {}",
            self.id,
            if locked { "ON (🔒 PROT)" } else { "OFF" }
        ));
    }

    /// Toggles the Read-Only / Protected 3270 Field Lock (`ECO-04`).
    pub fn toggle_read_only(&mut self) -> bool {
        let next = !self.read_only;
        self.set_read_only(next);
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_pty_bursts_are_processed_without_dropping_mode_changes() {
        let mut pane = CompositePane::new_headless(1, 25, 80);
        // A mode switch at the front of a >256 KB burst must never be discarded.
        let mut events = vec![PtyEvent::Output(b"\x1b[?1049h".to_vec())];
        for _ in 0..40 {
            events.push(PtyEvent::Output(vec![b'x'; 8192]));
        }
        pane.ingest_pty_events(events);
        assert!(pane.vt.is_alternate_screen());
        assert!(pane.vt.total_bytes_processed() >= 40 * 8192);
    }

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
