use std::path::{Path, PathBuf};

use crate::pane::history_store::HistoryStore;
use crate::pane::{CompositePane, FloatingPromptState, PaneId, PromptMouseDrag};
use crate::pty::PtySession;
use crate::pty::line_discipline::LineDisciplineState;
use crate::vt::snapshot::GridSnapshot;
use crate::vt::{CanvasHint, CanvasSearchMatch, VtParser, encode_osc52_clipboard};

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

    /// Extracts the selected text from `grid` according to `selection_kind` (`VAL-04`, `ARCH-R03`).
    pub fn extract_selected_text(&self, grid: &GridSnapshot) -> String {
        let (rows, cols) = (grid.rows, grid.cols);
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
                let include =
                    if self.anchor.is_some() && self.selection_kind != CopySelectionKind::None {
                        self.is_cell_selected(r, c)
                    } else {
                        r == self.cursor_row
                    };
                if include {
                    let ch = grid
                        .cell(r, c)
                        .map(|cell| cell.grapheme.as_str())
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

impl CompositePane {
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
        let (r, c) = self.vt.cursor_position();
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
        let snap = self.vt.snapshot();
        let text = self.copy_mode.extract_selected_text(&snap);
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

    pub fn id(&self) -> PaneId {
        self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    pub fn vt(&self) -> &VtParser {
        &self.vt
    }

    pub fn vt_mut(&mut self) -> &mut VtParser {
        &mut self.vt
    }

    pub fn has_pty(&self) -> bool {
        self.pty.is_some()
    }

    pub fn pty(&self) -> Option<&PtySession> {
        self.pty.as_ref()
    }

    pub fn prompt(&self) -> &FloatingPromptState {
        &self.prompt
    }

    pub fn prompt_mut(&mut self) -> &mut FloatingPromptState {
        &mut self.prompt
    }

    pub fn history_store(&self) -> &HistoryStore {
        &self.history_store
    }

    pub fn history_store_mut(&mut self) -> &mut HistoryStore {
        &mut self.history_store
    }

    pub fn export_history_to_temp_file(
        &mut self,
        custom_path: Option<&Path>,
    ) -> std::io::Result<PathBuf> {
        self.history_store
            .export_to_temp_file(&self.prompt.history, custom_path)
    }

    pub fn line_discipline(&self) -> &LineDisciplineState {
        &self.line_discipline
    }

    pub fn line_discipline_mut(&mut self) -> &mut LineDisciplineState {
        &mut self.line_discipline
    }

    pub fn is_password_prompt(&self) -> bool {
        self.line_discipline.is_password_prompt()
    }

    pub fn has_foreground_subcommand(&self) -> bool {
        self.line_discipline.has_foreground_subcommand()
    }

    pub fn mouse_drag(&self) -> Option<PromptMouseDrag> {
        self.mouse_drag
    }

    pub fn set_mouse_drag(&mut self, drag: Option<PromptMouseDrag>) {
        self.mouse_drag = drag;
    }

    pub fn take_mouse_drag(&mut self) -> Option<PromptMouseDrag> {
        self.mouse_drag.take()
    }

    pub fn canvas_search(&self) -> &CanvasSearchState {
        &self.canvas_search
    }

    pub fn copy_mode(&self) -> &CopyModeState {
        &self.copy_mode
    }

    pub fn copy_mode_mut(&mut self) -> &mut CopyModeState {
        &mut self.copy_mode
    }

    pub fn hints_mode(&self) -> &HintsModeState {
        &self.hints_mode
    }

    pub fn hints_mode_mut(&mut self) -> &mut HintsModeState {
        &mut self.hints_mode
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn remote_target(&self) -> Option<&str> {
        self.remote_target.as_deref()
    }

    pub fn set_remote_target(&mut self, target: Option<String>) {
        self.remote_target = target;
    }

    pub fn pending_unsafe_paste(&self) -> Option<&str> {
        self.pending_unsafe_paste.as_deref()
    }

    pub fn set_pending_unsafe_paste(&mut self, paste: Option<String>) {
        self.pending_unsafe_paste = paste;
    }

    pub fn take_pending_unsafe_paste(&mut self) -> Option<String> {
        self.pending_unsafe_paste.take()
    }

    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    pub fn set_status_message(&mut self, msg: Option<String>) {
        self.status_message = msg;
    }

    pub fn last_osc52_sequence(&self) -> Option<&str> {
        self.last_osc52_sequence.as_deref()
    }

    pub fn set_last_osc52_sequence(&mut self, seq: Option<String>) {
        self.last_osc52_sequence = seq;
    }

    pub fn take_last_osc52_sequence(&mut self) -> Option<String> {
        self.last_osc52_sequence.take()
    }

    pub fn take_all_outbound_osc52(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(seq) = self.last_osc52_sequence.take() {
            out.push(seq);
        }
        out.extend(self.vt.take_outbound_osc52());
        out
    }

    pub fn flush_query_responses(&mut self) {
        if let Some(pty) = &self.pty {
            for resp in self.vt.take_query_responses() {
                let _ = pty.write_all(&resp);
            }
        }
    }
}
