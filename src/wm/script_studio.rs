//! SCRIPT STUDIO floating window (Plan 51.2-03, WIN-03): Interactive Script Editor,
//! Single-Step Runner, and Live Action Recorder.

use std::path::{Path, PathBuf};
use ratatui::layout::Rect;

pub use super::script_studio_cmd::{ScriptLogLine, ScriptStudioOp};
use crate::wm::WindowManager;
use crate::wm::agents_list::ListDrag;

/// State of the SCRIPT STUDIO floating window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptStudioState {
    pub active: bool,
    pub focused: bool,
    pub pinned: bool,
    pub docked: bool,
    pub script_name: String,
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub scroll: usize,
    pub step_pc: usize,
    pub recording: bool,
    pub edit_mode: bool,
    pub run_log: Vec<ScriptLogLine>,
    pub float_pos: Option<(u16, u16)>,
    pub float_size: Option<(u16, u16)>,
    pub drag: Option<ListDrag>,
}

impl Default for ScriptStudioState {
    fn default() -> Self {
        Self {
            active: false,
            focused: false,
            pinned: true,
            docked: false,
            script_name: "scratch.bt".into(),
            lines: vec![
                "# Bilterm Script Studio (:colon or !shell per line)".into(),
                ":font status".into(),
            ],
            cursor_line: 0,
            cursor_col: 0,
            scroll: 0,
            step_pc: 0,
            recording: false,
            edit_mode: false,
            run_log: Vec::new(),
            float_pos: None,
            float_size: None,
            drag: None,
        }
    }
}

impl ScriptStudioState {
    pub fn captures_input(&self) -> bool {
        self.active && self.focused
    }

    pub fn blur(&mut self) {
        self.focused = false;
        self.edit_mode = false;
        self.drag = None;
    }

    pub fn record_colon_command(&mut self, colon_line: &str) {
        if self.recording && !colon_line.starts_with(":script") && !colon_line.starts_with(":studio") {
            self.lines.push(colon_line.to_string());
        }
    }
}

/// Resolves directory for script persistence (`~/.config/bilterm/scripts`).
pub fn scripts_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        PathBuf::from(xdg).join("bilterm").join("scripts")
    } else if let Ok(home) = std::env::var("HOME")
        && !home.is_empty()
    {
        PathBuf::from(home).join(".config").join("bilterm").join("scripts")
    } else {
        PathBuf::from(".config/bilterm/scripts")
    }
}

/// Ensures directory path and all missing parents exist with 0700 mode.
pub fn ensure_private_dir_all(path: &Path) -> std::io::Result<()> {
    let mut stack = Vec::new();
    let mut cur = path;
    while !cur.exists() {
        stack.push(cur);
        if let Some(p) = cur.parent() {
            cur = p;
        } else {
            break;
        }
    }
    while let Some(dir) = stack.pop() {
        crate::secure_fs::ensure_private_dir(dir)?;
    }
    crate::secure_fs::ensure_private_dir(path)
}

impl WindowManager {
    pub fn script_studio(&self) -> &ScriptStudioState {
        &self.script_studio
    }

    pub fn script_studio_mut(&mut self) -> &mut ScriptStudioState {
        &mut self.script_studio
    }

    pub fn open_script_studio(&mut self) {
        self.script_studio.active = true;
        self.script_studio.focused = true;
        self.agents_window.blur();
        self.agents_list.blur();
    }

    pub fn close_script_studio(&mut self) {
        self.script_studio.active = false;
        self.script_studio.focused = false;
        self.script_studio.edit_mode = false;
        self.script_studio.drag = None;
    }

