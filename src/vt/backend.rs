//! Terminal Emulation Backend Port (`trait TerminalBackend`, `MockTerminalBackend`, and conformance suite) (`ARCH-R03`).
//!
//! Isolates the underlying VT parser (`vt100-ctt`, `libghostty-vt`, or future `alacritty_terminal`)
//! behind a unified trait and [`GridSnapshot`]/[`CellSnapshot`] value types so no `vt100_ctt` type
//! ever crosses out of `src/vt/`.

use crate::vt::VtParser;
use crate::vt::snapshot::{
    CellColor, CellSnapshot, CellView, CursorShape, CursorSnapshot, GridSnapshot, RowSnapshot,
    UnderlineKind,
};

/// Unified port trait implemented by all virtual terminal backends (`VtParser`, `MockTerminalBackend`, `GhosttyVtEngine`) (`ARCH-R03`).
pub trait TerminalBackend {
    /// Feeds a chunk of raw PTY output bytes into the terminal state machine.
    fn process(&mut self, bytes: &[u8]);

    /// Resizes the terminal grid dimensions (`rows`, `cols`).
    fn set_size(&mut self, rows: u16, cols: u16);

    /// Returns the current `(rows, cols)` viewport dimensions.
    fn size(&self) -> (u16, u16);

    /// Returns the 0-indexed `(row, col)` cursor position within the active screen.
    fn cursor_position(&self) -> (u16, u16);

    /// Returns `true` when the terminal cursor is visible (`DECTCEM` `?25h`).
    fn is_cursor_visible(&self) -> bool {
        true
    }

    /// Materializes an immutable point-in-time [`GridSnapshot`] of the active viewport.
    fn snapshot(&self) -> GridSnapshot;

    /// Returns a zero-allocation borrowed [`CellView`] at `(row, col)` when supported by the backend.
    fn cell_view(&self, row: u16, col: u16) -> Option<CellView<'_>>;

    /// Returns a point-in-time [`CellSnapshot`] at `(row, col)` without exposing internal parser types.
    fn cell_snapshot(&self, row: u16, col: u16) -> Option<CellSnapshot> {
        self.snapshot().cell(row, col).cloned()
    }

    /// Returns the trimmed plain-text contents of a single visible `row`.
    fn row_text(&self, row: u16) -> String {
        self.snapshot().row_text(row)
    }

    /// Returns the formatted plain-text contents of the visible screen.
    fn contents(&self) -> String {
        self.snapshot().contents()
    }

    /// Returns `true` when `DECSET 47/1047/1049` alternate screen buffer is active.
    fn is_alternate_screen(&self) -> bool;

    /// Returns `true` when `DECSET 2004` bracketed paste mode is active.
    fn is_bracketed_paste(&self) -> bool;

    /// Returns `true` when `DECCKM` (`?1`) application cursor keys mode is active.
    fn is_application_cursor(&self) -> bool;

    /// Returns `true` when any VT mouse tracking mode (`?1000`/`?1002`/`?1003`/`?1006`) is active.
    fn is_mouse_sgr_or_tracking(&self) -> bool;

    /// Returns `true` when `DECSET 1004` focus in/out reporting is active.
    fn is_focus_reporting(&self) -> bool;

    /// Returns `true` when `DECSET 2026` synchronized output mode is active.
    fn is_synchronized_output(&self) -> bool;

    /// Returns `true` when `DECSET 1007` alternate scroll mode is active.
    fn is_mouse_alternate_scroll(&self) -> bool;

    /// Returns `true` when `DECSET 2027` Unicode grapheme cluster mode is active.
    fn is_grapheme_cluster_2027(&self) -> bool;

    /// Returns the active Kitty keyboard protocol flags on the top of the current screen's stack.
    fn kitty_keyboard_flags(&self) -> u8;

    /// Returns the current scrollback viewport offset (`0` = live bottom).
    fn scrollback_offset(&self) -> usize;

    /// Scrolls the viewport up into scrollback history by `lines`.
    fn scroll_up(&mut self, lines: usize);

    /// Scrolls the viewport down toward the live bottom by `lines`.
    fn scroll_down(&mut self, lines: usize);

