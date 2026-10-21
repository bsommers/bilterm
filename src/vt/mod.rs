use std::path::{Path, PathBuf};
use std::time::Instant;

use ratatui::style::{Color, Modifier, Style};

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

/// Virtual terminal state machine wrapping `vt100-ctt` with explicit `DECSET 47/1047/1049`
/// alternate screen tracking, `DECSET 2004` bracketed paste tracking, `DECSET 1007` alternate scroll,
/// `DECRPM` mode reporting, Kitty keyboard protocol stacks, `XTGETTCAP`/`DECRQSS`, `OSC 4/10/11/12`
/// color queries, `OSC 8` hyperlinks, `OSC 52` clipboard passthrough, and `OSC 133` command blocks.
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
    pending_osc52: Vec<String>,
    pending_query_responses: Vec<Vec<u8>>,
    total_lines_scrolled: usize,
    total_bytes_processed: usize,
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
            pending_osc52: Vec::new(),
            pending_query_responses: Vec::new(),
            total_lines_scrolled: 0,
            total_bytes_processed: 0,
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
        self.total_bytes_processed = self.total_bytes_processed.saturating_add(bytes.len());
        if !self.is_alternate_screen() && is_binary_noise_chunk(bytes) {
            let sanitized = sanitize_binary_noise_chunk(bytes);
            let newline_count = sanitized.iter().filter(|&&b| b == b'\n').count();
            self.total_lines_scrolled = self.total_lines_scrolled.saturating_add(newline_count);
            self.parser.process(&sanitized);
            return;
        }
        let newline_count = bytes.iter().filter(|&&b| b == b'\n').count();
        self.total_lines_scrolled = self.total_lines_scrolled.saturating_add(newline_count);
        self.parser.process(bytes);
        self.scan_control_sequences(bytes);
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

    /// Resizes the virtual terminal screen dimensions.
    pub fn set_size(&mut self, rows: u16, cols: u16) {
        let clamped_rows = rows.max(1);
        let clamped_cols = cols.max(1);
        if (self.rows, self.cols) != (clamped_rows, clamped_cols) {
            self.rows = clamped_rows;
            self.cols = clamped_cols;
            self.parser.screen_mut().set_size(clamped_rows, clamped_cols);
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
                } else if rest.starts_with("1049l")
                    || rest.starts_with("1047l")
                    || rest.starts_with("47l")
                {
                    self.alt_screen_47_or_1047 = false;
                } else if rest.starts_with("2004h") {
                    self.bracketed_paste_2004 = true;
                } else if rest.starts_with("2004l") {
                    self.bracketed_paste_2004 = false;
                } else if rest.starts_with("1h") {
                    self.application_cursor_decckm = true;
                } else if rest.starts_with("1l") {
                    self.application_cursor_decckm = false;
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
                } else if rest.starts_with("1000l")
                    || rest.starts_with("1002l")
                    || rest.starts_with("1003l")
                    || rest.starts_with("1006l")
                {
                    self.mouse_tracking_mode = false;
                } else if rest.starts_with("1004h") {
                    self.focus_reporting_1004 = true;
                } else if rest.starts_with("1004l") {
                    self.focus_reporting_1004 = false;
                } else if rest.starts_with("2026h") {
                    self.synchronized_output_2026 = true;
                } else if rest.starts_with("2026l") {
                    self.synchronized_output_2026 = false;
                } else if rest.starts_with("2027h") {
                    self.grapheme_cluster_2027 = true;
                } else if rest.starts_with("2027l") {
                    self.grapheme_cluster_2027 = false;
                } else if rest.starts_with("2031h") {
                    self.report_color_scheme_2031 = true;
                } else if rest.starts_with("2031l") {
                    self.report_color_scheme_2031 = false;
                } else if rest.starts_with("2048h") {
                    self.in_band_size_2048 = true;
                } else if rest.starts_with("2048l") {
                    self.in_band_size_2048 = false;
                } else if rest.starts_with('u') {
                    // Kitty keyboard protocol query: CSI ? u -> CSI ? <flags> u
                    let flags = self.kitty_keyboard_flags();
                    self.pending_query_responses
                        .push(format!("\x1b[?{flags}u").into_bytes());
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
                }
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

fn hex_decode_ascii(hex: &str) -> Option<String> {
    if hex.len() % 2 != 0 || hex.is_empty() {
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
}
