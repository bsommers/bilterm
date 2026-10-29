//! TRON-Inspired Master Control Program (`MCP`) Root Terminal (`MCP-01`, `MCP-02`).
//!
//! Provides the top-level supervisor console (`bilterm mcp` / `:mcp`) that inspects and
//! orchestrates the background [`DaemonServerState`] and [`SessionTree`] (`status`, `tree`,
//! `attach`, `spawn`, `graft`, `detach-subtree`, `broadcast`, `freeze`, `thaw`, `derez`,
//! `checkpoint`, `shutdown --confirm`).
//!
//! Closing the MCP root console (`close_root_terminal`) never terminates running child sessions;
//! only `derez <target> --confirm` or `shutdown --confirm` tears down sessions.

use ratatui::layout::Rect;

use crate::daemon::{DaemonRequest, DaemonResponse, DaemonServerState};
pub use crate::ui::mcp_ui::render_mcp_console;
use crate::wm::session_tree::SessionNodeKind;
use crate::wm::{DrawerEdge, DrawerLayoutMode};

/// Interactive target bound to a single line in the `:mcp` console window (`MCP-03`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum McpRowAction {
    #[default]
    None,
    /// Focus and raise a workspace pane (`Pane #<id>`).
    FocusPane(usize),
    /// Attach/switch to a session node in the `SessionTree`.
    SessionNode(String),
    /// Apply a theme preset or import file (`:theme <name>` / `import <path>`).
    ApplyTheme(String),
    /// Navigate the interactive File/Directory Browser into a directory.
    BrowseDir(std::path::PathBuf),
    /// Import and register all theme files inside a directory.
    ImportAllDir(std::path::PathBuf),
    /// Preview a text file inside the interactive File Browser.
    PreviewFile(std::path::PathBuf),
}

/// Infers an interactive [`McpRowAction`] from a formatted `:mcp` / `:session` / `:theme` console line (`MCP-03`).
pub fn infer_row_action(line: &str) -> McpRowAction {
    for marker in ["Pane #", "PANE #", "[#"] {
        if let Some(pos) = line.find(marker) {
            let after = &line[pos + marker.len()..];
            let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
            if let Ok(id) = digits.parse::<usize>() {
                return McpRowAction::FocusPane(id);
            }
        }
    }

    if let Some(rest) = line
        .strip_prefix("  • ")
        .or_else(|| line.strip_prefix("► • "))
    {
        if let Some((cmd_part, _)) = rest.split_once(" — ") {
            let trimmed_cmd = cmd_part.trim();
            if trimmed_cmd.starts_with("import ") || trimmed_cmd.starts_with("cursor ") {
                return McpRowAction::ApplyTheme(trimmed_cmd.to_string());
            }
        }
        if let Some(tok) = rest.split_whitespace().next()
            && !tok.is_empty()
        {
            return McpRowAction::ApplyTheme(tok.to_string());
        }
    }

    let trimmed = line.trim_start();
    if trimmed.starts_with("◈ ROOT")
        || trimmed.starts_with("◈ MASTER CONTROL")
        || trimmed.starts_with("DAEMON")
        || trimmed.starts_with("WORKSPACE")
        || trimmed.starts_with("SOCKET")
        || trimmed.starts_with("PROTECTION")
        || trimmed.starts_with("AESTHETICS")
        || trimmed.contains("END OF LINE.")
    {
        return McpRowAction::None;
    }

    let is_session_line = line.contains("[root]")
        || line.contains("[branch]")
        || line.contains("[MCP]")
        || line.contains("[SUB]")
        || line.contains("[IND]")
        || trimmed.starts_with("◈ ")
        || trimmed.starts_with("├─ ")
        || trimmed.starts_with("└─ ")
        || trimmed.starts_with("╰─ ")
        || trimmed.starts_with("│");

    if is_session_line {
        let stripped = line.trim_start_matches(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '│' | '├' | '╰' | '└' | '─' | '•' | '▸' | '▾' | '●' | '◈' | '►'
                )
        });
        let raw_token = stripped
            .split(|c: char| c.is_whitespace() || c == '[')
            .next()
            .unwrap_or("")
            .trim_end_matches('*');
        if !raw_token.is_empty() && !raw_token.eq_ignore_ascii_case("root") {
            return McpRowAction::SessionNode(raw_token.to_string());
        }
    }

    McpRowAction::None
}

