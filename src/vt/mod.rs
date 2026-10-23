pub mod ghostty_backend;
pub mod scrollback;
pub mod snapshot;
pub mod worker;

use std::path::{Path, PathBuf};
use std::time::Instant;

use ratatui::style::{Color, Modifier, Style};

use crate::vt::snapshot::{
    CellColor, CellSnapshot, CursorShape, CursorSnapshot, GridSnapshot, RgbTriplet, RowSnapshot,
    UnderlineKind,
};

/// Semantic shell integration markers (`OSC 133;A/B/C/D`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Osc133Phase {
    /// `OSC 133;A` — Shell prompt start.
    PromptStart,
    /// `OSC 133;B` — Command input start (shell waiting at prompt).
    CommandStart,
    /// `OSC 133;C` — Command execution started (foreground command running).
    CommandExecuted,
    /// `OSC 133;D[;exit_code]` — Command finished with optional exit status.
    CommandFinished(Option<i32>),
}

/// Recorded command block boundary in scrollback/screen history (`PANE-05`, `VAL-06`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandBlockRecord {
    pub index: usize,
    pub command: String,
    pub absolute_row: usize,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
}

/// Explicit `OSC 8` hyperlink recorded from the terminal stream (`VAL-03`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Osc8Hyperlink {
    pub id: Option<String>,
    pub uri: String,
}

/// Detected clickable URL span on the visible `OutputCanvas` (`VAL-03`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedUrl {
    pub row: u16,
    pub start_col: u16,
    pub end_col: u16,
    pub url: String,
}

/// Match location found by `VtParser::search_canvas` across visible screen and scrollback (`VAL-02`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasSearchMatch {
    pub row: u16,
    pub start_col: u16,
    pub end_col: u16,
}

/// ConEmu / Ghostty `OSC 9;4;<state>;<progress>` terminal progress indicator state (`BEL-02`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Osc9Progress {
    /// `OSC 9;4;1;<pct>` — Normal progress percentage (`0..=100`).
    Normal(u8),
    /// `OSC 9;4;2;<pct>` — Error progress state (`0..=100`).
    Error(u8),
    /// `OSC 9;4;3` — Indeterminate / pulsing progress indicator.
    Indeterminate,
    /// `OSC 9;4;4;<pct>` — Paused progress percentage (`0..=100`).
    Paused(u8),
}

/// Classification of an actionable token detected on the `OutputCanvas` by QuickSelect 3270 Field Hints (`ECO-01`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasHintKind {
    Url,
    Path,
    IpAddr,
    GitSha,
}

/// Actionable token on the `OutputCanvas` with a short alphabetic jump label (`[a]..[z]`, `ECO-01`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasHint {
    pub label: String,
    pub row: u16,
    pub start_col: u16,
    pub end_col: u16,
    pub text: String,
    pub kind: CanvasHintKind,
}

/// Terminal graphics protocol family (`GFX-01`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphicProtocol {
    /// Kitty Graphics Protocol (`APC G`, `\x1b_G...\x1b\\`).
    Kitty,
    /// DEC Sixel Graphics (`DCS q`, `\x1bP...q...\x1b\\`).
    Sixel,
}

/// Decoded RGBA image placement anchored to a terminal cell region (`GFX-01`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalGraphicPlacement {
    pub id: u32,
    pub protocol: GraphicProtocol,
    pub row: u16,
    pub col: u16,
    pub cols: u16,
    pub rows: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
    pub rgba_bytes: Vec<u8>,
}

/// Desktop / OIA notification emitted via `OSC 9;<msg>` or `OSC 777;notify;<title>;<body>` (`v0.7.0`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalNotification {
    pub title: String,
    pub body: String,
}

/// Virtual terminal state machine wrapping `vt100-ctt` with explicit `DECSET 47/1047/1049`
/// alternate screen tracking, `DECSET 2004` bracketed paste tracking, `DECSET 1007` alternate scroll,
/// `DECRPM` mode reporting, Kitty keyboard protocol stacks, `XTGETTCAP`/`DECRQSS`, `OSC 4/10/11/12`
/// color queries, `OSC 8` hyperlinks, `OSC 9;4` progress bars, `OSC 52` clipboard passthrough, and `OSC 133` command blocks.
pub struct VtParser {
    parser: vt100_ctt::Parser,
    rows: u16,
    cols: u16,
    scrollback_capacity: usize,
    alt_screen_47_or_1047: bool,
    bracketed_paste_2004: bool,
    application_cursor_decckm: bool,
    mouse_tracking_mode: bool,
    mouse_alternate_scroll_1007: bool,
    focus_reporting_1004: bool,
    synchronized_output_2026: bool,
    grapheme_cluster_2027: bool,
    report_color_scheme_2031: bool,
    in_band_size_2048: bool,
    backarrow_key_67: bool,
    kitty_kbd_stack_main: Vec<u8>,
    kitty_kbd_stack_alt: Vec<u8>,
    theme_fg_rgb: (u8, u8, u8),
    theme_bg_rgb: (u8, u8, u8),
    theme_cursor_rgb: (u8, u8, u8),
    window_title: Option<String>,
    reported_cwd: Option<PathBuf>,
    osc133_phase: Option<Osc133Phase>,
    command_blocks: Vec<CommandBlockRecord>,
    last_command_started_at: Option<Instant>,
    last_exit_code: Option<i32>,
    last_duration_ms: Option<u64>,
    selected_block_index: Option<usize>,
    osc8_links: Vec<Osc8Hyperlink>,
    osc9_progress: Option<Osc9Progress>,
    graphic_placements: Vec<TerminalGraphicPlacement>,
    next_graphic_id: u32,
    notifications: Vec<TerminalNotification>,
    unread_bell: bool,
    recording_started_at: Option<Instant>,
    recording_path: Option<PathBuf>,
    recording_events: Vec<(f64, String)>,
    recent_escape_sequences: Vec<String>,
    pending_osc52: Vec<String>,
    pending_query_responses: Vec<Vec<u8>>,
    total_lines_scrolled: usize,
    total_bytes_processed: usize,
    generation: u64,
    active_underline_kind: UnderlineKind,
    active_underline_color: Option<RgbTriplet>,
    active_overline: bool,
    last_styled_underline_kind: UnderlineKind,
    last_styled_underline_color: Option<RgbTriplet>,
    primary_stream_log: Vec<u8>,
    /// `false` once `primary_stream_log` has been truncated or invalidated by a rebuild, after
    /// which column changes reflow via [`VtParser::rebuild`] instead of replaying the log.
    primary_log_complete: bool,
}

impl VtParser {
    pub fn new(rows: u16, cols: u16, scrollback_capacity: usize) -> Self {
        let clamped_rows = rows.max(1);
        let clamped_cols = cols.max(1);
        Self {
            parser: vt100_ctt::Parser::new(clamped_rows, clamped_cols, scrollback_capacity),
            rows: clamped_rows,
            cols: clamped_cols,
            scrollback_capacity,
            alt_screen_47_or_1047: false,
            bracketed_paste_2004: false,
            application_cursor_decckm: false,
            mouse_tracking_mode: false,
            mouse_alternate_scroll_1007: true,
            focus_reporting_1004: false,
            synchronized_output_2026: false,
            grapheme_cluster_2027: false,
            report_color_scheme_2031: false,
            in_band_size_2048: false,
            backarrow_key_67: false,
            kitty_kbd_stack_main: Vec::new(),
            kitty_kbd_stack_alt: Vec::new(),
            theme_fg_rgb: (0xFF, 0xFF, 0xFF),
            theme_bg_rgb: (0x00, 0x00, 0x84),
            theme_cursor_rgb: (0xFF, 0xFF, 0x55),
            window_title: None,
            reported_cwd: None,
            osc133_phase: None,
            command_blocks: Vec::new(),
            last_command_started_at: None,
            last_exit_code: None,
            last_duration_ms: None,
            selected_block_index: None,
            osc8_links: Vec::new(),
            osc9_progress: None,
            graphic_placements: Vec::new(),
            next_graphic_id: 1,
            notifications: Vec::new(),
            unread_bell: false,
            recording_started_at: None,
            recording_path: None,
            recording_events: Vec::new(),
            recent_escape_sequences: Vec::new(),
            pending_osc52: Vec::new(),
            pending_query_responses: Vec::new(),
            total_lines_scrolled: 0,
            total_bytes_processed: 0,
            generation: 0,
            active_underline_kind: UnderlineKind::None,
            active_underline_color: None,
            active_overline: false,
            last_styled_underline_kind: UnderlineKind::None,
            last_styled_underline_color: None,
            primary_stream_log: Vec::new(),
            primary_log_complete: true,
        }
    }

    /// Processes a chunk of raw PTY output bytes, updating the VT100 grid, `DECSET 47/1047/1049`,
    /// `DECSET 2004`, `OSC 52`, and `OSC 133` semantic prompt markers.
    ///
    /// When a raw binary noise stream (such as `tail -f /dev/random` or `cat /dev/urandom`) is
    /// detected while not in alternate screen mode, stray `0x1b` (`ESC`) and raw C0/C1 control
    /// bytes are sanitized into visible glyphs so random bytes cannot falsely trigger `DECSET`
    /// alternate screen mode or lock the VT100 scroll region (`DECSTBM`).
    pub fn process(&mut self, bytes: &[u8]) {
        self.generation = self.generation.wrapping_add(1);
        self.total_bytes_processed = self.total_bytes_processed.saturating_add(bytes.len());
        if let Some(t0) = self.recording_started_at {
            let elapsed = t0.elapsed().as_secs_f64();
            self.recording_events
                .push((elapsed, String::from_utf8_lossy(bytes).into_owned()));
        }
        if !self.is_alternate_screen() && is_binary_noise_chunk(bytes) {
            let sanitized = sanitize_binary_noise_chunk(bytes);
            let newline_count = sanitized.iter().filter(|&&b| b == b'\n').count();
            self.total_lines_scrolled = self.total_lines_scrolled.saturating_add(newline_count);
            self.append_primary_stream_log(&sanitized);
            self.parser.process(&sanitized);
            return;
        }
        let was_alt = self.is_alternate_screen();
        let newline_count = bytes.iter().filter(|&&b| b == b'\n').count();
        self.total_lines_scrolled = self.total_lines_scrolled.saturating_add(newline_count);
        let normalized = normalize_extended_sgr_for_vt100(bytes);
        self.parser.process(&normalized);
        self.scan_control_sequences(bytes);
        if !was_alt && !self.is_alternate_screen() {
            self.append_primary_stream_log(&normalized);
        }
    }

    fn append_primary_stream_log(&mut self, bytes: &[u8]) {
        const MAX_PRIMARY_LOG: usize = 65_536;
        self.primary_stream_log.extend_from_slice(bytes);
        if self.primary_stream_log.len() > MAX_PRIMARY_LOG {
            let excess = self.primary_stream_log.len() - (MAX_PRIMARY_LOG / 2);
            let cut = self.primary_stream_log[excess..]
                .iter()
                .position(|&b| b == b'\n')
                .map(|i| excess + i + 1)
                .unwrap_or(excess);
            self.primary_stream_log.drain(..cut);
            self.primary_log_complete = false;
        }
    }

    /// Total number of output bytes processed by this pane's VT100 parser.
    pub fn total_bytes_processed(&self) -> usize {
        self.total_bytes_processed
    }

    /// Total number of newline transitions observed in the output stream.
    pub fn total_lines_scrolled(&self) -> usize {
        self.total_lines_scrolled
    }

    /// Total output line count (lines scrolled + current active cursor row).
    pub fn output_line_count(&self) -> usize {
        let (cur_r, cur_c) = self.parser.screen().cursor_position();
        let active_line = if cur_r > 0 || cur_c > 0 || self.total_bytes_processed > 0 {
            usize::from(cur_r) + 1
        } else {
            0
        };
        self.total_lines_scrolled.max(active_line)
    }

