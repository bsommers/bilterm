//! JSON-RPC 2.0 Control Plane (`AutomationControlPlane`, `SnapshotPolicy`, method dispatcher).

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

use ratatui::layout::Rect;

use super::json::JsonValue;
use super::mcp::parse_key_chord_str;
use super::playbook::{AutomationEvent, Playbook};
use crate::colon_command::parse_colon_command;
use crate::input::{AppAction, handle_wm_key_event};
use crate::pty::line_discipline::InputRoutingMode;
use crate::remote::{FleetSelector, RemoteHostProfile};
use crate::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer, WorkspaceFrameRenderer};
use crate::vt::VtParser;
use crate::wm::{WindowManager, WorkspaceLayoutMode};

pub(super) fn extract_vt_row_text(vt: &VtParser, row: u16) -> String {
    vt.row_text(row)
}

/// Stateful automation control plane wrapping a `WindowManager` and an event subscription ring buffer.
pub struct AutomationControlPlane {
    pub wm: WindowManager,
    pub events: VecDeque<AutomationEvent>,
    pub viewport: Rect,
    pub snapshot_dir: Option<PathBuf>,
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
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
        || name == "."
        || name == ".."
    {
        return Err("ppm_path must be a file name within the snapshot directory".to_string());
    }
    let path = Path::new(name);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None) => Ok(()),
        _ => Err("ppm_path must be a file name within the snapshot directory".to_string()),
    }
}

impl AutomationControlPlane {
    pub fn new(wm: WindowManager, viewport: Rect) -> Self {
        Self {
            wm,
            events: VecDeque::with_capacity(256),
            viewport,
            snapshot_dir: None,
        }
    }

    pub fn with_snapshot_dir(mut self, dir: PathBuf) -> Self {
        self.snapshot_dir = Some(dir);
        self
    }