    /// Resets the scrollback viewport to the live bottom (`0`).
    fn reset_scrollback(&mut self);

    /// Drains pending terminal query responses (`DSR`, `DA1`, `DECRPM`, `XTGETTCAP`, `OSC 10/11/12`).
    fn take_query_responses(&mut self) -> Vec<Vec<u8>>;

    /// Drains pending outbound `OSC 52` clipboard sequences emitted by the child application.
    fn take_outbound_osc52(&mut self) -> Vec<String>;

    /// Returns the DEC VT100/VT220 line-size attribute (`DECSWL`, `DECDWL`, `DECDHL`) for `row` (`FONT-01`).
    fn line_size_attr(&self, _row: u16) -> crate::ui::typography::LineSizeAttr {
        crate::ui::typography::LineSizeAttr::Single
    }
}

impl VtParser {
    /// Returns the 0-indexed `(row, col)` cursor position on the active screen (`ARCH-R03`).
    pub fn cursor_position(&self) -> (u16, u16) {
        self.parser.screen().cursor_position()
    }

    /// Returns `true` when the VT cursor is visible (`DECTCEM` `?25h`) (`ARCH-R03`).
    pub fn is_cursor_visible(&self) -> bool {
        !self.parser.screen().hide_cursor()
    }

    /// Returns a zero-allocation borrowed [`CellView`] for `(row, col)` on the active screen (`ARCH-R03`).
    pub fn cell_view(&self, row: u16, col: u16) -> Option<CellView<'_>> {
        let cell = self.parser.screen().cell(row, col)?;
        let width = if cell.is_wide_continuation() {
            0
        } else if cell.is_wide() {
            2
        } else {
            1
        };
        let underline = if cell.underline() {
            if self.active_underline_kind.is_underlined() {
                self.active_underline_kind
            } else if self.last_styled_underline_kind.is_underlined() {
                self.last_styled_underline_kind
            } else {
                UnderlineKind::Single
            }
        } else {
            UnderlineKind::None
        };
        Some(CellView {
            grapheme: cell.contents(),
            width,
            fg: super::vt_color_to_cell_color(cell.fgcolor()),
            bg: super::vt_color_to_cell_color(cell.bgcolor()),
            bold: cell.bold(),
            italic: cell.italic(),
            underline,
            inverse: cell.inverse(),
        })
    }

    /// Returns a renderer-agnostic [`CellSnapshot`] for `(row, col)` on the active screen (`ARCH-R03`).
    pub fn cell_snapshot(&self, row: u16, col: u16) -> Option<CellSnapshot> {
        let view = self.cell_view(row, col)?;
        let grapheme = if view.width == 0 {
            String::new()
        } else if view.grapheme.is_empty() {
            " ".to_string()
        } else {
            view.grapheme.to_string()
        };
        let underline_color = if view.underline.is_underlined() {
            self.active_underline_color.or(self.last_styled_underline_color)
        } else {
            None
        };
        Some(CellSnapshot {
            grapheme,
            width: view.width,
            fg: view.fg,
            bg: view.bg,
            underline_color,
            bold: view.bold,
            dim: false,
            italic: view.italic,
            underline: view.underline,
            overline: self.active_overline,
            inverse: view.inverse,
            strikethrough: false,
        })
    }

    /// Returns the trimmed plain-text contents of `row` without exposing `vt100_ctt::Screen` (`ARCH-R03`).
    pub fn row_text(&self, row: u16) -> String {
        let (_, cols) = self.size();
        let screen = self.parser.screen();
        let mut line = String::with_capacity(usize::from(cols));
        for c in 0..cols {
            if let Some(cell) = screen.cell(row, c) {
                if cell.is_wide_continuation() {
                    continue;
                }
                let contents = cell.contents();
                line.push_str(if contents.is_empty() { " " } else { contents });
            } else {
                line.push(' ');
            }
        }
        line.trim_end().to_string()
    }

    /// Returns the formatted plain-text contents of the screen (`ARCH-R03`).
    pub fn contents(&self) -> String {
        self.parser.screen().contents()
    }
}