/// Parsed command inside the Master Control Program (`MCP`) root console (`MCP-01`, `MCP-02`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpCommand {
    Status,
    Tree,
    Attach { node: String },
    Spawn { path: String },
    Graft { src: String, dst: String },
    DetachSubtree { node: String },
    Broadcast { subtree: String, command: String },
    Freeze { target: String },
    Thaw { target: String },
    Derez { target: String, confirmed: bool },
    Checkpoint,
    Shutdown { confirmed: bool },
    Help,
}

/// Parses an `MCP` command string (with or without leading `:mcp` / `mcp` prefix) (`MCP-02`).
pub fn parse_mcp_command(input: &str) -> Result<McpCommand, String> {
    let trimmed = input.trim().trim_start_matches(':').trim();
    if trimmed.is_empty() {
        return Ok(McpCommand::Status);
    }
    let body = if let Some(rest) = trimmed.strip_prefix("mcp") {
        if rest.is_empty() {
            return Ok(McpCommand::Status);
        }
        if !rest.starts_with(char::is_whitespace) {
            trimmed
        } else {
            rest.trim()
        }
    } else {
        trimmed
    };

    let mut parts = body.split_whitespace();
    let Some(verb_raw) = parts.next() else {
        return Ok(McpCommand::Status);
    };
    let verb = verb_raw.to_ascii_lowercase();

    match verb.as_str() {
        "status" | "stat" | "ping" => Ok(McpCommand::Status),
        "tree" | "ls" | "list" => Ok(McpCommand::Tree),
        "help" | "?" => Ok(McpCommand::Help),
        "checkpoint" | "save" => Ok(McpCommand::Checkpoint),
        "attach" => {
            let node = parts
                .next()
                .ok_or_else(|| "usage: mcp attach <node>".to_string())?
                .to_string();
            Ok(McpCommand::Attach { node })
        }
        "spawn" | "branch" | "new" => {
            let first = parts
                .next()
                .ok_or_else(|| "usage: mcp spawn <parent/child|name>".to_string())?;
            let path = if let Some(second) = parts.next() {
                format!("{first}/{second}")
            } else {
                first.to_string()
            };
            Ok(McpCommand::Spawn { path })
        }
        "graft" => {
            let src = parts
                .next()
                .ok_or_else(|| "usage: mcp graft <src> <dst>".to_string())?
                .to_string();
            let dst = parts
                .next()
                .ok_or_else(|| "usage: mcp graft <src> <dst>".to_string())?
                .to_string();
            Ok(McpCommand::Graft { src, dst })
        }
        "detach-subtree" | "detach" | "promote" => {
            let node = parts
                .next()
                .ok_or_else(|| "usage: mcp detach-subtree <node>".to_string())?
                .to_string();
            Ok(McpCommand::DetachSubtree { node })
        }
        "broadcast" | "bcast" => {
            let subtree = parts
                .next()
                .ok_or_else(|| "usage: mcp broadcast <subtree> <cmd...>".to_string())?
                .to_string();
            let cmd_vec: Vec<&str> = parts.collect();
            if cmd_vec.is_empty() {
                return Err("usage: mcp broadcast <subtree> <cmd...>".to_string());
            }
            Ok(McpCommand::Broadcast {
                subtree,
                command: cmd_vec.join(" "),
            })
        }
        "freeze" => {
            let target = parts
                .next()
                .ok_or_else(|| "usage: mcp freeze <target>".to_string())?
                .to_string();
            Ok(McpCommand::Freeze { target })
        }
        "thaw" | "unfreeze" => {
            let target = parts
                .next()
                .ok_or_else(|| "usage: mcp thaw <target>".to_string())?
                .to_string();
            Ok(McpCommand::Thaw { target })
        }
        "derez" | "kill-subtree" => {
            let mut target: Option<String> = None;
            let mut confirmed = false;
            for tok in parts {
                if tok == "--confirm" || tok == "-y" || tok == "--force" {
                    confirmed = true;
                } else if target.is_none() {
                    target = Some(tok.to_string());
                }
            }
            let target = target.ok_or_else(|| "usage: mcp derez <target> [--confirm]".to_string())?;
            Ok(McpCommand::Derez { target, confirmed })
        }
        "shutdown" => {
            let confirmed = parts.any(|tok| tok == "--confirm" || tok == "-y" || tok == "--force");
            Ok(McpCommand::Shutdown { confirmed })
        }
        other => Err(format!(
            "unknown MCP directive '{other}' (try 'mcp help'). END OF LINE."
        )),
    }
}

