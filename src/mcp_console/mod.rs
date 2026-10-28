//! TRON-Inspired Master Control Program (`MCP`) Root Terminal (`MCP-01`, `MCP-02`).
//!
//! Provides the top-level supervisor console (`bilterm mcp` / `:mcp`) that inspects and
//! orchestrates the background [`DaemonServerState`] and [`SessionTree`] (`status`, `tree`,
//! `attach`, `spawn`, `graft`, `detach-subtree`, `broadcast`, `freeze`, `thaw`, `derez`,
//! `checkpoint`, `shutdown --confirm`).
//!
//! Closing the MCP root console (`close_root_terminal`) never terminates running child sessions;
//! only `derez <target> --confirm` or `shutdown --confirm` tears down sessions.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::daemon::{DaemonRequest, DaemonResponse, DaemonServerState};
use crate::wm::session_tree::SessionNodeKind;
use crate::wm::{DrawerEdge, DrawerLayoutMode};

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

/// Interactive state for the Master Control Program (`MCP`) root terminal & pop-up/push inspector (`MCP-01`, `MCP-02`).
#[derive(Clone, Debug)]
pub struct McpConsoleState {
    pub active: bool,
    pub title: String,
    pub view_lines: Vec<String>,
    pub scroll_offset: usize,
    /// `DrawerLayoutMode::Overlay` (Floating pop-up window) vs `DrawerLayoutMode::Push` (pushes workspace panes).
    pub layout_mode: DrawerLayoutMode,
    /// Dock edge (`Top`, `Bottom`, `Left`, `Right`) used when `layout_mode == DrawerLayoutMode::Push`.
    pub edge: DrawerEdge,
    pub log_lines: Vec<String>,
    pub last_status_banner: String,
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
            scroll_offset: 0,
            layout_mode: DrawerLayoutMode::Overlay,
            edge: DrawerEdge::Top,
            log_lines: vec![
                "◈ MASTER CONTROL PROGRAM [MCP ROOT CONSOLE] ONLINE. END OF LINE.".to_string(),
            ],
            last_status_banner: "MCP ROOT READY — END OF LINE.".to_string(),
        }
    }

    /// Opens the MCP root terminal overlay.
    pub fn open_root_terminal(&mut self) {
        self.active = true;
        self.scroll_offset = 0;
    }

    /// Opens the MCP floating/push window with a specific title and multi-line content view.
    pub fn open_with_view(&mut self, title: impl Into<String>, lines: Vec<String>) {
        self.active = true;
        self.title = title.into();
        self.view_lines = lines;
        self.scroll_offset = 0;
    }

    /// Closes the MCP root terminal without terminating the daemon or any child sessions (`MCP-01`).
    pub fn close_root_terminal(&mut self) {
        self.active = false;
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

    /// Scrolls the MCP pop-up/panel content up by `rows`.
    pub fn scroll_up(&mut self, rows: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(rows);
    }

    /// Scrolls the MCP pop-up/panel content down by `rows`.
    pub fn scroll_down(&mut self, rows: usize) {
        let max_off = self.view_lines.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + rows).min(max_off);
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

/// Renders the TRON-inspired Master Control Program (`MCP`) root dashboard into `buf` (`MCP-01`, `MCP-02`).
///
/// Safe for any `area` dimensions (including `0×0` or tiny viewports).
pub fn render_mcp_console(
    buf: &mut Buffer,
    area: Rect,
    state: &McpConsoleState,
    daemon: &DaemonServerState,
) {
    let bounds = area.intersection(*buf.area());
    if bounds.width < 4 || bounds.height < 3 {
        return;
    }

    let bg_style = Style::default()
        .fg(Color::Rgb(0x00, 0xF0, 0xFF))
        .bg(Color::Rgb(0x08, 0x04, 0x10));
    let border_style = Style::default()
        .fg(Color::Rgb(0xFF, 0x2A, 0x2A))
        .bg(Color::Rgb(0x08, 0x04, 0x10))
        .add_modifier(Modifier::BOLD);
    let gold_style = Style::default()
        .fg(Color::Rgb(0xFF, 0xCC, 0x00))
        .bg(Color::Rgb(0x08, 0x04, 0x10))
        .add_modifier(Modifier::BOLD);

    for y in bounds.y..bounds.y + bounds.height {
        for x in bounds.x..bounds.x + bounds.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_style(bg_style);
            }
        }
    }

    // Double-line TRON crimson cyber-grid border
    let right = bounds.x + bounds.width - 1;
    let bottom = bounds.y + bounds.height - 1;
    for x in bounds.x..=right {
        if let Some(top_c) = buf.cell_mut((x, bounds.y)) {
            top_c.set_symbol("═");
            top_c.set_style(border_style);
        }
        if let Some(bot_c) = buf.cell_mut((x, bottom)) {
            bot_c.set_symbol("═");
            bot_c.set_style(border_style);
        }
    }
    for y in bounds.y..=bottom {
        if let Some(left_c) = buf.cell_mut((bounds.x, y)) {
            left_c.set_symbol("║");
            left_c.set_style(border_style);
        }
        if let Some(right_c) = buf.cell_mut((right, y)) {
            right_c.set_symbol("║");
            right_c.set_style(border_style);
        }
    }
    if let Some(c) = buf.cell_mut((bounds.x, bounds.y)) {
        c.set_symbol("╔");
    }
    if let Some(c) = buf.cell_mut((right, bounds.y)) {
        c.set_symbol("╗");
    }
    if let Some(c) = buf.cell_mut((bounds.x, bottom)) {
        c.set_symbol("╚");
    }
    if let Some(c) = buf.cell_mut((right, bottom)) {
        c.set_symbol("╝");
    }

    let mode_tag = match state.layout_mode {
        DrawerLayoutMode::Overlay => "[m:FLOAT]",
        DrawerLayoutMode::Push => match state.edge {
            DrawerEdge::Top => "[m:PUSH-TOP][d:Edge]",
            DrawerEdge::Bottom => "[m:PUSH-BOT][d:Edge]",
            DrawerEdge::Left => "[m:PUSH-LEFT][d:Edge]",
            DrawerEdge::Right => "[m:PUSH-RIGHT][d:Edge]",
        },
    };
    let title = format!(" ◈ {} {mode_tag} [▪] ", state.title);
    write_clipped(buf, bounds.x + 2, bounds.y, right.saturating_sub(1), &title, gold_style);

    let footer = " [↑/↓:Scroll] [m:Float↔Push] [d:Edge] [Esc/q:Close] — END OF LINE. ";
    write_clipped(buf, bounds.x + 2, bottom, right.saturating_sub(1), footer, gold_style);

    let mut row = bounds.y + 1;
    let max_row = bottom;
    let inner_left = bounds.x + 2;
    let inner_right = right.saturating_sub(1);

    if !state.view_lines.is_empty() {
        for line in state.view_lines.iter().skip(state.scroll_offset) {
            if row >= max_row {
                break;
            }
            let st = if line.starts_with("◈") || line.starts_with("DAEMON") || line.starts_with("WORKSPACE") {
                gold_style
            } else {
                bg_style
            };
            write_clipped(buf, inner_left, row, inner_right, line, st);
            row += 1;
        }
        return;
    }

    if row < max_row {
        let header = format!(
            "DAEMON: {} | SESSIONS: {} | ATTACHED CLIENTS: {} | BANNER: {}",
            if daemon.running { "ONLINE" } else { "OFFLINE" },
            daemon.session_tree.nodes.len(),
            daemon.clients.len(),
            state.last_status_banner
        );
        write_clipped(buf, inner_left, row, inner_right, &header, gold_style);
        row += 1;
    }

    for tree_line in daemon.session_tree.render_herdr_tree_lines("main") {
        if row >= max_row {
            break;
        }
        write_clipped(buf, inner_left, row, inner_right, &tree_line, bg_style);
        row += 1;
    }

    let tail_count = (max_row.saturating_sub(row)) as usize;
    let start_idx = state.log_lines.len().saturating_sub(tail_count);
    for log_line in &state.log_lines[start_idx..] {
        if row >= max_row {
            break;
        }
        write_clipped(buf, inner_left, row, inner_right, log_line, border_style);
        row += 1;
    }
}

fn write_clipped(
    buf: &mut Buffer,
    start_x: u16,
    y: u16,
    max_x: u16,
    text: &str,
    style: Style,
) {
    let mut x = start_x;
    for ch in text.chars() {
        if x >= max_x {
            break;
        }
        if let Some(cell) = buf.cell_mut((x, y)) {
            let mut s = [0u8; 4];
            cell.set_symbol(ch.encode_utf8(&mut s));
            cell.set_style(style);
        }
        x += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