    /// Resizes the virtual terminal screen dimensions, reflowing soft-wrapped lines on the primary
    /// screen when horizontal columns change (`FND-08`) and emitting in-band size reports when
    /// `DECSET ?2048` is active (`VAL-05`).
    pub fn set_size(&mut self, rows: u16, cols: u16) {
        let clamped_rows = rows.max(1);
        let clamped_cols = cols.max(1);
        if (self.rows, self.cols) != (clamped_rows, clamped_cols) {
            let cols_changed = self.cols != clamped_cols;
            self.rows = clamped_rows;
            self.cols = clamped_cols;
            self.generation = self.generation.wrapping_add(1);
            let reflow = cols_changed
                && clamped_rows >= 2
                && clamped_cols >= 4
                && !self.is_alternate_screen();
            if reflow && self.primary_log_complete && !self.primary_stream_log.is_empty() {
                let mut replayed =
                    vt100_ctt::Parser::new(clamped_rows, clamped_cols, self.scrollback_capacity);
                replayed.process(&self.primary_stream_log);
                self.parser = replayed;
            } else if reflow && !self.primary_log_complete {
                self.rebuild(clamped_rows, clamped_cols, self.scrollback_capacity);
            } else {
                self.parser
                    .screen_mut()
                    .set_size(clamped_rows.max(2), clamped_cols.max(2));
            }
            if self.in_band_size_2048 {
                self.pending_query_responses.push(
                    format!("\x1b[48;{clamped_rows};{clamped_cols};0;0t").into_bytes(),
                );
            }
        }
    }

    /// Maximum scrollback rows this parser retains.
    pub fn scrollback_capacity(&self) -> usize {
        self.scrollback_capacity
    }

    /// Rows currently held in scrollback history (clamped probe via `set_scrollback`).
    pub fn scrollback_used_rows(&mut self) -> usize {
        let screen = self.parser.screen_mut();
        let offset = screen.scrollback();
        screen.set_scrollback(usize::MAX);
        let used = screen.scrollback();
        screen.set_scrollback(offset);
        used
    }

    /// Estimated heap bytes held by scrollback history.
    pub fn estimated_scrollback_bytes(&mut self) -> u64 {
        self.scrollback_used_rows() as u64 * scrollback::row_bytes(self.cols)
    }

    /// Changes the scrollback capacity, rebuilding the parser and keeping the newest history.
    /// Returns `false` (no-op) while the alternate screen is active.
    pub fn set_scrollback_capacity(&mut self, capacity: usize) -> bool {
        if self.is_alternate_screen() {
            return false;
        }
        if capacity != self.scrollback_capacity {
            self.rebuild(self.rows, self.cols, capacity);
        }
        true
    }

    /// Drops all scrollback history, keeping the visible screen. Returns `false` on the alt screen.
    pub fn clear_scrollback(&mut self) -> bool {
        if self.is_alternate_screen() {
            return false;
        }
        let capacity = self.scrollback_capacity;
        self.rebuild_keeping(self.rows, self.cols, capacity, 0);
        true
    }

    /// Rebuilds the underlying parser at `rows`x`cols` with `capacity` scrollback rows, keeping
    /// the newest `min(used, capacity)` history rows. Soft-wrapped rows are rejoined into logical
    /// lines so a column change genuinely reflows history. The viewport snaps to the bottom.
    pub fn rebuild(&mut self, rows: u16, cols: u16, capacity: usize) {
        self.rebuild_keeping(rows, cols, capacity, usize::MAX);
    }

    fn rebuild_keeping(&mut self, rows: u16, cols: u16, capacity: usize, max_history: usize) {
        let rows = rows.max(1);
        let cols = cols.max(1);
        let used = self.scrollback_used_rows();
        let keep = used.min(capacity).min(max_history);
        let (old_rows, old_cols) = self.parser.screen().size();
        let same_size = (old_rows, old_cols) == (rows, cols);

        // Collect logical lines (bytes, cell width) from history, oldest kept row first.
        let mut lines: Vec<Vec<u8>> = Vec::new();
        let mut current: Vec<u8> = Vec::new();
        let mut current_width = 0usize;
        {
            let screen = self.parser.screen_mut();
            let mut j = used - keep;
            while j < used {
                let off = used - j;
                screen.set_scrollback(off);
                let take = usize::from(old_rows).min(off);
                for r in 0..take {
                    let r = r as u16;
                    let wrapped = screen.row_wrapped(r);
                    current_width += scrollback::serialize_row(screen, r, wrapped, &mut current);
                    if !wrapped {
                        lines.push(std::mem::take(&mut current));
                        current_width = 0;
                    }
                }
                j += take;
            }
            screen.set_scrollback(0);
        }

        let mut out: Vec<u8> = Vec::new();
        let screen = self.parser.screen();
        if same_size {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            for line in &lines {
                out.extend_from_slice(line);
                out.extend_from_slice(b"\x1b[0m\r\n");
            }
            if !lines.is_empty() {
                out.extend(std::iter::repeat_n(b'\n', usize::from(rows) - 1));
            }
            out.extend_from_slice(&screen.state_formatted());
        } else {
            // Reflow the screen rows up to the cursor as logical lines, too.
            let (cur_row, cur_col) = screen.cursor_position();
            let mut cursor_offset = 0usize;
            for r in 0..=cur_row.min(old_rows.saturating_sub(1)) {
                if r == cur_row {
                    cursor_offset = current_width + usize::from(cur_col);
                    current_width += scrollback::serialize_row(screen, r, false, &mut current);
                } else {
                    let wrapped = screen.row_wrapped(r);
                    current_width += scrollback::serialize_row(screen, r, wrapped, &mut current);
                    if !wrapped {
                        lines.push(std::mem::take(&mut current));
                        current_width = 0;
                    }
                }
            }
            for line in &lines {
                out.extend_from_slice(line);
                out.extend_from_slice(b"\x1b[0m\r\n");
            }
            out.extend_from_slice(&current);
            out.extend_from_slice(b"\x1b[0m");
            let cols_usize = usize::from(cols);
            if cursor_offset != current_width {
                // Cursor sits before/after the end of its logical line: move it there.
                let end_row = current_width.saturating_sub(1) / cols_usize;
                let target_row = cursor_offset / cols_usize;
                if end_row > target_row {
                    out.extend_from_slice(format!("\x1b[{}A", end_row - target_row).as_bytes());
                } else if target_row > end_row {
                    out.extend_from_slice(format!("\x1b[{}B", target_row - end_row).as_bytes());
                }
                out.extend_from_slice(
                    format!("\x1b[{}G", (cursor_offset % cols_usize) + 1).as_bytes(),
                );
            }
            out.extend_from_slice(&screen.input_mode_formatted());
            out.extend_from_slice(&screen.attributes_formatted());
        }

        let mut rebuilt = vt100_ctt::Parser::new(rows, cols, capacity);
        rebuilt.process(&out);
        self.parser = rebuilt;
        self.rows = rows;
        self.cols = cols;
        self.scrollback_capacity = capacity;
        self.generation = self.generation.wrapping_add(1);
        // A rebuilt parser no longer matches the byte log; keep exact replay only if nothing was logged.
        self.primary_log_complete &= self.primary_stream_log.is_empty();
        self.primary_stream_log.clear();
        self.selected_block_index = None;
        let retained = self.scrollback_used_rows();
        let oldest = self.total_lines_scrolled.saturating_sub(retained);
        let before = self.command_blocks.len();
        self.command_blocks.retain(|b| b.absolute_row >= oldest);
        if self.command_blocks.len() != before {
            for (i, b) in self.command_blocks.iter_mut().enumerate() {
                b.index = i;
            }
        }
    }

    /// Returns the most recently set extended SGR underline style (`SGR 4:0..4:5`, `FND-09`).
    pub fn active_underline_kind(&self) -> UnderlineKind {
        self.active_underline_kind
    }

    /// Returns the most recently set extended SGR underline RGB color (`SGR 58;2;r;g;b`, `FND-09`).
    pub fn active_underline_color(&self) -> Option<RgbTriplet> {
        self.active_underline_color
    }

    /// Returns `true` when `SGR 53` overline is currently active (`FND-09`).
    pub fn is_overline(&self) -> bool {
        self.active_overline
    }