impl TerminalBackend for VtParser {
    fn process(&mut self, bytes: &[u8]) { VtParser::process(self, bytes); }
    fn set_size(&mut self, rows: u16, cols: u16) { VtParser::set_size(self, rows, cols); }
    fn size(&self) -> (u16, u16) { VtParser::size(self) }
    fn cursor_position(&self) -> (u16, u16) { VtParser::cursor_position(self) }
    fn is_cursor_visible(&self) -> bool { VtParser::is_cursor_visible(self) }
    fn snapshot(&self) -> GridSnapshot { VtParser::snapshot(self) }
    fn cell_view(&self, row: u16, col: u16) -> Option<CellView<'_>> { VtParser::cell_view(self, row, col) }
    fn cell_snapshot(&self, row: u16, col: u16) -> Option<CellSnapshot> { VtParser::cell_snapshot(self, row, col) }
    fn row_text(&self, row: u16) -> String { VtParser::row_text(self, row) }
    fn contents(&self) -> String { VtParser::contents(self) }
    fn is_alternate_screen(&self) -> bool { VtParser::is_alternate_screen(self) }
    fn is_bracketed_paste(&self) -> bool { VtParser::is_bracketed_paste(self) }
    fn is_application_cursor(&self) -> bool { VtParser::is_application_cursor(self) }
    fn is_mouse_sgr_or_tracking(&self) -> bool { VtParser::is_mouse_sgr_or_tracking(self) }
    fn is_focus_reporting(&self) -> bool { VtParser::is_focus_reporting(self) }
    fn is_synchronized_output(&self) -> bool { VtParser::is_synchronized_output(self) }
    fn is_mouse_alternate_scroll(&self) -> bool { VtParser::is_mouse_alternate_scroll(self) }
    fn is_grapheme_cluster_2027(&self) -> bool { VtParser::is_grapheme_cluster_2027(self) }
    fn kitty_keyboard_flags(&self) -> u8 { VtParser::kitty_keyboard_flags(self) }
    fn scrollback_offset(&self) -> usize { VtParser::scrollback_offset(self) }
    fn scroll_up(&mut self, lines: usize) { VtParser::scroll_up(self, lines); }
    fn scroll_down(&mut self, lines: usize) { VtParser::scroll_down(self, lines); }
    fn reset_scrollback(&mut self) { VtParser::reset_scrollback(self); }
    fn take_query_responses(&mut self) -> Vec<Vec<u8>> { VtParser::take_query_responses(self) }
    fn take_outbound_osc52(&mut self) -> Vec<String> { VtParser::take_outbound_osc52(self) }
    fn line_size_attr(&self, row: u16) -> crate::ui::typography::LineSizeAttr { VtParser::line_size_attr(self, row) }
}

/// Lightweight, self-contained reference [`TerminalBackend`] with zero `vt100_ctt` dependency (`ARCH-R03`).
///
/// Used to verify backend-agnostic consumers and run the shared [`TerminalBackend`] conformance suite.
#[derive(Clone, Debug)]
pub struct MockTerminalBackend {
    rows: u16,
    cols: u16,
    scrollback_capacity: usize,
    primary_grid: Vec<Vec<CellSnapshot>>,
    alt_grid: Vec<Vec<CellSnapshot>>,
    scrollback_rows: Vec<Vec<CellSnapshot>>,
    primary_cursor: (u16, u16),
    alt_cursor: (u16, u16),
    alternate_screen: bool,
    bracketed_paste: bool,
    application_cursor: bool,
    mouse_tracking: bool,
    focus_reporting: bool,
    synchronized_output: bool,
    mouse_alternate_scroll: bool,
    grapheme_cluster_2027: bool,
    kitty_stack_main: Vec<u8>,
    kitty_stack_alt: Vec<u8>,
    scrollback_offset: usize,
    cur_style: CellSnapshot,
    pending_queries: Vec<Vec<u8>>,
    pending_osc52: Vec<String>,
    generation: u64,
}

