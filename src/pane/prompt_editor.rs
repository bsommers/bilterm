use serde::{Deserialize, Serialize};

use crate::pane::completion::{compute_completions, longest_common_prefix, TabCompletionState};
use crate::pane::fuzzy_history::FuzzyHistorySearchState;

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
    pub(crate) input: String,
    /// Multi-line buffer (`PANE-04`). Always has at least 1 line.
    pub(crate) lines: Vec<String>,
    /// Active cursor row index (`0..lines.len()`).
    pub(crate) cursor_row: usize,
    /// Active cursor character column within `lines[cursor_row]`.
    pub(crate) cursor_col: usize,
    /// Global character offset across `input` (`lines.join("\n")`).
    pub(crate) cursor_char_idx: usize,
    /// Optional selection anchor `(row, col)` for visual Shift+Arrow or mouse drag selection.
    pub(crate) selection_anchor: Option<(usize, usize)>,
    pub anchor: PromptAnchor,
    /// Minimum/base prompt box height in rows (default `3`, range `3..=12`).
    pub height: u16,
    /// Optional custom width in columns (`None` = full width minus 1-col margin).
    pub width_cols: Option<u16>,
    /// Optional custom horizontal column offset relative to the canvas left edge when `width_cols` is set.
    pub x_offset: Option<u16>,
    /// Vertical line scroll offset inside the command prompt editor (`0..lines.len()`).
    pub(crate) scroll_offset: usize,
    /// Horizontal character scroll offset inside the command prompt editor.
    pub(crate) h_scroll_offset: usize,
    /// Last submitted command shown on the prompt banner while a foreground command (`tail -f /dev/null`, etc.) is running or pending.
    pub running_command: Option<String>,
    /// Whether the interactive Borland-style Command History Pop-up Window is open.
    pub history_popup_open: bool,
    /// Selected entry index inside the Command History Pop-up Window (`0..history.len()`).
    pub history_popup_selected: usize,
    pub history: Vec<String>,
    pub(crate) history_index: Option<usize>,
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
    /// Explicit user override (`F4`, `F12`, `Ctrl+X e/t/b/s`, `:mode float/fixed`) that forces
    /// `CommandPrompt` mode when a PTY wrapper (`asciinema rec`, `script`) sets `!ECHO` on the outer PTY (`REC-02`).
    pub force_command_prompt: bool,
}

