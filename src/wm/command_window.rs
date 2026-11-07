//! First-Class Pinnable COMMAND PALETTE Window (WIN-02, Phase 51.2-02).

use ratatui::layout::Rect;
use std::sync::LazyLock;

use crate::wm::WindowManager;
use crate::wm::WindowResizeEdge;
use crate::wm::agents_list::ListDrag;
use crate::wm::rect_contains;

/// Category mapping for colon commands.
pub fn category_for_cmd(cmd: &str) -> &'static str {
    let head = cmd.trim_start_matches(':').split_whitespace().next().unwrap_or("");
    match head {
        "split" | "vsplit" | "zoom" | "close" | "layout" | "resize" | "move" | "deck" | "arrange" | "cascade" | "equalize" => "Window",
        "profile" | "session" | "attach" | "detach" | "save" | "restore" => "Session",
        "ssh" | "ssh-tmux" | "remote" => "Remote",
        "mcp" => "MCP",
        "agents" => "Agents",
        "drawer" | "navigator" => "Drawer",
        "modern" | "liquid" | "snazzy" | "glass" | "retro" | "theme" | "font" | "appearance" | "style" | "shadows" | "outline" | "border" => "Appearance",
        "commands" | "palette" | "help" | "fkeys" => "System",
        "record" | "asciinema" | "rec" => "Record",
        "exit-guard" | "respawn-pane" | "quit" | "exit" => "Process",
        "history" | "scroll" | "select" | "rect" | "copy" | "hints" | "find" | "search" => "Pane",
        _ => "General",
    }
}

/// Catalog item representing a command palette entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteCommandItem {
    pub category: &'static str,
    pub title: &'static str,
    pub colon_command: &'static str,
    pub shortcut: &'static str,
}

/// Globally cached command palette items.
pub static PALETTE_ITEMS: LazyLock<Vec<PaletteCommandItem>> = LazyLock::new(|| {
    use crate::ui::modals::PALETTE_CATALOG;
    PALETTE_CATALOG.iter().map(|&(cmd, shortcut, desc)| PaletteCommandItem {
        category: category_for_cmd(cmd),
        title: desc,
        colon_command: cmd,
        shortcut,
    }).collect()
});

/// Operation on the Command Palette window.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CommandWindowOp {
    Open,
    Close,
    Toggle,
    Dismiss,
    Blur,
    Pin(Option<bool>),
    Dock(Option<bool>),
    Step(i32),
    Page(i32),
    First,
    Last,
    Scroll(i32),
    SelectRow(usize),
    QueryChar(char),
    QueryBackspace,
    QueryClear,
    SetQuery(String),
    ExecuteSelected,
    ExecuteCommand(String),
    ClearLog,
    Move { dx: i16, dy: i16 },
    Resize { dw: i16, dh: i16 },
    SetRect { x: u16, y: u16, w: u16, h: u16 },
}

impl CommandWindowOp {
    /// Canonical colon form for command dispatch.
    pub fn colon_form(&self) -> String {
        let p = ":commands";
        match self {
            Self::Open => format!("{p} open"),
            Self::Close => format!("{p} close"),
            Self::Toggle => format!("{p} toggle"),
            Self::Dismiss => format!("{p} dismiss"),
            Self::Blur => format!("{p} blur"),
            Self::Pin(Some(true)) => format!("{p} pin"),
            Self::Pin(Some(false)) => format!("{p} unpin"),
            Self::Pin(None) => format!("{p} pin toggle"),
            Self::Dock(Some(true)) => format!("{p} dock"),
            Self::Dock(Some(false)) => format!("{p} float"),
            Self::Dock(None) => format!("{p} dock toggle"),
            Self::Step(n) if *n < 0 => format!("{p} select up"),
            Self::Step(_) => format!("{p} select down"),
            Self::Page(n) if *n < 0 => format!("{p} select page-up"),
            Self::Page(_) => format!("{p} select page-down"),
            Self::First => format!("{p} select first"),
            Self::Last => format!("{p} select last"),
            Self::Scroll(n) => format!("{p} scroll {n:+}"),
            Self::SelectRow(idx) => format!("{p} select-row {idx}"),
            Self::QueryChar(c) => format!("{p} query-char {c}"),
            Self::QueryBackspace => format!("{p} query-backspace"),
            Self::QueryClear => format!("{p} query-clear"),
            Self::SetQuery(q) => format!("{p} query {q}"),
            Self::ExecuteSelected => format!("{p} run"),
            Self::ExecuteCommand(cmd) => format!("{p} run {cmd}"),
            Self::ClearLog => format!("{p} clear"),
            Self::Move { dx, dy } => format!("{p} move {dx} {dy}"),
            Self::Resize { dw, dh } => format!("{p} resize {dw} {dh}"),
            Self::SetRect { x, y, w, h } => format!("{p} rect {x} {y} {w} {h}"),
        }
    }
}