    pub fn script_studio_rect(&self, ws: Rect) -> Option<Rect> {
        if !self.script_studio.active || ws.width < 28 || ws.height < 10 {
            return None;
        }
        let (raw_w, raw_h) = self.script_studio.float_size.unwrap_or((66, 20));
        let w = raw_w.min(ws.width.saturating_sub(2)).max(28);
        let h = raw_h.min(ws.height.saturating_sub(2)).max(10);

        if self.script_studio.docked {
            let x = ws.x + ws.width.saturating_sub(w);
            let y = ws.y + 1;
            let dh = ws.height.saturating_sub(2);
            return Some(Rect::new(x, y, w, dh.max(10)));
        }

        let (x, y) = if let Some((px, py)) = self.script_studio.float_pos {
            let max_x = (ws.x + ws.width).saturating_sub(w);
            let max_y = (ws.y + ws.height).saturating_sub(h);
            (px.clamp(ws.x, max_x), py.clamp(ws.y, max_y))
        } else {
            let cx = ws.x + (ws.width.saturating_sub(w)) / 2;
            let cy = ws.y + (ws.height.saturating_sub(h)) / 2;
            (cx, cy)
        };

        Some(Rect::new(x, y, w, h))
    }

    pub fn execute_script_line(&mut self, line_idx: usize, raw_line: &str) -> ScriptLogLine {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return ScriptLogLine {
                line_no: line_idx + 1,
                source: raw_line.to_string(),
                output: "(comment)".to_string(),
                ok: true,
            };
        }

