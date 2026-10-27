//! Visual Text Selection, Keyboard Shift-Select & Mouse Drag-Select for [`FloatingPromptState`].

use crate::clipboard::set_system_clipboard;
use crate::pane::prompt_editor::FloatingPromptState;

impl FloatingPromptState {
    /// Returns the raw selection anchor `(row, col)`, if any.
    pub fn selection_anchor(&self) -> Option<(usize, usize)> {
        self.selection_anchor
    }

    /// Clears any active visual selection highlight in the prompt editor.
    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    /// Returns `true` if a non-empty character span is currently selected in the prompt editor.
    pub fn has_selection(&self) -> bool {
        self.selection_range().is_some()
    }

    /// Returns the ordered `((start_row, start_col), (end_row, end_col))` character range
    /// when a non-empty selection is active (`start < end`).
    pub fn selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
        let (ar, ac) = self.selection_anchor?;
        let (cr, cc) = (self.cursor_row, self.cursor_col);
        if (ar, ac) == (cr, cc) {
            return None;
        }
        if (ar, ac) < (cr, cc) {
            Some(((ar, ac), (cr, cc)))
        } else {
            Some(((cr, cc), (ar, ac)))
        }
    }

    /// Returns `true` if the character at `(row, col)` (0-indexed line and char index) lies
    /// within the active visual selection `[start, end)`.
    pub fn is_char_selected(&self, row: usize, col: usize) -> bool {
        let Some(((sr, sc), (er, ec))) = self.selection_range() else {
            return false;
        };
        if row < sr || row > er {
            false
        } else if sr == er {
            col >= sc && col < ec
        } else if row == sr {
            col >= sc
        } else if row == er {
            col < ec
        } else {
            true
        }
    }

    /// Extracts the currently selected text across single or multiple lines.
    pub fn selected_text(&self) -> Option<String> {
        let ((sr, sc), (er, ec)) = self.selection_range()?;
        if sr >= self.lines.len() {
            return None;
        }
        let er = er.min(self.lines.len() - 1);
        if sr == er {
            let line = &self.lines[sr];
            let b0 = Self::char_to_byte(line, sc);
            let b1 = Self::char_to_byte(line, ec);
            return Some(line[b0..b1].to_string());
        }
        let mut out = String::new();
        for r in sr..=er {
            let line = &self.lines[r];
            if r == sr {
                let b0 = Self::char_to_byte(line, sc);
                out.push_str(&line[b0..]);
                out.push('\n');
            } else if r == er {
                let b1 = Self::char_to_byte(line, ec);
                out.push_str(&line[..b1]);
            } else {
                out.push_str(line);
                out.push('\n');
            }
        }
        Some(out)
    }

    /// Internal helper: removes the selected character span without recording a separate undo step
    /// (used when `insert_char` / `insert_str` / `delete_selection` already recorded undo).
    pub(crate) fn remove_selected_span_raw(&mut self) -> Option<String> {
        let text = self.selected_text()?;
        let ((sr, sc), (er, ec)) = self.selection_range()?;
        let er = er.min(self.lines.len() - 1);
        if sr == er {
            let b0 = Self::char_to_byte(&self.lines[sr], sc);
            let b1 = Self::char_to_byte(&self.lines[sr], ec);
            self.lines[sr].replace_range(b0..b1, "");
        } else {
            let b0 = Self::char_to_byte(&self.lines[sr], sc);
            let b1 = Self::char_to_byte(&self.lines[er], ec);
            let tail = self.lines[er][b1..].to_string();
            self.lines[sr].truncate(b0);
            self.lines[sr].push_str(&tail);
            self.lines.drain((sr + 1)..=er);
        }
        self.cursor_row = sr;
        self.cursor_col = sc;
        self.selection_anchor = None;
        self.sync_from_lines();
        Some(text)
    }

    /// Deletes the active selection (recording undo) and returns the removed text.
    pub fn delete_selection(&mut self) -> Option<String> {
        if !self.has_selection() {
            return None;
        }
        self.record_undo();
        self.remove_selected_span_raw()
    }

    /// Copies the active selection to `yank_buffer` and the OS system clipboard without deleting it.
    pub fn copy_selection(&mut self) -> Option<String> {
        let text = self.selected_text()?;
        if !text.is_empty() {
            self.yank_buffer = text.clone();
            set_system_clipboard(&text);
        }
        Some(text)
    }

    /// Cuts the active selection (or the entire current line if no selection is active) to
    /// `yank_buffer` and the OS system clipboard.
    pub fn cut_selection_or_line(&mut self) -> Option<String> {
        if let Some(deleted) = self.delete_selection() {
            if !deleted.is_empty() {
                self.yank_buffer = deleted.clone();
                set_system_clipboard(&deleted);
            }
            return Some(deleted);
        }
        if self.input.is_empty() {
            return None;
        }
        let line_text = self.lines[self.cursor_row].clone();
        self.record_undo();
        if self.lines.len() > 1 {
            self.lines.remove(self.cursor_row);
            if self.cursor_row >= self.lines.len() {
                self.cursor_row = self.lines.len() - 1;
            }
            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_row].chars().count());
        } else {
            self.lines[0].clear();
            self.cursor_col = 0;
        }
        self.selection_anchor = None;
        self.sync_from_lines();
        if !line_text.is_empty() {
            self.yank_buffer = line_text.clone();
            set_system_clipboard(&line_text);
        }
        Some(line_text)
    }

    /// Selects the entire multi-line prompt buffer (`Ctrl+A` / `Cmd+A` / `Alt+A`).
    pub fn select_all(&mut self) {
        self.completion.clear();
        if self.input.is_empty() {
            self.selection_anchor = None;
            return;
        }
        self.selection_anchor = Some((0, 0));
        self.cursor_row = self.lines.len().saturating_sub(1);
        self.cursor_col = self.lines[self.cursor_row].chars().count();
        self.sync_from_lines();
    }

    fn ensure_selection_anchor(&mut self) {
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some((self.cursor_row, self.cursor_col));
        }
    }

    /// Extends visual selection one character left (`Shift+Left`).
    pub fn select_left(&mut self) {
        self.ensure_selection_anchor();
        let anchor = self.selection_anchor;
        self.move_cursor_left();
        self.selection_anchor = anchor;
    }

    /// Extends visual selection one character right (`Shift+Right`).
    pub fn select_right(&mut self) {
        self.ensure_selection_anchor();
        let anchor = self.selection_anchor;
        self.move_cursor_right();
        self.selection_anchor = anchor;
    }

    /// Extends visual selection one line up (`Shift+Up`).
    pub fn select_up(&mut self) {
        self.ensure_selection_anchor();
        let anchor = self.selection_anchor;
        if !self.move_cursor_up() {
            self.cursor_col = 0;
            self.sync_from_lines();
        }
        self.selection_anchor = anchor;
    }

    /// Extends visual selection one line down (`Shift+Down`).
    pub fn select_down(&mut self) {
        self.ensure_selection_anchor();
        let anchor = self.selection_anchor;
        if !self.move_cursor_down() {
            self.cursor_col = self.lines[self.cursor_row].chars().count();
            self.sync_from_lines();
        }
        self.selection_anchor = anchor;
    }

    /// Extends visual selection to start of line (`Shift+Home`).
    pub fn select_home(&mut self) {
        self.ensure_selection_anchor();
        let anchor = self.selection_anchor;
        self.move_cursor_home();
        self.selection_anchor = anchor;
    }

    /// Extends visual selection to end of line (`Shift+End`).
    pub fn select_end(&mut self) {
        self.ensure_selection_anchor();
        let anchor = self.selection_anchor;
        self.move_cursor_end();
        self.selection_anchor = anchor;
    }

    /// Extends visual selection one word left (`Shift+Alt+Left` / `Shift+Ctrl+Left`).
    pub fn select_word_left(&mut self) {
        self.ensure_selection_anchor();
        let anchor = self.selection_anchor;
        self.move_word_left();
        self.selection_anchor = anchor;
    }

    /// Extends visual selection one word right (`Shift+Alt+Right` / `Shift+Ctrl+Right`).
    pub fn select_word_right(&mut self) {
        self.ensure_selection_anchor();
        let anchor = self.selection_anchor;
        self.move_word_right();
        self.selection_anchor = anchor;
    }

    /// Positions the cursor at `(row, col)` from a mouse click or drag inside the prompt editor.
    /// When `extend_selection` is `true` (mouse drag or `Shift+Click`), sets/preserves `selection_anchor`.
    pub fn set_cursor_at(&mut self, row: usize, col: usize, extend_selection: bool) {
        self.completion.clear();
        if extend_selection {
            self.ensure_selection_anchor();
        } else {
            self.selection_anchor = None;
        }
        let target_row = row.min(self.lines.len().saturating_sub(1));
        let max_col = self.lines[target_row].chars().count();
        self.cursor_row = target_row;
        self.cursor_col = col.min(max_col);
        self.sync_from_lines();
    }
}

#[cfg(test)]
mod tests {
    use crate::pane::prompt_editor::FloatingPromptState;

    #[test]
    fn shift_selection_and_cut_copy_replace_work_in_prompt() {
        let mut p = FloatingPromptState::default();
        p.insert_str("echo hello world");
        assert!(!p.has_selection());

        // Select "world" using select_word_left
        p.select_word_left();
        assert!(p.has_selection());
        assert_eq!(p.selected_text().as_deref(), Some("world"));
        assert!(p.is_char_selected(0, 11));
        assert!(!p.is_char_selected(0, 10));

        // Copy selection
        assert_eq!(p.copy_selection().as_deref(), Some("world"));
        assert_eq!(p.yank_buffer, "world");

        // Typing replaces active selection
        p.insert_str("bilterm");
        assert!(!p.has_selection());
        assert_eq!(p.text(), "echo hello bilterm");

        // Select all and cut
        p.select_all();
        assert_eq!(p.selected_text().as_deref(), Some("echo hello bilterm"));
        assert_eq!(
            p.cut_selection_or_line().as_deref(),
            Some("echo hello bilterm")
        );
        assert_eq!(p.text(), "");
        assert_eq!(p.yank_buffer, "echo hello bilterm");
    }
}