/// One entry in the command output and history log.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CommandLogEntry {
    pub command: String,
    pub status: String,
    pub ok: bool,
}

/// State of the pinnable Command Palette window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandWindowState {
    pub active: bool,
    pub focused: bool,
    pub pinned: bool,
    pub docked: bool,
    pub query: String,
    pub selected: usize,
    pub scroll: usize,
    pub history_log: Vec<CommandLogEntry>,
    pub float_pos: Option<(u16, u16)>,
    pub float_size: Option<(u16, u16)>,
    pub drag: Option<ListDrag>,
}

impl CommandWindowState {
    pub fn captures_input(&self) -> bool {
        self.active && self.focused
    }

    pub fn blur(&mut self) {
        self.focused = false;
        self.drag = None;
    }

    pub fn push_log(&mut self, command: String, status: String, ok: bool) {
        self.history_log.push(CommandLogEntry { command, status, ok });
        if self.history_log.len() > 64 {
            self.history_log.remove(0);
        }
    }
}

/// Title bar badges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandWindowBadge {
    Pin,
    Dock,
    Close,
}

/// Hit-testing results for the command window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandWindowHit {
    Outside,
    Title,
    Badge(CommandWindowBadge),
    Border(WindowResizeEdge),
    InputRow,
    Row(usize),
    HistoryRow(usize),
    Footer,
}

fn border_edge(outer: Rect, col: u16, row: u16) -> Option<WindowResizeEdge> {
    if !rect_contains(outer, col, row) { return None; }
    let (t, b, l, r) = (row == outer.y, row == outer.y + outer.height.saturating_sub(1), col == outer.x, col == outer.x + outer.width.saturating_sub(1));
    match (t, b, l, r) {
        (true, _, true, _) => Some(WindowResizeEdge::TopLeft),
        (true, _, _, true) => Some(WindowResizeEdge::TopRight),
        (_, true, true, _) => Some(WindowResizeEdge::BottomLeft),
        (_, true, _, true) => Some(WindowResizeEdge::BottomRight),
        (true, ..) => Some(WindowResizeEdge::Top),
        (_, true, ..) => Some(WindowResizeEdge::Bottom),
        (.., true, _) => Some(WindowResizeEdge::Left),
        (.., true) => Some(WindowResizeEdge::Right),
        _ => None,
    }
}

impl WindowManager {
    pub fn command_window(&self) -> &CommandWindowState { &self.command_window }
    pub fn command_window_mut(&mut self) -> &mut CommandWindowState { &mut self.command_window }

    pub fn open_command_window(&mut self) {
        self.command_window.active = true;
        self.command_window.focused = true;
        self.agents_list.blur();
        self.agents_window.blur();
        self.fonts_window.blur();
        self.script_studio.blur();
        self.session_map.blur();
    }

    pub fn close_command_window(&mut self) {
        self.command_window.active = false;
        self.command_window.focused = false;
        self.command_window.drag = None;
    }

    pub fn command_window_items(&self) -> Vec<(usize, &'static PaletteCommandItem)> {
        self.filtered_palette_items_for(&self.command_window.query)
    }