impl Default for FloatingPromptState {
    fn default() -> Self {
        Self {
            input: String::new(),
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            cursor_char_idx: 0,
            selection_anchor: None,
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
            force_command_prompt: false,
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

    pub(crate) fn record_undo(&mut self) {
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

    pub(crate) fn sync_from_lines(&mut self) {
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

    /// Read-only view of the multi-line prompt buffer (`P1.3`).
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Active 0-indexed cursor row (`0..lines.len()`) (`P1.3`).
    pub fn cursor_row(&self) -> usize {
        self.cursor_row
    }

    /// Active 0-indexed cursor column within `lines()[cursor_row()]` (`P1.3`).
    pub fn cursor_col(&self) -> usize {
        self.cursor_col
    }

    /// Global character offset across the flattened `text()` buffer (`P1.3`).
    pub fn cursor_char_idx(&self) -> usize {
        self.cursor_char_idx
    }

    /// Vertical line scroll offset (`P1.3`).
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Horizontal character scroll offset (`P1.3`).
    pub fn h_scroll_offset(&self) -> usize {
        self.h_scroll_offset
    }

    /// Active history recall index, if any (`P1.3`).
    pub fn history_index(&self) -> Option<usize> {
        self.history_index
    }

    pub fn insert_char(&mut self, ch: char) {
        if ch == '\n' {
            self.insert_newline();
            return;
        }
        self.record_undo();
        let _ = self.remove_selected_span_raw();
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
        let _ = self.remove_selected_span_raw();
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
        let _ = self.remove_selected_span_raw();
        let byte_idx = Self::char_to_byte(&self.lines[self.cursor_row], self.cursor_col);
        let tail = self.lines[self.cursor_row].split_off(byte_idx);
        self.cursor_row += 1;
        self.lines.insert(self.cursor_row, tail);
        self.cursor_col = 0;
        self.sync_from_lines();
    }

    pub fn backspace(&mut self) {
        if self.delete_selection().is_some() {
            return;
        }
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
        if self.delete_selection().is_some() {
            return;
        }
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
        self.selection_anchor = None;
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
        self.selection_anchor = None;
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
        self.selection_anchor = None;
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
        self.selection_anchor = None;
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
        self.selection_anchor = None;
        self.cursor_col = 0;
        self.sync_from_lines();
    }

    pub fn move_cursor_end(&mut self) {
        self.completion.clear();
        self.selection_anchor = None;
        self.cursor_col = self.lines[self.cursor_row].chars().count();
        self.sync_from_lines();
    }

    pub fn move_word_left(&mut self) {
        self.completion.clear();
        self.selection_anchor = None;
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
        self.selection_anchor = None;
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

    /// `Ctrl+K`: kills active selection (or from cursor to end of current line) and stores in `yank_buffer` + OS clipboard.
    pub fn kill_to_end_of_line(&mut self) {
        if let Some(killed) = self.delete_selection() {
            if !killed.is_empty() {
                self.yank_buffer = killed.clone();
                crate::clipboard::set_system_clipboard(&killed);
            }
            return;
        }
        self.record_undo();
        let byte_idx = Self::char_to_byte(&self.lines[self.cursor_row], self.cursor_col);
        let killed = self.lines[self.cursor_row].split_off(byte_idx);
        if !killed.is_empty() {
            self.yank_buffer = killed.clone();
            crate::clipboard::set_system_clipboard(&killed);
        }
        self.sync_from_lines();
    }

    /// `Ctrl+W`: deletes active selection (or the word preceding the cursor) and stores it in `yank_buffer` + OS clipboard.
    pub fn kill_prev_word(&mut self) {
        if let Some(killed) = self.delete_selection() {
            if !killed.is_empty() {
                self.yank_buffer = killed.clone();
                crate::clipboard::set_system_clipboard(&killed);
            }
            return;
        }
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
        crate::clipboard::set_system_clipboard(&self.yank_buffer);
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
        crate::clipboard::set_system_clipboard(&self.yank_buffer);
        self.lines[self.cursor_row].replace_range(start_byte..end_byte, "");
        self.sync_from_lines();
    }

    /// `Ctrl+Y`: yanks the last killed/copied text at the cursor.
    pub fn yank(&mut self) {
        let text = if !self.yank_buffer.is_empty() {
            self.yank_buffer.clone()
        } else {
            crate::clipboard::get_system_clipboard().unwrap_or_default()
        };
        self.insert_str(&text);
    }

    pub fn undo(&mut self) -> bool {
        self.completion.clear();
        self.selection_anchor = None;
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
        self.selection_anchor = None;
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
        self.selection_anchor = None;
        if !self.input.is_empty() {
            self.record_undo();
            self.yank_buffer = self.input.clone();
            crate::clipboard::set_system_clipboard(&self.yank_buffer);
        }
        self.lines = vec![String::new()];
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.sync_from_lines();
    }

    pub fn take_submission(&mut self) -> String {
        self.completion.clear();
        self.selection_anchor = None;
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
            (self.lines.len() as u16 + 3 + running_extra).clamp(self.height.clamp(4, 12), 12)
        } else {
            (self.lines.len() as u16 + 2 + running_extra).clamp(self.height.min(12), 12)
        };
        content_rows.clamp(3, canvas_height.max(3)).min(canvas_height)
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

    pub(crate) fn char_to_byte(s: &str, char_idx: usize) -> usize {
        s.char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(s.len())
    }
}

/// Active mouse drag state for moving or resizing a pane's `FloatingCommandPrompt` (`PANE-03`),
/// or dragging a character/rectangular selection on the `OutputCanvas` or inside the prompt (`VAL-04`).
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
    /// Dragging a character-stream selection (`LeftDrag`) on the `OutputCanvas`.
    CanvasCharSelect,
    /// Dragging a rectangular block selection (`Alt+LeftDrag`) on the `OutputCanvas` (`VAL-04`).
    CanvasRectSelect,
    /// Dragging a character selection (`LeftDrag`) inside the `FloatingCommandPrompt` editor.
    PromptTextSelect,
}