impl MockTerminalBackend {
    pub fn new(rows: u16, cols: u16, scrollback_capacity: usize) -> Self {
        let r = rows.max(1);
        let c = cols.max(1);
        let blank_grid = (0..r)
            .map(|_| vec![CellSnapshot::default(); usize::from(c)])
            .collect::<Vec<_>>();
        Self {
            rows: r,
            cols: c,
            scrollback_capacity,
            primary_grid: blank_grid.clone(),
            alt_grid: blank_grid,
            scrollback_rows: Vec::new(),
            primary_cursor: (0, 0),
            alt_cursor: (0, 0),
            alternate_screen: false,
            bracketed_paste: false,
            application_cursor: false,
            mouse_tracking: false,
            focus_reporting: false,
            synchronized_output: false,
            mouse_alternate_scroll: true,
            grapheme_cluster_2027: false,
            kitty_stack_main: Vec::new(),
            kitty_stack_alt: Vec::new(),
            scrollback_offset: 0,
            cur_style: CellSnapshot::default(),
            pending_queries: Vec::new(),
            pending_osc52: Vec::new(),
            generation: 0,
        }
    }

    fn active_grid_mut(&mut self) -> &mut Vec<Vec<CellSnapshot>> {
        if self.alternate_screen {
            &mut self.alt_grid
        } else {
            &mut self.primary_grid
        }
    }

    fn active_cursor_mut(&mut self) -> &mut (u16, u16) {
        if self.alternate_screen {
            &mut self.alt_cursor
        } else {
            &mut self.primary_cursor
        }
    }

    fn linefeed(&mut self) {
        let rows = self.rows;
        let cols = self.cols;
        let (cur_r, _) = *self.active_cursor_mut();
        if cur_r + 1 < rows {
            self.active_cursor_mut().0 = cur_r + 1;
        } else if !self.alternate_screen {
            let evicted = self.primary_grid.remove(0);
            if self.scrollback_capacity > 0 {
                self.scrollback_rows.push(evicted);
                if self.scrollback_rows.len() > self.scrollback_capacity {
                    self.scrollback_rows.remove(0);
                }
            }
            self.primary_grid
                .push(vec![CellSnapshot::default(); usize::from(cols)]);
        } else {
            self.alt_grid.remove(0);
            self.alt_grid
                .push(vec![CellSnapshot::default(); usize::from(cols)]);
        }
    }

    fn put_char(&mut self, ch: char) {
        let rows = self.rows;
        let cols = self.cols;
        let (mut r, mut c) = *self.active_cursor_mut();
        if c >= cols {
            c = 0;
            self.active_cursor_mut().1 = 0;
            self.linefeed();
            r = self.active_cursor_mut().0;
        }
        if r < rows && c < cols {
            let mut cell = self.cur_style.clone();
            cell.grapheme = ch.to_string();
            cell.width = 1;
            self.active_grid_mut()[usize::from(r)][usize::from(c)] = cell;
            self.active_cursor_mut().1 = (c + 1).min(cols);
        }
    }