/// Interactive state for the Master Control Program (`MCP`) root terminal & pop-up/push inspector (`MCP-01..03`).
#[derive(Clone, Debug)]
pub struct McpConsoleState {
    pub active: bool,
    pub title: String,
    pub view_lines: Vec<String>,
    pub selected_row: usize,
    pub row_actions: Vec<McpRowAction>,
    pub scroll_offset: usize,
    /// `DrawerLayoutMode::Overlay` (Floating pop-up window) vs `DrawerLayoutMode::Push` (pushes workspace panes).
    pub layout_mode: DrawerLayoutMode,
    /// Dock edge (`Top`, `Bottom`, `Left`, `Right`) used when `layout_mode == DrawerLayoutMode::Push`.
    pub edge: DrawerEdge,
    pub log_lines: Vec<String>,
    pub last_status_banner: String,
    /// Active interactive file/directory browser state when `:browse` / `:theme import` browser is open.
    pub file_browser: Option<crate::ui::file_browser::FileBrowserState>,
}

impl Default for McpConsoleState {
    fn default() -> Self {
        Self::new()
    }
}

impl McpConsoleState {
    /// Creates a new [`McpConsoleState`] with the initial TRON greeting banner.
    pub fn new() -> Self {
        Self {
            active: false,
            title: "MASTER CONTROL PROGRAM [MCP ROOT CONSOLE]".to_string(),
            view_lines: Vec::new(),
            selected_row: 0,
            row_actions: Vec::new(),
            scroll_offset: 0,
            layout_mode: DrawerLayoutMode::Overlay,
            edge: DrawerEdge::Top,
            log_lines: vec![
                "◈ MASTER CONTROL PROGRAM [MCP ROOT CONSOLE] ONLINE. END OF LINE.".to_string(),
            ],
            last_status_banner: "MCP ROOT READY — END OF LINE.".to_string(),
            file_browser: None,
        }
    }

    /// Opens the MCP root terminal overlay.
    pub fn open_root_terminal(&mut self) {
        self.active = true;
        self.file_browser = None;
        self.scroll_offset = 0;
    }

    /// Opens the MCP floating/push window with a specific title and multi-line content view,
    /// automatically inferring interactive [`McpRowAction`] targets per row (`MCP-03`).
    pub fn open_with_view(&mut self, title: impl Into<String>, lines: Vec<String>) {
        self.file_browser = None;
        let actions: Vec<McpRowAction> = lines.iter().map(|l| infer_row_action(l)).collect();
        self.open_with_interactive_view(title, lines, actions);
    }

    /// Opens the interactive File & Directory Browser at `dir` with optional `filter` (`THM-01`).
    pub fn open_file_browser(
        &mut self,
        dir: std::path::PathBuf,
        filter: Option<String>,
    ) {
        let fb = crate::ui::file_browser::FileBrowserState::new(dir, filter);
        let (title, lines, actions) = fb.build_view();
        self.file_browser = Some(fb);
        self.open_with_interactive_view(title, lines, actions);
    }

    /// Rebuilds the view lines and interactive row actions from the active [`crate::ui::file_browser::FileBrowserState`].
    pub fn refresh_file_browser(&mut self) {
        if let Some(ref fb) = self.file_browser {
            let (title, lines, actions) = fb.build_view();
            self.open_with_interactive_view(title, lines, actions);
        }
    }

