//! JSON-RPC 2.0 Control Plane (`AutomationControlPlane`, `SnapshotPolicy`, method dispatcher).

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

use ratatui::layout::Rect;

use super::json::JsonValue;
use super::playbook::AutomationEvent;
use crate::pty::line_discipline::InputRoutingMode;
use crate::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer, WorkspaceFrameRenderer};
use crate::vt::VtParser;
use crate::wm::{WindowManager, WorkspaceLayoutMode};

pub(super) fn extract_vt_row_text(vt: &VtParser, row: u16) -> String {
    vt.row_text(row)
}

/// Every method name `AutomationControlPlane::dispatch_method` handles (drift-tested against the dispatcher).
pub const RPC_METHODS: &[&str] = &[
    "workspace.status",
    "workspace.snapshot",
    "workspace.fontsize",
    "pane.exec",
    "pane.read",
    "pane.feed_output",
    "pane.send_keys",
    "wm.command",
    "agents.list",
    "agents.demo",
    "session.map",
    "events.poll",
    "playbook.run",
    "remote.connect_ssh",
    "remote.connect_tmux",
    "remote.link",
    "fleet.register",
    "fleet.list",
    "fleet.exec",
    "fleet.disconnect",
    "find.agents",
    "find.sessions",
    "find.history",
    "find.locate",
    "find.focus",
    "find.send",
    "find.beep",
    "find.flash",
    "find.select",
    "canvas.command",
    "canvas.status",
    "session.save",
    "session.restore",
    "window.move",
    "window.reorder",
    "window.place",
    "session.move",
    "session.reorder",
    "window.rename",
    "session.rename",
    "window.select",
    "window.bulk",
    "window.undo",
    "agent.name",
    "agent.send",
    "agent.broadcast",
    "agent.inbox",
    "agent.ack",
    "agent.list",
    "agent.wait",
    "agent.prompt",
    "agent.approve",
    "agent.last_turn",
    "agent.hud",
    "agent.replay",
    "agent.watchdog",
    "pane.list",
    "pane.split",
    "pane.focus",
    "pane.zoom",
    "ui.repaint",
    "pane.send",
    "remote.status",
    "pane.mail_state",
    "pane.mail_ack",
    "workspace.list",
    "workspace.select",
    "workspace.new",
    "workspace.rename",
    "daemon.list",
    "daemon.status",
    "daemon.start",
    "daemon.new",
    "daemon.stop",
    "daemon.switch",
    "daemon.host_state",
    "dos.run",
    "dos.setup",
    "dos.doctor",
    "dos.remove",
    "dos.status",
    "dos.reset",
    "dos.snapshot",
    "dos.restore",
    "editor.open",
    "editor.scratch",
    "editor.artifact",
    "editor.board",
    "editor.save",
    "editor.save_as",
    "editor.wrap",
    "editor.render",
    "editor.check",
    "editor.broadcast",
    "editor.preview",
    "editor.find",
    "editor.undo",
    "editor.redo",
    "editor.goto",
    "editor.revert",
    "editor.insert",
    "editor.text",
    "editor.status",
    "editor.close",
    "editor.menu",
    "attach.clients",
    "attach.kick",
    "session.transfer",
    "daemon.restart",
    "drawer.preview",
    "clipboard.list",
    "clipboard.get",
    "clipboard.copy",
    "clipboard.paste",
    "clipboard.pin",
    "clipboard.delete",
    "clipboard.clear",
    "clipboard.window",
    "coords.resolve",
    "coords.format",
    "context.set",
    "context.get",
    "context.list",
    "agent.queue",
    "agent.guide",
    "swarm.status",
    "swarm.load",
    "swarm.clear",
    "swarm.run",
    "swarm.spawn",
    "swarm.verify",
    "swarm.diff",
    "swarm.merge",
    "swarm.clean",
    "traffic.snapshot",
    "traffic.history",
    "traffic.window",
    "traffic.reset",
    "traffic.config",
    "voice.toggle",
    "voice.start",
    "voice.stop",
    "voice.cancel",
    "voice.status",
    "voice.provider",
    "voice.inject_wav",
    "speech.say",
    "speech.stop",
    "speech.status",
    "speech.voices",
    "browser.open",
    "browser.control",
    "browser.state",
    "video.open",
    "video.control",
    "video.state",
    "workspace.view_model",
    "web.status",
    "web.open",
    "web.open_external",
    "web.port",
    "web.stop",
    "web.pump",
    "web.agents",
    "web.agent",
    "web.logs",
    "web.search",
    "web.input",
    "web.create",
    "web.kill",
    "web.update",
    "keys.list",
    "keys.check",
    "keys.explain",
    "keys.profile",
    "keys.window_mode",
    "menu.open",
];