    /// Materializes a renderer-agnostic point-in-time [`GridSnapshot`] of the current viewport (`ARCH-01`).
    pub fn snapshot(&self) -> GridSnapshot {
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        let r_max = rows.max(1);
        let c_max = cols.max(1);
        let mut grid_rows = Vec::with_capacity(usize::from(r_max));

        for r in 0..r_max {
            let mut row_cells = Vec::with_capacity(usize::from(c_max));
            for c in 0..c_max {
                if let Some(cell) = screen.cell(r, c) {
                    let width = if cell.is_wide_continuation() {
                        0
                    } else if cell.is_wide() {
                        2
                    } else {
                        1
                    };
                    let grapheme = if cell.is_wide_continuation() {
                        String::new()
                    } else if cell.contents().is_empty() {
                        " ".to_string()
                    } else {
                        cell.contents().to_string()
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
                    let ul_color = if cell.underline() {
                        self.active_underline_color.or(self.last_styled_underline_color)
                    } else {
                        None
                    };
                    row_cells.push(CellSnapshot {
                        grapheme,
                        width,
                        fg: vt_color_to_cell_color(cell.fgcolor()),
                        bg: vt_color_to_cell_color(cell.bgcolor()),
                        underline_color: ul_color,
                        bold: cell.bold(),
                        dim: false,
                        italic: cell.italic(),
                        underline,
                        overline: self.active_overline,
                        inverse: cell.inverse(),
                        strikethrough: false,
                    });
                } else {
                    row_cells.push(CellSnapshot::default());
                }
            }
            grid_rows.push(RowSnapshot {
                cells: row_cells,
                dirty: true,
                wrapped: false,
            });
        }

        let (cur_r, cur_c) = screen.cursor_position();
        GridSnapshot {
            rows: r_max,
            cols: c_max,
            grid_rows,
            cursor: CursorSnapshot {
                row: cur_r,
                col: cur_c,
                visible: !screen.hide_cursor(),
                blinking: false,
                shape: CursorShape::Block,
                at_wide_tail: false,
            },
            alternate_screen: self.is_alternate_screen(),
            scrollback_offset: screen.scrollback(),
            total_scrollback_rows: self.total_lines_scrolled,
            generation: self.generation,
        }
    }

    pub fn size(&self) -> (u16, u16) {
        (self.rows, self.cols)
    }

    pub fn screen(&self) -> &vt100_ctt::Screen {
        self.parser.screen()
    }

    /// Returns `true` when the child program is inside the alternate screen buffer
    /// (`DECSET 1049`, `DECSET 1047`, or `DECSET 47`).
    pub fn is_alternate_screen(&self) -> bool {
        self.parser.screen().alternate_screen() || self.alt_screen_47_or_1047
    }

    /// Returns `true` when the child program has enabled `DECSET 2004` bracketed paste mode (`\x1b[?2004h`).
    pub fn is_bracketed_paste(&self) -> bool {
        self.parser.screen().bracketed_paste() || self.bracketed_paste_2004
    }

    /// Returns `true` when `DECCKM` (`\x1b[?1h`) Application Cursor Keys mode is active.
    pub fn is_application_cursor(&self) -> bool {
        self.parser.screen().application_cursor() || self.application_cursor_decckm
    }

    /// Returns `true` when mouse tracking (`DECSET 1000`, `1002`, `1003`, or `1006`) is active.
    pub fn is_mouse_sgr_or_tracking(&self) -> bool {
        !matches!(
            self.parser.screen().mouse_protocol_mode(),
            vt100_ctt::MouseProtocolMode::None
        ) || self.mouse_tracking_mode
    }

    #[allow(non_snake_case)]
    pub fn is_mouse_SGR_or_tracking(&self) -> bool {
        self.is_mouse_sgr_or_tracking()
    }

    /// Returns `true` when `DECSET 1004` focus in/out reporting (`\x1b[?1004h`) is active.
    pub fn is_focus_reporting(&self) -> bool {
        self.focus_reporting_1004
    }

    /// Returns `true` when `DECSET 2026` synchronized output (`\x1b[?2026h`) is active.
    pub fn is_synchronized_output(&self) -> bool {
        self.synchronized_output_2026
    }

    /// Returns `true` when `DECSET 1007` Mouse Alternate Scroll (`\x1b[?1007h`, default `true`) is active (`FND-02`).
    pub fn is_mouse_alternate_scroll(&self) -> bool {
        self.mouse_alternate_scroll_1007
    }

    /// Returns `true` when `DECSET 2027` Unicode grapheme cluster mode (`\x1b[?2027h`) is active (`FND-03`).
    pub fn is_grapheme_cluster_2027(&self) -> bool {
        self.grapheme_cluster_2027
    }

    /// Returns `true` when `DECSET 2031` color-scheme reporting (`\x1b[?2031h`) is active (`FND-03`).
    pub fn is_report_color_scheme_2031(&self) -> bool {
        self.report_color_scheme_2031
    }

    /// Returns `true` when `DECSET 2048` in-band resize notifications (`\x1b[?2048h`) is active (`FND-03`).
    pub fn is_in_band_size_2048(&self) -> bool {
        self.in_band_size_2048
    }

    /// Returns `true` when `DECSET 67` (`DECBKM`) backarrow sends `BS` (`0x08`) instead of `DEL` (`0x7F`).
    pub fn is_backarrow_key_67(&self) -> bool {
        self.backarrow_key_67
    }

    /// Returns the active Kitty Keyboard Protocol (`CSI > flags u`) bitflags for the current screen (`FND-04`).
    pub fn kitty_keyboard_flags(&self) -> u8 {
        let stack = if self.is_alternate_screen() {
            &self.kitty_kbd_stack_alt
        } else {
            &self.kitty_kbd_stack_main
        };
        stack.last().copied().unwrap_or(0)
    }

    fn kitty_keyboard_stack_mut(&mut self) -> &mut Vec<u8> {
        if self.is_alternate_screen() {
            &mut self.kitty_kbd_stack_alt
        } else {
            &mut self.kitty_kbd_stack_main
        }
    }

    /// Updates the active theme RGB palette used to answer `OSC 10` / `OSC 11` / `OSC 12` queries (`FND-05`).
    pub fn set_theme_colors(&mut self, fg: (u8, u8, u8), bg: (u8, u8, u8), cursor: (u8, u8, u8)) {
        self.theme_fg_rgb = fg;
        self.theme_bg_rgb = bg;
        self.theme_cursor_rgb = cursor;
    }

    /// Returns the `CSI ? 997 ; <code > n` color-scheme code (`1` = dark, `2` = light) derived from `theme_bg_rgb` luminance (`BEL-03`).
    pub fn color_scheme_code(&self) -> u8 {
        let (r, g, b) = self.theme_bg_rgb;
        let luma = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
        if luma >= 128 { 2 } else { 1 }
    }

    /// Updates the theme palette and, if `DECSET ?2031` is enabled, queues an unsolicited `CSI ? 997 ; 1|2 n` notification (`BEL-03`).
    pub fn notify_color_scheme_changed(
        &mut self,
        fg: (u8, u8, u8),
        bg: (u8, u8, u8),
        cursor: (u8, u8, u8),
    ) {
        self.set_theme_colors(fg, bg, cursor);
        if self.report_color_scheme_2031 {
            let code = self.color_scheme_code();
            self.pending_query_responses
                .push(format!("\x1b[?997;{code}n").into_bytes());
        }
    }

    /// Returns the active `OSC 9;4` progress indicator state, if any (`BEL-02`).
    pub fn osc9_progress(&self) -> Option<Osc9Progress> {
        self.osc9_progress
    }

    /// Returns all active Kitty Graphics (`APC G`) and DEC Sixel (`DCS q`) placements on the canvas (`GFX-01`).
    pub fn graphic_placements(&self) -> &[TerminalGraphicPlacement] {
        &self.graphic_placements
    }

    /// Clears all active graphics placements (`GFX-01`).
    pub fn clear_graphics(&mut self) {
        self.graphic_placements.clear();
    }

    /// Returns all captured `OSC 9` / `OSC 777` notifications (`v0.7.0`).
    pub fn notifications(&self) -> &[TerminalNotification] {
        &self.notifications
    }

    /// Clears captured notifications and resets the unread bell indicator (`v0.7.0`).
    pub fn clear_notifications(&mut self) {
        self.notifications.clear();
        self.unread_bell = false;
    }

    /// Returns `true` if a bell or notification is unread (`v0.7.0`).
    pub fn unread_bell(&self) -> bool {
        self.unread_bell
    }

    /// Starts recording terminal output to an asciinema v2 `.cast` file (`v0.7.0`).
    pub fn start_recording(&mut self, path: PathBuf) {
        self.recording_started_at = Some(Instant::now());
        self.recording_path = Some(path);
        self.recording_events.clear();
    }

    /// Returns `true` when asciinema v2 `.cast` recording is active (`v0.7.0`).
    pub fn is_recording(&self) -> bool {
        self.recording_started_at.is_some()
    }

    /// Stops recording and writes the asciinema v2 `.cast` file to disk (`v0.7.0`).
    pub fn stop_recording(&mut self) -> std::io::Result<Option<PathBuf>> {
        self.recording_started_at = None;
        let Some(path) = self.recording_path.take() else {
            return Ok(None);
        };
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = format!(
            "{{\"version\":2,\"width\":{},\"height\":{},\"timestamp\":1792550000}}\n",
            self.cols, self.rows
        );
        for (elapsed, chunk) in self.recording_events.drain(..) {
            let escaped = escape_json_string(&chunk);
            out.push_str(&format!("[{elapsed:.6},\"o\",\"{escaped}\"]\n"));
        }
        std::fs::write(&path, out)?;
        Ok(Some(path))
    }

    /// Returns the bounded ring buffer of recent CSI/OSC/DCS escape sequences for the 3270 Terminal Inspector (`BEL-01`).
    pub fn recent_escape_sequences(&self) -> &[String] {
        &self.recent_escape_sequences
    }

    fn record_escape_sequence(&mut self, seq: impl Into<String>) {
        if self.recent_escape_sequences.len() >= 16 {
            self.recent_escape_sequences.remove(0);
        }
        self.recent_escape_sequences.push(seq.into());
    }

    /// Returns the exit code of the most recently finished command (`OSC 133;D;<exit>`, `VAL-06`).
    pub fn last_exit_code(&self) -> Option<i32> {
        self.last_exit_code
    }

    /// Returns the execution duration in milliseconds of the most recently finished command (`VAL-06`).
    pub fn last_duration_ms(&self) -> Option<u64> {
        self.last_duration_ms
    }

    /// Returns all explicit `OSC 8` hyperlinks recorded in the stream (`VAL-03`).
    pub fn osc8_links(&self) -> &[Osc8Hyperlink] {
        &self.osc8_links
    }

    /// Scans the currently visible viewport grid for `https://`, `http://`, and `file://` URLs (`VAL-03`).
    pub fn detect_visible_urls(&self) -> Vec<DetectedUrl> {
        let (rows, cols) = self.parser.screen().size();
        let mut urls = Vec::new();
        for r in 0..rows {
            let mut row_str = String::with_capacity(usize::from(cols));
            for c in 0..cols {
                let ch = self
                    .parser
                    .screen()
                    .cell(r, c)
                    .map(|cell| cell.contents())
                    .unwrap_or_default();
                if ch.is_empty() {
                    row_str.push(' ');
                } else {
                    row_str.push_str(ch);
                }
            }
            for scheme in ["https://", "http://", "file://"] {
                let mut search_from = 0usize;
                while let Some(rel_idx) = row_str[search_from..].find(scheme) {
                    let start_byte = search_from + rel_idx;
                    let after = &row_str[start_byte..];
                    let raw_len = after
                        .find(|ch: char| {
                            ch.is_whitespace()
                                || matches!(ch, '<' | '>' | '"' | '\'' | '(' | ')' | '[' | ']')
                        })
                        .unwrap_or(after.len());
                    let trimmed =
                        after[..raw_len].trim_end_matches(['.', ',', ';', ':', '!', '?']);
                    if trimmed.len() > scheme.len() {
                        let start_col = row_str[..start_byte].chars().count() as u16;
                        let url_cols = trimmed.chars().count() as u16;
                        urls.push(DetectedUrl {
                            row: r,
                            start_col,
                            end_col: start_col.saturating_add(url_cols).saturating_sub(1),
                            url: trimmed.to_string(),
                        });
                    }
                    search_from = start_byte + raw_len.max(scheme.len());
                }
            }
        }
        urls
    }

    /// Returns the URL at viewport cell `(row, col)` if present (`VAL-03`).
    pub fn url_at(&self, row: u16, col: u16) -> Option<String> {
        self.detect_visible_urls()
            .into_iter()
            .find(|u| u.row == row && col >= u.start_col && col <= u.end_col)
            .map(|u| u.url)
    }

    /// Scans the visible `OutputCanvas` grid for actionable tokens (URLs, file paths, IPv4 addresses,
    /// and Git commit SHAs) and assigns keyboard jump labels (`a`..`z`, `aa`..`az`) (`ECO-01`).
    pub fn detect_visible_hints(&self) -> Vec<CanvasHint> {
        let (rows, cols) = self.parser.screen().size();
        let mut raw_hints: Vec<(u16, u16, u16, String, CanvasHintKind)> = Vec::new();

        for r in 0..rows {
            let mut row_str = String::with_capacity(usize::from(cols));
            for c in 0..cols {
                let ch = self
                    .parser
                    .screen()
                    .cell(r, c)
                    .map(|cell| cell.contents())
                    .unwrap_or_default();
                if ch.is_empty() {
                    row_str.push(' ');
                } else {
                    row_str.push_str(ch);
                }
            }

            let mut byte_idx = 0usize;
            for token in row_str.split_whitespace() {
                let Some(rel) = row_str[byte_idx..].find(token) else {
                    continue;
                };
                let start_byte = byte_idx + rel;
                byte_idx = start_byte + token.len();

                let clean = token
                    .trim_start_matches(['(', '[', '<', '"', '\''])
                    .trim_end_matches([')', ']', '>', '"', '\'', ',', ';', '.']);
                if clean.len() < 4 {
                    continue;
                }
                let trim_prefix_bytes = token.find(clean).unwrap_or(0);
                let clean_start_byte = start_byte + trim_prefix_bytes;
                let start_col = row_str[..clean_start_byte].chars().count() as u16;
                let width_cols = clean.chars().count() as u16;
                let end_col = start_col.saturating_add(width_cols).saturating_sub(1);

                let kind_opt = if clean.starts_with("https://")
                    || clean.starts_with("http://")
                    || clean.starts_with("file://")
                {
                    Some(CanvasHintKind::Url)
                } else if is_ipv4_token(clean) {
                    Some(CanvasHintKind::IpAddr)
                } else if is_path_token(clean) {
                    Some(CanvasHintKind::Path)
                } else if is_git_sha_token(clean) {
                    Some(CanvasHintKind::GitSha)
                } else {
                    None
                };

                if let Some(kind) = kind_opt {
                    raw_hints.push((r, start_col, end_col, clean.to_string(), kind));
                }
            }
        }

        raw_hints
            .into_iter()
            .enumerate()
            .map(|(idx, (row, start_col, end_col, text, kind))| CanvasHint {
                label: hint_index_to_label(idx),
                row,
                start_col,
                end_col,
                text,
                kind,
            })
            .collect()
    }

    /// Searches the currently visible viewport grid for case-insensitive occurrences of `query` (`VAL-02`).
    pub fn search_canvas(&self, query: &str) -> Vec<CanvasSearchMatch> {
        if query.is_empty() {
            return Vec::new();
        }
        let needle = query.to_ascii_lowercase();
        let needle_cols = needle.chars().count() as u16;
        if needle_cols == 0 {
            return Vec::new();
        }
        let (rows, cols) = self.parser.screen().size();
        let mut matches = Vec::new();
        for r in 0..rows {
            let mut row_chars: Vec<char> = Vec::with_capacity(usize::from(cols));
            for c in 0..cols {
                let ch = self
                    .parser
                    .screen()
                    .cell(r, c)
                    .and_then(|cell| cell.contents().chars().next())
                    .unwrap_or(' ');
                row_chars.push(ch.to_ascii_lowercase());
            }
            let hay: String = row_chars.iter().collect();
            let mut from = 0usize;
            while let Some(pos) = hay[from..].find(&needle) {
                let byte_idx = from + pos;
                let start_col = hay[..byte_idx].chars().count() as u16;
                matches.push(CanvasSearchMatch {
                    row: r,
                    start_col,
                    end_col: start_col.saturating_add(needle_cols).saturating_sub(1),
                });
                from = byte_idx + needle.len().max(1);
            }
        }
        matches
    }

    /// Returns the most recently set `OSC 0` / `OSC 2` window title, if any.
    pub fn window_title(&self) -> Option<&str> {
        self.window_title.as_deref()
    }

    /// Returns the most recently reported `OSC 7` working directory (`\x1b]7;file://host/path\x07`), if any.
    pub fn reported_cwd(&self) -> Option<&Path> {
        self.reported_cwd.as_deref()
    }

    /// Takes any pending `DSR` (`CSI 5 n`), `CPR` (`CSI 6 n`), `DA1` (`CSI c`), `DECRPM`, `OSC 10/11/12`, or `XTGETTCAP` query responses.
    pub fn take_query_responses(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.pending_query_responses)
    }