    fn handle_csi(&mut self, params: &str, final_byte: u8) {
        match final_byte {
            b'h' | b'l' if params.starts_with('?') => {
                let enable = final_byte == b'h';
                for part in params[1..].split(';') {
                    match part {
                        "1" => self.application_cursor = enable,
                        "47" | "1047" | "1049" => {
                            if enable && !self.alternate_screen {
                                self.alternate_screen = true;
                                self.alt_cursor = (0, 0);
                                for row in &mut self.alt_grid {
                                    row.fill(CellSnapshot::default());
                                }
                            } else if !enable && self.alternate_screen {
                                self.alternate_screen = false;
                            }
                        }
                        "1000" | "1002" | "1003" | "1006" => self.mouse_tracking = enable,
                        "1004" => self.focus_reporting = enable,
                        "1007" => self.mouse_alternate_scroll = enable,
                        "2004" => self.bracketed_paste = enable,
                        "2026" => self.synchronized_output = enable,
                        "2027" => self.grapheme_cluster_2027 = enable,
                        _ => {}
                    }
                }
            }
            b'u' if params.starts_with('>') => {
                let flags = params[1..]
                    .split(';')
                    .next()
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(0);
                let stack = if self.alternate_screen {
                    &mut self.kitty_stack_alt
                } else {
                    &mut self.kitty_stack_main
                };
                stack.push(flags);
            }
            b'u' if params.starts_with('<') => {
                let stack = if self.alternate_screen {
                    &mut self.kitty_stack_alt
                } else {
                    &mut self.kitty_stack_main
                };
                stack.pop();
            }
            b'H' | b'f' => {
                let mut parts = params.split(';');
                let r = parts
                    .next()
                    .and_then(|s| s.parse::<u16>().ok())
                    .unwrap_or(1)
                    .saturating_sub(1)
                    .min(self.rows.saturating_sub(1));
                let c = parts
                    .next()
                    .and_then(|s| s.parse::<u16>().ok())
                    .unwrap_or(1)
                    .saturating_sub(1)
                    .min(self.cols.saturating_sub(1));
                *self.active_cursor_mut() = (r, c);
            }
            b'J' => {
                if params.is_empty() || params == "0" || params == "2" {
                    for row in self.active_grid_mut() {
                        row.fill(CellSnapshot::default());
                    }
                }
            }
            b'm' => {
                if params.is_empty() || params == "0" {
                    self.cur_style = CellSnapshot::default();
                    return;
                }
                let nums: Vec<u16> = params
                    .split(';')
                    .map(|s| s.parse::<u16>().unwrap_or(0))
                    .collect();
                let mut i = 0;
                while i < nums.len() {
                    match nums[i] {
                        0 => self.cur_style = CellSnapshot::default(),
                        1 => self.cur_style.bold = true,
                        3 => self.cur_style.italic = true,
                        4 => self.cur_style.underline = UnderlineKind::Single,
                        7 => self.cur_style.inverse = true,
                        38 if i + 4 < nums.len() && nums[i + 1] == 2 => {
                            self.cur_style.fg = CellColor::Rgb(
                                nums[i + 2] as u8,
                                nums[i + 3] as u8,
                                nums[i + 4] as u8,
                            );
                            i += 4;
                        }
                        38 if i + 2 < nums.len() && nums[i + 1] == 5 => {
                            self.cur_style.fg = CellColor::Indexed(nums[i + 2] as u8);
                            i += 2;
                        }
                        48 if i + 4 < nums.len() && nums[i + 1] == 2 => {
                            self.cur_style.bg = CellColor::Rgb(
                                nums[i + 2] as u8,
                                nums[i + 3] as u8,
                                nums[i + 4] as u8,
                            );
                            i += 4;
                        }
                        48 if i + 2 < nums.len() && nums[i + 1] == 5 => {
                            self.cur_style.bg = CellColor::Indexed(nums[i + 2] as u8);
                            i += 2;
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            b'n' if params == "6" => {
                let (r, c) = self.cursor_position();
                self.pending_queries
                    .push(format!("\x1b[{};{}R", r + 1, c + 1).into_bytes());
            }
            _ => {}
        }
    }
}

impl TerminalBackend for MockTerminalBackend {
    fn process(&mut self, bytes: &[u8]) {
        self.generation = self.generation.wrapping_add(1);
        let text = String::from_utf8_lossy(bytes);
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '\x1b' if i + 1 < chars.len() => match chars[i + 1] {
                    '[' => {
                        i += 2;
                        let mut params = String::new();
                        while i < chars.len() && !(0x40..=0x7E).contains(&(chars[i] as u32)) {
                            params.push(chars[i]);
                            i += 1;
                        }
                        if i < chars.len() {
                            self.handle_csi(&params, chars[i] as u8);
                            i += 1;
                        }
                    }
                    ']' => {
                        i += 2;
                        let mut osc = String::new();
                        while i < chars.len() {
                            if chars[i] == '\x07' {
                                i += 1;
                                break;
                            }
                            if chars[i] == '\x1b' && i + 1 < chars.len() && chars[i + 1] == '\\' {
                                i += 2;
                                break;
                            }
                            osc.push(chars[i]);
                            i += 1;
                        }
                        if osc.starts_with("52;") {
                            self.pending_osc52.push(format!("\x1b]{osc}\x07"));
                        }
                    }
                    _ => i += 2,
                },
                '\r' => {
                    self.active_cursor_mut().1 = 0;
                    i += 1;
                }
                '\n' => {
                    self.linefeed();
                    i += 1;
                }
                ch if !ch.is_control() => {
                    self.put_char(ch);
                    i += 1;
                }
                _ => i += 1,
            }
        }
    }

    fn set_size(&mut self, rows: u16, cols: u16) {
        let r = rows.max(1);
        let c = cols.max(1);
        self.rows = r;
        self.cols = c;
        self.primary_grid
            .resize(usize::from(r), vec![CellSnapshot::default(); usize::from(c)]);
        for row in &mut self.primary_grid {
            row.resize(usize::from(c), CellSnapshot::default());
        }
        self.alt_grid
            .resize(usize::from(r), vec![CellSnapshot::default(); usize::from(c)]);
        for row in &mut self.alt_grid {
            row.resize(usize::from(c), CellSnapshot::default());
        }
        self.primary_cursor.0 = self.primary_cursor.0.min(r.saturating_sub(1));
        self.primary_cursor.1 = self.primary_cursor.1.min(c.saturating_sub(1));
        self.alt_cursor.0 = self.alt_cursor.0.min(r.saturating_sub(1));
        self.alt_cursor.1 = self.alt_cursor.1.min(c.saturating_sub(1));
    }

    fn size(&self) -> (u16, u16) {
        (self.rows, self.cols)
    }

    fn cursor_position(&self) -> (u16, u16) {
        let (r, c) = if self.alternate_screen {
            self.alt_cursor
        } else {
            self.primary_cursor
        };
        (r.min(self.rows.saturating_sub(1)), c.min(self.cols.saturating_sub(1)))
    }

    fn snapshot(&self) -> GridSnapshot {
        let grid = if self.alternate_screen {
            &self.alt_grid
        } else {
            &self.primary_grid
        };
        let grid_rows = grid
            .iter()
            .map(|cells| RowSnapshot {
                cells: cells.clone(),
                dirty: true,
                wrapped: false,
                line_size: crate::ui::typography::LineSizeAttr::Single,
            })
            .collect();
        let (row, col) = self.cursor_position();
        GridSnapshot {
            rows: self.rows,
            cols: self.cols,
            grid_rows,
            cursor: CursorSnapshot {
                row,
                col,
                visible: true,
                blinking: false,
                shape: CursorShape::Block,
                at_wide_tail: false,
            },
            alternate_screen: self.alternate_screen,
            scrollback_offset: self.scrollback_offset,
            total_scrollback_rows: self.scrollback_rows.len(),
            generation: self.generation,
        }
    }

    fn cell_view(&self, row: u16, col: u16) -> Option<CellView<'_>> {
        let grid = if self.alternate_screen {
            &self.alt_grid
        } else {
            &self.primary_grid
        };
        grid.get(usize::from(row))
            .and_then(|r| r.get(usize::from(col)))
            .map(CellSnapshot::as_view)
    }

    fn is_alternate_screen(&self) -> bool {
        self.alternate_screen
    }

    fn is_bracketed_paste(&self) -> bool {
        self.bracketed_paste
    }

    fn is_application_cursor(&self) -> bool {
        self.application_cursor
    }

    fn is_mouse_sgr_or_tracking(&self) -> bool {
        self.mouse_tracking
    }

    fn is_focus_reporting(&self) -> bool {
        self.focus_reporting
    }

    fn is_synchronized_output(&self) -> bool {
        self.synchronized_output
    }

    fn is_mouse_alternate_scroll(&self) -> bool {
        self.mouse_alternate_scroll
    }

    fn is_grapheme_cluster_2027(&self) -> bool {
        self.grapheme_cluster_2027
    }

    fn kitty_keyboard_flags(&self) -> u8 {
        let stack = if self.alternate_screen {
            &self.kitty_stack_alt
        } else {
            &self.kitty_stack_main
        };
        stack.last().copied().unwrap_or(0)
    }

    fn scrollback_offset(&self) -> usize {
        self.scrollback_offset
    }

    fn scroll_up(&mut self, lines: usize) {
        let max_off = self.scrollback_rows.len().min(self.scrollback_capacity);
        self.scrollback_offset = self.scrollback_offset.saturating_add(lines).min(max_off);
    }

    fn scroll_down(&mut self, lines: usize) {
        self.scrollback_offset = self.scrollback_offset.saturating_sub(lines);
    }

    fn reset_scrollback(&mut self) {
        self.scrollback_offset = 0;
    }

    fn take_query_responses(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.pending_queries)
    }