    /// Opens the MCP floating/push window with explicit per-row [`McpRowAction`] metadata (`MCP-03`).
    pub fn open_with_interactive_view(
        &mut self,
        title: impl Into<String>,
        lines: Vec<String>,
        actions: Vec<McpRowAction>,
    ) {
        self.active = true;
        self.title = title.into();
        self.selected_row = actions
            .iter()
            .position(|a| *a != McpRowAction::None)
            .unwrap_or(0);
        self.view_lines = lines;
        self.row_actions = actions;
        self.scroll_offset = 0;
    }

    /// Moves `selected_row` to the next interactive row (or increments by 1 if none are interactive) (`MCP-03`).
    pub fn select_next_row(&mut self) {
        if self.view_lines.is_empty() {
            return;
        }
        if let Some(next_idx) = ((self.selected_row + 1)..self.row_actions.len())
            .find(|&i| self.row_actions[i] != McpRowAction::None)
        {
            self.selected_row = next_idx;
        } else if self.row_actions.iter().all(|a| *a == McpRowAction::None) {
            self.selected_row = (self.selected_row + 1).min(self.view_lines.len().saturating_sub(1));
        }
        if self.selected_row >= self.scroll_offset + 10 {
            self.scroll_offset = self.selected_row.saturating_sub(9);
        }
    }

    /// Moves `selected_row` to the previous interactive row (or decrements by 1 if none are interactive) (`MCP-03`).
    pub fn select_prev_row(&mut self) {
        if self.view_lines.is_empty() {
            return;
        }
        if let Some(prev_idx) = (0..self.selected_row)
            .rev()
            .find(|&i| self.row_actions.get(i).is_some_and(|a| *a != McpRowAction::None))
        {
            self.selected_row = prev_idx;
        } else if self.row_actions.iter().all(|a| *a == McpRowAction::None) {
            self.selected_row = self.selected_row.saturating_sub(1);
        }
        if self.selected_row < self.scroll_offset {
            self.scroll_offset = self.selected_row;
        }
    }

    /// Selects a specific `row_idx` (e.g. from mouse click) and returns its [`McpRowAction`] (`MCP-03`).
    pub fn select_row_at(&mut self, row_idx: usize) -> Option<McpRowAction> {
        if row_idx < self.view_lines.len() {
            self.selected_row = row_idx;
            self.row_actions.get(row_idx).cloned()
        } else {
            None
        }
    }

    /// Returns the [`McpRowAction`] bound to the currently selected row (`MCP-03`).
    pub fn selected_action(&self) -> Option<McpRowAction> {
        self.row_actions.get(self.selected_row).cloned()
    }

    /// Closes the MCP root terminal without terminating the daemon or any child sessions (`MCP-01`).
    pub fn close_root_terminal(&mut self) {
        self.active = false;
        self.file_browser = None;
        self.last_status_banner =
            "MCP ROOT CONSOLE DETACHED (SESSIONS PRESERVED). END OF LINE.".to_string();
    }

    /// Toggles between Floating Pop-Up (`Overlay`) and Push-Out Docked (`Push`) mode.
    pub fn toggle_layout_mode(&mut self) -> DrawerLayoutMode {
        self.layout_mode = self.layout_mode.toggle();
        self.layout_mode
    }

    /// Cycles the dock edge (`Left` -> `Right` -> `Top` -> `Bottom`) for `Push` mode.
    pub fn cycle_edge(&mut self) -> DrawerEdge {
        self.edge = self.edge.cycle();
        self.edge
    }

    /// Scrolls the MCP pop-up/panel content up by `rows` and moves `selected_row` accordingly.
    pub fn scroll_up(&mut self, rows: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(rows);
        for _ in 0..rows {
            self.select_prev_row();
        }
    }