        if trimmed.starts_with(':') {
            match crate::colon_command::parse_colon_command(trimmed) {
                Some(Ok(cmd)) => {
                    self.active_pane_mut().set_status_message(None);
                    match self.apply(cmd) {
                        Ok(_) => {
                            let status = self
                                .active_pane()
                                .status_message()
                                .map(str::to_string)
                                .unwrap_or_else(|| "ok".to_string());
                            ScriptLogLine {
                                line_no: line_idx + 1,
                                source: trimmed.to_string(),
                                output: status,
                                ok: true,
                            }
                        }
                        Err(e) => ScriptLogLine {
                            line_no: line_idx + 1,
                            source: trimmed.to_string(),
                            output: format!("error: {e}"),
                            ok: false,
                        },
                    }
                }
                Some(Err(e)) => ScriptLogLine {
                    line_no: line_idx + 1,
                    source: trimmed.to_string(),
                    output: format!("parse error: {e}"),
                    ok: false,
                },
                None => ScriptLogLine {
                    line_no: line_idx + 1,
                    source: trimmed.to_string(),
                    output: "unknown colon command".to_string(),
                    ok: false,
                },
            }
        } else {
            let cmd = trimmed.strip_prefix('!').unwrap_or(trimmed);
            let payload = format!("{}\n", cmd);
            match self.active_pane_mut().write_input(payload.as_bytes()) {
                Ok(()) => ScriptLogLine {
                    line_no: line_idx + 1,
                    source: trimmed.to_string(),
                    output: "sent to focused pane".to_string(),
                    ok: true,
                },
                Err(e) => ScriptLogLine {
                    line_no: line_idx + 1,
                    source: trimmed.to_string(),
                    output: format!("pty error: {e}"),
                    ok: false,
                },
            }
        }
    }

    pub fn script_studio_apply(&mut self, op: &ScriptStudioOp) -> Option<String> {
        match op {
            ScriptStudioOp::Open => { self.open_script_studio(); Some("SCRIPT STUDIO: open".into()) }
            ScriptStudioOp::Close => { self.close_script_studio(); Some("SCRIPT STUDIO: closed".into()) }
            ScriptStudioOp::Toggle => {
                if self.script_studio.active { self.close_script_studio(); Some("SCRIPT STUDIO: closed".into()) }
                else { self.open_script_studio(); Some("SCRIPT STUDIO: open".into()) }
            }
            ScriptStudioOp::Dismiss => {
                if self.script_studio.pinned { self.script_studio.blur(); Some("SCRIPT STUDIO: blurred (pinned)".into()) }
                else { self.close_script_studio(); Some("SCRIPT STUDIO: dismissed".into()) }
            }
            ScriptStudioOp::Blur => { self.script_studio.blur(); Some("SCRIPT STUDIO: blurred".into()) }
            ScriptStudioOp::Pin(opt) => {
                let pin = opt.unwrap_or(!self.script_studio.pinned);
                self.script_studio.pinned = pin;
                Some(if pin { "SCRIPT STUDIO: pinned" } else { "SCRIPT STUDIO: unpinned" }.into())
            }
            ScriptStudioOp::Dock(opt) => {
                let dock = opt.unwrap_or(!self.script_studio.docked);
                self.script_studio.docked = dock;
                Some(if dock { "SCRIPT STUDIO: docked" } else { "SCRIPT STUDIO: undocked" }.into())
            }
            ScriptStudioOp::RunAll => {
                let lines_to_run: Vec<(usize, String)> = self.script_studio.lines.iter().enumerate()
                    .filter(|(_, l)| { let t = l.trim(); !t.is_empty() && !t.starts_with('#') })
                    .map(|(i, l)| (i, l.clone())).collect();
                let mut ok_count = 0;
                let total = lines_to_run.len();
                for (idx, line) in lines_to_run {
                    let log = self.execute_script_line(idx, &line);
                    if log.ok { ok_count += 1; }
                    self.script_studio.run_log.push(log);
                }
                self.script_studio.step_pc = 0;
                Some(format!("SCRIPT RUN: {ok_count}/{total} step(s) ok"))
            }
            ScriptStudioOp::StepNext => {
                let total_lines = self.script_studio.lines.len();
                if total_lines == 0 { return Some("SCRIPT STEP: no lines".into()); }
                let find_exec = |start: usize| -> Option<usize> {
                    (start..total_lines).chain(0..start)
                        .find(|&i| { let t = self.script_studio.lines[i].trim(); !t.is_empty() && !t.starts_with('#') })
                };
                let target_idx = match find_exec(self.script_studio.step_pc) {
                    Some(i) => i,
                    None => return Some("SCRIPT STEP: no executable lines".into()),
                };
                let raw = self.script_studio.lines[target_idx].clone();
                let log = self.execute_script_line(target_idx, &raw);
                let out = log.output.clone();
                self.script_studio.run_log.push(log);
                self.script_studio.cursor_line = target_idx;
                let next_step = (target_idx + 1..total_lines).chain(0..=target_idx)
                    .find(|&i| { let t = self.script_studio.lines[i].trim(); !t.is_empty() && !t.starts_with('#') })
                    .unwrap_or(0);
                self.script_studio.step_pc = next_step;
                Some(format!("SCRIPT STEP #L{:02}: {}", target_idx + 1, out))
            }
            ScriptStudioOp::ResetStep => {
                self.script_studio.step_pc = 0;
                self.script_studio.run_log.clear();
                Some("SCRIPT STEP: reset".into())
            }
            ScriptStudioOp::Record(opt) => {
                let rec = opt.unwrap_or(!self.script_studio.recording);
                self.script_studio.recording = rec;
                Some(if rec { "SCRIPT RECORDER: ON" } else { "SCRIPT RECORDER: OFF" }.into())
            }
            ScriptStudioOp::EditMode(opt) => {
                let edit = opt.unwrap_or(!self.script_studio.edit_mode);
                self.script_studio.edit_mode = edit;
                Some(if edit { "SCRIPT STUDIO: edit mode" } else { "SCRIPT STUDIO: nav mode" }.into())
            }
            ScriptStudioOp::CursorUp => { self.script_studio.cursor_line = self.script_studio.cursor_line.saturating_sub(1); self.clamp_cursor(); None }
            ScriptStudioOp::CursorDown => {
                if self.script_studio.cursor_line + 1 < self.script_studio.lines.len() { self.script_studio.cursor_line += 1; }
                self.clamp_cursor(); None
            }
            ScriptStudioOp::CursorLeft => { self.script_studio.cursor_col = self.script_studio.cursor_col.saturating_sub(1); None }
            ScriptStudioOp::CursorRight => {
                if self.script_studio.cursor_col < self.current_line_char_count() { self.script_studio.cursor_col += 1; }
                None
            }
            ScriptStudioOp::SelectLine(idx) => {
                if *idx < self.script_studio.lines.len() { self.script_studio.cursor_line = *idx; self.script_studio.cursor_col = 0; }
                None
            }
            ScriptStudioOp::InsertChar(c) => {
                if self.script_studio.lines.is_empty() { self.script_studio.lines.push(String::new()); }
                let line_idx = self.script_studio.cursor_line.min(self.script_studio.lines.len() - 1);
                let line = &mut self.script_studio.lines[line_idx];
                let mut chars: Vec<char> = line.chars().collect();
                let col = self.script_studio.cursor_col.min(chars.len());
                chars.insert(col, *c);
                *line = chars.into_iter().collect();
                self.script_studio.cursor_col = col + 1;
                None
            }
            ScriptStudioOp::Backspace => {
                if !self.script_studio.lines.is_empty() {
                    let line_idx = self.script_studio.cursor_line.min(self.script_studio.lines.len() - 1);
                    if self.script_studio.cursor_col > 0 {
                        let line = &mut self.script_studio.lines[line_idx];
                        let mut chars: Vec<char> = line.chars().collect();
                        let col = self.script_studio.cursor_col.min(chars.len());
                        chars.remove(col - 1);
                        *line = chars.into_iter().collect();
                        self.script_studio.cursor_col = col - 1;
                    } else if line_idx > 0 {
                        let cur = self.script_studio.lines.remove(line_idx);
                        self.script_studio.cursor_line = line_idx - 1;
                        let prev_len = self.script_studio.lines[line_idx - 1].chars().count();
                        self.script_studio.lines[line_idx - 1].push_str(&cur);
                        self.script_studio.cursor_col = prev_len;
                    }
                }
                None
            }
            ScriptStudioOp::NewLine => {
                if self.script_studio.lines.is_empty() {
                    self.script_studio.lines.push(String::new());
                    self.script_studio.cursor_line = 0;
                    self.script_studio.cursor_col = 0;
                } else {
                    let line_idx = self.script_studio.cursor_line.min(self.script_studio.lines.len() - 1);
                    let line = &self.script_studio.lines[line_idx];
                    let chars: Vec<char> = line.chars().collect();
                    let col = self.script_studio.cursor_col.min(chars.len());
                    let left: String = chars[..col].iter().collect();
                    let right: String = chars[col..].iter().collect();
                    self.script_studio.lines[line_idx] = left;
                    self.script_studio.lines.insert(line_idx + 1, right);
                    self.script_studio.cursor_line = line_idx + 1;
                    self.script_studio.cursor_col = 0;
                }
                None
            }
            ScriptStudioOp::DeleteLine => {
                if self.script_studio.lines.len() > 1 {
                    let line_idx = self.script_studio.cursor_line.min(self.script_studio.lines.len() - 1);
                    self.script_studio.lines.remove(line_idx);
                    self.script_studio.cursor_line = self.script_studio.cursor_line.min(self.script_studio.lines.len() - 1);
                    self.script_studio.cursor_col = 0;
                } else if self.script_studio.lines.len() == 1 {
                    self.script_studio.lines[0].clear();
                    self.script_studio.cursor_col = 0;
                }
                None
            }
            ScriptStudioOp::AppendLine(s) => {
                self.script_studio.lines.push(s.clone());
                self.script_studio.cursor_line = self.script_studio.lines.len() - 1;
                self.script_studio.cursor_col = self.script_studio.lines.last().map_or(0, |l| l.chars().count());
                None
            }
            ScriptStudioOp::ClearScript => {
                self.script_studio.lines.clear();
                self.script_studio.cursor_line = 0;
                self.script_studio.cursor_col = 0;
                self.script_studio.step_pc = 0;
                Some("SCRIPT STUDIO: script cleared".into())
            }
            ScriptStudioOp::ClearLog => {
                self.script_studio.run_log.clear();
                Some("SCRIPT STUDIO: log cleared".into())
            }
            ScriptStudioOp::Save(opt_name) => {
                let default_name = self.script_studio.script_name.trim_end_matches(".bt").to_string();
                let name = opt_name.as_deref().unwrap_or(&default_name);
                if let Err(e) = crate::wm::journal::validate_safe_name(name) {
                    return Some(format!("SCRIPT SAVE ERROR: {e}"));
                }
                let dir = scripts_dir();
                if let Err(e) = ensure_private_dir_all(&dir) {
                    return Some(format!("SCRIPT SAVE ERROR: {e}"));
                }
                let file_path = dir.join(format!("{name}.bt"));
                let content = self.script_studio.lines.join("\n");
                if let Err(e) = crate::secure_fs::write_private_file(&file_path, content.as_bytes()) {
                    return Some(format!("SCRIPT SAVE ERROR: {e}"));
                }
                self.script_studio.script_name = format!("{name}.bt");
                Some(format!("SCRIPT SAVED: {name}.bt"))
            }
            ScriptStudioOp::Load(name) => {
                let clean_name = name.trim_end_matches(".bt");
                if let Err(e) = crate::wm::journal::validate_safe_name(clean_name) {
                    return Some(format!("SCRIPT LOAD ERROR: {e}"));
                }
                let dir = scripts_dir();
                let file_path = dir.join(format!("{clean_name}.bt"));
                match std::fs::read_to_string(&file_path) {
                    Ok(text) => {
                        self.script_studio.lines = text.lines().map(String::from).collect();
                        self.script_studio.script_name = format!("{clean_name}.bt");
                        self.script_studio.cursor_line = 0;
                        self.script_studio.cursor_col = 0;
                        self.script_studio.step_pc = 0;
                        Some(format!("SCRIPT LOADED: {clean_name}.bt ({} lines)", self.script_studio.lines.len()))
                    }
                    Err(e) => Some(format!("SCRIPT LOAD ERROR: {e}")),
                }
            }
            ScriptStudioOp::ListSaved => {
                let dir = scripts_dir();
                match std::fs::read_dir(&dir) {
                    Ok(entries) => {
                        let mut names = Vec::new();
                        for entry in entries.flatten() {
                            if let Some(s) = entry.file_name().to_str().filter(|s| s.ends_with(".bt")) {
                                names.push(s.to_string());
                            }
                        }
                        names.sort();
                        if names.is_empty() {
                            Some("No saved scripts found".into())
                        } else {
                            Some(format!("Saved scripts ({}): {}", names.len(), names.join(", ")))
                        }
                    }
                    Err(_) => Some("No saved scripts found".into()),
                }
            }
            ScriptStudioOp::Move { dx, dy } => {
                let (cx, cy) = self.script_studio.float_pos.unwrap_or((10, 5));
                self.script_studio.float_pos = Some(((cx as i32 + *dx as i32).max(0) as u16, (cy as i32 + *dy as i32).max(0) as u16));
                None
            }
            ScriptStudioOp::Resize { dw, dh } => {
                let (cw, ch) = self.script_studio.float_size.unwrap_or((66, 20));
                self.script_studio.float_size = Some(((cw as i32 + *dw as i32).max(28) as u16, (ch as i32 + *dh as i32).max(10) as u16));
                None
            }
            ScriptStudioOp::SetRect { x, y, w, h } => {
                self.script_studio.float_pos = Some((*x, *y));
                self.script_studio.float_size = Some((*w, *h));
                None
            }
        }
    }

    fn clamp_cursor(&mut self) {
        let max_col = self.current_line_char_count();
        if self.script_studio.cursor_col > max_col {
            self.script_studio.cursor_col = max_col;
        }
    }

    fn current_line_char_count(&self) -> usize {
        self.script_studio.lines.get(self.script_studio.cursor_line).map_or(0, |l| l.chars().count())
    }
}