    fn resolved_snapshot_dir(&self) -> std::result::Result<PathBuf, String> {
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
            Ok(result) => JsonValue::obj([
                ("jsonrpc", JsonValue::String("2.0".to_string())),
                ("id", id),
                ("result", result),
            ])
            .to_json_string(),
            Err(msg) => Self::error_response(id, -32000, &msg).to_json_string(),
        }
    }

    /// Dispatches a JSON-RPC 2.0 control plane method against the workspace.
    pub fn dispatch_method(
        &mut self,
        method: &str,
        params: &JsonValue,
    ) -> std::result::Result<JsonValue, String> {
        match method {
            "workspace.status" => Ok(self.workspace_status_json()),
            "workspace.snapshot" => {
                if let Some(ppm_path) = params.get("ppm_path").and_then(JsonValue::as_str) {
                    self.write_snapshot_confined(ppm_path)?;
                }
                Ok(self.workspace_status_json())
            }
            "pane.exec" => {
                let cmd = params
                    .get("command")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| "missing 'command' parameter".to_string())?;
                let target_id = params
                    .get("pane_id")
                    .and_then(JsonValue::as_usize)
                    .unwrap_or_else(|| self.wm.active_pane_id());
                let allow_echo_lock = params
                    .get("allow_echo_lock")
                    .and_then(JsonValue::as_bool)
                    .unwrap_or(false);

                let pane = self
                    .wm
                    .pane_mut(target_id)
                    .ok_or_else(|| format!("pane #{target_id} not found"))?;
                if pane.is_read_only() {
                    return Err(format!("pane #{target_id} is locked read-only (🔒 PROT)"));
                }
                if pane.is_password_prompt() && !allow_echo_lock {
                    return Err(format!(
                        "pane #{target_id} is in !ECHO LOCK password prompt; set allow_echo_lock=true to override"
                    ));
                }
                pane.submit_command(cmd).map_err(|e| e.to_string())?;

                self.push_event(AutomationEvent::CommandSubmitted {
                    pane_id: target_id,
                    command: cmd.to_string(),
                });

                Ok(JsonValue::obj([
                    ("status", JsonValue::String("submitted".to_string())),
                    ("pane_id", JsonValue::Number(target_id as i64)),
                    ("command", JsonValue::String(cmd.to_string())),
                ]))
            }
            "pane.read" => {
                let target_id = params
                    .get("pane_id")
                    .and_then(JsonValue::as_usize)
                    .unwrap_or_else(|| self.wm.active_pane_id());
                let pane = self
                    .wm
                    .pane(target_id)
                    .ok_or_else(|| format!("pane #{target_id} not found"))?;
                let (rows, cols) = pane.vt().size();
                let mut lines = Vec::with_capacity(rows as usize);
                for r in 0..rows {
                    lines.push(JsonValue::String(extract_vt_row_text(pane.vt(), r)));
                }
                let hints: Vec<JsonValue> = pane
                    .vt()
                    .detect_visible_hints()
                    .into_iter()
                    .map(|h| {
                        JsonValue::obj([
                            ("label", JsonValue::String(h.label)),
                            ("text", JsonValue::String(h.text)),
                            ("kind", JsonValue::String(format!("{:?}", h.kind))),
                            ("row", JsonValue::Number(i64::from(h.row))),
                            ("col", JsonValue::Number(i64::from(h.start_col))),
                        ])
                    })
                    .collect();
                let blocks: Vec<JsonValue> = pane
                    .vt()
                    .command_blocks()
                    .iter()
                    .map(|b| {
                        JsonValue::obj([
                            ("command", JsonValue::String(b.command.clone())),
                            ("row", JsonValue::Number(b.absolute_row as i64)),
                            (
                                "exit_code",
                                b.exit_code
                                    .map_or(JsonValue::Null, |c| JsonValue::Number(i64::from(c))),
                            ),
                            (
                                "duration_ms",
                                b.duration_ms
                                    .map_or(JsonValue::Null, |d| JsonValue::Number(d as i64)),
                            ),
                        ])
                    })
                    .collect();
                let notifications: Vec<JsonValue> = pane
                    .vt()
                    .notifications()
                    .iter()
                    .map(|n| {
                        JsonValue::obj([
                            ("title", JsonValue::String(n.title.clone())),
                            ("body", JsonValue::String(n.body.clone())),
                        ])
                    })
                    .collect();
                let (cur_r, cur_c) = pane.vt().cursor_position();

                Ok(JsonValue::obj([
                    ("pane_id", JsonValue::Number(target_id as i64)),
                    ("title", JsonValue::String(pane.title().to_string())),
                    ("rows", JsonValue::Number(i64::from(rows))),
                    ("cols", JsonValue::Number(i64::from(cols))),
                    ("cursor_row", JsonValue::Number(i64::from(cur_r))),
                    ("cursor_col", JsonValue::Number(i64::from(cur_c))),
                    (
                        "alternate_screen",
                        JsonValue::Bool(pane.vt().is_alternate_screen()),
                    ),
                    (
                        "echo_lock",
                        JsonValue::Bool(pane.is_password_prompt()),
                    ),
                    ("read_only", JsonValue::Bool(pane.is_read_only())),
                    ("recording", JsonValue::Bool(pane.vt().is_recording())),
                    ("lines", JsonValue::Array(lines)),
                    ("hints", JsonValue::Array(hints)),
                    ("command_blocks", JsonValue::Array(blocks)),
                    ("notifications", JsonValue::Array(notifications)),
                ]))
            }
            "pane.feed_output" => {
                let target_id = params
                    .get("pane_id")
                    .and_then(JsonValue::as_usize)
                    .unwrap_or_else(|| self.wm.active_pane_id());
                let data = params
                    .get("data")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| "missing 'data' parameter".to_string())?;
                self.feed_output_and_collect_events(target_id, data.as_bytes())?;
                Ok(JsonValue::obj([
                    ("status", JsonValue::String("processed".to_string())),
                    ("pane_id", JsonValue::Number(target_id as i64)),
                    ("bytes", JsonValue::Number(data.len() as i64)),
                ]))
            }
            "pane.send_keys" => {
                let mut chords = Vec::new();
                if let Some(arr) = params.get("keys").and_then(JsonValue::as_array) {
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            chords.push(s.to_string());
                        }
                    }
                } else if let Some(single) = params.get("keys").and_then(JsonValue::as_str) {
                    for token in single.split_whitespace() {
                        chords.push(token.to_string());
                    }
                }
                for chord in &chords {
                    let key_ev = parse_key_chord_str(chord)
                        .ok_or_else(|| format!("unrecognized key chord: {chord}"))?;
                    handle_wm_key_event(&mut self.wm, key_ev)
                        .map_err(|e| format!("key dispatch error: {e}"))?;
                }
                Ok(JsonValue::obj([
                    ("status", JsonValue::String("dispatched".to_string())),
                    ("count", JsonValue::Number(chords.len() as i64)),
                ]))
            }
            "wm.command" => {
                let raw_cmd = params
                    .get("command")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| "missing 'command' parameter".to_string())?;
                let normalized = if raw_cmd.starts_with(':') {
                    raw_cmd.to_string()
                } else {
                    format!(":{raw_cmd}")
                };
                let action = parse_colon_command(&normalized)
                    .ok_or_else(|| format!("not a valid :colon command: {normalized}"))??;
                let app_action = self
                    .wm
                    .apply(action)
                    .map_err(|e| format!("action execution error: {e}"))?;
                let outcome_str = if app_action == AppAction::Quit {
                    "quit"
                } else {
                    "handled"
                };
                Ok(JsonValue::obj([
                    ("status", JsonValue::String(outcome_str.to_string())),
                    ("command", JsonValue::String(normalized)),
                    (
                        "active_pane_id",
                        JsonValue::Number(self.wm.active_pane_id() as i64),
                    ),
                    (
                        "pane_count",
                        JsonValue::Number(self.wm.pane_count() as i64),
                    ),
                ]))
            }
            "events.poll" => {
                let drain = params
                    .get("drain")
                    .and_then(JsonValue::as_bool)
                    .unwrap_or(true);
                let items: Vec<JsonValue> = if drain {
                    self.events.drain(..).map(|e| e.to_json()).collect()
                } else {
                    self.events.iter().map(AutomationEvent::to_json).collect()
                };
                Ok(JsonValue::obj([("events", JsonValue::Array(items))]))
            }
            "playbook.run" => {
                let raw_toml = params
                    .get("toml")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| "missing 'toml' parameter".to_string())?;
                let playbook = Playbook::from_toml_str(raw_toml).map_err(|e| e.to_string())?;
                let report = self.run_playbook_with_policy(&playbook, SnapshotPolicy::Confined)?;
                Ok(JsonValue::obj([
                    ("name", JsonValue::String(report.name)),
                    ("passed", JsonValue::Bool(report.passed)),
                    (
                        "steps_completed",
                        JsonValue::Number(report.steps_completed as i64),
                    ),
                    (
                        "events",
                        JsonValue::Array(
                            report.events.iter().map(AutomationEvent::to_json).collect(),
                        ),
                    ),
                ]))
            }
            "remote.connect_ssh" => {
                let target = params
                    .get("target")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| "missing 'target' parameter".to_string())?;
                let port = params
                    .get("port")
                    .and_then(JsonValue::as_i64)
                    .and_then(|p| u16::try_from(p).ok());
                let tags: Vec<String> = params
                    .get("tags")
                    .and_then(JsonValue::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(ToString::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(tmux_sess) = params.get("tmux_session").and_then(JsonValue::as_str) {
                    let pane_id = self
                        .wm
                        .connect_ssh_tmux(target, tmux_sess, tags)
                        .map_err(|e| e.to_string())?;
                    return Ok(JsonValue::obj([
                        ("status", JsonValue::String("connected_ssh_tmux".to_string())),
                        ("pane_id", JsonValue::Number(pane_id as i64)),
                        ("target", JsonValue::String(target.to_string())),
                        ("tmux_session", JsonValue::String(tmux_sess.to_string())),
                    ]));
                }
                let pane_id = self
                    .wm
                    .spawn_ssh_pane(target, port, tags)
                    .map_err(|e| e.to_string())?;
                Ok(JsonValue::obj([
                    ("status", JsonValue::String("connected_ssh".to_string())),
                    ("pane_id", JsonValue::Number(pane_id as i64)),
                    ("target", JsonValue::String(target.to_string())),
                ]))
            }
            "remote.connect_tmux" => {
                let target = params
                    .get("target")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| "missing 'target' parameter".to_string())?;
                let session = params
                    .get("session")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("bilterm");
                let tags: Vec<String> = params
                    .get("tags")
                    .and_then(JsonValue::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(ToString::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                let pane_id = self
                    .wm
                    .connect_ssh_tmux(target, session, tags)
                    .map_err(|e| e.to_string())?;
                Ok(JsonValue::obj([
                    ("status", JsonValue::String("connected_ssh_tmux".to_string())),
                    ("pane_id", JsonValue::Number(pane_id as i64)),
                    ("target", JsonValue::String(target.to_string())),
                    ("session", JsonValue::String(session.to_string())),
                ]))
            }
            "fleet.register" => {
                let name = params
                    .get("name")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| "missing 'name' parameter".to_string())?;
                let destination = params
                    .get("destination")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| "missing 'destination' parameter".to_string())?;
                crate::remote::validate_destination(destination).map_err(|e| e.to_string())?;
                let mut profile = RemoteHostProfile::new(name, destination);
                if let Some(arr) = params.get("tags").and_then(JsonValue::as_array) {
                    profile.tags = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(ToString::to_string))
                        .collect();
                }
                if let Some(ep) = params.get("rpc_endpoint").and_then(JsonValue::as_str) {
                    profile.rpc_endpoint = Some(ep.to_string());
                }
                self.wm.fleet_mut().register_profile(profile);
                Ok(JsonValue::obj([
                    ("status", JsonValue::String("registered".to_string())),
                    ("name", JsonValue::String(name.to_string())),
                    ("node_count", JsonValue::Number(self.wm.fleet().nodes.len() as i64)),
                ]))
            }
            "fleet.list" => {
                let nodes: Vec<JsonValue> = self
                    .wm
                    .fleet()
                    .nodes
                    .values()
                    .map(|n| {
                        JsonValue::obj([
                            ("name", JsonValue::String(n.name.clone())),
                            ("connected", JsonValue::Bool(n.connected)),
                            (
                                "tags",
                                JsonValue::Array(
                                    n.tags.iter().cloned().map(JsonValue::String).collect(),
                                ),
                            ),
                            (
                                "last_command",
                                n.last_dispatched_command
                                    .as_ref()
                                    .map_or(JsonValue::Null, |c| JsonValue::String(c.clone())),
                            ),
                        ])
                    })
                    .collect();
                Ok(JsonValue::obj([("nodes", JsonValue::Array(nodes))]))
            }
            "fleet.exec" => {
                let selector_str = params
                    .get("selector")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("all");
                let command = params
                    .get("command")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| "missing 'command' parameter".to_string())?;
                let selector = FleetSelector::parse(selector_str);
                let panes = self.wm.exec_fleet_command(&selector, command);
                let matched_nodes = self.wm.fleet().matching_nodes(&selector);
                for &pid in &panes {
                    self.push_event(AutomationEvent::CommandSubmitted {
                        pane_id: pid,
                        command: command.to_string(),
                    });
                }
                Ok(JsonValue::obj([
                    ("status", JsonValue::String("dispatched".to_string())),
                    ("selector", JsonValue::String(selector_str.to_string())),
                    (
                        "panes",
                        JsonValue::Array(
                            panes
                                .into_iter()
                                .map(|id| JsonValue::Number(id as i64))
                                .collect(),
                        ),
                    ),
                    (
                        "nodes",
                        JsonValue::Array(
                            matched_nodes
                                .into_iter()
                                .map(JsonValue::String)
                                .collect(),
                        ),
                    ),
                ]))
            }
            "fleet.disconnect" => {
                let name = params
                    .get("name")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| "missing 'name' parameter".to_string())?;
                let removed = self.wm.disconnect_fleet_node(name);
                Ok(JsonValue::obj([
                    ("removed", JsonValue::Bool(removed)),
                    ("name", JsonValue::String(name.to_string())),
                ]))
            }
            "workspace.fontsize" => self.wm.handle_rpc_fontsize(params),
            _ => Err(format!("Method not found: {method}")),
        }
    }

    pub fn workspace_status_json(&self) -> JsonValue {
        let layout_str = match self.wm.layout_mode() {
            WorkspaceLayoutMode::Tiled => "tiled",
            WorkspaceLayoutMode::Stacked => "stacked",
        };
        let mut panes = Vec::new();
        for (&id, pane) in self.wm.panes() {
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
                JsonValue::Number(self.wm.pane_count() as i64),
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

