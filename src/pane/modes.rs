use crate::pane:: CompositePane;
use crate::vt::snapshot::GridSnapshot;
use crate::vt::{CanvasHint, CanvasSearchMatch, encode_osc52_clipboard};

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

/// Indicates whether Copy Mode was initiated by keyboard navigation or a mouse drag gesture (`COPY-01`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CopyOrigin {
    #[default]
    Keyboard,
    Mouse,
}

/// View of an active or flashed selection in absolute coordinates (`COPY-02`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionView {
    pub start_abs: (usize, u16),
    pub end_abs: (usize, u16),
    pub kind: CopySelectionKind,
}

impl SelectionView {
    pub fn contains(&self, abs_row: usize, col: u16) -> bool {
        let ((sr, sc), (er, ec)) = if self.start_abs <= self.end_abs {
            (self.start_abs, self.end_abs)
        } else {
            (self.end_abs, self.start_abs)
        };
        match self.kind {
            CopySelectionKind::None => false,
            CopySelectionKind::Line => abs_row >= sr && abs_row <= er,
            CopySelectionKind::Block => {
                let min_c = sc.min(ec);
                let max_c = sc.max(ec);
                abs_row >= sr && abs_row <= er && col >= min_c && col <= max_c
            }
            CopySelectionKind::Char => {
                if abs_row < sr || abs_row > er {
                    false
                } else if sr == er {
                    col >= sc && col <= ec
                } else if abs_row == sr {
                    col >= sc
                } else if abs_row == er {
                    col <= ec
                } else {
                    true
                }
            }
        }
    }
}

/// Keyboard-driven Vi Copy Mode & rectangular block selection state for the `OutputCanvas` (`VAL-04`).
#[derive(Clone, Debug, Default)]
pub struct CopyModeState {
    pub active: bool,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub anchor: Option<(u16, u16)>,
    pub anchor_abs: Option<(usize, u16)>,
    pub viewport_top_abs: usize,
    pub selection_kind: CopySelectionKind,
    /// When `true`, selection was anchored via `v`/`V`/`b`/`r`/`Space` so plain arrow/`hjkl` keys extend it.
    pub visual_lock: bool,
    /// When `true`, plain mouse drag (`LeftDrag`) and `Shift+Arrows` default to rectangular (`Block`) selection.
    pub rect_mode: bool,
    /// Origin of this Copy Mode session (`Keyboard` vs `Mouse`, `COPY-01`).
    pub origin: CopyOrigin,
    /// State when `g` is pressed once in Vi mode waiting for a second `g` (`COPY-02`).
    pub pending_g: bool,
    /// Active interactive scrollback search state (`COPY-02`).
    pub search_state: Option<crate::pane::copy_search::CopySearchState>,
    /// Visual flash highlighting of recently copied cells until toast expires (`COPY-02`).
    pub flash_region: Option<SelectionView>,
}

impl CopyModeState {
    /// Returns `true` if a visual selection (`Char`, `Line`, or `Block`) is currently anchored.
    pub fn has_selection(&self) -> bool {
        self.anchor.is_some() && self.selection_kind != CopySelectionKind::None
    }

    /// Returns `(rows, cols)` of the active selection bounding box when `has_selection()` is true.
    pub fn selection_dimensions(&self) -> Option<(u16, u16)> {
        if !self.has_selection() {
            return None;
        }
        let (ar, ac) = self.anchor?;
        let h = ar.abs_diff(self.cursor_row) + 1;
        let w = ac.abs_diff(self.cursor_col) + 1;
        Some((h, w))
    }

    /// Begins a visual selection anchored at `(row, col)` with known viewport scroll offset (`COPY-02`).
    pub fn begin_selection(
        &mut self,
        row: u16,
        col: u16,
        kind: CopySelectionKind,
        top_abs: usize,
    ) {
        self.anchor = Some((row, col));
        self.anchor_abs = Some((top_abs + usize::from(row), col));
        self.cursor_row = row;
        self.cursor_col = col;
        self.viewport_top_abs = top_abs;
        self.selection_kind = kind;
        self.visual_lock = true;
    }

