pub mod backend;
pub mod codec;
pub mod detect;
pub mod ghostty_backend;
pub mod graphics;
pub mod recording;
pub mod scan;
mod scan_osc_dcs;
pub mod scrollback;
pub mod snapshot;
pub mod tektronix;
pub mod worker;

pub use backend::*;
pub use codec::*;
pub use detect::*;
pub use graphics::*;
pub use tektronix::{
    TEK_MAX_X, TEK_MAX_Y, TekAlphaLabel, TekLineStyle, TekMode, TekVectorSegment, TektronixState,
    encode_tek_10bit_point, encode_tek_12bit_point,
};

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::vt::codec::{is_binary_noise_chunk, sanitize_binary_noise_chunk};
use crate::vt::scan::normalize_extended_sgr_for_vt100;
pub use crate::vt::scrollback::PaneScrollMode;
pub use crate::vt::snapshot::{LineSizeAttr, scan_dec_line_size_escapes};
use crate::vt::snapshot::{
    CellColor, CellSnapshot, CursorShape, CursorSnapshot, GridSnapshot, RgbTriplet, RowSnapshot,
    UnderlineKind,
};

pub use crate::pty::line_discipline::Osc133Phase;

/// Recorded command block boundary in scrollback/screen history (`PANE-05`, `VAL-06`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandBlockRecord {
    pub index: usize,
    pub command: String,
    pub absolute_row: usize,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
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
    recording_private: bool,
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
    line_size_attrs: std::collections::BTreeMap<u16, LineSizeAttr>,
    tektronix: TektronixState,
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
            recording_private: false,
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
            line_size_attrs: std::collections::BTreeMap::new(),
            tektronix: TektronixState::new(),
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
        let (start_row, _) = self.parser.screen().cursor_position();
        let vt_bytes = self.tektronix.process_and_filter_vt100(bytes);
        let newline_count = vt_bytes.iter().filter(|&&b| b == b'\n').count();
        self.total_lines_scrolled = self.total_lines_scrolled.saturating_add(newline_count);
        if !vt_bytes.is_empty() {
            let normalized = normalize_extended_sgr_for_vt100(&vt_bytes);
            self.parser.process(&normalized);
            scan_dec_line_size_escapes(
                &vt_bytes,
                start_row,
                self.rows,
                &mut self.line_size_attrs,
            );
            if !was_alt && !self.is_alternate_screen() {
                self.append_primary_stream_log(&normalized);
            }
        }
        self.scan_control_sequences(bytes);
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
                line_size: self.line_size_attr(r),
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

    pub fn size(&self) -> (u16, u16) { (self.rows, self.cols) }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn line_size_attr(&self, row: u16) -> LineSizeAttr {
        self.line_size_attrs.get(&row).copied().unwrap_or_default()
    }
    pub fn tektronix(&self) -> &TektronixState { &self.tektronix }
    pub fn tektronix_mut(&mut self) -> &mut TektronixState { &mut self.tektronix }

    #[cfg(test)]
    pub(in crate::vt) fn screen(&self) -> &vt100_ctt::Screen { self.parser.screen() }

    pub fn is_alternate_screen(&self) -> bool {
        self.parser.screen().alternate_screen() || self.alt_screen_47_or_1047
    }

    pub fn is_bracketed_paste(&self) -> bool {
        self.parser.screen().bracketed_paste() || self.bracketed_paste_2004
    }

    pub fn is_application_cursor(&self) -> bool {
        self.parser.screen().application_cursor() || self.application_cursor_decckm
    }

    pub fn is_mouse_sgr_or_tracking(&self) -> bool {
        !matches!(
            self.parser.screen().mouse_protocol_mode(),
            vt100_ctt::MouseProtocolMode::None
        ) || self.mouse_tracking_mode
    }

    pub fn is_focus_reporting(&self) -> bool { self.focus_reporting_1004 }
    pub fn is_synchronized_output(&self) -> bool { self.synchronized_output_2026 }
    pub fn is_mouse_alternate_scroll(&self) -> bool { self.mouse_alternate_scroll_1007 }
    pub fn is_grapheme_cluster_2027(&self) -> bool { self.grapheme_cluster_2027 }
    pub fn is_report_color_scheme_2031(&self) -> bool { self.report_color_scheme_2031 }
    pub fn is_in_band_size_2048(&self) -> bool { self.in_band_size_2048 }
    pub fn is_backarrow_key_67(&self) -> bool { self.backarrow_key_67 }

    pub fn kitty_keyboard_flags(&self) -> u8 {
        let stack = if self.is_alternate_screen() {
            &self.kitty_kbd_stack_alt
        } else {
            &self.kitty_kbd_stack_main
        };
        stack.last().copied().unwrap_or(0)
    }

    pub(super) fn kitty_keyboard_stack_mut(&mut self) -> &mut Vec<u8> {
        if self.is_alternate_screen() {
            &mut self.kitty_kbd_stack_alt
        } else {
            &mut self.kitty_kbd_stack_main
        }
    }

    pub fn set_theme_colors(&mut self, fg: (u8, u8, u8), bg: (u8, u8, u8), cursor: (u8, u8, u8)) {
        self.theme_fg_rgb = fg;
        self.theme_bg_rgb = bg;
        self.theme_cursor_rgb = cursor;
    }

    pub fn color_scheme_code(&self) -> u8 {
        let (r, g, b) = self.theme_bg_rgb;
        let luma = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
        if luma >= 128 { 2 } else { 1 }
    }

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

    pub fn osc9_progress(&self) -> Option<Osc9Progress> { self.osc9_progress }
    pub fn notifications(&self) -> &[TerminalNotification] { &self.notifications }

    pub fn clear_notifications(&mut self) {
        self.notifications.clear();
        self.unread_bell = false;
    }

    pub fn unread_bell(&self) -> bool { self.unread_bell }
    pub fn recent_escape_sequences(&self) -> &[String] { &self.recent_escape_sequences }
    pub fn last_exit_code(&self) -> Option<i32> { self.last_exit_code }
    pub fn last_duration_ms(&self) -> Option<u64> { self.last_duration_ms }
    pub fn window_title(&self) -> Option<&str> { self.window_title.as_deref() }
    pub fn reported_cwd(&self) -> Option<&Path> { self.reported_cwd.as_deref() }
    pub fn take_query_responses(&mut self) -> Vec<Vec<u8>> { std::mem::take(&mut self.pending_query_responses) }
    pub fn osc133_phase(&self) -> Option<Osc133Phase> { self.osc133_phase }

    pub fn record_command_submission(&mut self, command: impl Into<String>) {
        let (cursor_row, _) = self.parser.screen().cursor_position();
        let abs_row = self.total_lines_scrolled + usize::from(cursor_row);
        let index = self.command_blocks.len();
        let now = Instant::now();
        self.last_command_started_at = Some(now);
        self.osc133_phase = Some(Osc133Phase::CommandExecuted);
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

    pub fn selected_command_block(&self) -> Option<&CommandBlockRecord> {
        self.selected_block_index
            .and_then(|idx| self.command_blocks.get(idx))
    }

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

    pub fn take_outbound_osc52(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_osc52)
    }

    pub fn scrollback_offset(&self) -> usize {
        self.parser.screen().scrollback()
    }

    pub fn set_scrollback_offset(&mut self, offset: usize) {
        let clamped = offset.min(self.scrollback_capacity);
        self.parser.screen_mut().set_scrollback(clamped);
        if self.scrollback_offset() == 0 {
            self.selected_block_index = None;
        }
    }

    pub fn scroll_up(&mut self, lines: usize) {
        let next = self
            .scrollback_offset()
            .saturating_add(lines)
            .min(self.scrollback_capacity);
        self.parser.screen_mut().set_scrollback(next);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        let next = self.scrollback_offset().saturating_sub(lines);
        self.parser.screen_mut().set_scrollback(next);
        if self.scrollback_offset() == 0 {
            self.selected_block_index = None;
        }
    }

    pub fn reset_scrollback(&mut self) {
        self.parser.screen_mut().set_scrollback(0);
        self.selected_block_index = None;
    }
}

fn vt_color_to_cell_color(color: vt100_ctt::Color) -> CellColor {
    match color {
        vt100_ctt::Color::Default => CellColor::Default,
        vt100_ctt::Color::Idx(idx) => CellColor::Indexed(idx),
        vt100_ctt::Color::Rgb(r, g, b) => CellColor::Rgb(r, g, b),
    }
}

#[cfg(test)]
mod tests;