/// Stateful automation control plane wrapping a `WindowManager` and an event subscription ring buffer.
pub struct AutomationControlPlane {
    pub wm: WindowManager,
    pub events: VecDeque<AutomationEvent>,
    pub viewport: Rect,
    pub snapshot_dir: Option<PathBuf>,
    /// Per-pane output rings streamed to WebSocket clients (`WEB-FE-01`, Phase 60).
    pub output_hub: super::output_ring::OutputHub,
    /// Session-host persistence target and last written state (`host_tick`); `None` unless hosting.
    pub(super) host_persist: Option<super::host_tick::HostPersist>,
    /// Validated attach-client requests awaiting `host_tick` (`T-A5`); `None` without `--attach`.
    pub(super) attach_requests: Option<tokio::sync::mpsc::Receiver<crate::attach::input::HostRequest>>,
    /// Streaming rendezvous with attach connections (`T-A6`); `None` without `--attach`.
    pub(super) attach_hub: Option<std::sync::Arc<crate::attach::hub::AttachHub>>,
    /// Name of the daemon this plane serves when it was started WITHOUT `--host-ptys` (its panes
    /// have no shell); command-sending RPCs then fail loudly instead of reporting success.
    pub(super) no_pty_daemon: Option<String>,
    /// Accepts live session transfers from other hosts (`host-<name>.xfer.sock`, unix, hosts only).
    #[cfg(unix)]
    pub(super) transfer_listener: Option<crate::daemon::transfer::TransferListener>,
}

/// Execution policy determining whether snapshot exports are confined to the designated snapshot dir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotPolicy {
    /// Trusted execution (CLI --playbook), where file paths are chosen by local operator.
    Trusted,
    /// Confined execution (RPC playbook.run or MCP bilterm_run_playbook), strictly confined to snapshot dir.
    Confined,
}

/// Pure helper resolving the default snapshot directory according to the XDG Base Directory specification.
pub fn default_snapshot_dir_from(
    xdg_state_home: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    if let Some(state) = xdg_state_home
        && !state.is_empty()
    {
        return Some(PathBuf::from(state).join("bilterm").join("snapshots"));
    }
    if let Some(h) = home
        && !h.is_empty()
    {
        return Some(
            PathBuf::from(h)
                .join(".local")
                .join("state")
                .join("bilterm")
                .join("snapshots"),
        );
    }
    None
}

/// Validates that a requested snapshot file name is a single file name within the snapshot directory.
pub fn validate_snapshot_name(name: &str) -> std::result::Result<(), String> {
    crate::secure_fs::validate_plain_file_name(name)
}

impl AutomationControlPlane {
    pub fn new(wm: WindowManager, viewport: Rect) -> Self {
        Self {
            wm,
            events: VecDeque::with_capacity(256),
            viewport,
            snapshot_dir: None,
            output_hub: super::output_ring::OutputHub::new(),
            host_persist: None,
            attach_requests: None,
            attach_hub: None,
            no_pty_daemon: None,
            #[cfg(unix)]
            transfer_listener: None,
        }
    }

    /// Marks this plane as a daemon `name` without host PTYs (see [`Self::no_pty_error`]).
    pub fn with_no_pty_daemon(mut self, name: &str) -> Self {
        self.no_pty_daemon = Some(name.to_string());
        self
    }

    /// The structured error for a command-sending RPC on a PTY-less daemon, else `None`.
    pub(super) fn no_pty_error(&self, method: &str) -> Option<String> {
        let name = self.no_pty_daemon.as_deref()?;
        Some(format!(
            "no_host_ptys: daemon '{name}' was started without --host-ptys; its panes have no shell, so {method} cannot run anything. \
             Restart it with PTYs: `bilterm daemon stop {name}` then `bilterm daemon new {name} --host-ptys` (`:daemon new {name} --host-ptys` in-app), or `bilterm daemon new --host-ptys` for a fresh name. \
             Check `host_ptys` in daemon.status / workspace.status first."
        ))
    }

    pub fn with_snapshot_dir(mut self, dir: PathBuf) -> Self {
        self.snapshot_dir = Some(dir);
        self
    }