    /// Returns the active selection bounded by `anchor_abs` and current cursor position (`COPY-02`).
    pub fn current_selection_view(&self) -> Option<SelectionView> {
        if !self.has_selection() {
            return None;
        }
        let anchor = self.anchor_abs.unwrap_or_else(|| {
            let (ar, ac) = self.anchor.unwrap();
            (self.viewport_top_abs + usize::from(ar), ac)
        });
        let cursor = (self.viewport_top_abs + usize::from(self.cursor_row), self.cursor_col);
        Some(SelectionView {
            start_abs: anchor,
            end_abs: cursor,
            kind: self.selection_kind,
        })
    }

    /// Clears any active selection and exits Copy Mode (while preserving persistent `rect_mode`).
    pub fn clear_selection(&mut self) {
        self.active = false;
        self.anchor = None;
        self.anchor_abs = None;
        self.selection_kind = CopySelectionKind::None;
        self.visual_lock = false;
        self.origin = CopyOrigin::Keyboard;
        self.pending_g = false;
        self.search_state = None;
        self.flash_region = None;
    }

    /// Returns `true` if cell `(row, col)` lies inside the current selection (`Char`, `Line`, or `Block`).
    pub fn is_cell_selected(&self, row: u16, col: u16) -> bool {
        let abs_r = self.viewport_top_abs + usize::from(row);
        if let Some(ref view) = self.current_selection_view()
            && view.contains(abs_r, col)
        {
            return true;
        }
        if let Some(ref flash) = self.flash_region
            && flash.contains(abs_r, col)
        {
            return true;
        }
        false
    }