    /// Scrolls the MCP pop-up/panel content down by `rows` and moves `selected_row` accordingly.
    pub fn scroll_down(&mut self, rows: usize) {
        let max_off = self.view_lines.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + rows).min(max_off);
        for _ in 0..rows {
            self.select_next_row();
        }
    }

    /// Selects the first interactive row (`Home`).
    pub fn select_first_row(&mut self) {
        self.selected_row = self
            .row_actions
            .iter()
            .position(|a| *a != McpRowAction::None)
            .unwrap_or(0);
        self.scroll_offset = 0;
    }

    /// Selects the last interactive row (`End`).
    pub fn select_last_row(&mut self) {
        if let Some(last) = self
            .row_actions
            .iter()
            .rposition(|a| *a != McpRowAction::None)
        {
            self.selected_row = last;
            self.scroll_offset = last.saturating_sub(9);
        }
    }

    /// Computes the bounding [`Rect`] of the MCP window within `workspace_area` (`Overlay` floating center vs `Push` edge).
    pub fn panel_rect(&self, workspace_area: Rect) -> Option<Rect> {
        if !self.active || workspace_area.width < 12 || workspace_area.height < 4 {
            return None;
        }
        match self.layout_mode {
            DrawerLayoutMode::Overlay => {
                let content_w = self
                    .view_lines
                    .iter()
                    .map(|l| l.chars().count() as u16)
                    .max()
                    .unwrap_or(56)
                    .saturating_add(6);
                let w = content_w
                    .clamp(36.min(workspace_area.width), workspace_area.width.saturating_sub(4).max(12))
                    .min(workspace_area.width);
                let desired_h = (self.view_lines.len() as u16).saturating_add(4).clamp(8, 20);
                let h = desired_h
                    .min(workspace_area.height.saturating_sub(2).max(4))
                    .min(workspace_area.height);
                let x = workspace_area.x + workspace_area.width.saturating_sub(w) / 2;
                let y = workspace_area.y + workspace_area.height.saturating_sub(h) / 2;
                Some(Rect::new(x, y, w, h))
            }
            DrawerLayoutMode::Push => {
                let max_w = (workspace_area.width / 2).max(16);
                let w = 48u16.clamp(16, max_w).min(workspace_area.width);
                let desired_h = (self.view_lines.len() as u16).saturating_add(3).clamp(6, 14);
                let max_h = (workspace_area.height / 2).max(4);
                let h = desired_h.clamp(4, max_h).min(workspace_area.height);
                Some(match self.edge {
                    DrawerEdge::Left => {
                        Rect::new(workspace_area.x, workspace_area.y, w, workspace_area.height)
                    }
                    DrawerEdge::Right => Rect::new(
                        workspace_area.x + workspace_area.width.saturating_sub(w),
                        workspace_area.y,
                        w,
                        workspace_area.height,
                    ),
                    DrawerEdge::Top => {
                        Rect::new(workspace_area.x, workspace_area.y, workspace_area.width, h)
                    }
                    DrawerEdge::Bottom => Rect::new(
                        workspace_area.x,
                        workspace_area.y + workspace_area.height.saturating_sub(h),
                        workspace_area.width,
                        h,
                    ),
                })
            }
        }
    }

    /// Parses and executes `cmd_str` against `daemon`, recording TRON-styled output (`MCP-02`).
    pub fn execute(
        &mut self,
        cmd_str: &str,
        daemon: &mut DaemonServerState,
    ) -> Result<String, String> {
        let cmd = parse_mcp_command(cmd_str)?;
        let raw_msg = match cmd {
            McpCommand::Status => match daemon.handle_request(DaemonRequest::Ping) {
                DaemonResponse::Pong {
                    pid,
                    uptime_secs,
                    session_count,
                    client_count,
                } => format!(
                    "MCP STATUS: PID={pid} UPTIME={uptime_secs}s SESSIONS={session_count} CLIENTS={client_count} RUNNING={}. END OF LINE.",
                    daemon.running
                ),
                other => format!("{other:?} END OF LINE."),
            },
            McpCommand::Tree => {
                let lines = daemon.session_tree.render_herdr_tree_lines("main");
                format!("{}\nEND OF LINE.", lines.join("\n"))
            }
            McpCommand::Attach { node } => {
                let resp = daemon.handle_request(DaemonRequest::Attach {
                    client_id: 9000,
                    session_name: node,
                    is_gui: false,
                    cols: 120,
                    rows: 40,
                    font_scale_pct: 100,
                });
                match resp {
                    DaemonResponse::Attached {
                        session_name,
                        effective_cols,
                        effective_rows,
                        client_count,
                    } => format!(
                        "ATTACHED TO '{session_name}' ({effective_cols}x{effective_rows}, {client_count} client(s)). END OF LINE."
                    ),
                    DaemonResponse::Error { message } => {
                        return Err(format!("MCP ERROR: {message} END OF LINE."));
                    }
                    other => format!("{other:?} END OF LINE."),
                }
            }
            McpCommand::Spawn { path } => {
                if let Some((parent, child)) = path.split_once('/') {
                    match daemon.handle_request(DaemonRequest::BranchSession {
                        parent: parent.to_string(),
                        child: child.to_string(),
                    }) {
                        DaemonResponse::Ok { message } => format!("{message}. END OF LINE."),
                        DaemonResponse::Error { message } => {
                            return Err(format!("MCP ERROR: {message} END OF LINE."));
                        }
                        other => format!("{other:?} END OF LINE."),
                    }
                } else {
                    daemon
                        .session_tree
                        .upsert_session(&path, SessionNodeKind::Independent, None);
                    daemon.session_panes.entry(path.clone()).or_insert(1);
                    format!("SPAWNED INDEPENDENT ROOT SESSION '{path}'. END OF LINE.")
                }
            }
            McpCommand::Graft { src, dst } => {
                match daemon.handle_request(DaemonRequest::GraftSession { src, dst }) {
                    DaemonResponse::Ok { message } => format!("{message}. END OF LINE."),
                    DaemonResponse::Error { message } => {
                        return Err(format!("MCP ERROR: {message} END OF LINE."));
                    }
                    other => format!("{other:?} END OF LINE."),
                }
            }
            McpCommand::DetachSubtree { node } => {
                daemon
                    .session_tree
                    .detach_subtree(&node)
                    .map_err(|e| format!("MCP ERROR: {e} END OF LINE."))?;
                format!(
                    "DETACHED SUBTREE '{node}' AS INDEPENDENT ROOT SESSION. END OF LINE."
                )
            }
            McpCommand::Broadcast { subtree, command } => {
                match daemon.handle_request(DaemonRequest::BroadcastSubtree {
                    target: subtree,
                    command,
                }) {
                    DaemonResponse::Ok { message } => format!("{message}. END OF LINE."),
                    DaemonResponse::Error { message } => {
                        return Err(format!("MCP ERROR: {message} END OF LINE."));
                    }
                    other => format!("{other:?} END OF LINE."),
                }
            }
            McpCommand::Freeze { target } => {
                match daemon.handle_request(DaemonRequest::FreezeSubtree { target }) {
                    DaemonResponse::Ok { message } => format!("{message}. END OF LINE."),
                    DaemonResponse::Error { message } => {
                        return Err(format!("MCP ERROR: {message} END OF LINE."));
                    }
                    other => format!("{other:?} END OF LINE."),
                }
            }
            McpCommand::Thaw { target } => {
                match daemon.handle_request(DaemonRequest::ThawSubtree { target }) {
                    DaemonResponse::Ok { message } => format!("{message}. END OF LINE."),
                    DaemonResponse::Error { message } => {
                        return Err(format!("MCP ERROR: {message} END OF LINE."));
                    }
                    other => format!("{other:?} END OF LINE."),
                }
            }
            McpCommand::Derez { target, confirmed } => {
                match daemon.handle_request(DaemonRequest::Derez { target, confirmed }) {
                    DaemonResponse::Ok { message } => format!("{message}. END OF LINE."),
                    DaemonResponse::Error { message } => {
                        return Err(format!("MCP ERROR: {message} END OF LINE."));
                    }
                    other => format!("{other:?} END OF LINE."),
                }
            }
            McpCommand::Checkpoint => match daemon.handle_request(DaemonRequest::Checkpoint) {
                DaemonResponse::Ok { message } => format!("{message}. END OF LINE."),
                DaemonResponse::Error { message } => {
                    return Err(format!("MCP ERROR: {message} END OF LINE."));
                }
                other => format!("{other:?} END OF LINE."),
            },
            McpCommand::Shutdown { confirmed } => {
                match daemon.handle_request(DaemonRequest::Shutdown { confirmed }) {
                    DaemonResponse::Ok { message } => message,
                    DaemonResponse::Error { message } => {
                        return Err(format!("MCP ERROR: {message} END OF LINE."));
                    }
                    other => format!("{other:?} END OF LINE."),
                }
            }
            McpCommand::Help => {
                "MCP DIRECTIVES: status | tree | attach <node> | spawn <p/c|name> | graft <src> <dst> | detach-subtree <node> | broadcast <node> <cmd> | freeze <node> | thaw <node> | derez <node> --confirm | checkpoint | shutdown --confirm. END OF LINE."
                    .to_string()
            }
        };

        for line in raw_msg.lines() {
            self.log_lines.push(line.to_string());
        }
        self.last_status_banner = raw_msg.lines().last().unwrap_or(&raw_msg).to_string();
        Ok(raw_msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn mcp_parse_and_execute_full_directive_suite() {
        let tmp = tempfile::tempdir().unwrap();
        let mut daemon = DaemonServerState::with_journal_dir(tmp.path().join("journals"));
        let mut mcp = McpConsoleState::new();
        mcp.open_root_terminal();
        assert!(mcp.active);

        // 1. spawn parent/child and independent root
        let out = mcp.execute("mcp spawn main/agents", &mut daemon).unwrap();
        assert!(out.contains("END OF LINE."));
        mcp.execute("mcp spawn agents/worker-1", &mut daemon).unwrap();
        mcp.execute("mcp spawn sandbox", &mut daemon).unwrap();

        // 2. freeze / thaw
        let frz = mcp.execute("mcp freeze agents", &mut daemon).unwrap();
        assert!(frz.contains("agents") && frz.contains("worker-1"));
        assert!(daemon.session_tree.node("worker-1").unwrap().frozen);
        mcp.execute("mcp thaw agents", &mut daemon).unwrap();
        assert!(!daemon.session_tree.node("worker-1").unwrap().frozen);

        // 3. broadcast
        let bcast = mcp
            .execute("mcp broadcast agents cargo test -q", &mut daemon)
            .unwrap();
        assert!(bcast.contains("cargo test -q"));
        assert_eq!(daemon.broadcast_log.len(), 2);

        // 4. graft & detach-subtree
        mcp.execute("mcp graft agents sandbox", &mut daemon).unwrap();
        assert_eq!(
            daemon.session_tree.node("agents").unwrap().parent.as_deref(),
            Some("sandbox")
        );
        mcp.execute("mcp detach-subtree agents", &mut daemon).unwrap();
        assert_eq!(
            daemon.session_tree.node("agents").unwrap().kind,
            SessionNodeKind::Independent
        );

        // 5. Closing MCP root terminal is non-destructive!
        mcp.close_root_terminal();
        assert!(!mcp.active);
        assert!(daemon.running);
        assert!(daemon.session_tree.node("agents").is_some());

        // 6. derez requires --confirm
        assert!(mcp.execute("mcp derez agents", &mut daemon).is_err());
        mcp.execute("mcp derez agents --confirm", &mut daemon).unwrap();
        assert!(daemon.session_tree.node("agents").is_none());

        // 7. checkpoint & shutdown --confirm
        mcp.execute("mcp checkpoint", &mut daemon).unwrap();
        assert!(mcp.execute("mcp shutdown", &mut daemon).is_err());
        mcp.execute("mcp shutdown --confirm", &mut daemon).unwrap();
        assert!(!daemon.running);
    }

    #[test]
    fn render_mcp_console_is_clamp_safe_and_draws_header() {
        let daemon = DaemonServerState::new();
        let mcp = McpConsoleState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 24));

        // Zero-area render must not panic
        render_mcp_console(&mut buf, Rect::new(0, 0, 0, 0), &mcp, &daemon);

        // Full-area render draws MASTER CONTROL PROGRAM title
        render_mcp_console(&mut buf, Rect::new(0, 0, 100, 24), &mcp, &daemon);
        let top_row: String = (0..100)
            .map(|x| buf.cell((x, 0)).unwrap().symbol())
            .collect();
        assert!(top_row.contains("MASTER CONTROL PROGRAM"));
    }
}