    /// Returns the most recently observed `OSC 133` semantic prompt phase.
    pub fn osc133_phase(&self) -> Option<Osc133Phase> {
        self.osc133_phase
    }

    /// Records a command submission from the floating command prompt or `OSC 133` marker.
    pub fn record_command_submission(&mut self, command: impl Into<String>) {
        let (cursor_row, _) = self.parser.screen().cursor_position();
        let abs_row = self.total_lines_scrolled + usize::from(cursor_row);
        let index = self.command_blocks.len();
        let now = Instant::now();
        self.last_command_started_at = Some(now);
        self.command_blocks.push(CommandBlockRecord {
            index,
            command: command.into(),
            absolute_row: abs_row,
            exit_code: None,
            duration_ms: None,
        });
        self.selected_block_index = None;
    }

    pub fn command_blocks(&self) -> &[CommandBlockRecord] {
        &self.command_blocks
    }

    /// Returns the currently focused command block when navigating via `Alt+PageUp` / `Alt+PageDown`.
    pub fn selected_command_block(&self) -> Option<&CommandBlockRecord> {
        self.selected_block_index
            .and_then(|idx| self.command_blocks.get(idx))
    }

    /// Jumps scrollback to the previous recorded `CommandBlockRecord` (`Alt+PageUp` / `:block prev`, `PANE-05`).
    pub fn jump_to_prev_command_block(&mut self) -> Option<&CommandBlockRecord> {
        if self.command_blocks.is_empty() {
            return None;
        }
        let target_idx = match self.selected_block_index {
            None => self.command_blocks.len() - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        };
        self.selected_block_index = Some(target_idx);
        self.scroll_to_command_block(target_idx);
        self.command_blocks.get(target_idx)
    }

    /// Jumps scrollback to the next recorded `CommandBlockRecord` (`Alt+PageDown` / `:block next`, `PANE-05`).
    pub fn jump_to_next_command_block(&mut self) -> Option<&CommandBlockRecord> {
        let curr = self.selected_block_index?;
        if curr + 1 < self.command_blocks.len() {
            let next_idx = curr + 1;
            self.selected_block_index = Some(next_idx);
            self.scroll_to_command_block(next_idx);
            self.command_blocks.get(next_idx)
        } else {
            self.selected_block_index = None;
            self.reset_scrollback();
            None
        }
    }

    fn scroll_to_command_block(&mut self, idx: usize) {
        if let Some(block) = self.command_blocks.get(idx) {
            let current_bottom =
                self.total_lines_scrolled + usize::from(self.parser.screen().cursor_position().0);
            let offset = current_bottom
                .saturating_sub(block.absolute_row)
                .min(self.scrollback_capacity);
            self.parser.screen_mut().set_scrollback(offset);
        }
    }