    fn take_outbound_osc52(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_osc52)
    }
}

/// Shared conformance test suite verifying that any [`TerminalBackend`] implementation honors
/// Bilterm's grid, SGR, DECSET mode, Kitty keyboard stack, scrollback, and query contracts (`ARCH-R03`).
pub fn assert_terminal_backend_conformance<B, F>(make_backend: F)
where
    B: TerminalBackend,
    F: Fn(u16, u16, usize) -> B,
{
    let mut backend = make_backend(6, 20, 100);
    assert_eq!(backend.size(), (6, 20));
    assert_eq!(backend.cursor_position(), (0, 0));

    // 1. Printable text, SGR TrueColor/Indexed, and cursor movement
    backend.process(b"\x1b[1;3;4;7;38;2;10;20;30;48;5;196mHI\x1b[0m\r\nROW2");
    assert_eq!(backend.row_text(0), "HI");
    assert_eq!(backend.row_text(1), "ROW2");
    assert_eq!(backend.cursor_position(), (1, 4));
    let c0 = backend.cell_snapshot(0, 0).expect("cell (0,0)");
    assert_eq!(c0.grapheme, "H");
    assert_eq!(c0.fg, CellColor::Rgb(10, 20, 30));
    assert_eq!(c0.bg, CellColor::Indexed(196));
    assert!(c0.bold && c0.italic && c0.inverse && c0.underline.is_underlined());

    // 2. DECSET alternate screen (?1049) preserves primary buffer on exit
    assert!(!backend.is_alternate_screen());
    backend.process(b"\x1b[?1049h\x1b[HALT_VIEW");
    assert!(backend.is_alternate_screen());
    assert_eq!(backend.row_text(0), "ALT_VIEW");
    backend.process(b"\x1b[?1049l");
    assert!(!backend.is_alternate_screen());
    assert_eq!(backend.row_text(0), "HI");

    // 3. DECSET mode flags & Kitty keyboard stack
    backend.process(b"\x1b[?1h\x1b[?2004h\x1b[?1006h\x1b[?1004h\x1b[?2026h\x1b[?2027h\x1b[>5u");
    assert!(backend.is_application_cursor());
    assert!(backend.is_bracketed_paste());
    assert!(backend.is_mouse_sgr_or_tracking());
    assert!(backend.is_focus_reporting());
    assert!(backend.is_synchronized_output());
    assert!(backend.is_grapheme_cluster_2027());
    assert_eq!(backend.kitty_keyboard_flags(), 5);
    backend.process(b"\x1b[<u\x1b[?1l\x1b[?2004l\x1b[?1006l");
    assert_eq!(backend.kitty_keyboard_flags(), 0);
    assert!(!backend.is_application_cursor());
    assert!(!backend.is_bracketed_paste());
    assert!(!backend.is_mouse_sgr_or_tracking());

    // 4. DSR 6n cursor position report & OSC 52 clipboard
    backend.process(b"\x1b[3;7H\x1b[6n\x1b]52;c;aGVsbG8=\x07");
    assert_eq!(backend.cursor_position(), (2, 6));
    assert_eq!(backend.take_query_responses(), vec![b"\x1b[3;7R".to_vec()]);
    assert_eq!(backend.take_outbound_osc52().len(), 1);

    // 5. Scrollback navigation & resize
    for idx in 0..10 {
        backend.process(format!("LINE_{idx}\r\n").as_bytes());
    }
    backend.scroll_up(2);
    assert!(backend.scrollback_offset() > 0);
    backend.reset_scrollback();
    assert_eq!(backend.scrollback_offset(), 0);
    backend.set_size(10, 40);
    assert_eq!(backend.size(), (10, 40));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vt_parser_passes_terminal_backend_conformance() {
        assert_terminal_backend_conformance(VtParser::new);
    }

    #[test]
    fn mock_terminal_backend_passes_terminal_backend_conformance() {
        assert_terminal_backend_conformance(MockTerminalBackend::new);
    }
}