    pub(super) fn resolved_snapshot_dir(&self) -> std::result::Result<PathBuf, String> {
        let dir = if let Some(ref d) = self.snapshot_dir {
            d.clone()
        } else if let Some(d) = default_snapshot_dir_from(
            std::env::var_os("XDG_STATE_HOME").as_deref(),
            std::env::var_os("HOME").as_deref(),
        ) {
            d
        } else {
            #[cfg(unix)]
            {
                crate::secure_fs::private_runtime_dir()
                    .map(|p| p.join("snapshots"))
                    .map_err(|e| format!("failed to resolve private runtime dir: {e}"))?
            }
            #[cfg(not(unix))]
            {
                std::env::temp_dir().join("bilterm-snapshots")
            }
        };

        if !dir.exists() {
            if let Some(parent) = dir.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            #[cfg(unix)]
            {
                crate::secure_fs::ensure_private_dir(&dir).map_err(|e| {
                    format!(
                        "failed to create private snapshot directory {}: {e}",
                        dir.display()
                    )
                })?;
            }
            #[cfg(not(unix))]
            {
                std::fs::create_dir_all(&dir).map_err(|e| {
                    format!("failed to create snapshot directory {}: {e}", dir.display())
                })?;
            }
        } else {
            let meta = std::fs::symlink_metadata(&dir).map_err(|e| {
                format!(
                    "failed to read metadata for snapshot dir {}: {e}",
                    dir.display()
                )
            })?;
            if !meta.is_dir() || meta.file_type().is_symlink() {
                return Err(format!(
                    "snapshot directory {} is not a directory",
                    dir.display()
                ));
            }
        }

        std::fs::canonicalize(&dir).map_err(|e| {
            format!(
                "failed to canonicalize snapshot directory {}: {e}",
                dir.display()
            )
        })
    }

    /// Exports a PPM workspace snapshot to a trusted caller-specified path (CLI --playbook).
    pub fn write_snapshot_trusted(&self, path: &Path) -> std::result::Result<(), String> {
        let mut renderer = GuiWorkspaceRenderer::default();
        let mut surface = GuiFrameSurface::new(8, 16);
        renderer.render_workspace(&self.wm, self.viewport, &mut surface);
        std::fs::write(path, surface.export_ppm())
            .map_err(|e| format!("failed to write PPM snapshot: {e}"))
    }

    /// Exports a PPM workspace snapshot confined to the snapshot directory (RPC/MCP).
    pub fn write_snapshot_confined(&self, name: &str) -> std::result::Result<PathBuf, String> {
        validate_snapshot_name(name)?;
        let dir = self.resolved_snapshot_dir()?;
        let target = dir.join(name);

        use std::io::Write;
        #[cfg(unix)]
        let mut file = crate::secure_fs::private_open_options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&target)
            .map_err(|e| {
                format!("ppm_path must be a file name within the snapshot directory: {e}")
            })?;

