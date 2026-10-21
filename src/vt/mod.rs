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

/// Recorded command block boundary in scrollback/screen history (`PANE-05`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandBlockRecord {
    pub index: usize,
    pub command: String,
    pub absolute_row: usize,
    pub exit_code: Option<i32>,
}

/// Virtual terminal state machine wrapping `vt100-ctt` with explicit `DECSET 47/1047/1049`
/// alternate screen tracking, `DECSET 2004` bracketed paste tracking, `OSC 52` clipboard
/// passthrough, `OSC 133` semantic prompt parsing, and command-block scrollback navigation.
pub struct VtParser {
    parser: vt100_ctt::Parser,
    rows: u16,
    cols: u16,
    scrollback_capacity: usize,
    alt_screen_47_or_1047: bool,
    bracketed_paste_2004: bool,
    osc133_phase: Option<Osc133Phase>,
    command_blocks: Vec<CommandBlockRecord>,
    selected_block_index: Option<usize>,
    pending_osc52: Vec<String>,
    total_lines_scrolled: usize,
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
            osc133_phase: None,
            command_blocks: Vec::new(),
            selected_block_index: None,
            pending_osc52: Vec::new(),
            total_lines_scrolled: 0,
        }
    }

    /// Processes a chunk of raw PTY output bytes, updating the VT100 grid, `DECSET 47/1047/1049`,
    /// `DECSET 2004`, `OSC 52`, and `OSC 133` semantic prompt markers.
    pub fn process(&mut self, bytes: &[u8]) {
        self.scan_control_sequences(bytes);
        let newline_count = bytes.iter().filter(|&&b| b == b'\n').count();
        self.total_lines_scrolled = self.total_lines_scrolled.saturating_add(newline_count);
        self.parser.process(bytes);
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

    /// Returns the most recently observed `OSC 133` semantic prompt phase.
    pub fn osc133_phase(&self) -> Option<Osc133Phase> {
        self.osc133_phase
    }

    /// Records a command submission from the floating command prompt or `OSC 133` marker.
    pub fn record_command_submission(&mut self, command: impl Into<String>) {
        let (cursor_row, _) = self.parser.screen().cursor_position();
        let abs_row = self.total_lines_scrolled + usize::from(cursor_row);
        let index = self.command_blocks.len();
        self.command_blocks.push(CommandBlockRecord {
            index,
            command: command.into(),
            absolute_row: abs_row,
            exit_code: None,
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

    /// Scans raw bytes for `DECSET 47/1047/1049`, `DECSET 2004`, `OSC 52`, and `OSC 133;A/B/C/D` sequences.
    fn scan_control_sequences(&mut self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);

        // Track DECSET 47 / 1047 / 1049 and DECSET 2004 enter/exit sequences explicitly
        for (idx, _) in text.match_indices("\x1b[?") {
            let rest = &text[idx + 3..];
            if rest.starts_with("1049h") || rest.starts_with("1047h") || rest.starts_with("47h") {
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
                    }
                    'D' => {
                        let exit_code = rest.strip_prefix("D;").and_then(|s| {
                            s.chars()
                                .take_while(|c| c.is_ascii_digit() || *c == '-')
                                .collect::<String>()
                                .parse::<i32>()
                                .ok()
                        });
                        self.osc133_phase = Some(Osc133Phase::CommandFinished(exit_code));
                        if let Some(last) = self.command_blocks.last_mut() {
                            last.exit_code = exit_code;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
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
}