    /// Appends the text of the selected cell `(r, c)` to `line`.
    ///
    /// A wide glyph (CJK, emoji) occupies a lead cell plus an empty continuation tail
    /// (`width == 0`); the tail carries no text and must never become a stray space, otherwise
    /// `世界` is copied as `世 界`. When a selection edge lands on the tail while the lead cell is
    /// outside it (rectangular block starting mid-glyph), the whole glyph is emitted once so no
    /// character is lost.
    fn push_selected_cell(
        line: &mut String,
        grid: &GridSnapshot,
        r: u16,
        c: u16,
        is_selected: &impl Fn(u16) -> bool,
    ) {
        let Some(cell) = grid.cell(r, c) else {
            line.push(' ');
            return;
        };
        if cell.is_wide_continuation() {
            if c > 0
                && !is_selected(c - 1)
                && let Some(lead) = grid.cell(r, c - 1)
            {
                line.push_str(&lead.grapheme);
            }
            return;
        }
        if cell.grapheme.is_empty() {
            line.push(' ');
        } else {
            line.push_str(&cell.grapheme);
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

        let has_selection = self.anchor.is_some() && self.selection_kind != CopySelectionKind::None;
        let mut lines_out = Vec::new();
        for r in min_r..=max_r {
            let is_selected = |col: u16| {
                if has_selection {
                    self.is_cell_selected(r, col)
                } else {
                    r == self.cursor_row
                }
            };
            let mut line = String::new();
            for c in 0..cols {
                if is_selected(c) {
                    Self::push_selected_cell(&mut line, grid, r, c, &is_selected);
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
    /// Replaces the prompt's in-memory history with the newest persisted commands of the
    /// pane's history session and restores persisted cut-and-paste yank history when enabled.
    pub fn restore_persisted_history(&mut self) {
        self.prompt.history = self.history_store.load_recent_commands();
        self.prompt.history_index = None;
        self.prompt.history_popup_selected = 0;
        let clips = self
            .history_store
            .load_recent_clipboard(crate::pane::prompt_selection::MAX_YANK_HISTORY);
        if !clips.is_empty() {
            if self.prompt.yank_buffer.is_empty() {
                self.prompt.yank_buffer = clips[0].clone();
            }
            for item in clips.into_iter().rev() {
                self.prompt.push_yank_entry(&item);
            }
        }
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

    /// Enters keyboard-driven Vi Copy Mode on the `OutputCanvas` (`F8` / `Alt+C` / `:copy`, `VAL-04`).
    pub fn enter_copy_mode(&mut self) {
        if self.copy_mode.rect_mode {
            self.enter_rect_copy_mode();
            return;
        }
        let (r, c) = self.vt.cursor_position();
        self.copy_mode.active = true;
        self.copy_mode.cursor_row = r;
        self.copy_mode.cursor_col = c;
        self.copy_mode.anchor = None;
        self.copy_mode.selection_kind = CopySelectionKind::None;
        self.copy_mode.visual_lock = false;
        self.copy_mode.origin = CopyOrigin::Keyboard;
        self.status_message = Some(
            "CANVAS SELECT: Arrows/hjkl move | r/b/Ctrl+V=Rect | v/Space=Char | V=Line | Enter/y copy | x to prompt | Esc exit"
                .to_string(),
        );
    }

    /// Enters Rectangular Block Selection & Copy Mode on the `OutputCanvas` (`Alt+R` / `:rect` / `:copy rect`).
    pub fn enter_rect_copy_mode(&mut self) {
        let (r, c) = if self.copy_mode.active {
            (self.copy_mode.cursor_row, self.copy_mode.cursor_col)
        } else {
            self.vt.cursor_position()
        };
        self.copy_mode.active = true;
        self.copy_mode.cursor_row = r;
        self.copy_mode.cursor_col = c;
        self.copy_mode.anchor = Some((r, c));
        self.copy_mode.selection_kind = CopySelectionKind::Block;
        self.copy_mode.visual_lock = true;
        self.copy_mode.rect_mode = true;
        self.status_message = Some(
            "RECTANGULAR SELECT [1×1]: Arrows/hjkl/Mouse resize box | Enter/y/Ctrl+C copy | x to prompt | v=Char V=Line | Esc exit"
                .to_string(),
        );
    }

    /// Enables or disables persistent rectangular selection mode (`rect_mode`) for mouse drag and `Shift+Arrows`.
    pub fn set_rect_select_mode(&mut self, enabled: bool) {
        self.copy_mode.rect_mode = enabled;
        if enabled {
            let (r, c) = if self.copy_mode.active {
                (self.copy_mode.cursor_row, self.copy_mode.cursor_col)
            } else {
                self.vt.cursor_position()
            };
            self.copy_mode.active = true;
            self.copy_mode.cursor_row = r;
            self.copy_mode.cursor_col = c;
            if self.copy_mode.anchor.is_none() {
                self.copy_mode.anchor = Some((r, c));
            }
            self.copy_mode.selection_kind = CopySelectionKind::Block;
            self.copy_mode.visual_lock = true;
        } else {
            self.copy_mode.active = false;
            self.copy_mode.anchor = None;
            self.copy_mode.selection_kind = CopySelectionKind::None;
            self.copy_mode.visual_lock = false;
        }
        self.status_message = Some(format!(
            "Rectangular selection mode: {} (Mouse drag & Arrows/Shift+Arrows select {})",
            if enabled { "ON [RECT]" } else { "OFF [CHAR]" },
            if enabled {
                "2D rectangular blocks"
            } else {
                "character streams"
            }
        ));
    }

    /// Toggles persistent rectangular selection mode (`rect_mode`).
    pub fn toggle_rect_select_mode(&mut self) -> bool {
        let currently_active_rect = self.copy_mode.rect_mode && self.copy_mode.active;
        let next = !currently_active_rect;
        self.set_rect_select_mode(next);
        next
    }

    /// Programmatically highlights a 2D rectangular region `(r1, c1)..=(r2, c2)` on the `OutputCanvas`
    /// with `CopySelectionKind::Block` and optionally copies it to the clipboard & `OSC 52`.
    pub fn select_canvas_rect(
        &mut self,
        r1: u16,
        c1: u16,
        r2: u16,
        c2: u16,
        auto_copy: bool,
    ) -> Option<String> {
        let (rows, cols) = self.vt.size();
        let max_r = rows.saturating_sub(1);
        let max_c = cols.saturating_sub(1);
        let sr = r1.min(max_r);
        let sc = c1.min(max_c);
        let er = r2.min(max_r);
        let ec = c2.min(max_c);
        self.prompt.clear_selection();
        self.copy_mode.active = true;
        self.copy_mode.anchor = Some((sr, sc));
        self.copy_mode.cursor_row = er;
        self.copy_mode.cursor_col = ec;
        self.copy_mode.selection_kind = CopySelectionKind::Block;
        self.copy_mode.visual_lock = true;
        if auto_copy {
            self.yank_mouse_canvas_selection()
        } else {
            let (h, w) = self.copy_mode.selection_dimensions().unwrap_or((1, 1));
            let text = self.copy_mode.extract_selected_text(&self.vt.snapshot());
            self.status_message = Some(format!(
                "RECT HIGHLIGHT [{h}×{w}]: ({sr},{sc})..=({er},{ec}) — Enter/y copy | x to prompt | Esc clear"
            ));
            Some(text)
        }
    }

    /// Executes `:rect ...`, `:select ...`, or `:copy <subcmd>` canvas selection commands.
    pub fn execute_select_command(&mut self, raw_subcmd: &str) {
        let trimmed = raw_subcmd.trim();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let first_lower = parts
            .first()
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();

        // Check for coordinate tuple: `:rect <r1> <c1> <r2> <c2> [copy|highlight]`
        // or `:select rect|highlight|copy <r1> <c1> <r2> <c2> [copy|highlight]`
        let coord_start = if matches!(
            first_lower.as_str(),
            "rect" | "block" | "box" | "highlight" | "copy" | "yank"
        ) && parts.len() >= 5
        {
            Some((1usize, first_lower != "highlight"))
        } else if parts.len() >= 4 && parts[0].parse::<u16>().is_ok() {
            let auto_copy = !parts
                .get(4)
                .is_some_and(|m| m.eq_ignore_ascii_case("highlight"));
            Some((0usize, auto_copy))
        } else {
            None
        };

        if let Some((idx, default_copy)) = coord_start
            && let (Ok(r1), Ok(c1), Ok(r2), Ok(c2)) = (
                parts[idx].parse::<u16>(),
                parts[idx + 1].parse::<u16>(),
                parts[idx + 2].parse::<u16>(),
                parts[idx + 3].parse::<u16>(),
            )
        {
            let auto_copy = match parts.get(idx + 4).map(|s| s.to_ascii_lowercase()).as_deref() {
                Some("highlight" | "hl" | "nocopy") => false,
                Some("copy" | "yank") => true,
                _ => default_copy,
            };
            let (zr1, zc1, zr2, zc2) = if r1 >= 1 && c1 >= 1 && r2 >= 1 && c2 >= 1 {
                (r1 - 1, c1 - 1, r2 - 1, c2 - 1)
            } else {
                (r1, c1, r2, c2)
            };
            let _ = self.select_canvas_rect(zr1, zc1, zr2, zc2, auto_copy);
            return;
        }

        match first_lower.as_str() {
            "word" => {
                let top_abs = self.vt.viewport_top_abs();
                self.enter_copy_mode();
                let r = self.copy_mode.cursor_row;
                let c = self.copy_mode.cursor_col;
                let text = self.vt.abs_row_text(top_abs + usize::from(r)).unwrap_or_default();
                let (sc, ec) = crate::pane::copy_motion::word_bounds_at(&text, c);
                self.copy_mode.begin_selection(r, sc, CopySelectionKind::Char, top_abs);
                self.copy_mode.cursor_col = ec;
            }
            "line" => {
                let top_abs = self.vt.viewport_top_abs();
                self.enter_copy_mode();
                let r = self.copy_mode.cursor_row;
                self.copy_mode.begin_selection(r, 0, CopySelectionKind::Line, top_abs);
            }
            "search" | "find" => {
                let query = parts[1..].join(" ");
                if !query.is_empty() {
                    self.enter_copy_mode();
                    let _ = crate::input::copy_mode_keys::execute_search_jump(
                        self,
                        &query,
                        crate::pane::copy_search::SearchDir::Forward,
                    );
                }
            }
            "top" => {
                self.enter_copy_mode();
                self.vt.set_scrollback_offset(usize::MAX);
                self.copy_mode.cursor_row = 0;
                self.copy_mode.cursor_col = 0;
            }
            "bottom" => {
                self.enter_copy_mode();
                self.vt.reset_scrollback();
                let (rows, _) = self.vt.size();
                self.copy_mode.cursor_row = rows.saturating_sub(1);
            }
            "" | "rect" | "block" | "box" | "rectangular" => {
                if parts.len() == 1
                    && matches!(
                        parts.first().copied(),
                        Some("toggle")
                    )
                {
                    self.toggle_rect_select_mode();
                } else if let Some(arg) = parts.get(1) {
                    match arg.to_ascii_lowercase().as_str() {
                        "on" | "true" | "enable" => self.set_rect_select_mode(true),
                        "off" | "false" | "disable" => self.set_rect_select_mode(false),
                        "toggle" | "mode" => {
                            self.toggle_rect_select_mode();
                        }
                        "copy" | "yank" => {
                            let _ = self.yank_copy_selection();
                        }
                        "prompt" | "cut" | "insert" => {
                            let _ = self.cut_copy_selection_to_prompt();
                        }
                        "clear" | "exit" | "stop" | "close" | "quit" => {
                            self.copy_mode.rect_mode = false;
                            self.exit_copy_mode();
                            self.clear_canvas_search();
                        }
                        _ => self.enter_rect_copy_mode(),
                    }
                } else if self.copy_mode.active
                    && self.copy_mode.selection_kind == CopySelectionKind::Block
                    && trimmed.is_empty()
                {
                    self.exit_copy_mode();
                } else {
                    self.enter_rect_copy_mode();
                }
            }
            "mode" | "rect-mode" => match parts.get(1).map(|s| s.to_ascii_lowercase()).as_deref() {
                Some("on" | "true" | "rect" | "block" | "box") => self.set_rect_select_mode(true),
                Some("off" | "false" | "char" | "stream") => self.set_rect_select_mode(false),
                Some("line" | "lines") => {
                    self.enter_copy_mode();
                    let cur = (self.copy_mode.cursor_row, self.copy_mode.cursor_col);
                    self.copy_mode.anchor = Some(cur);
                    self.copy_mode.selection_kind = CopySelectionKind::Line;
                    self.copy_mode.visual_lock = true;
                    self.status_message = Some("COPY MODE: VISUAL LINE".to_string());
                }
                _ => {
                    self.toggle_rect_select_mode();
                }
            },
            "toggle" => {
                self.toggle_rect_select_mode();
            }
            "on" | "enable" => self.set_rect_select_mode(true),
            "off" | "disable" | "clear" | "none" | "exit" | "stop" | "close" | "quit" => {
                self.copy_mode.rect_mode = false;
                self.copy_mode.clear_selection();
                self.clear_canvas_search();
                self.status_message = Some("Cleared canvas selection".to_string());
            }
            "char" | "stream" => {
                self.copy_mode.rect_mode = false;
                self.enter_copy_mode();
                let cur = (self.copy_mode.cursor_row, self.copy_mode.cursor_col);
                self.copy_mode.anchor = Some(cur);
                self.copy_mode.selection_kind = CopySelectionKind::Char;
                self.copy_mode.visual_lock = true;
                self.status_message = Some(
                    "COPY MODE: VISUAL CHAR (Arrows/hjkl select, Enter/y=Copy, x=To Prompt)"
                        .to_string(),
                );
            }
            "lines" => {
                self.enter_copy_mode();
                let cur = (self.copy_mode.cursor_row, self.copy_mode.cursor_col);
                self.copy_mode.anchor = Some(cur);
                self.copy_mode.selection_kind = CopySelectionKind::Line;
                self.copy_mode.visual_lock = true;
                self.status_message = Some("COPY MODE: VISUAL LINE".to_string());
            }
            "copy" | "yank" => {
                if self.copy_mode.active || self.copy_mode.has_selection() {
                    let _ = self.yank_copy_selection();
                } else {
                    self.enter_rect_copy_mode();
                }
            }
            "prompt" | "cut" | "insert" | "transfer" => {
                let _ = self.cut_copy_selection_to_prompt();
            }
            _ => {
                self.status_message = Some(
                    "Usage: :rect [on|off|toggle|copy|prompt|clear|<r1> <c1> <r2> <c2> [highlight]] | :select [rect|char|line|mode|copy|clear]"
                        .to_string(),
                );
            }
        }
    }

    /// Exits Vi Copy Mode (`VAL-04`).
    pub fn exit_copy_mode(&mut self) {
        self.copy_mode.clear_selection();
        self.status_message = Some(if self.is_prompt_visible() {
            "Returned focus to Command Prompt".to_string()
        } else {
            "Returned focus to Terminal".to_string()
        });
    }

    /// Records `text` into the pane's yank buffer, yank history ring, session `clipboard.log`
    /// (when enabled), OS system clipboard, and `OSC 52` sequence.
    pub fn record_yank(&mut self, text: &str) {
        self.record_yank_with_source(
            crate::clipboard::ClipboardSource::CopyModeYank,
            text,
        );
    }

    /// Records `text` with an explicit [`crate::clipboard::ClipboardSource`] (`CLIP-02`).
    pub fn record_yank_with_source(
        &mut self,
        source: crate::clipboard::ClipboardSource,
        text: &str,
    ) {
        if text.is_empty() {
            return;
        }
        self.prompt.push_yank_entry(text);
        let _ = self.history_store.record_clipboard_entry(text);
        crate::clipboard::set_system_clipboard(text);
        self.last_osc52_sequence = Some(encode_osc52_clipboard(text));
        self.record_clipboard_event(source, text.to_string());
    }

    /// Extracts the text for the currently active copy mode selection using scrollback-absolute rows (`COPY-02`).
    pub fn extract_copy_selection_text(&mut self) -> String {
        self.copy_mode.viewport_top_abs = self.vt.viewport_top_abs();
        let Some(view) = self.copy_mode.current_selection_view() else {
            return String::new();
        };
        let ((sr, sc), (er, ec)) = if view.start_abs <= view.end_abs {
            (view.start_abs, view.end_abs)
        } else {
            (view.end_abs, view.start_abs)
        };
        let mut out = Vec::new();
        for r in sr..=er {
            let row_text = self.vt.abs_row_text(r).unwrap_or_default();
            let row_chars: Vec<char> = row_text.chars().collect();
            match view.kind {
                CopySelectionKind::None => {}
                CopySelectionKind::Line => {
                    let s: String = row_chars.iter().collect();
                    out.push(s.trim_end().to_string());
                }
                CopySelectionKind::Block => {
                    let min_c = sc.min(ec) as usize;
                    let max_c = sc.max(ec) as usize;
                    let s: String = row_chars
                        .iter()
                        .skip(min_c)
                        .take(max_c.saturating_sub(min_c) + 1)
                        .collect();
                    out.push(s.trim_end().to_string());
                }
                CopySelectionKind::Char => {
                    if sr == er {
                        let min_c = sc.min(ec) as usize;
                        let max_c = sc.max(ec) as usize;
                        let s: String = row_chars
                            .iter()
                            .skip(min_c)
                            .take(max_c.saturating_sub(min_c) + 1)
                            .collect();
                        out.push(s.trim_end().to_string());
                    } else if r == sr {
                        let s: String = row_chars.iter().skip(sc as usize).collect();
                        out.push(s.trim_end().to_string());
                    } else if r == er {
                        let s: String = row_chars.iter().take(ec as usize + 1).collect();
                        out.push(s.trim_end().to_string());
                    } else {
                        let s: String = row_chars.iter().collect();
                        out.push(s.trim_end().to_string());
                    }
                }
            }
        }
        out.join("\n")
    }

    /// Yanks the active `CopyModeState` selection (or current line) into the prompt yank buffer,
    /// yank history ring, OS system clipboard, and `OSC 52` (`VAL-04`).
    pub fn yank_copy_selection(&mut self) -> Option<String> {
        let was_block = self.copy_mode.selection_kind == CopySelectionKind::Block;
        let dims = self.copy_mode.selection_dimensions();
        self.copy_mode.viewport_top_abs = self.vt.viewport_top_abs();
        let flash = self.copy_mode.current_selection_view();
        let mut text = self.extract_copy_selection_text();
        if text.is_empty() {
            let snap = self.vt.snapshot();
            text = self.copy_mode.extract_selected_text(&snap);
        }
        self.copy_mode.flash_region = flash;
        self.copy_mode.active = false;
        self.copy_mode.anchor = None;
        self.copy_mode.anchor_abs = None;
        self.copy_mode.selection_kind = CopySelectionKind::None;
        self.copy_mode.visual_lock = false;
        if text.is_empty() {
            // Nothing copied: no flash (it would never be cleared without a toast TTL).
            self.copy_mode.flash_region = None;
            self.status_message = Some("Copy Mode: empty selection".to_string());
            return None;
        }
        self.record_yank_with_source(
            if was_block {
                crate::clipboard::ClipboardSource::RectBlock
            } else {
                crate::clipboard::ClipboardSource::CopyModeYank
            },
            &text,
        );
        if was_block && let Some((h, w)) = dims {
            self.set_status_toast(
                format!(
                    "Yanked rectangular block [{h}×{w}] ({} chars) to clipboard",
                    text.chars().count()
                ),
                3000,
            );
        } else {
            self.set_status_toast(
                format!("Copied {} chars -> clipboard (OSC52 sent)", text.chars().count()),
                3000,
            );
        }
        Some(text)
    }

    /// Stages a mouse-dragged canvas selection visually without overwriting `yank_buffer`,
    /// the OS clipboard, or `OSC 52` until the user explicitly copies (`Enter`/`y`/`Ctrl+C`).
    pub fn stage_mouse_canvas_selection(&mut self) -> Option<String> {
        let snap = self.vt.snapshot();
        let is_block = self.copy_mode.selection_kind == CopySelectionKind::Block;
        let dims = self.copy_mode.selection_dimensions();
        let text = self.copy_mode.extract_selected_text(&snap);
        if text.is_empty() {
            self.copy_mode.anchor = None;
            self.copy_mode.selection_kind = CopySelectionKind::None;
            self.copy_mode.visual_lock = false;
            if self.copy_mode.origin == crate::pane::CopyOrigin::Mouse {
                // A mouse click/drag over blank cells must not leave a stray copy-mode cursor.
                self.copy_mode.clear_selection();
            }
            return None;
        }
        if is_block && let Some((h, w)) = dims {
            self.status_message = Some(format!(
                "RECT SELECT [{h}×{w}]: Selected {} chars (Enter/y/Ctrl+C copy | x to prompt | Esc clear)",
                text.chars().count()
            ));
        } else {
            self.status_message = Some(format!(
                "Selected {} chars (Enter/y/Ctrl+C copy | x to prompt | Esc clear)",
                text.chars().count()
            ));
        }
        Some(text)
    }

    /// Yanks a mouse-dragged canvas selection into the prompt yank buffer, OS system clipboard,
    /// and `OSC 52` while keeping the visual highlight and canvas cursor active.
    pub fn yank_mouse_canvas_selection(&mut self) -> Option<String> {
        let snap = self.vt.snapshot();
        let is_block = self.copy_mode.selection_kind == CopySelectionKind::Block;
        let dims = self.copy_mode.selection_dimensions();
        let text = self.copy_mode.extract_selected_text(&snap);
        if text.is_empty() {
            self.copy_mode.anchor = None;
            self.copy_mode.selection_kind = CopySelectionKind::None;
            self.copy_mode.visual_lock = false;
            return None;
        }
        self.record_yank_with_source(
            if is_block {
                crate::clipboard::ClipboardSource::RectBlock
            } else {
                crate::clipboard::ClipboardSource::CopyModeYank
            },
            &text,
        );
        if is_block && let Some((h, w)) = dims {
            self.set_status_toast(
                format!(
                    "RECT SELECT [{h}×{w}]: Copied {} chars to clipboard (Shift+Arrows refine | Enter/y copy | x to prompt | Esc clear)",
                    text.chars().count()
                ),
                3000,
            );
        } else {
            self.set_status_toast(
                format!(
                    "Copied {} chars to clipboard (Shift+Arrows refine | x to prompt | Tab/Esc -> Prompt)",
                    text.chars().count()
                ),
                3000,
            );
        }
        Some(text)
    }

    /// Yanks the active `CopyModeState` selection to the clipboard AND inserts it into the
    /// `FloatingCommandPrompt` at the cursor (or pastes into the raw PTY when in Raw mode) (`x` / `Ctrl+X` in Copy Mode).
    pub fn cut_copy_selection_to_prompt(&mut self) -> Option<String> {
        let text = self.yank_copy_selection()?;
        if !self.read_only {
            self.prompt.insert_str(&text);
            if !self.is_prompt_visible() {
                let _ = crate::input::handle_paste_event(self, &text);
                self.status_message = Some(format!(
                    "Transferred {} chars from canvas to terminal",
                    text.chars().count()
                ));
            } else {
                self.status_message = Some(format!(
                    "Transferred {} chars from canvas to command prompt",
                    text.chars().count()
                ));
            }
        }
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
        self.record_yank(&hint.text);
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