    /// Takes any pending `OSC 52` clipboard sequences emitted by the child PTY for forwarding to the host terminal.
    pub fn take_outbound_osc52(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_osc52)
    }

    /// Current scrollback viewport offset (0 = live bottom of terminal).
    pub fn scrollback_offset(&self) -> usize {
        self.parser.screen().scrollback()
    }

    /// Scrolls the viewport up into history by `lines`.
    pub fn scroll_up(&mut self, lines: usize) {
        let next = self
            .scrollback_offset()
            .saturating_add(lines)
            .min(self.scrollback_capacity);
        self.parser.screen_mut().set_scrollback(next);
    }

    /// Scrolls the viewport down toward the live bottom by `lines`.
    pub fn scroll_down(&mut self, lines: usize) {
        let next = self.scrollback_offset().saturating_sub(lines);
        self.parser.screen_mut().set_scrollback(next);
        if self.scrollback_offset() == 0 {
            self.selected_block_index = None;
        }
    }

    /// Snaps the viewport back to the live bottom (`scrollback = 0`).
    pub fn reset_scrollback(&mut self) {
        self.parser.screen_mut().set_scrollback(0);
        self.selected_block_index = None;
    }

    /// Scans raw bytes for `DECSET 47/1047/1049`, `DECSET 2004`, `DECRPM`, Kitty keyboard protocol,
    /// `OSC 4/10/11/12` color queries, `OSC 8` hyperlinks, `OSC 52`, `OSC 133;A/B/C/D`, `XTGETTCAP`, and `DECRQSS`.
    fn scan_control_sequences(&mut self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);

        // Track CSI sequences in chronological order: DECSET/DECRST (?modes), DECRPM ($p),
        // DSR (5n), CPR (6n), Primary DA (c), and Kitty Keyboard Protocol (>u, <u, =u, ?u)
        for (idx, _) in text.match_indices("\x1b[") {
            let csi_rest = &text[idx + 2..];
            if let Some(rest) = csi_rest.strip_prefix('?') {
                if rest.starts_with("1049h") || rest.starts_with("1047h") || rest.starts_with("47h")
                {
                    self.alt_screen_47_or_1047 = true;
                    self.record_escape_sequence("CSI ?1049h (AltScreen ON)");
                } else if rest.starts_with("1049l")
                    || rest.starts_with("1047l")
                    || rest.starts_with("47l")
                {
                    self.alt_screen_47_or_1047 = false;
                    self.record_escape_sequence("CSI ?1049l (AltScreen OFF)");
                } else if rest.starts_with("2004h") {
                    self.bracketed_paste_2004 = true;
                    self.record_escape_sequence("CSI ?2004h (BracketedPaste ON)");
                } else if rest.starts_with("2004l") {
                    self.bracketed_paste_2004 = false;
                    self.record_escape_sequence("CSI ?2004l (BracketedPaste OFF)");
                } else if rest.starts_with("1h") {
                    self.application_cursor_decckm = true;
                    self.record_escape_sequence("CSI ?1h (DECCKM ON)");
                } else if rest.starts_with("1l") {
                    self.application_cursor_decckm = false;
                    self.record_escape_sequence("CSI ?1l (DECCKM OFF)");
                } else if rest.starts_with("67h") {
                    self.backarrow_key_67 = true;
                } else if rest.starts_with("67l") {
                    self.backarrow_key_67 = false;
                } else if rest.starts_with("1007h") {
                    self.mouse_alternate_scroll_1007 = true;
                } else if rest.starts_with("1007l") {
                    self.mouse_alternate_scroll_1007 = false;
                } else if rest.starts_with("1000h")
                    || rest.starts_with("1002h")
                    || rest.starts_with("1003h")
                    || rest.starts_with("1006h")
                {
                    self.mouse_tracking_mode = true;
                    self.record_escape_sequence("CSI ?1006h (MouseSGR ON)");
                } else if rest.starts_with("1000l")
                    || rest.starts_with("1002l")
                    || rest.starts_with("1003l")
                    || rest.starts_with("1006l")
                {
                    self.mouse_tracking_mode = false;
                    self.record_escape_sequence("CSI ?1006l (MouseSGR OFF)");
                } else if rest.starts_with("1004h") {
                    self.focus_reporting_1004 = true;
                } else if rest.starts_with("1004l") {
                    self.focus_reporting_1004 = false;
                } else if rest.starts_with("2026h") {
                    self.synchronized_output_2026 = true;
                    self.record_escape_sequence("CSI ?2026h (SyncOutput ON)");
                } else if rest.starts_with("2026l") {
                    self.synchronized_output_2026 = false;
                    self.record_escape_sequence("CSI ?2026l (SyncOutput OFF)");
                } else if rest.starts_with("2027h") {
                    self.grapheme_cluster_2027 = true;
                    self.record_escape_sequence("CSI ?2027h (GraphemeCluster ON)");
                } else if rest.starts_with("2027l") {
                    self.grapheme_cluster_2027 = false;
                    self.record_escape_sequence("CSI ?2027l (GraphemeCluster OFF)");
                } else if rest.starts_with("2031h") {
                    self.report_color_scheme_2031 = true;
                    self.record_escape_sequence("CSI ?2031h (ColorSchemeNotify ON)");
                } else if rest.starts_with("2031l") {
                    self.report_color_scheme_2031 = false;
                    self.record_escape_sequence("CSI ?2031l (ColorSchemeNotify OFF)");
                } else if rest.starts_with("2048h") {
                    self.in_band_size_2048 = true;
                    self.record_escape_sequence("CSI ?2048h (InBandResize ON)");
                } else if rest.starts_with("2048l") {
                    self.in_band_size_2048 = false;
                    self.record_escape_sequence("CSI ?2048l (InBandResize OFF)");
                } else if rest.starts_with("996n") {
                    // Color scheme DSR query: CSI ? 996 n -> CSI ? 997 ; 1|2 n (BEL-03)
                    let code = self.color_scheme_code();
                    self.pending_query_responses
                        .push(format!("\x1b[?997;{code}n").into_bytes());
                    self.record_escape_sequence(format!("CSI ?996n -> ?997;{code}n"));
                } else if rest.starts_with('u') {
                    // Kitty keyboard protocol query: CSI ? u -> CSI ? <flags> u
                    let flags = self.kitty_keyboard_flags();
                    self.pending_query_responses
                        .push(format!("\x1b[?{flags}u").into_bytes());
                    self.record_escape_sequence(format!("CSI ?u -> ?{flags}u"));
                } else {
                    // Check for DECRPM private mode query: CSI ? <mode> $ p
                    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if !digits.is_empty()
                        && rest[digits.len()..].starts_with("$p")
                        && let Ok(mode) = digits.parse::<u16>()
                    {
                        let ps = self.decrpm_private_mode_state(mode);
                        self.pending_query_responses
                            .push(format!("\x1b[?{mode};{ps}$y").into_bytes());
                    }
                }
                continue;
            }

            let rest = csi_rest;
            if rest.starts_with("5n") {
                self.pending_query_responses.push(b"\x1b[0n".to_vec());
            } else if rest.starts_with("6n") {
                let (r, c) = self.parser.screen().cursor_position();
                self.pending_query_responses
                    .push(format!("\x1b[{};{}R", r + 1, c + 1).into_bytes());
            } else if rest.starts_with('c') || rest.starts_with("0c") {
                self.pending_query_responses.push(b"\x1b[?62;22c".to_vec());
            } else if let Some(after_gt) = rest.strip_prefix('>') {
                // Kitty keyboard push: CSI > <flags> u
                let digits: String = after_gt
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if after_gt[digits.len()..].starts_with('u') {
                    let flags = digits.parse::<u8>().unwrap_or(0) & 0x1F;
                    let stack = self.kitty_keyboard_stack_mut();
                    if stack.len() >= 16 {
                        stack.remove(0);
                    }
                    stack.push(flags);
                }
            } else if let Some(after_lt) = rest.strip_prefix('<') {
                // Kitty keyboard pop: CSI < <count> u
                let digits: String = after_lt
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if after_lt[digits.len()..].starts_with('u') {
                    let count = digits.parse::<usize>().unwrap_or(1).max(1);
                    let stack = self.kitty_keyboard_stack_mut();
                    for _ in 0..count {
                        if stack.len() > 1 {
                            stack.pop();
                        } else {
                            stack[0] = 0;
                        }
                    }
                }
            } else if let Some(after_eq) = rest.strip_prefix('=') {
                // Kitty keyboard set: CSI = <flags> ; <mode> u (or CSI = <flags> u)
                let param_str: String = after_eq
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == ';')
                    .collect();
                if !param_str.is_empty() && after_eq[param_str.len()..].starts_with('u') {
                    let mut parts = param_str.split(';');
                    let flags = parts
                        .next()
                        .and_then(|s| s.parse::<u8>().ok())
                        .unwrap_or(0)
                        & 0x1F;
                    let mode = parts
                        .next()
                        .and_then(|s| s.parse::<u8>().ok())
                        .unwrap_or(1);
                    let stack = self.kitty_keyboard_stack_mut();
                    if let Some(top) = stack.last_mut() {
                        match mode {
                            2 => *top |= flags,
                            3 => *top &= !flags,
                            _ => *top = flags,
                        }
                    }
                }
            } else {
                // ANSI DECRPM query: CSI <mode> $ p -> CSI <mode> ; <state> $ y
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if !digits.is_empty()
                    && rest[digits.len()..].starts_with("$p")
                    && let Ok(mode) = digits.parse::<u16>()
                {
                    let ps = match mode {
                        4 | 20 => 2u8, // IRM (Insert Mode) and LNM (Linefeed/Newline) recognized as reset
                        _ => 0u8,
                    };
                    self.pending_query_responses
                        .push(format!("\x1b[{mode};{ps}$y").into_bytes());
                } else {
                    // Check for extended SGR sequence: CSI <params> m (FND-09)
                    let sgr_params: String = rest
                        .chars()
                        .take_while(|c| c.is_ascii_digit() || *c == ';' || *c == ':')
                        .collect();
                    if rest[sgr_params.len()..].starts_with('m') {
                        self.apply_extended_sgr(&sgr_params);
                    }
                }
            }
        }

        // Track OSC 0 / OSC 2 window title sequences: \x1b]0;<title>(\x07 or \x1b\\) and \x1b]2;<title>(\x07 or \x1b\\)
        for prefix in ["\x1b]0;", "\x1b]2;"] {
            for (idx, _) in text.match_indices(prefix) {
                let rest = &text[idx + prefix.len()..];
                if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                    self.window_title = Some(rest[..end].to_string());
                }
            }
        }

        // Track OSC 4 / 10 / 11 / 12 dynamic color queries (\x1b]10;?(\x07|\x1b\\), etc., FND-05)
        for (code, rgb) in [
            (10u8, self.theme_fg_rgb),
            (11u8, self.theme_bg_rgb),
            (12u8, self.theme_cursor_rgb),
        ] {
            let prefix = format!("\x1b]{code};?");
            for (idx, _) in text.match_indices(&prefix) {
                let rest = &text[idx + prefix.len()..];
                if rest.starts_with('\x07') || rest.starts_with("\x1b\\") {
                    let reply = format_osc_color_reply(code, None, rgb);
                    self.pending_query_responses.push(reply.into_bytes());
                }
            }
        }
        for (idx, _) in text.match_indices("\x1b]4;") {
            let rest = &text[idx + 4..];
            if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                let body = &rest[..end];
                if let Some((idx_str, spec)) = body.split_once(';')
                    && spec.trim() == "?"
                    && let Ok(palette_idx) = idx_str.trim().parse::<u8>()
                {
                    let rgb = ansi_256_index_to_rgb(palette_idx);
                    let reply = format_osc_color_reply(4, Some(palette_idx), rgb);
                    self.pending_query_responses.push(reply.into_bytes());
                }
            }
        }

        // Track OSC 7 working directory sequences: \x1b]7;<file-uri-or-path>(\x07 or \x1b\\)
        for (idx, _) in text.match_indices("\x1b]7;") {
            let rest = &text[idx + 4..];
            if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                let raw_uri = rest[..end].trim();
                let path_part = if let Some(after_scheme) = raw_uri.strip_prefix("file://") {
                    if let Some(slash_idx) = after_scheme.find('/') {
                        &after_scheme[slash_idx..]
                    } else {
                        after_scheme
                    }
                } else {
                    raw_uri
                };
                if !path_part.is_empty() {
                    self.reported_cwd = Some(PathBuf::from(path_part));
                }
            }
        }

        // Track OSC 8 explicit hyperlinks: \x1b]8;<params>;<uri>(\x07 or \x1b\\) (VAL-03)
        for (idx, _) in text.match_indices("\x1b]8;") {
            let rest = &text[idx + 4..];
            if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                let payload = &rest[..end];
                if let Some((params, uri)) = payload.split_once(';')
                    && !uri.is_empty()
                {
                    let id = params.split(':').find_map(|kv| kv.strip_prefix("id=")).map(str::to_string);
                    self.osc8_links.push(Osc8Hyperlink {
                        id,
                        uri: uri.to_string(),
                    });
                    self.record_escape_sequence(format!("OSC 8 {uri}"));
                }
            }
        }

        // Track OSC 9;<msg> notifications (when not OSC 9;4;... progress) and OSC 777;notify;<title>;<body> (v0.7.0)
        for (idx, _) in text.match_indices("\x1b]9;") {
            let rest = &text[idx + 4..];
            if !rest.starts_with("4;")
                && let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\"))
            {
                let msg = rest[..end].trim();
                if !msg.is_empty() {
                    self.notifications.push(TerminalNotification {
                        title: "OSC 9".to_string(),
                        body: msg.to_string(),
                    });
                    self.unread_bell = true;
                    self.record_escape_sequence(format!("OSC 9 Notify: {msg}"));
                }
            }
        }
        for (idx, _) in text.match_indices("\x1b]777;notify;") {
            let rest = &text[idx + 13..];
            if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                let payload = &rest[..end];
                let (title, body) = payload.split_once(';').unwrap_or(("NOTIFY", payload));
                self.notifications.push(TerminalNotification {
                    title: title.trim().to_string(),
                    body: body.trim().to_string(),
                });
                self.unread_bell = true;
                self.record_escape_sequence(format!("OSC 777 Notify: {title}"));
            }
        }

        // Track OSC 9;4 progress bar reports: \x1b]9;4;<state>[;<pct>](\x07 or \x1b\\) (BEL-02)
        for (idx, _) in text.match_indices("\x1b]9;4;") {
            let rest = &text[idx + 6..];
            if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                let payload = rest[..end].trim();
                let mut parts = payload.split(';');
                let state = parts
                    .next()
                    .and_then(|s| s.trim().parse::<u8>().ok())
                    .unwrap_or(0);
                let pct = parts
                    .next()
                    .and_then(|s| s.trim().parse::<u8>().ok())
                    .unwrap_or(0)
                    .min(100);
                self.osc9_progress = match state {
                    0 => None,
                    1 => Some(Osc9Progress::Normal(pct)),
                    2 => Some(Osc9Progress::Error(pct)),
                    3 => Some(Osc9Progress::Indeterminate),
                    4 => Some(Osc9Progress::Paused(pct)),
                    _ => self.osc9_progress,
                };
                self.record_escape_sequence(format!("OSC 9;4;{payload}"));
            }
        }

        // Track OSC 52 clipboard sequences: \x1b]52;<target>;<base64>(\x07 or \x1b\\)
        for (idx, _) in text.match_indices("\x1b]52;") {
            let rest = &text[idx..];
            if let Some(end_bel) = rest.find('\x07') {
                self.pending_osc52.push(rest[..=end_bel].to_string());
            } else if let Some(end_st) = rest.find("\x1b\\") {
                self.pending_osc52.push(rest[..end_st + 2].to_string());
            }
        }

        // Track DCS XTGETTCAP (\x1bP+q<hex>\x1b\\), DECRQSS (\x1bP$q<pt>\x1b\\), and DEC Sixel (\x1bP...q...\x1b\\)
        for (idx, _) in text.match_indices("\x1bP") {
            let rest = &text[idx + 2..];
            if let Some(end) = rest.find("\x1b\\").or_else(|| rest.find('\x07')) {
                let dcs_body = &rest[..end];
                if !dcs_body.starts_with("+q")
                    && !dcs_body.starts_with("$q")
                    && !dcs_body.starts_with("tmux;")
                    && let Some(q_pos) = dcs_body.find('q')
                {
                    let params = &dcs_body[..q_pos];
                    if params
                        .chars()
                        .all(|c| c.is_ascii_digit() || c == ';')
                    {
                        let sixel_payload = &dcs_body[q_pos + 1..];
                        let (cur_row, cur_col) = self.parser.screen().cursor_position();
                        let id = self.next_graphic_id;
                        self.next_graphic_id = self.next_graphic_id.wrapping_add(1);
                        let placement = decode_sixel_placement(id, cur_row, cur_col, sixel_payload);
                        self.record_escape_sequence(format!(
                            "DCS Sixel id={id} {}x{}px",
                            placement.pixel_width, placement.pixel_height
                        ));
                        self.graphic_placements.push(placement);
                    }
                }
            }
        }

        // Track Kitty Graphics Protocol APC sequences: \x1b_G<control>;<base64>(\x1b\\ or \x07) (GFX-01)
        for (idx, _) in text.match_indices("\x1b_G") {
            let rest = &text[idx + 3..];
            if let Some(end) = rest.find("\x1b\\").or_else(|| rest.find('\x07')) {
                let body = &rest[..end];
                let (ctrl_str, b64_payload) = body.split_once(';').unwrap_or((body, ""));
                let mut action = 'T';
                let mut image_id: Option<u32> = None;
                let mut format_code: u32 = 32;
                let mut px_w: u16 = 0;
                let mut px_h: u16 = 0;
                let mut cell_c: u16 = 0;
                let mut cell_r: u16 = 0;
                let mut quiet: u8 = 0;
                for kv in ctrl_str.split(',') {
                    if let Some((k, v)) = kv.split_once('=') {
                        match k.trim() {
                            "a" => action = v.chars().next().unwrap_or('T'),
                            "i" => image_id = v.parse::<u32>().ok(),
                            "f" => format_code = v.parse::<u32>().unwrap_or(32),
                            "s" => px_w = v.parse::<u16>().unwrap_or(0),
                            "v" => px_h = v.parse::<u16>().unwrap_or(0),
                            "c" => cell_c = v.parse::<u16>().unwrap_or(0),
                            "r" => cell_r = v.parse::<u16>().unwrap_or(0),
                            "q" => quiet = v.parse::<u8>().unwrap_or(0),
                            _ => {}
                        }
                    }
                }
                let resolved_id = image_id.unwrap_or_else(|| {
                    let id = self.next_graphic_id;
                    self.next_graphic_id = self.next_graphic_id.wrapping_add(1);
                    id
                });
                match action {
                    'q' => {
                        if quiet < 2 {
                            self.pending_query_responses
                                .push(format!("\x1b_Gi={resolved_id};OK\x1b\\").into_bytes());
                        }
                        self.record_escape_sequence(format!("APC Kitty Query i={resolved_id}"));
                    }
                    'd' => {
                        if let Some(target_id) = image_id {
                            self.graphic_placements.retain(|g| g.id != target_id);
                        } else {
                            self.graphic_placements.clear();
                        }
                        self.record_escape_sequence("APC Kitty Delete");
                    }
                    't' | 'T' | 'p' => {
                        let raw_bytes = base64_decode(b64_payload);
                        let w = px_w.max(2);
                        let h = px_h.max(2);
                        let rgba = expand_kitty_payload_to_rgba(&raw_bytes, format_code, w, h);
                        let (cur_row, cur_col) = self.parser.screen().cursor_position();
                        let cols = if cell_c > 0 { cell_c } else { w.div_ceil(8).max(1) };
                        let rows = if cell_r > 0 { cell_r } else { h.div_ceil(16).max(1) };
                        self.graphic_placements.push(TerminalGraphicPlacement {
                            id: resolved_id,
                            protocol: GraphicProtocol::Kitty,
                            row: cur_row,
                            col: cur_col,
                            cols,
                            rows,
                            pixel_width: w,
                            pixel_height: h,
                            rgba_bytes: rgba,
                        });
                        if image_id.is_some() && quiet == 0 {
                            self.pending_query_responses
                                .push(format!("\x1b_Gi={resolved_id};OK\x1b\\").into_bytes());
                        }
                        self.record_escape_sequence(format!(
                            "APC Kitty {action} i={resolved_id} {w}x{h}px"
                        ));
                    }
                    _ => {}
                }
            }
        }

        // Track DCS XTGETTCAP (\x1bP+q<hex>\x1b\\) and DECRQSS (\x1bP$q<pt>\x1b\\) (FND-06)
        for (idx, _) in text.match_indices("\x1bP+q") {
            let rest = &text[idx + 4..];
            if let Some(end) = rest.find("\x1b\\").or_else(|| rest.find('\x07')) {
                let query_body = &rest[..end];
                for hex_cap in query_body.split(';') {
                    if let Some(cap_name) = hex_decode_ascii(hex_cap.trim()) {
                        let reply = match xtgettcap_lookup(&cap_name) {
                            Some(val) => {
                                let name_hex = hex_encode_ascii(cap_name.as_bytes());
                                let val_hex = hex_encode_ascii(val.as_bytes());
                                format!("\x1bP1+r{name_hex}={val_hex}\x1b\\")
                            }
                            None => {
                                let name_hex = hex_encode_ascii(cap_name.as_bytes());
                                format!("\x1bP0+r{name_hex}\x1b\\")
                            }
                        };
                        self.pending_query_responses.push(reply.into_bytes());
                    }
                }
            }
        }
        for (idx, _) in text.match_indices("\x1bP$q") {
            let rest = &text[idx + 4..];
            if let Some(end) = rest.find("\x1b\\").or_else(|| rest.find('\x07')) {
                let pt = rest[..end].trim();
                let reply = match pt {
                    "m" => "\x1bP1$r0m\x1b\\".to_string(),
                    "r" => {
                        let (rows, _) = self.parser.screen().size();
                        format!("\x1bP1$r1;{rows}r\x1b\\")
                    }
                    " q" => "\x1bP1$r2 q\x1b\\".to_string(),
                    _ => format!("\x1bP0$r{pt}\x1b\\"),
                };
                self.pending_query_responses.push(reply.into_bytes());
            }
        }

        // Track OSC 133 semantic shell markers: \x1b]133;<phase>...(\x07 or \x1b\\)
        for (idx, _) in text.match_indices("\x1b]133;") {
            let rest = &text[idx + 6..];
            if let Some(first_char) = rest.chars().next() {
                match first_char {
                    'A' => {
                        self.osc133_phase = Some(Osc133Phase::PromptStart);
                    }
                    'B' => {
                        self.osc133_phase = Some(Osc133Phase::CommandStart);
                    }
                    'C' => {
                        self.osc133_phase = Some(Osc133Phase::CommandExecuted);
                        if self.last_command_started_at.is_none() {
                            self.last_command_started_at = Some(Instant::now());
                        }
                    }
                    'D' => {
                        let exit_code = rest.strip_prefix("D;").and_then(|s| {
                            s.chars()
                                .take_while(|c| c.is_ascii_digit() || *c == '-')
                                .collect::<String>()
                                .parse::<i32>()
                                .ok()
                        });
                        let elapsed_ms = self
                            .last_command_started_at
                            .take()
                            .map(|t| t.elapsed().as_millis() as u64);
                        self.last_exit_code = exit_code;
                        self.last_duration_ms = elapsed_ms;
                        self.osc133_phase = Some(Osc133Phase::CommandFinished(exit_code));
                        if let Some(last) = self.command_blocks.last_mut() {
                            last.exit_code = exit_code;
                            if last.duration_ms.is_none() {
                                last.duration_ms = elapsed_ms;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn decrpm_private_mode_state(&self, mode: u16) -> u8 {
        // 1 = set, 2 = reset, 0 = not recognized
        let is_set = match mode {
            1 => self.is_application_cursor(),
            7 => true, // DECAWM auto-wrap mode
            25 => !self.parser.screen().hide_cursor(),
            47 | 1047 | 1049 => self.is_alternate_screen(),
            67 => self.backarrow_key_67,
            1000 | 1002 | 1003 | 1006 => self.is_mouse_sgr_or_tracking(),
            1004 => self.focus_reporting_1004,
            1007 => self.mouse_alternate_scroll_1007,
            2004 => self.is_bracketed_paste(),
            2026 => self.synchronized_output_2026,
            2027 => self.grapheme_cluster_2027,
            2031 => self.report_color_scheme_2031,
            2048 => self.in_band_size_2048,
            _ => return 0,
        };
        if is_set { 1 } else { 2 }
    }

    fn apply_extended_sgr(&mut self, params: &str) {
        if params.is_empty() {
            self.active_underline_kind = UnderlineKind::None;
            self.active_underline_color = None;
            self.active_overline = false;
            return;
        }
        let tokens: Vec<&str> = params.split(';').collect();
        let mut idx = 0usize;
        while idx < tokens.len() {
            let tok = tokens[idx];
            if let Some(sub) = tok.strip_prefix("4:") {
                self.active_underline_kind = match sub {
                    "0" => UnderlineKind::None,
                    "1" => UnderlineKind::Single,
                    "2" => UnderlineKind::Double,
                    "3" => UnderlineKind::Curly,
                    "4" => UnderlineKind::Dotted,
                    "5" => UnderlineKind::Dashed,
                    _ => UnderlineKind::Single,
                };
                if self.active_underline_kind.is_underlined() {
                    self.last_styled_underline_kind = self.active_underline_kind;
                }
            } else if let Some(sub) = tok.strip_prefix("58:2:") {
                // Colon-separated SGR 58:2::r:g:b or 58:2:r:g:b
                let nums: Vec<u8> = sub
                    .split(':')
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse::<u8>().ok())
                    .collect();
                if nums.len() >= 3 {
                    let n = nums.len();
                    let rgb = RgbTriplet::new(nums[n - 3], nums[n - 2], nums[n - 1]);
                    self.active_underline_color = Some(rgb);
                    self.last_styled_underline_color = Some(rgb);
                }
            } else if tok == "58" && idx + 4 < tokens.len() && tokens[idx + 1] == "2" {
                // Semicolon-separated SGR 58;2;r;g;b
                if let (Ok(r), Ok(g), Ok(b)) = (
                    tokens[idx + 2].parse::<u8>(),
                    tokens[idx + 3].parse::<u8>(),
                    tokens[idx + 4].parse::<u8>(),
                ) {
                    let rgb = RgbTriplet::new(r, g, b);
                    self.active_underline_color = Some(rgb);
                    self.last_styled_underline_color = Some(rgb);
                }
                idx += 4;
            } else {
                match tok {
                    "0" | "" => {
                        self.active_underline_kind = UnderlineKind::None;
                        self.active_underline_color = None;
                        self.active_overline = false;
                    }
                    "4" => {
                        self.active_underline_kind = UnderlineKind::Single;
                        self.last_styled_underline_kind = UnderlineKind::Single;
                    }
                    "21" => {
                        self.active_underline_kind = UnderlineKind::Double;
                        self.last_styled_underline_kind = UnderlineKind::Double;
                    }
                    "24" => self.active_underline_kind = UnderlineKind::None,
                    "53" => self.active_overline = true,
                    "55" => self.active_overline = false,
                    "59" => self.active_underline_color = None,
                    _ => {}
                }
            }
            idx += 1;
        }
    }
}

/// Sanitizes pasted text by replacing dangerous C0/DEL control bytes (`NUL`, `ESC`, `DEL`,
/// `VINTR 0x03`, `VQUIT 0x1C`, `VSUSP 0x1A`, `VWERASE 0x17`, `VLNEXT 0x16`, etc.) with spaces
/// (`FND-01`, modelled after Ghostty's `src/input/paste.zig`).
pub fn sanitize_paste_bytes(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let code = ch as u32;
        if matches!(
            code,
            0x00
                | 0x03
                | 0x04
                | 0x05
                | 0x08
                | 0x0F
                | 0x11
                | 0x12
                | 0x13
                | 0x15
                | 0x16
                | 0x17
                | 0x1A
                | 0x1B
                | 0x1C
                | 0x7F
        ) {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

/// Returns `true` when pasting `raw` into a raw PTY presents an immediate execution or
/// bracketed-paste escape breakout hazard (`FND-01`).
pub fn is_unsafe_paste(raw: &str, bracketed_paste_active: bool) -> bool {
    if raw.contains("\x1b[201~") {
        return true;
    }
    if !bracketed_paste_active && (raw.contains('\n') || raw.contains('\r')) {
        return true;
    }
    false
}

fn format_osc_color_reply(code: u8, palette_idx: Option<u8>, (r, g, b): (u8, u8, u8)) -> String {
    let r16 = u16::from(r) * 0x0101;
    let g16 = u16::from(g) * 0x0101;
    let b16 = u16::from(b) * 0x0101;
    match palette_idx {
        Some(idx) => format!("\x1b]{code};{idx};rgb:{r16:04x}/{g16:04x}/{b16:04x}\x1b\\"),
        None => format!("\x1b]{code};rgb:{r16:04x}/{g16:04x}/{b16:04x}\x1b\\"),
    }
}

fn hint_index_to_label(idx: usize) -> String {
    if idx < 26 {
        ((b'a' + idx as u8) as char).to_string()
    } else {
        let first = (b'a' + ((idx / 26 - 1) % 26) as u8) as char;
        let second = (b'a' + (idx % 26) as u8) as char;
        format!("{first}{second}")
    }
}

fn is_ipv4_token(token: &str) -> bool {
    let host = token.split(':').next().unwrap_or(token);
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    parts.iter().all(|p| !p.is_empty() && p.parse::<u8>().is_ok())
}

fn is_path_token(token: &str) -> bool {
    let path_part = token.split(':').next().unwrap_or(token);
    if path_part.starts_with('/')
        || path_part.starts_with("./")
        || path_part.starts_with("../")
        || path_part.starts_with("~/")
    {
        return path_part.len() > 2
            && path_part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '~'));
    }
    if path_part.contains('/') && path_part.len() >= 4 {
        let valid_chars = path_part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'));
        let has_alpha = path_part.chars().any(|c| c.is_ascii_alphabetic());
        return valid_chars && has_alpha;
    }
    false
}

fn is_git_sha_token(token: &str) -> bool {
    if !(7..=40).contains(&token.len()) {
        return false;
    }
    let all_hex = token.chars().all(|c| c.is_ascii_hexdigit());
    let has_digit = token.chars().any(|c| c.is_ascii_digit());
    let has_hex_alpha = token
        .chars()
        .any(|c| matches!(c, 'a'..='f' | 'A'..='F'));
    all_hex && has_digit && has_hex_alpha
}

fn ansi_256_index_to_rgb(idx: u8) -> (u8, u8, u8) {
    match idx {
        0 => (0x00, 0x00, 0x00),
        1 => (0xAA, 0x00, 0x00),
        2 => (0x00, 0xAA, 0x00),
        3 => (0xAA, 0x55, 0x00),
        4 => (0x00, 0x00, 0xAA),
        5 => (0xAA, 0x00, 0xAA),
        6 => (0x00, 0xAA, 0xAA),
        7 => (0xAA, 0xAA, 0xAA),
        8 => (0x55, 0x55, 0x55),
        9 => (0xFF, 0x55, 0x55),
        10 => (0x55, 0xFF, 0x55),
        11 => (0xFF, 0xFF, 0x55),
        12 => (0x55, 0x55, 0xFF),
        13 => (0xFF, 0x55, 0xFF),
        14 => (0x55, 0xFF, 0xFF),
        15 => (0xFF, 0xFF, 0xFF),
        16..=231 => {
            let n = idx - 16;
            let r = (n / 36) % 6;
            let g = (n / 6) % 6;
            let b = n % 6;
            let map = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (map(r), map(g), map(b))
        }
        232..=255 => {
            let gray = 8 + (idx - 232) * 10;
            (gray, gray, gray)
        }
    }
}

fn hex_encode_ascii(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push_str(&format!("{b:02X}"));
    }
    out
}

fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn hex_decode_ascii(hex: &str) -> Option<String> {
    if !hex.len().is_multiple_of(2) || hex.is_empty() {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let b = u8::from_str_radix(&hex[i..i + 2], 16).ok()?;
        bytes.push(b);
    }
    String::from_utf8(bytes).ok()
}

fn xtgettcap_lookup(cap: &str) -> Option<&'static str> {
    match cap {
        "TN" | "name" => Some("xterm-256color"),
        "Co" | "colors" => Some("256"),
        "RGB" => Some("8/8/8"),
        "Tc" | "setrgbf" | "setrgbb" => Some("1"),
        "Smulx" => Some("\x1b[4:%p1%dm"),
        "Setulc" => Some("\x1b[58:2::%p1%{65536}%/%d:%p1%{256}%/%{255}%&%d:%p1%{255}%&%dm"),
        "Ms" => Some("\x1b]52;%p1%s;%p2%s\x07"),
        _ => None,
    }
}

/// Detects raw binary noise chunks (such as `tail -f /dev/random` or `cat /dev/urandom`) by
/// checking for invalid UTF-8 combined with raw binary C0 control bytes (`0x00..=0x06`, `0x0E..=0x1A`, `0x1C..=0x1F`).
fn is_binary_noise_chunk(bytes: &[u8]) -> bool {
    if bytes.len() < 8 || std::str::from_utf8(bytes).is_ok() {
        return false;
    }
    let binary_ctrl_count = bytes
        .iter()
        .filter(|&&b| matches!(b, 0x00..=0x06 | 0x0E..=0x1A | 0x1C..=0x1F))
        .count();
    binary_ctrl_count >= 2
}

/// Converts a raw binary stream chunk into safe printable ASCII + `\r\n` line breaks so
/// `tail -f /dev/random` scrolls smoothly without triggering bogus VT100 escape sequences.
fn sanitize_binary_noise_chunk(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 32);
    for &b in bytes {
        match b {
            b'\n' => {
                out.push(b'\r');
                out.push(b'\n');
            }
            b'\r' => {}
            0x20..=0x7E => out.push(b),
            _ => {
                // Map non-printable/binary bytes to deterministic visible ASCII characters
                out.push(b'!' + (b % 94));
            }
        }
    }
    out
}

/// Encodes arbitrary UTF-8 text into a standard `OSC 52` clipboard escape sequence (`\x1b]52;c;<base64>\x07`, `PTY-04`).
pub fn encode_osc52_clipboard(text: &str) -> String {
    let encoded = base64_encode(text.as_bytes());
    format!("\x1b]52;c;{encoded}\x07")
}

/// Pure-Rust RFC 4648 Base64 encoder for `OSC 52` clipboard payloads.
pub fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Pure-Rust RFC 4648 Base64 decoder for Kitty Graphics (`APC G`) payloads (`GFX-01`).
pub fn base64_decode(input: &str) -> Vec<u8> {
    fn val(b: u8) -> Option<u8> {
        match b {
            b'A'..=b'Z' => Some(b - b'A'),
            b'a'..=b'z' => Some(b - b'a' + 26),
            b'0'..=b'9' => Some(b - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let clean: Vec<u8> = input
        .bytes()
        .filter(|&b| !b.is_ascii_whitespace() && b != b'=')
        .filter_map(val)
        .collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    for chunk in clean.chunks(4) {
        if chunk.len() >= 2 {
            out.push((chunk[0] << 2) | (chunk[1] >> 4));
        }
        if chunk.len() >= 3 {
            out.push((chunk[1] << 4) | (chunk[2] >> 2));
        }
        if chunk.len() >= 4 {
            out.push((chunk[2] << 6) | chunk[3]);
        }
    }
    out
}

fn strip_apc_and_dcs_envelopes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while !rest.is_empty() {
        let next_apc = rest.find("\x1b_");
        let next_dcs = rest.find("\x1bP");
        let next_env = match (next_apc, next_dcs) {
            (Some(a), Some(d)) => Some(a.min(d)),
            (Some(a), None) => Some(a),
            (None, Some(d)) => Some(d),
            (None, None) => None,
        };
        if let Some(pos) = next_env {
            out.push_str(&rest[..pos]);
            let after_start = &rest[pos + 2..];
            if let Some(end_st) = after_start.find("\x1b\\") {
                rest = &after_start[end_st + 2..];
            } else if let Some(end_bel) = after_start.find('\x07') {
                rest = &after_start[end_bel + 1..];
            } else {
                break;
            }
        } else {
            out.push_str(rest);
            break;
        }
    }
    out
}

fn expand_kitty_payload_to_rgba(raw: &[u8], format_code: u32, w: u16, h: u16) -> Vec<u8> {
    let total_px = usize::from(w) * usize::from(h);
    let mut rgba = Vec::with_capacity(total_px * 4);
    match format_code {
        24 => {
            for px in raw.chunks(3).take(total_px) {
                let r = *px.first().unwrap_or(&0);
                let g = *px.get(1).unwrap_or(&0);
                let b = *px.get(2).unwrap_or(&0);
                rgba.extend_from_slice(&[r, g, b, 255]);
            }
        }
        _ => {
            for px in raw.chunks(4).take(total_px) {
                let r = *px.first().unwrap_or(&0);
                let g = *px.get(1).unwrap_or(&0);
                let b = *px.get(2).unwrap_or(&0);
                let a = *px.get(3).unwrap_or(&255);
                rgba.extend_from_slice(&[r, g, b, a]);
            }
        }
    }
    while rgba.len() < total_px * 4 {
        rgba.extend_from_slice(&[0, 0xFF, 0x66, 255]);
    }
    rgba
}

fn decode_sixel_placement(
    id: u32,
    row: u16,
    col: u16,
    payload: &str,
) -> TerminalGraphicPlacement {
    let mut palette: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (51, 102, 255),
        (255, 51, 51),
        (0, 255, 102),
        (255, 51, 255),
        (51, 255, 255),
        (255, 255, 85),
        (255, 255, 255),
        (128, 128, 128),
        (0, 170, 0),
        (255, 176, 0),
        (0, 170, 170),
        (170, 0, 170),
        (170, 170, 0),
        (85, 85, 255),
        (200, 200, 200),
    ];
    let mut active_reg: usize = 3;
    let mut width_px: u16 = 0;
    let mut height_px: u16 = 6;
    let mut bands: Vec<Vec<(u8, u8, u8, u8)>> = vec![Vec::new()];
    let mut cur_x: usize = 0;

    let bytes = payload.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                // Raster attributes "Pan;Pad;Ph;Pv
                i += 1;
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b';') {
                    i += 1;
                }
                let attr = &payload[start..i];
                let nums: Vec<u16> = attr
                    .split(';')
                    .filter_map(|s| s.parse::<u16>().ok())
                    .collect();
                if nums.len() >= 4 {
                    width_px = width_px.max(nums[2]);
                    height_px = height_px.max(nums[3]);
                }
            }
            b'#' => {
                // Color specifier #<reg> or #<reg>;2;<r>;<g>;<b>
                i += 1;
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b';') {
                    i += 1;
                }
                let spec = &payload[start..i];
                let nums: Vec<u16> = spec
                    .split(';')
                    .filter_map(|s| s.parse::<u16>().ok())
                    .collect();
                if let Some(&reg) = nums.first() {
                    active_reg = usize::from(reg) % palette.len();
                    if nums.len() >= 5 && nums[1] == 2 {
                        let r = ((u32::from(nums[2].min(100)) * 255) / 100) as u8;
                        let g = ((u32::from(nums[3].min(100)) * 255) / 100) as u8;
                        let b = ((u32::from(nums[4].min(100)) * 255) / 100) as u8;
                        palette[active_reg] = (r, g, b);
                    }
                }
            }
            b'!' => {
                // RLE repeat !<count><sixel_char>
                i += 1;
                let start = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                let count = payload[start..i]
                    .parse::<usize>()
                    .unwrap_or(1)
                    .clamp(1, 512);
                if i < bytes.len() && (0x3F..=0x7E).contains(&bytes[i]) {
                    let bits = bytes[i] - 0x3F;
                    let (r, g, b) = palette[active_reg];
                    let band = bands.last_mut().expect("band");
                    for _ in 0..count {
                        if cur_x >= band.len() {
                            band.push((r, g, b, bits));
                        } else if bits != 0 {
                            band[cur_x] = (r, g, b, bits);
                        }
                        cur_x += 1;
                    }
                    width_px = width_px.max(cur_x as u16);
                    i += 1;
                }
            }
            b'$' => {
                cur_x = 0;
                i += 1;
            }
            b'-' => {
                cur_x = 0;
                bands.push(Vec::new());
                height_px = height_px.max((bands.len() * 6) as u16);
                i += 1;
            }
            0x3F..=0x7E => {
                let bits = bytes[i] - 0x3F;
                let (r, g, b) = palette[active_reg];
                let band = bands.last_mut().expect("band");
                if cur_x >= band.len() {
                    band.push((r, g, b, bits));
                } else if bits != 0 {
                    band[cur_x] = (r, g, b, bits);
                }
                cur_x += 1;
                width_px = width_px.max(cur_x as u16);
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    let w = width_px.clamp(1, 512);
    let h = height_px.clamp(6, 512);
    let mut rgba = vec![0u8; usize::from(w) * usize::from(h) * 4];
    for (band_idx, band) in bands.iter().enumerate() {
        for (x, &(r, g, b, bits)) in band.iter().enumerate() {
            if x >= usize::from(w) {
                break;
            }
            for bit in 0..6usize {
                let y = band_idx * 6 + bit;
                if y >= usize::from(h) {
                    break;
                }
                if (bits & (1 << bit)) != 0 {
                    let off = (y * usize::from(w) + x) * 4;
                    rgba[off] = r;
                    rgba[off + 1] = g;
                    rgba[off + 2] = b;
                    rgba[off + 3] = 255;
                }
            }
        }
    }

    let cols = w.div_ceil(8).max(1);
    let rows = h.div_ceil(16).max(1);
    TerminalGraphicPlacement {
        id,
        protocol: GraphicProtocol::Sixel,
        row,
        col,
        cols,
        rows,
        pixel_width: w,
        pixel_height: h,
        rgba_bytes: rgba,
    }
}

