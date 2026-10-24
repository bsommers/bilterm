use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};

use super::VtParser;
use super::codec::escape_json_string;

/// Whole-workspace Asciicast v2 (`.cast`) compositor frame recorder (`REC-02`).
///
/// Diffs rendered [`ratatui::buffer::Buffer`] frames into compact ANSI escape sequences so
/// multi-pane splits, z-order stacked windows, floating command prompts, Borland menus,
/// 3270 cross-hairs, and modals are captured into a single standard `asciinema` v2 `.cast` stream.
#[derive(Debug)]
pub struct WorkspaceRecorder {
    started_at: Instant,
    path: PathBuf,
    private: bool,
    width: u16,
    height: u16,
    prev_cells: Vec<(String, Style)>,
    events: Vec<(f64, String)>,
}

impl WorkspaceRecorder {
    /// Creates a new whole-workspace recorder targeting `path`.
    pub fn new(path: PathBuf, private: bool, width: u16, height: u16) -> Self {
        Self {
            started_at: Instant::now(),
            path,
            private,
            width: width.max(1),
            height: height.max(1),
            prev_cells: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Returns the target `.cast` output path.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Returns the number of composited workspace frame diffs captured so far.
    pub fn frame_count(&self) -> usize {
        self.events.len()
    }

    /// Diffs `buf` against the previous captured frame and records a compact ANSI frame event if any cell changed.
    pub fn capture_buffer(&mut self, buf: &Buffer) {
        let area = buf.area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        let total = usize::from(area.width) * usize::from(area.height);
        let resized = self.width != area.width
            || self.height != area.height
            || self.prev_cells.len() != total;
        if resized {
            self.width = area.width;
            self.height = area.height;
            self.prev_cells.clear();
            self.prev_cells
                .resize(total, (String::new(), Style::default()));
        }

        let mut ansi = String::new();
        if resized {
            ansi.push_str("\x1b[0m\x1b[2J\x1b[?25l");
        }
        let mut last_pos: Option<(u16, u16)> = None;
        let mut last_style: Option<Style> = None;

        for row in 0..area.height {
            for col in 0..area.width {
                let idx = usize::from(row) * usize::from(area.width) + usize::from(col);
                let Some(cell) = buf.cell((area.x + col, area.y + row)) else {
                    continue;
                };
                let sym = cell.symbol();
                let style = cell.style();
                if !resized && self.prev_cells[idx].0 == sym && self.prev_cells[idx].1 == style {
                    continue;
                }
                self.prev_cells[idx] = (sym.to_string(), style);
                if last_pos != Some((row, col)) {
                    let _ = write!(ansi, "\x1b[{};{}H", row + 1, col + 1);
                }
                if last_style != Some(style) {
                    write_ansi_style(&mut ansi, style);
                    last_style = Some(style);
                }
                ansi.push_str(if sym.is_empty() { " " } else { sym });
                let adv = unicode_width::UnicodeWidthStr::width(sym).max(1) as u16;
                last_pos = Some((row, col.saturating_add(adv)));
            }
        }

        if !ansi.is_empty() {
            ansi.push_str("\x1b[0m");
            let elapsed = self.started_at.elapsed().as_secs_f64();
            self.events.push((elapsed, ansi));
        }
    }

    /// Flushes the recorded workspace `.cast` stream to disk (`0600` when `private == true`).
    pub fn stop(mut self) -> std::io::Result<PathBuf> {
        if !self.private
            && let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = format!(
            "{{\"version\":2,\"width\":{},\"height\":{},\"timestamp\":1792550000}}\n",
            self.width, self.height
        );
        for (elapsed, chunk) in self.events.drain(..) {
            let escaped = escape_json_string(&chunk);
            out.push_str(&format!("[{elapsed:.6},\"o\",\"{escaped}\"]\n"));
        }
        if self.private {
            use std::io::Write;
            let mut file = crate::secure_fs::create_private_file(&self.path)?;
            file.write_all(out.as_bytes())?;
            file.sync_all()?;
        } else {
            std::fs::write(&self.path, out)?;
        }
        Ok(self.path)
    }
}

fn write_ansi_style(out: &mut String, style: Style) {
    out.push_str("\x1b[0");
    if style.add_modifier.contains(Modifier::BOLD) {
        out.push_str(";1");
    }
    if style.add_modifier.contains(Modifier::DIM) {
        out.push_str(";2");
    }
    if style.add_modifier.contains(Modifier::ITALIC) {
        out.push_str(";3");
    }
    if style.add_modifier.contains(Modifier::UNDERLINED) {
        out.push_str(";4");
    }
    if style.add_modifier.contains(Modifier::REVERSED) {
        out.push_str(";7");
    }
    if let Some(fg) = style.fg {
        write_ansi_color(out, fg, true);
    }
    if let Some(bg) = style.bg {
        write_ansi_color(out, bg, false);
    }
    out.push('m');
}

fn write_ansi_color(out: &mut String, color: Color, is_fg: bool) {
    let base = if is_fg { 30 } else { 40 };
    let bright_base = if is_fg { 90 } else { 100 };
    match color {
        Color::Reset => {}
        Color::Black => {
            let _ = write!(out, ";{}", base);
        }
        Color::Red => {
            let _ = write!(out, ";{}", base + 1);
        }
        Color::Green => {
            let _ = write!(out, ";{}", base + 2);
        }
        Color::Yellow => {
            let _ = write!(out, ";{}", base + 3);
        }
        Color::Blue => {
            let _ = write!(out, ";{}", base + 4);
        }
        Color::Magenta => {
            let _ = write!(out, ";{}", base + 5);
        }
        Color::Cyan => {
            let _ = write!(out, ";{}", base + 6);
        }
        Color::Gray => {
            let _ = write!(out, ";{}", base + 7);
        }
        Color::DarkGray => {
            let _ = write!(out, ";{}", bright_base);
        }
        Color::LightRed => {
            let _ = write!(out, ";{}", bright_base + 1);
        }
        Color::LightGreen => {
            let _ = write!(out, ";{}", bright_base + 2);
        }
        Color::LightYellow => {
            let _ = write!(out, ";{}", bright_base + 3);
        }
        Color::LightBlue => {
            let _ = write!(out, ";{}", bright_base + 4);
        }
        Color::LightMagenta => {
            let _ = write!(out, ";{}", bright_base + 5);
        }
        Color::LightCyan => {
            let _ = write!(out, ";{}", bright_base + 6);
        }
        Color::White => {
            let _ = write!(out, ";{}", bright_base + 7);
        }
        Color::Indexed(idx) => {
            let code = if is_fg { 38 } else { 48 };
            let _ = write!(out, ";{code};5;{idx}");
        }
        Color::Rgb(r, g, b) => {
            let code = if is_fg { 38 } else { 48 };
            let _ = write!(out, ";{code};2;{r};{g};{b}");
        }
    }
}

impl VtParser {
    /// Starts recording terminal output to an asciinema v2 `.cast` file (`v0.7.0`).
    pub fn start_recording(&mut self, path: PathBuf) {
        self.recording_started_at = Some(Instant::now());
        self.recording_path = Some(path);
        self.recording_private = false;
        self.recording_events.clear();
    }

    /// Starts recording terminal output to an asciinema v2 `.cast` file with private permissions (`v1.7.0`, `SEC-R03`).
    pub fn start_recording_private(&mut self, path: PathBuf) {
        self.recording_started_at = Some(Instant::now());
        self.recording_path = Some(path);
        self.recording_private = true;
        self.recording_events.clear();
    }

    /// Cancels any active pane-level recording without writing a file (used when `WorkspaceRecorder` writes the `.cast`).
    pub fn cancel_recording(&mut self) {
        self.recording_started_at = None;
        self.recording_path = None;
        self.recording_private = false;
        self.recording_events.clear();
    }

    /// Returns `true` when asciinema v2 `.cast` recording is active (`v0.7.0`).
    pub fn is_recording(&self) -> bool {
        self.recording_started_at.is_some()
    }

    /// Stops recording and writes the asciinema v2 `.cast` file to disk (`v0.7.0`).
    pub fn stop_recording(&mut self) -> std::io::Result<Option<PathBuf>> {
        self.recording_started_at = None;
        let is_private = std::mem::replace(&mut self.recording_private, false);
        let Some(path) = self.recording_path.take() else {
            return Ok(None);
        };
        if !is_private
            && let Some(parent) = path.parent()
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
        if is_private {
            use std::io::Write;
            let mut file = crate::secure_fs::create_private_file(&path)?;
            file.write_all(out.as_bytes())?;
            file.sync_all()?;
        } else {
            std::fs::write(&path, out)?;
        }
        Ok(Some(path))
    }
}