        #[cfg(not(unix))]
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&target)
            .map_err(|e| {
                format!("ppm_path must be a file name within the snapshot directory: {e}")
            })?;

        if let Some(parent) = target.parent() {
            let canonical_parent = std::fs::canonicalize(parent).map_err(|e| {
                format!("ppm_path must be a file name within the snapshot directory: {e}")
            })?;
            if canonical_parent != dir {
                return Err(
                    "ppm_path must be a file name within the snapshot directory".to_string(),
                );
            }
        }

        let mut renderer = GuiWorkspaceRenderer::default();
        let mut surface = GuiFrameSurface::new(8, 16);
        renderer.render_workspace(&self.wm, self.viewport, &mut surface);
        file.write_all(&surface.export_ppm())
            .map_err(|e| format!("failed to write PPM snapshot: {e}"))?;
        file.flush()
            .map_err(|e| format!("failed to flush PPM snapshot: {e}"))?;

        Ok(target)
    }

    pub fn new_headless(rows: u16, cols: u16) -> Self {
        let wm = WindowManager::new_headless(rows, cols);
        Self::new(wm, Rect::new(0, 0, cols, rows))
    }

    pub fn push_event(&mut self, event: AutomationEvent) {
        if self.events.len() >= 512 {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    /// Handles a single JSON-RPC 2.0 request line and returns a serialized JSON-RPC 2.0 response line.
    pub fn handle_jsonrpc_line(&mut self, line: &str) -> String {
        let Some(req) = JsonValue::parse(line) else {
            return Self::error_response(JsonValue::Null, -32700, "Parse error").to_json_string();
        };
        let id = req.get("id").cloned().unwrap_or(JsonValue::Null);
        let Some(method) = req.get("method").and_then(JsonValue::as_str) else {
            return Self::error_response(id, -32600, "Invalid Request: missing method")
                .to_json_string();
        };
        let params = req
            .get("params")
            .cloned()
            .unwrap_or_else(|| JsonValue::Object(BTreeMap::new()));

        match self.dispatch_method(method, &params) {
            Ok(result) => Self::result_response(id, result).to_json_string(),
            Err(msg) => Self::error_response(id, -32000, &msg).to_json_string(),
        }
    }

    /// JSON-RPC 2.0 success envelope `{"jsonrpc":"2.0","id":..,"result":..}`.
    pub(crate) fn result_response(id: JsonValue, result: JsonValue) -> JsonValue {
        JsonValue::obj([
            ("jsonrpc", JsonValue::String("2.0".to_string())),
            ("id", id),
            ("result", result),
        ])
    }

    pub fn workspace_status_json(&self) -> JsonValue {
        let layout_str = match self.wm.layout_mode() {
            WorkspaceLayoutMode::Tiled => "tiled",
            WorkspaceLayoutMode::Stacked => "stacked",
        };
        let mut panes = Vec::new();
        for (&id, pane) in self.wm.window_panes() {
            let (rows, cols) = pane.vt().size();
            let (cur_r, cur_c) = pane.vt().cursor_position();
            let input_mode = match pane.effective_routing_mode() {
                InputRoutingMode::CommandPrompt => "cmd_box",
                InputRoutingMode::RawPtyPassthrough => "raw_pty",
            };
            panes.push(JsonValue::obj([
                ("id", JsonValue::Number(id as i64)),
                ("title", JsonValue::String(pane.title().to_string())),
                ("rows", JsonValue::Number(i64::from(rows))),
                ("cols", JsonValue::Number(i64::from(cols))),
                ("cursor_row", JsonValue::Number(i64::from(cur_r))),
                ("cursor_col", JsonValue::Number(i64::from(cur_c))),
                ("input_target", JsonValue::String(input_mode.to_string())),
                (
                    "alternate_screen",
                    JsonValue::Bool(pane.vt().is_alternate_screen()),
                ),
                (
                    "echo_lock",
                    JsonValue::Bool(pane.is_password_prompt()),
                ),
                ("read_only", JsonValue::Bool(pane.is_read_only())),
                ("mail", self.wm.pane_mail_json(id, std::time::Instant::now())),
                ("remote", self.wm.pane_origin(id).to_json()),
                ("recording", JsonValue::Bool(pane.vt().is_recording())),
                (
                    "notifications",
                    JsonValue::Number(pane.vt().notifications().len() as i64),
                ),
            ]));
        }
        JsonValue::obj([
            ("layout_mode", JsonValue::String(layout_str.to_string())),
            (
                "active_pane_id",
                JsonValue::Number(self.wm.active_pane_id() as i64),
            ),
            (
                "pane_count",
                JsonValue::Number(self.wm.window_count() as i64),
            ),
            ("remote_panes", JsonValue::Number(self.wm.remote_status_json(None).get("remote_panes").and_then(JsonValue::as_i64).unwrap_or(0))),
            ("host_ptys", JsonValue::Bool(self.wm.is_host())),
            ("session_empty", JsonValue::Bool(self.wm.is_session_empty())),
            (
                "session_on_empty",
                JsonValue::String(self.wm.config().session_on_empty.label().to_string()),
            ),
            ("broadcast_input", JsonValue::Bool(self.wm.is_broadcast_input())),
            (
                "menu_bar_visible",
                JsonValue::Bool(self.wm.config().show_top_menubar),
            ),
            (
                "crt_scanlines",
                JsonValue::Bool(self.wm.config().crt_scanlines),
            ),
            (
                "palette_visible",
                JsonValue::Bool(self.wm.is_palette_visible()),
            ),
            ("panes", JsonValue::Array(panes)),
        ])
    }

    pub(crate) fn error_response(id: JsonValue, code: i64, message: &str) -> JsonValue {
        JsonValue::obj([
            ("jsonrpc", JsonValue::String("2.0".to_string())),
            ("id", id),
            (
                "error",
                JsonValue::obj([
                    ("code", JsonValue::Number(code)),
                    ("message", JsonValue::String(message.to_string())),
                ]),
            ),
        ])
    }
}