    pub fn filtered_palette_items_for(&self, query: &str) -> Vec<(usize, &'static PaletteCommandItem)> {
        use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
        use nucleo_matcher::{Config, Matcher, Utf32Str};

        let q = query.trim();
        if q.is_empty() {
            return PALETTE_ITEMS.iter().enumerate().collect();
        }
        let q_lower = q.to_lowercase();
        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(&q_lower, CaseMatching::Ignore, Normalization::Smart);
        let mut scored: Vec<(u32, usize, &'static PaletteCommandItem)> = PALETTE_ITEMS
            .iter()
            .enumerate()
            .filter_map(|(idx, item)| {
                let combined = format!("{} {} {} {}", item.category, item.colon_command, item.shortcut, item.title);
                let score_opt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut hay_buf = Vec::new();
                    let hay = Utf32Str::new(&combined, &mut hay_buf);
                    pattern.score(hay, &mut matcher)
                }))
                .ok()
                .flatten()
                .or_else(|| fallback_score(&combined, &q_lower));
                score_opt.map(|score| (score, idx, item))
            })
            .collect();
        scored.sort_by_key(|a| std::cmp::Reverse(a.0));
        scored.into_iter().map(|(_, idx, item)| (idx, item)).collect()
    }

    pub fn command_window_rect(&self, ws: Rect) -> Option<Rect> {
        if !self.command_window.active || ws.width < 24 || ws.height < 8 { return None; }
        let width = self.command_window.float_size.map(|(w, _)| w.clamp(24, ws.width.saturating_sub(2)))
            .unwrap_or_else(|| 64.min(ws.width.saturating_sub(2)).max(24.min(ws.width)));
        let height = self.command_window.float_size.map(|(_, h)| h.clamp(8, ws.height.saturating_sub(2)))
            .unwrap_or_else(|| 20.min(ws.height.saturating_sub(2)).max(8.min(ws.height)));
        let x = self.command_window.float_pos.map(|(px, _)| px.min(ws.width.saturating_sub(width)) + ws.x)
            .unwrap_or_else(|| ws.x + ws.width.saturating_sub(width) / 2);
        let y = self.command_window.float_pos.map(|(_, py)| py.min(ws.height.saturating_sub(height)) + ws.y)
            .unwrap_or_else(|| ws.y + ws.height.saturating_sub(height) / 2);
        Some(Rect::new(x, y, width, height))
    }

    pub fn command_window_hit(&self, ws: Rect, col: u16, row: u16) -> CommandWindowHit {
        let Some(outer) = self.command_window_rect(ws) else { return CommandWindowHit::Outside; };
        if !rect_contains(outer, col, row) { return CommandWindowHit::Outside; }
        if row == outer.y {
            if col + 4 >= outer.x + outer.width && col < outer.x + outer.width - 1 { return CommandWindowHit::Badge(CommandWindowBadge::Close); }
            if col + 8 >= outer.x + outer.width && col < outer.x + outer.width - 5 { return CommandWindowHit::Badge(CommandWindowBadge::Dock); }
            if col + 12 >= outer.x + outer.width && col < outer.x + outer.width - 9 { return CommandWindowHit::Badge(CommandWindowBadge::Pin); }
            return CommandWindowHit::Title;
        }
        if let Some(edge) = border_edge(outer, col, row) { return CommandWindowHit::Border(edge); }
        let inner = Rect::new(outer.x + 1, outer.y + 1, outer.width.saturating_sub(2), outer.height.saturating_sub(2));
        if row == inner.y { return CommandWindowHit::InputRow; }
        if row == inner.y + inner.height.saturating_sub(1) { return CommandWindowHit::Footer; }
        let hist_h = if inner.height >= 12 { 4 } else { 2 };
        let div_y = (inner.y + inner.height).saturating_sub(hist_h + 2);
        if row == div_y { return CommandWindowHit::Outside; }
        if row > div_y && row < inner.y + inner.height.saturating_sub(1) {
            return CommandWindowHit::HistoryRow((row - div_y - 1) as usize);
        }
        CommandWindowHit::Row(self.command_window.scroll + (row.saturating_sub(inner.y + 1)) as usize)
    }

    fn execute_command_string(&mut self, raw: &str) -> Option<String> {
        let cmd_str = if raw.trim().starts_with(':') { raw.trim().to_string() } else { format!(":{}", raw.trim()) };
        let (ok, status) = match crate::colon_command::parse_colon_command(&cmd_str) {
            Some(Ok(cmd)) => match self.apply(cmd) {
                Ok(_) => (true, self.active_pane().status_message().unwrap_or("OK").to_string()),
                Err(e) => (false, format!("ERR: {e}")),
            },
            Some(Err(err)) => (false, err),
            None => (false, format!("Unknown command: {cmd_str}")),
        };
        self.command_window.push_log(cmd_str, status.clone(), ok);
        if !self.command_window.pinned { self.close_command_window(); } else { self.command_window.active = true; self.command_window.focused = true; }
        Some(status)
    }

    fn reset_sel(&mut self) { self.command_window.selected = 0; self.command_window.scroll = 0; }
    fn adjust_scroll(&mut self) {
        if self.command_window.selected < self.command_window.scroll {
            self.command_window.scroll = self.command_window.selected;
        } else if self.command_window.selected >= self.command_window.scroll + 8 {
            self.command_window.scroll = self.command_window.selected.saturating_sub(7);
        }
    }

    pub fn command_window_apply(&mut self, op: &CommandWindowOp) -> Option<String> {
        match op {
            CommandWindowOp::Open => { self.open_command_window(); None }
            CommandWindowOp::Close => { self.close_command_window(); None }
            CommandWindowOp::Toggle => { if self.command_window.active { self.close_command_window(); } else { self.open_command_window(); } None }
            CommandWindowOp::Dismiss => {
                if self.command_window.pinned {
                    self.command_window.blur();
                    Some("COMMAND WINDOW: released focus (pinned)".to_string())
                } else {
                    self.close_command_window();
                    None
                }
            }
            CommandWindowOp::Blur => { self.command_window.blur(); None }
            CommandWindowOp::Pin(opt) => {
                let next = opt.unwrap_or(!self.command_window.pinned);
                self.command_window.pinned = next;
                Some(format!("COMMAND WINDOW: pinned={}", self.command_window.pinned))
            }
            CommandWindowOp::Dock(opt) => {
                let next = opt.unwrap_or(!self.command_window.docked);
                self.command_window.docked = next;
                Some(format!("COMMAND WINDOW: docked={}", self.command_window.docked))
            }
            CommandWindowOp::Step(n) => {
                let len = self.command_window_items().len();
                if len > 0 {
                    let cur = self.command_window.selected as i64;
                    self.command_window.selected = (cur + *n as i64).clamp(0, (len - 1) as i64) as usize;
                    self.adjust_scroll();
                }
                None
            }
            CommandWindowOp::Page(n) => {
                let len = self.command_window_items().len();
                if len > 0 {
                    let cur = self.command_window.selected as i64;
                    self.command_window.selected = (cur + (*n as i64) * 8).clamp(0, (len - 1) as i64) as usize;
                    self.adjust_scroll();
                }
                None
            }
            CommandWindowOp::First => { self.reset_sel(); None }
            CommandWindowOp::Last => {
                let len = self.command_window_items().len();
                self.command_window.selected = len.saturating_sub(1);
                self.adjust_scroll();
                None
            }
            CommandWindowOp::Scroll(n) => {
                let cur = self.command_window.scroll as i64;
                self.command_window.scroll = (cur + *n as i64).max(0) as usize;
                None
            }
            CommandWindowOp::SelectRow(idx) => {
                let len = self.command_window_items().len();
                if len > 0 {
                    self.command_window.selected = (*idx).min(len - 1);
                    self.adjust_scroll();
                }
                None
            }
            CommandWindowOp::QueryChar(c) => { self.command_window.query.push(*c); self.reset_sel(); None }
            CommandWindowOp::QueryBackspace => { self.command_window.query.pop(); self.reset_sel(); None }
            CommandWindowOp::QueryClear => { self.command_window.query.clear(); self.reset_sel(); None }
            CommandWindowOp::SetQuery(q) => { self.command_window.query = q.clone(); self.reset_sel(); None }
            CommandWindowOp::ExecuteSelected => {
                let raw = if self.command_window.query.trim().starts_with(':') {
                    self.command_window.query.trim().to_string()
                } else {
                    let items = self.command_window_items();
                    if let Some((_, item)) = items.get(self.command_window.selected) {
                        item.colon_command.to_string()
                    } else if !self.command_window.query.trim().is_empty() {
                        format!(":{}", self.command_window.query.trim())
                    } else {
                        return None;
                    }
                };
                self.execute_command_string(&raw)
            }
            CommandWindowOp::ExecuteCommand(raw) => self.execute_command_string(raw),
            CommandWindowOp::ClearLog => {
                self.command_window.history_log.clear();
                Some("COMMAND WINDOW: log cleared".to_string())
            }
            CommandWindowOp::Move { dx, dy } => {
                let ws = Rect::new(0, 0, 120, 40);
                let cur = self.command_window_rect(ws).unwrap_or(Rect::new(28, 10, 64, 20));
                let nx = (cur.x as i32 + *dx as i32).max(0) as u16;
                let ny = (cur.y as i32 + *dy as i32).max(0) as u16;
                self.command_window.float_pos = Some((nx, ny));
                None
            }
            CommandWindowOp::Resize { dw, dh } => {
                let ws = Rect::new(0, 0, 120, 40);
                let cur = self.command_window_rect(ws).unwrap_or(Rect::new(28, 10, 64, 20));
                let nw = (cur.width as i32 + *dw as i32).max(24) as u16;
                let nh = (cur.height as i32 + *dh as i32).max(8) as u16;
                self.command_window.float_size = Some((nw, nh));
                None
            }
            CommandWindowOp::SetRect { x, y, w, h } => {
                self.command_window.float_pos = Some((*x, *y));
                self.command_window.float_size = Some((*w, *h));
                None
            }
        }
    }
}

fn fallback_score(haystack: &str, needle_lower: &str) -> Option<u32> {
    let hay_lower = haystack.to_lowercase();
    if hay_lower.contains(needle_lower) { return Some(100); }
    let mut hay_chars = hay_lower.chars();
    needle_lower.chars().all(|nc| hay_chars.any(|hc| hc == nc)).then_some(10)
}