/// Converts a `vt100_ctt::Cell` into a `ratatui::style::Style` preserving 16-color, 256-color,
/// and 24-bit TrueColor attributes plus bold, italic, underline, and inverse modifiers.
pub fn cell_to_ratatui_style(cell: &vt100_ctt::Cell) -> Style {
    let mut style = Style::default()
        .fg(vt_color_to_ratatui(cell.fgcolor()))
        .bg(vt_color_to_ratatui(cell.bgcolor()));

    let mut modifiers = Modifier::empty();
    if cell.bold() {
        modifiers |= Modifier::BOLD;
    }
    if cell.italic() {
        modifiers |= Modifier::ITALIC;
    }
    if cell.underline() {
        modifiers |= Modifier::UNDERLINED;
    }
    if cell.inverse() {
        modifiers |= Modifier::REVERSED;
    }
    style = style.add_modifier(modifiers);
    style
}

fn vt_color_to_ratatui(color: vt100_ctt::Color) -> Color {
    match color {
        vt100_ctt::Color::Default => Color::Reset,
        vt100_ctt::Color::Idx(idx) => Color::Indexed(idx),
        vt100_ctt::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

fn vt_color_to_cell_color(color: vt100_ctt::Color) -> CellColor {
    match color {
        vt100_ctt::Color::Default => CellColor::Default,
        vt100_ctt::Color::Idx(idx) => CellColor::Indexed(idx),
        vt100_ctt::Color::Rgb(r, g, b) => CellColor::Rgb(r, g, b),
    }
}

/// Rewrites extended SGR sub-parameters (`4:1..4:5` -> `4`, `4:0` -> `24`, and strips `58;2;r;g;b` / `58:2:...`)
/// and strips APC (`\x1b_...`) / DCS (`\x1bP...`) envelopes so `vt100_ctt::Parser` applies standard attributes
/// without dumping raw Kitty/Sixel graphics payloads onto the character grid.
fn normalize_extended_sgr_for_vt100(bytes: &[u8]) -> Vec<u8> {
    let Ok(raw_text) = std::str::from_utf8(bytes) else {
        return bytes.to_vec();
    };
    let stripped_cow = if raw_text.contains("\x1b_") || raw_text.contains("\x1bP") {
        std::borrow::Cow::Owned(strip_apc_and_dcs_envelopes(raw_text))
    } else {
        std::borrow::Cow::Borrowed(raw_text)
    };
    let text = stripped_cow.as_ref();
    if !text.contains("\x1b[") || (!text.contains("4:") && !text.contains("58")) {
        return text.as_bytes().to_vec();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find("\x1b[") {
        out.push_str(&rest[..pos]);
        out.push_str("\x1b[");
        let after_csi = &rest[pos + 2..];
        let param_len = after_csi
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == ';' || *c == ':')
            .map(char::len_utf8)
            .sum::<usize>();
        if after_csi[param_len..].starts_with('m') {
            let params = &after_csi[..param_len];
            let tokens: Vec<&str> = params.split(';').collect();
            let mut norm_tokens = Vec::with_capacity(tokens.len());
            let mut idx = 0usize;
            while idx < tokens.len() {
                let tok = tokens[idx];
                if let Some(sub) = tok.strip_prefix("4:") {
                    if sub == "0" {
                        norm_tokens.push("24");
                    } else {
                        norm_tokens.push("4");
                    }
                } else if tok.starts_with("58:") {
                    // Strip colon-style underline color for vt100-ctt
                } else if tok == "58" && idx + 4 < tokens.len() && tokens[idx + 1] == "2" {
                    idx += 4;
                } else {
                    norm_tokens.push(tok);
                }
                idx += 1;
            }
            out.push_str(&norm_tokens.join(";"));
            out.push('m');
            rest = &after_csi[param_len + 1..];
        } else {
            rest = after_csi;
        }
    }
    out.push_str(rest);
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_truecolor_and_256_color_cells() {
        let mut vt = VtParser::new(24, 80, 1000);
        vt.process(b"\x1b[38;2;12;34;56;48;5;208;1;3;4mA\x1b[0m");
        let cell = vt.screen().cell(0, 0).expect("cell (0,0)");
        assert_eq!(cell.contents(), "A");
        let style = cell_to_ratatui_style(cell);
        assert_eq!(style.fg, Some(Color::Rgb(12, 34, 56)));
        assert_eq!(style.bg, Some(Color::Indexed(208)));
        assert!(style.add_modifier.contains(Modifier::BOLD));
        assert!(style.add_modifier.contains(Modifier::ITALIC));
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn tracks_decset_1049_1047_and_47_alternate_screen() {
        let mut vt = VtParser::new(24, 80, 1000);
        assert!(!vt.is_alternate_screen());

        vt.process(b"\x1b[?1049h");
        assert!(vt.is_alternate_screen());
        vt.process(b"\x1b[?1049l");
        assert!(!vt.is_alternate_screen());

        vt.process(b"\x1b[?1047h");
        assert!(vt.is_alternate_screen());
        vt.process(b"\x1b[?1047l");
        assert!(!vt.is_alternate_screen());

        vt.process(b"\x1b[?47h");
        assert!(vt.is_alternate_screen());
        vt.process(b"\x1b[?47l");
        assert!(!vt.is_alternate_screen());
    }

    #[test]
    fn parses_osc_133_prompt_and_command_markers() {
        let mut vt = VtParser::new(24, 80, 1000);
        vt.process(b"\x1b]133;A\x07$ \x1b]133;B\x07");
        assert_eq!(vt.osc133_phase(), Some(Osc133Phase::CommandStart));

        vt.record_command_submission("ls -la");
        vt.process(b"\x1b]133;C\x07file1.txt\r\n");
        assert_eq!(vt.osc133_phase(), Some(Osc133Phase::CommandExecuted));

        vt.process(b"\x1b]133;D;0\x07");
        assert_eq!(vt.osc133_phase(), Some(Osc133Phase::CommandFinished(Some(0))));
        assert_eq!(vt.command_blocks().len(), 1);
        assert_eq!(vt.command_blocks()[0].exit_code, Some(0));
        assert!(vt.command_blocks()[0].duration_ms.is_some());
        assert_eq!(vt.last_exit_code(), Some(0));
    }

    #[test]
    fn supports_scrollback_navigation() {
        let mut vt = VtParser::new(5, 40, 100);
        for i in 0..15 {
            vt.process(format!("line {i}\r\n").as_bytes());
        }
        assert_eq!(vt.scrollback_offset(), 0);
        vt.scroll_up(4);
        assert!(vt.scrollback_offset() > 0);
        vt.reset_scrollback();
        assert_eq!(vt.scrollback_offset(), 0);
    }

    #[test]
    fn tracks_decset_2004_bracketed_paste_and_osc52_clipboard() {
        let mut vt = VtParser::new(10, 40, 100);
        assert!(!vt.is_bracketed_paste());
        vt.process(b"\x1b[?2004h");
        assert!(vt.is_bracketed_paste());
        vt.process(b"\x1b[?2004l");
        assert!(!vt.is_bracketed_paste());

        let seq = encode_osc52_clipboard("bilterm-3270");
        assert_eq!(seq, "\x1b]52;c;YmlsdGVybS0zMjcw\x07");

        vt.process(seq.as_bytes());
        let captured = vt.take_outbound_osc52();
        assert_eq!(captured, vec![seq]);
    }

    #[test]
    fn jumps_between_osc133_command_blocks_in_scrollback() {
        let mut vt = VtParser::new(4, 40, 200);
        vt.record_command_submission("cmd-one");
        for i in 0..8 {
            vt.process(format!("out1-{i}\r\n").as_bytes());
        }
        vt.record_command_submission("cmd-two");
        for i in 0..8 {
            vt.process(format!("out2-{i}\r\n").as_bytes());
        }

        let b2 = vt.jump_to_prev_command_block().expect("jump to cmd-two");
        assert_eq!(b2.command, "cmd-two");
        let b1 = vt.jump_to_prev_command_block().expect("jump to cmd-one");
        assert_eq!(b1.command, "cmd-one");
        let b2_again = vt.jump_to_next_command_block().expect("jump forward to cmd-two");
        assert_eq!(b2_again.command, "cmd-two");
        assert!(vt.jump_to_next_command_block().is_none());
        assert_eq!(vt.scrollback_offset(), 0);
    }

    #[test]
    fn sanitizes_paste_control_bytes_and_detects_unsafe_paste() {
        let malicious = "echo safe\x1b[2J\x03\x7f\x00\nrm -rf /";
        let clean = sanitize_paste_bytes(malicious);
        assert!(!clean.contains('\x1b'));
        assert!(!clean.contains('\x03'));
        assert!(!clean.contains('\x7f'));
        assert!(!clean.contains('\x00'));
        assert!(clean.contains("echo safe"));
        assert!(clean.contains("\nrm -rf /"));

        // Multi-line paste is unsafe when bracketed paste (?2004) is off
        assert!(is_unsafe_paste("line1\nline2", false));
        assert!(!is_unsafe_paste("line1\nline2", true));
        // Embedded bracketed-paste end marker is always unsafe
        assert!(is_unsafe_paste("evil\x1b[201~payload", true));
    }

    #[test]
    fn replies_to_decrpm_kitty_keyboard_osc_colors_and_xtgettcap() {
        let mut vt = VtParser::new(24, 80, 200);
        // Default ?1007 is set (1), ?2004 is reset (2)
        vt.process(b"\x1b[?1007$p\x1b[?2004$p\x1b[4$p");
        let replies = vt.take_query_responses();
        assert_eq!(
            replies,
            vec![
                b"\x1b[?1007;1$y".to_vec(),
                b"\x1b[?2004;2$y".to_vec(),
                b"\x1b[4;2$y".to_vec(),
            ]
        );

        // Kitty keyboard push/query/pop
        vt.process(b"\x1b[>5u\x1b[?u");
        assert_eq!(vt.kitty_keyboard_flags(), 5);
        assert_eq!(vt.take_query_responses(), vec![b"\x1b[?5u".to_vec()]);
        vt.process(b"\x1b[<1u");
        assert_eq!(vt.kitty_keyboard_flags(), 0);

        // OSC 10 / 11 color queries
        vt.set_theme_colors((0x55, 0xFF, 0x55), (0x00, 0x00, 0xA8), (0xFF, 0xFF, 0x55));
        vt.process(b"\x1b]10;?\x07\x1b]11;?\x1b\\");
        let osc_replies = vt.take_query_responses();
        assert_eq!(
            osc_replies,
            vec![
                b"\x1b]10;rgb:5555/ffff/5555\x1b\\".to_vec(),
                b"\x1b]11;rgb:0000/0000/a8a8\x1b\\".to_vec(),
            ]
        );

        // XTGETTCAP query for "TN" (544E) and "RGB" (524742)
        vt.process(b"\x1bP+q544E;524742\x1b\\");
        let cap_replies = vt.take_query_responses();
        assert_eq!(cap_replies.len(), 2);
        assert!(String::from_utf8_lossy(&cap_replies[0]).starts_with("\x1bP1+r544E="));
        assert!(String::from_utf8_lossy(&cap_replies[1]).starts_with("\x1bP1+r524742="));
    }

    #[test]
    fn detects_urls_and_searches_canvas_grid() {
        let mut vt = VtParser::new(6, 60, 100);
        vt.process(b"Visit https://github.com/ghostty-org/ghostty for details.\r\n");
        vt.process(b"\x1b]8;id=doc;https://ratatui.rs\x07Ratatui Docs\x1b]8;;\x07\r\n");

        let urls = vt.detect_visible_urls();
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://github.com/ghostty-org/ghostty");
        assert_eq!(
            vt.url_at(0, 10).as_deref(),
            Some("https://github.com/ghostty-org/ghostty")
        );
        assert_eq!(vt.osc8_links().len(), 1);
        assert_eq!(vt.osc8_links()[0].uri, "https://ratatui.rs");

        let matches = vt.search_canvas("GHOSTTY");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].row, 0);
    }

    fn history_text(vt: &mut VtParser) -> String {
        let used = vt.scrollback_used_rows();
        let (rows, _) = vt.size();
        let mut out = String::new();
        let mut j = 0;
        while j < used {
            let off = used - j;
            vt.parser.screen_mut().set_scrollback(off);
            let take = usize::from(rows).min(off);
            for r in 0..take {
                let (_, cols) = vt.parser.screen().size();
                for c in 0..cols {
                    if let Some(cell) = vt.parser.screen().cell(r as u16, c) {
                        out.push_str(cell.contents());
                    }
                }
                out.push('\n');
            }
            j += take;
        }
        vt.parser.screen_mut().set_scrollback(0);
        out
    }

    #[test]
    fn column_resize_preserves_scrollback_beyond_primary_log_cap() {
        let mut vt = VtParser::new(24, 80, 50_000);
        let mut bytes = Vec::new();
        for i in 0..20_000 {
            bytes.extend_from_slice(format!("line-{i:06}\r\n").as_bytes());
        }
        assert!(bytes.len() > 200_000);
        vt.process(&bytes);
        vt.set_size(24, 60);
        let text = history_text(&mut vt);
        assert!(text.contains("line-000000"), "oldest line lost after column resize");
        assert!(text.contains("line-010000"));
        assert!(vt.scrollback_used_rows() >= 19_000);
        assert_eq!(vt.size(), (24, 60));
    }

    #[test]
    fn rebuild_reflows_wrapped_lines_and_honours_new_capacity() {
        let mut vt = VtParser::new(5, 20, 1000);
        let long: String = "abcdefghij".repeat(4); // 40 cols -> 2 rows at width 20
        vt.process(format!("{long}\r\n").as_bytes());
        for i in 0..100 {
            vt.process(format!("n{i}\r\n").as_bytes());
        }
        vt.process(b"$ ");
        vt.rebuild(5, 40, 1000);
        let text = history_text(&mut vt);
        assert!(text.contains(&long), "wrapped line should be rejoined at width 40");
        assert_eq!(vt.screen().cursor_position().1, 2);
        // Capacity shrink keeps only the newest lines.
        vt.rebuild(5, 40, 10);
        assert_eq!(vt.scrollback_capacity(), 10);
        assert!(vt.scrollback_used_rows() <= 10);
        let text = history_text(&mut vt);
        assert!(text.contains("n95"));
        assert!(!text.contains("n10\n"));
    }

    #[test]
    fn same_size_rebuild_restores_screen_and_cursor_exactly() {
        let mut vt = VtParser::new(4, 20, 100);
        for i in 0..10 {
            vt.process(format!("r{i}\r\n").as_bytes());
        }
        vt.process(b"\x1b[31mred\x1b[0m $ ");
        let before = vt.screen().contents();
        let cursor = vt.screen().cursor_position();
        vt.set_scrollback_capacity(500);
        assert_eq!(vt.scrollback_capacity(), 500);
        assert_eq!(vt.screen().contents(), before);
        assert_eq!(vt.screen().cursor_position(), cursor);
        assert_eq!(vt.screen().cell(3, 0).unwrap().fgcolor(), vt100_ctt::Color::Idx(1));
        assert!(history_text(&mut vt).contains("r0"));
    }

    #[test]
    fn clear_scrollback_frees_history_and_skips_alt_screen() {
        let mut vt = VtParser::new(4, 20, 100);
        for i in 0..30 {
            vt.process(format!("r{i}\r\n").as_bytes());
        }
        vt.record_command_submission("old");
        assert!(vt.scrollback_used_rows() > 0);
        vt.clear_scrollback();
        assert_eq!(vt.scrollback_used_rows(), 0);
        assert!(vt.screen().contents().contains("r29"));
        vt.process(b"\x1b[?1049h");
        assert!(!vt.set_scrollback_capacity(50));
        assert_eq!(vt.scrollback_capacity(), 100);
    }
}
