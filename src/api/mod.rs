//! Automation, Robotics, JSON-RPC 2.0 Control Plane, Native MCP Server & Declarative Playbook Engine (`v1.1.0`–`v1.3.0`).
//!
//! Exposes 100% of Bilterm's `WindowManager`, `CompositePane`, `VtParser`, and `:colon` command
//! capabilities over:
//! 1. **JSON-RPC 2.0 Control Plane** (`AutomationControlPlane`, `run_headless_tcp_server`, `bilterm ctl`)
//! 2. **Model Context Protocol (MCP) Server** (`bilterm --mcp` over `stdio` JSON-RPC 2.0)
//! 3. **Real-Time Event Subscription Bus & Declarative Robot Playbooks** (`AutomationEvent`, `Playbook`, `bilterm --playbook`)

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

use crate::error::{BiltermError, Result};
use crate::input::handle_wm_key_event;
use crate::pane::colon_command::{execute_wm_colon_command, parse_colon_command};
use crate::pty::line_discipline::InputRoutingMode;
use crate::remote::{FleetSelector, RemoteHostProfile};
use crate::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer, WorkspaceFrameRenderer};
use crate::vt::VtParser;
use crate::wm::{WindowManager, WorkspaceLayoutMode};

/// Lightweight, zero-external-dependency RFC 8259 JSON AST used by the JSON-RPC 2.0 and MCP servers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(i64),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    pub fn obj(entries: impl IntoIterator<Item = (impl Into<String>, JsonValue)>) -> Self {
        let mut map = BTreeMap::new();
        for (k, v) in entries {
            map.insert(k.into(), v);
        }
        Self::Object(map)
    }

    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            Self::Object(map) => map.get(key),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_usize(&self) -> Option<usize> {
        self.as_i64().and_then(|n| usize::try_from(n).ok())
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            Self::Array(arr) => Some(arr.as_slice()),
            _ => None,
        }
    }

    /// Serializes this value into a compact RFC 8259 JSON string.
    pub fn to_json_string(&self) -> String {
        let mut out = String::new();
        self.write_json(&mut out);
        out
    }

    fn write_json(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(true) => out.push_str("true"),
            Self::Bool(false) => out.push_str("false"),
            Self::Number(n) => out.push_str(&n.to_string()),
            Self::String(s) => {
                out.push('"');
                for ch in s.chars() {
                    match ch {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        c if (c as u32) < 0x20 => {
                            out.push_str(&format!("\\u{:04x}", c as u32));
                        }
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            Self::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write_json(out);
                }
                out.push(']');
            }
            Self::Object(map) => {
                out.push('{');
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    Self::String(k.clone()).write_json(out);
                    out.push(':');
                    v.write_json(out);
                }
                out.push('}');
            }
        }
    }

    /// Parses an RFC 8259 JSON string into a `JsonValue`.
    pub fn parse(input: &str) -> Option<Self> {
        let bytes = input.as_bytes();
        let mut pos = 0;
        let val = parse_json_value(bytes, &mut pos)?;
        skip_ws(bytes, &mut pos);
        if pos == bytes.len() { Some(val) } else { None }
    }
}

fn skip_ws(bytes: &[u8], pos: &mut usize) {
    while *pos < bytes.len() && matches!(bytes[*pos], b' ' | b'\t' | b'\r' | b'\n') {
        *pos += 1;
    }
}

fn parse_json_value(bytes: &[u8], pos: &mut usize) -> Option<JsonValue> {
    skip_ws(bytes, pos);
    if *pos >= bytes.len() {
        return None;
    }
    match bytes[*pos] {
        b'n' if bytes[*pos..].starts_with(b"null") => {
            *pos += 4;
            Some(JsonValue::Null)
        }
        b't' if bytes[*pos..].starts_with(b"true") => {
            *pos += 4;
            Some(JsonValue::Bool(true))
        }
        b'f' if bytes[*pos..].starts_with(b"false") => {
            *pos += 5;
            Some(JsonValue::Bool(false))
        }
        b'"' => parse_json_string(bytes, pos).map(JsonValue::String),
        b'[' => {
            *pos += 1;
            skip_ws(bytes, pos);
            let mut arr = Vec::new();
            if *pos < bytes.len() && bytes[*pos] == b']' {
                *pos += 1;
                return Some(JsonValue::Array(arr));
            }
            loop {
                arr.push(parse_json_value(bytes, pos)?);
                skip_ws(bytes, pos);
                if *pos >= bytes.len() {
                    return None;
                }
                match bytes[*pos] {
                    b',' => *pos += 1,
                    b']' => {
                        *pos += 1;
                        return Some(JsonValue::Array(arr));
                    }
                    _ => return None,
                }
            }
        }
        b'{' => {
            *pos += 1;
            skip_ws(bytes, pos);
            let mut map = BTreeMap::new();
            if *pos < bytes.len() && bytes[*pos] == b'}' {
                *pos += 1;
                return Some(JsonValue::Object(map));
            }
            loop {
                skip_ws(bytes, pos);
                let key = parse_json_string(bytes, pos)?;
                skip_ws(bytes, pos);
                if *pos >= bytes.len() || bytes[*pos] != b':' {
                    return None;
                }
                *pos += 1;
                let val = parse_json_value(bytes, pos)?;
                map.insert(key, val);
                skip_ws(bytes, pos);
                if *pos >= bytes.len() {
                    return None;
                }
                match bytes[*pos] {
                    b',' => *pos += 1,
                    b'}' => {
                        *pos += 1;
                        return Some(JsonValue::Object(map));
                    }
                    _ => return None,
                }
            }
        }
        b'-' | b'0'..=b'9' => {
            let start = *pos;
            if bytes[*pos] == b'-' {
                *pos += 1;
            }
            while *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
                *pos += 1;
            }
            let int_end = *pos;
            while *pos < bytes.len()
                && matches!(bytes[*pos], b'.' | b'e' | b'E' | b'+' | b'-' | b'0'..=b'9')
            {
                *pos += 1;
            }
            let num_str = std::str::from_utf8(&bytes[start..int_end]).ok()?;
            let n = num_str.parse::<i64>().ok()?;
            Some(JsonValue::Number(n))
        }
        _ => None,
    }
}

fn parse_json_string(bytes: &[u8], pos: &mut usize) -> Option<String> {
    if *pos >= bytes.len() || bytes[*pos] != b'"' {
        return None;
    }
    *pos += 1;
    let mut out = String::new();
    while *pos < bytes.len() {
        let b = bytes[*pos];
        *pos += 1;
        match b {
            b'"' => return Some(out),
            b'\\' => {
                if *pos >= bytes.len() {
                    return None;
                }
                let esc = bytes[*pos];
                *pos += 1;
                match esc {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000c}'),
                    b'u' => {
                        if *pos + 4 > bytes.len() {
                            return None;
                        }
                        let hex = std::str::from_utf8(&bytes[*pos..*pos + 4]).ok()?;
                        *pos += 4;
                        let code = u32::from_str_radix(hex, 16).ok()?;
                        out.push(char::from_u32(code)?);
                    }
                    _ => out.push(esc as char),
                }
            }
            _ => {
                let start = *pos - 1;
                let rest = std::str::from_utf8(&bytes[start..]).ok()?;
                let ch = rest.chars().next()?;
                *pos = start + ch.len_utf8();
                out.push(ch);
            }
        }
    }
    None
}

fn extract_vt_row_text(vt: &VtParser, row: u16) -> String {
    let (_, cols) = vt.size();
    let screen = vt.screen();
    let mut line = String::with_capacity(usize::from(cols));
    for c in 0..cols {
        if let Some(cell) = screen.cell(row, c) {
            if cell.is_wide_continuation() {
                continue;
            }
            let contents = cell.contents();
            if contents.is_empty() {
                line.push(' ');
            } else {
                line.push_str(contents);
            }
        } else {
            line.push(' ');
        }
    }
    line.trim_end().to_string()
}

/// Structured automation event emitted by the Bilterm Control Plane (`v1.3.0`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutomationEvent {
    CommandSubmitted {
        pane_id: usize,
        command: String,
    },
    CommandCompleted {
        pane_id: usize,
        command: String,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    },
    TriggerMatched {
        pane_id: usize,
        pattern: String,
        row: u16,
        line: String,
    },
    NotificationReceived {
        pane_id: usize,
        title: String,
        body: String,
    },
    EchoLockChanged {
        pane_id: usize,
        locked: bool,
    },
    AlternateScreenChanged {
        pane_id: usize,
        active: bool,
    },
}

impl AutomationEvent {
    pub fn to_json(&self) -> JsonValue {
        match self {
            Self::CommandSubmitted { pane_id, command } => JsonValue::obj([
                ("type", JsonValue::String("command_submitted".to_string())),
                ("pane_id", JsonValue::Number(*pane_id as i64)),
                ("command", JsonValue::String(command.clone())),
            ]),
            Self::CommandCompleted {
                pane_id,
                command,
                exit_code,
                duration_ms,
            } => JsonValue::obj([
                ("type", JsonValue::String("command_completed".to_string())),
                ("pane_id", JsonValue::Number(*pane_id as i64)),
                ("command", JsonValue::String(command.clone())),
                (
                    "exit_code",
                    exit_code.map_or(JsonValue::Null, |c| JsonValue::Number(i64::from(c))),
                ),
                (
                    "duration_ms",
                    duration_ms.map_or(JsonValue::Null, |d| JsonValue::Number(d as i64)),
                ),
            ]),
            Self::TriggerMatched {
                pane_id,
                pattern,
                row,
                line,
            } => JsonValue::obj([
                ("type", JsonValue::String("trigger_matched".to_string())),
                ("pane_id", JsonValue::Number(*pane_id as i64)),
                ("pattern", JsonValue::String(pattern.clone())),
                ("row", JsonValue::Number(i64::from(*row))),
                ("line", JsonValue::String(line.clone())),
            ]),
            Self::NotificationReceived {
                pane_id,
                title,
                body,
            } => JsonValue::obj([
                (
                    "type",
                    JsonValue::String("notification_received".to_string()),
                ),
                ("pane_id", JsonValue::Number(*pane_id as i64)),
                ("title", JsonValue::String(title.clone())),
                ("body", JsonValue::String(body.clone())),
            ]),
            Self::EchoLockChanged { pane_id, locked } => JsonValue::obj([
                ("type", JsonValue::String("echo_lock_changed".to_string())),
                ("pane_id", JsonValue::Number(*pane_id as i64)),
                ("locked", JsonValue::Bool(*locked)),
            ]),
            Self::AlternateScreenChanged { pane_id, active } => JsonValue::obj([
                (
                    "type",
                    JsonValue::String("alternate_screen_changed".to_string()),
                ),
                ("pane_id", JsonValue::Number(*pane_id as i64)),
                ("active", JsonValue::Bool(*active)),
            ]),
        }
    }
}

/// Declarative automation playbook (`bilterm --playbook <file.toml>`) (`v1.3.0`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Playbook {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub steps: Vec<PlaybookStep>,
}

/// Single step in a declarative automation playbook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookStep {
    /// Action type: `"colon"`, `"exec"`, `"send_keys"`, `"feed_output"`,
    /// `"assert_screen_contains"`, `"assert_trigger_fired"`, `"assert_exit_code"`, or `"export_ppm"`.
    pub action: String,
    #[serde(default)]
    pub pane_id: Option<usize>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub data: Option<String>,
    #[serde(default)]
    pub expected: Option<String>,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub path: Option<String>,
}

/// Result of executing a declarative automation playbook.
#[derive(Debug, Clone)]
pub struct PlaybookReport {
    pub name: String,
    pub passed: bool,
    pub steps_completed: usize,
    pub failure_reason: Option<String>,
    pub events: Vec<AutomationEvent>,
}

impl Playbook {
    pub fn from_toml_str(raw: &str) -> Result<Self> {
        toml::from_str(raw).map_err(|e| BiltermError::Config(format!("invalid playbook TOML: {e}")))
    }
}

/// Stateful automation control plane wrapping a `WindowManager` and an event subscription ring buffer.
pub struct AutomationControlPlane {
    pub wm: WindowManager,
    pub events: VecDeque<AutomationEvent>,
    pub viewport: Rect,
}

impl AutomationControlPlane {
    pub fn new(wm: WindowManager, viewport: Rect) -> Self {
        Self {
            wm,
            events: VecDeque::with_capacity(256),
            viewport,
        }
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

    /// Handles a single Model Context Protocol (MCP) JSON-RPC 2.0 message line (`bilterm --mcp`).
    /// Returns `Some(response_json_line)` for requests, or `None` for notifications.
    pub fn handle_mcp_line(&mut self, line: &str) -> Option<String> {
        let req = JsonValue::parse(line)?;
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(JsonValue::as_str)?;
        let params = req
            .get("params")
            .cloned()
            .unwrap_or_else(|| JsonValue::Object(BTreeMap::new()));

        let id = id?;

        let result = match method {
            "initialize" => Ok(JsonValue::obj([
                (
                    "protocolVersion",
                    JsonValue::String("2024-11-05".to_string()),
                ),
                (
                    "capabilities",
                    JsonValue::obj([(
                        "tools",
                        JsonValue::obj([("listChanged", JsonValue::Bool(false))]),
                    )]),
                ),
                (
                    "serverInfo",
                    JsonValue::obj([
                        ("name", JsonValue::String("bilterm-mcp".to_string())),
                        (
                            "version",
                            JsonValue::String(env!("CARGO_PKG_VERSION").to_string()),
                        ),
                    ]),
                ),
            ])),
            "ping" => Ok(JsonValue::Object(BTreeMap::new())),
            "tools/list" => Ok(Self::mcp_tools_list()),
            "tools/call" => {
                let tool_name = params.get("name").and_then(JsonValue::as_str).unwrap_or("");
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| JsonValue::Object(BTreeMap::new()));
                let mapped_method = match tool_name {
                    "bilterm_workspace_status" => Some("workspace.status"),
                    "bilterm_exec_command" => Some("pane.exec"),
                    "bilterm_inspect_pane" => Some("pane.read"),
                    "bilterm_send_keys" => Some("pane.send_keys"),
                    "bilterm_colon_command" => Some("wm.command"),
                    "bilterm_run_playbook" => Some("playbook.run"),
                    "bilterm_remote_connect" => Some("remote.connect_ssh"),
                    "bilterm_fleet_exec" => Some("fleet.exec"),
                    _ => None,
                };
                match mapped_method {
                    Some(rpc_method) => match self.dispatch_method(rpc_method, &args) {
                        Ok(val) => Ok(JsonValue::obj([
                            (
                                "content",
                                JsonValue::Array(vec![JsonValue::obj([
                                    ("type", JsonValue::String("text".to_string())),
                                    ("text", JsonValue::String(val.to_json_string())),
                                ])]),
                            ),
                            ("isError", JsonValue::Bool(false)),
                        ])),
                        Err(err_msg) => Ok(JsonValue::obj([
                            (
                                "content",
                                JsonValue::Array(vec![JsonValue::obj([
                                    ("type", JsonValue::String("text".to_string())),
                                    ("text", JsonValue::String(err_msg)),
                                ])]),
                            ),
                            ("isError", JsonValue::Bool(true)),
                        ])),
                    },
                    None => Err(format!("Unknown MCP tool: {tool_name}")),
                }
            }
            _ => Err(format!("Unsupported MCP method: {method}")),
        };

        Some(match result {
            Ok(res) => JsonValue::obj([
                ("jsonrpc", JsonValue::String("2.0".to_string())),
                ("id", id),
                ("result", res),
            ])
            .to_json_string(),
            Err(msg) => Self::error_response(id, -32601, &msg).to_json_string(),
        })
    }

    pub fn mcp_tools_list() -> JsonValue {
        let tools = vec![
            Self::mcp_tool_schema(
                "bilterm_workspace_status",
                "Inspect all Bilterm composite panes, layout topology (Tiled/Stacked), focused pane, and 3270 OIA telemetry.",
                &[],
            ),
            Self::mcp_tool_schema(
                "bilterm_exec_command",
                "Submit a shell command into a Bilterm composite pane (or broadcast across panes) with !ECHO password-lock safety.",
                &[
                    ("command", "string", "Shell command to execute"),
                    ("pane_id", "integer", "Optional target pane ID (defaults to active pane)"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_inspect_pane",
                "Read visible terminal lines, cursor coordinates, OSC 133 command blocks, 3270 field hints (URLs, paths, IPs, SHAs), and notifications.",
                &[("pane_id", "integer", "Optional target pane ID (defaults to active pane)")],
            ),
            Self::mcp_tool_schema(
                "bilterm_send_keys",
                "Send synthetic key chords (e.g. Ctrl+C, F12, Enter, Up, Alt+H) or text to drive interactive TUIs.",
                &[("keys", "string", "Space-separated key chords or single key chord")],
            ),
            Self::mcp_tool_schema(
                "bilterm_colon_command",
                "Execute any Bilterm :colon multiplexer command (e.g. :split v, :layout stack, :focus 2, :theme 3278, :trigger add ERROR).",
                &[("command", "string", "Bilterm :colon command")],
            ),
            Self::mcp_tool_schema(
                "bilterm_run_playbook",
                "Execute a declarative TOML automation playbook against the Bilterm workspace.",
                &[("toml", "string", "Declarative playbook TOML string")],
            ),
            Self::mcp_tool_schema(
                "bilterm_remote_connect",
                "Spawn a remote SSH or remote tmux -CC CompositePane and register it in Bilterm's central fleet registry.",
                &[
                    ("target", "string", "SSH destination (user@host) or configured remote profile name"),
                    ("tmux_session", "string", "Optional remote tmux -CC session name to attach"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_fleet_exec",
                "Execute a shell command across local and remote SSH sessions matching a herdr-style selector (all, local, remote, @tag, or node name).",
                &[
                    ("selector", "string", "Fleet target selector: all, local, remote, @tag, or node name"),
                    ("command", "string", "Shell command to fan out across matching sessions"),
                ],
            ),
        ];
        JsonValue::obj([("tools", JsonValue::Array(tools))])
    }

    fn mcp_tool_schema(name: &str, description: &str, props: &[(&str, &str, &str)]) -> JsonValue {
        let mut properties = BTreeMap::new();
        for (prop_name, prop_type, prop_desc) in props {
            properties.insert(
                (*prop_name).to_string(),
                JsonValue::obj([
                    ("type", JsonValue::String((*prop_type).to_string())),
                    ("description", JsonValue::String((*prop_desc).to_string())),
                ]),
            );
        }
        JsonValue::obj([
            ("name", JsonValue::String(name.to_string())),
            ("description", JsonValue::String(description.to_string())),
            (
                "inputSchema",
                JsonValue::obj([
                    ("type", JsonValue::String("object".to_string())),
                    ("properties", JsonValue::Object(properties)),
                ]),
            ),
        ])
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
                    let mut renderer = GuiWorkspaceRenderer::default();
                    let mut surface = GuiFrameSurface::new(8, 16);
                    renderer.render_workspace(&self.wm, self.viewport, &mut surface);
                    std::fs::write(ppm_path, surface.export_ppm())
                        .map_err(|e| format!("failed to write PPM snapshot: {e}"))?;
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
                    .panes
                    .get_mut(&target_id)
                    .ok_or_else(|| format!("pane #{target_id} not found"))?;
                if pane.read_only {
                    return Err(format!("pane #{target_id} is locked read-only (🔒 PROT)"));
                }
                if pane.line_discipline.is_password_prompt() && !allow_echo_lock {
                    return Err(format!(
                        "pane #{target_id} is in !ECHO LOCK password prompt; set allow_echo_lock=true to override"
                    ));
                }
                pane.prompt.history.push(cmd.to_string());
                let _ = pane.history_store.record_command(cmd);
                pane.prompt.running_command = Some(cmd.to_string());
                pane.vt.record_command_submission(cmd);
                if let Some(pty) = &pane.pty {
                    let mut payload = cmd.as_bytes().to_vec();
                    payload.push(b'\n');
                    let _ = pty.write_all(&payload);
                }

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
                    .panes
                    .get(&target_id)
                    .ok_or_else(|| format!("pane #{target_id} not found"))?;
                let (rows, cols) = pane.vt.size();
                let mut lines = Vec::with_capacity(rows as usize);
                for r in 0..rows {
                    lines.push(JsonValue::String(extract_vt_row_text(&pane.vt, r)));
                }
                let hints: Vec<JsonValue> = pane
                    .vt
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
                    .vt
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
                    .vt
                    .notifications()
                    .iter()
                    .map(|n| {
                        JsonValue::obj([
                            ("title", JsonValue::String(n.title.clone())),
                            ("body", JsonValue::String(n.body.clone())),
                        ])
                    })
                    .collect();
                let (cur_r, cur_c) = pane.vt.screen().cursor_position();

                Ok(JsonValue::obj([
                    ("pane_id", JsonValue::Number(target_id as i64)),
                    ("title", JsonValue::String(pane.title.clone())),
                    ("rows", JsonValue::Number(i64::from(rows))),
                    ("cols", JsonValue::Number(i64::from(cols))),
                    ("cursor_row", JsonValue::Number(i64::from(cur_r))),
                    ("cursor_col", JsonValue::Number(i64::from(cur_c))),
                    ("alternate_screen", JsonValue::Bool(pane.vt.is_alternate_screen())),
                    (
                        "echo_lock",
                        JsonValue::Bool(pane.line_discipline.is_password_prompt()),
                    ),
                    ("read_only", JsonValue::Bool(pane.read_only)),
                    ("recording", JsonValue::Bool(pane.vt.is_recording())),
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
                let parsed = parse_colon_command(&normalized)
                    .ok_or_else(|| format!("not a valid :colon command: {normalized}"))??;
                let should_quit = execute_wm_colon_command(&mut self.wm, parsed);
                let outcome_str = if should_quit { "quit" } else { "handled" };
                Ok(JsonValue::obj([
                    ("status", JsonValue::String(outcome_str.to_string())),
                    ("command", JsonValue::String(normalized)),
                    (
                        "active_pane_id",
                        JsonValue::Number(self.wm.active_pane_id() as i64),
                    ),
                    (
                        "pane_count",
                        JsonValue::Number(self.wm.panes.len() as i64),
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
                let report = self.run_playbook(&playbook)?;
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
                self.wm.fleet.register_profile(profile);
                Ok(JsonValue::obj([
                    ("status", JsonValue::String("registered".to_string())),
                    ("name", JsonValue::String(name.to_string())),
                    ("node_count", JsonValue::Number(self.wm.fleet.nodes.len() as i64)),
                ]))
            }
            "fleet.list" => {
                let nodes: Vec<JsonValue> = self
                    .wm
                    .fleet
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
                let matched_nodes = self.wm.fleet.matching_nodes(&selector);
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
                let removed = self.wm.fleet.disconnect(name);
                Ok(JsonValue::obj([
                    ("removed", JsonValue::Bool(removed)),
                    ("name", JsonValue::String(name.to_string())),
                ]))
            }
            _ => Err(format!("Method not found: {method}")),
        }
    }

    /// Feeds raw VT/ANSI output bytes into a pane and emits structured `AutomationEvent` notifications.
    pub fn feed_output_and_collect_events(
        &mut self,
        pane_id: usize,
        bytes: &[u8],
    ) -> std::result::Result<(), String> {
        let triggers = self.wm.config.triggers.clone();
        let pane = self
            .wm
            .panes
            .get_mut(&pane_id)
            .ok_or_else(|| format!("pane #{pane_id} not found"))?;
        let prev_alt = pane.vt.is_alternate_screen();
        let prev_blocks = pane.vt.command_blocks().len();
        let prev_prev_exit = pane
            .vt
            .command_blocks()
            .last()
            .and_then(|b| b.exit_code);
        let prev_notifs = pane.vt.notifications().len();

        pane.process_pty_output(bytes);

        let now_alt = pane.vt.is_alternate_screen();
        let new_notifs: Vec<(String, String)> = pane
            .vt
            .notifications()
            .iter()
            .skip(prev_notifs)
            .map(|n| (n.title.clone(), n.body.clone()))
            .collect();
        let completed_block = pane.vt.command_blocks().last().and_then(|b| {
            if (pane.vt.command_blocks().len() > prev_blocks || b.exit_code != prev_prev_exit)
                && b.exit_code.is_some()
            {
                Some((b.command.clone(), b.exit_code, b.duration_ms))
            } else {
                None
            }
        });

        let mut matched_triggers = Vec::new();
        if !triggers.is_empty() {
            let (rows, _) = pane.vt.size();
            for r in 0..rows {
                let row_str = extract_vt_row_text(&pane.vt, r);
                let row_lower = row_str.to_ascii_lowercase();
                for pat in &triggers {
                    if !pat.is_empty() && row_lower.contains(&pat.to_ascii_lowercase()) {
                        matched_triggers.push((pat.clone(), r, row_str.clone()));
                    }
                }
            }
        }

        if now_alt != prev_alt {
            self.push_event(AutomationEvent::AlternateScreenChanged {
                pane_id,
                active: now_alt,
            });
        }
        for (title, body) in new_notifs {
            self.push_event(AutomationEvent::NotificationReceived {
                pane_id,
                title,
                body,
            });
        }
        if let Some((command, exit_code, duration_ms)) = completed_block {
            self.push_event(AutomationEvent::CommandCompleted {
                pane_id,
                command,
                exit_code,
                duration_ms,
            });
        }
        for (pattern, row, line) in matched_triggers {
            self.push_event(AutomationEvent::TriggerMatched {
                pane_id,
                pattern,
                row,
                line,
            });
        }
        Ok(())
    }

    /// Executes a declarative `Playbook` step-by-step and returns a `PlaybookReport`.
    pub fn run_playbook(
        &mut self,
        playbook: &Playbook,
    ) -> std::result::Result<PlaybookReport, String> {
        let mut completed = 0;
        for (idx, step) in playbook.steps.iter().enumerate() {
            let step_num = idx + 1;
            match step.action.as_str() {
                "colon" => {
                    let cmd = step
                        .command
                        .as_deref()
                        .ok_or_else(|| format!("step {step_num}: 'colon' requires 'command'"))?;
                    self.dispatch_method(
                        "wm.command",
                        &JsonValue::obj([("command", JsonValue::String(cmd.to_string()))]),
                    )?;
                }
                "exec" => {
                    let cmd = step
                        .command
                        .as_deref()
                        .ok_or_else(|| format!("step {step_num}: 'exec' requires 'command'"))?;
                    let pane_id = step.pane_id.unwrap_or_else(|| self.wm.active_pane_id());
                    self.dispatch_method(
                        "pane.exec",
                        &JsonValue::obj([
                            ("pane_id", JsonValue::Number(pane_id as i64)),
                            ("command", JsonValue::String(cmd.to_string())),
                        ]),
                    )?;
                }
                "send_keys" => {
                    let arr: Vec<JsonValue> =
                        step.keys.iter().cloned().map(JsonValue::String).collect();
                    self.dispatch_method(
                        "pane.send_keys",
                        &JsonValue::obj([("keys", JsonValue::Array(arr))]),
                    )?;
                }
                "feed_output" => {
                    let pane_id = step.pane_id.unwrap_or_else(|| self.wm.active_pane_id());
                    let data = step.data.as_deref().unwrap_or("");
                    self.feed_output_and_collect_events(pane_id, data.as_bytes())?;
                }
                "assert_screen_contains" => {
                    let pane_id = step.pane_id.unwrap_or_else(|| self.wm.active_pane_id());
                    let expected = step.expected.as_deref().unwrap_or("");
                    let pane = self
                        .wm
                        .panes
                        .get(&pane_id)
                        .ok_or_else(|| format!("step {step_num}: pane #{pane_id} not found"))?;
                    let (rows, _) = pane.vt.size();
                    let found = (0..rows).any(|r| extract_vt_row_text(&pane.vt, r).contains(expected));
                    if !found {
                        return Err(format!(
                            "step {step_num}: assertion failed — pane #{pane_id} screen does not contain '{expected}'"
                        ));
                    }
                }
                "assert_trigger_fired" => {
                    let pat = step.pattern.as_deref().unwrap_or("");
                    let fired = self.events.iter().any(|ev| {
                        matches!(
                            ev,
                            AutomationEvent::TriggerMatched { pattern, .. }
                            if pattern.eq_ignore_ascii_case(pat)
                        )
                    });
                    if !fired {
                        return Err(format!(
                            "step {step_num}: assertion failed — trigger '{pat}' did not fire"
                        ));
                    }
                }
                "assert_exit_code" => {
                    let pane_id = step.pane_id.unwrap_or_else(|| self.wm.active_pane_id());
                    let expected_code = step.exit_code.unwrap_or(0);
                    let pane = self
                        .wm
                        .panes
                        .get(&pane_id)
                        .ok_or_else(|| format!("step {step_num}: pane #{pane_id} not found"))?;
                    let actual = pane.vt.command_blocks().last().and_then(|b| b.exit_code);
                    if actual != Some(expected_code) {
                        return Err(format!(
                            "step {step_num}: assertion failed — expected exit_code {expected_code}, got {actual:?}"
                        ));
                    }
                }
                "export_ppm" => {
                    let path = step
                        .path
                        .as_deref()
                        .ok_or_else(|| format!("step {step_num}: 'export_ppm' requires 'path'"))?;
                    self.dispatch_method(
                        "workspace.snapshot",
                        &JsonValue::obj([("ppm_path", JsonValue::String(path.to_string()))]),
                    )?;
                }
                other => {
                    return Err(format!("step {step_num}: unknown playbook action '{other}'"));
                }
            }
            completed += 1;
        }
        Ok(PlaybookReport {
            name: playbook.name.clone(),
            passed: true,
            steps_completed: completed,
            failure_reason: None,
            events: self.events.iter().cloned().collect(),
        })
    }

    pub fn workspace_status_json(&self) -> JsonValue {
        let layout_str = match self.wm.layout_mode {
            WorkspaceLayoutMode::Tiled => "tiled",
            WorkspaceLayoutMode::Stacked => "stacked",
        };
        let mut panes = Vec::new();
        for (&id, pane) in &self.wm.panes {
            let (rows, cols) = pane.vt.size();
            let (cur_r, cur_c) = pane.vt.screen().cursor_position();
            let input_mode = match pane.effective_routing_mode() {
                InputRoutingMode::CommandPrompt => "cmd_box",
                InputRoutingMode::RawPtyPassthrough => "raw_pty",
            };
            panes.push(JsonValue::obj([
                ("id", JsonValue::Number(id as i64)),
                ("title", JsonValue::String(pane.title.clone())),
                ("rows", JsonValue::Number(i64::from(rows))),
                ("cols", JsonValue::Number(i64::from(cols))),
                ("cursor_row", JsonValue::Number(i64::from(cur_r))),
                ("cursor_col", JsonValue::Number(i64::from(cur_c))),
                ("input_target", JsonValue::String(input_mode.to_string())),
                ("alternate_screen", JsonValue::Bool(pane.vt.is_alternate_screen())),
                (
                    "echo_lock",
                    JsonValue::Bool(pane.line_discipline.is_password_prompt()),
                ),
                ("read_only", JsonValue::Bool(pane.read_only)),
                ("recording", JsonValue::Bool(pane.vt.is_recording())),
                (
                    "notifications",
                    JsonValue::Number(pane.vt.notifications().len() as i64),
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
                JsonValue::Number(self.wm.panes.len() as i64),
            ),
            ("broadcast_input", JsonValue::Bool(self.wm.broadcast_input)),
            (
                "menu_bar_visible",
                JsonValue::Bool(self.wm.config.show_top_menubar),
            ),
            (
                "crt_scanlines",
                JsonValue::Bool(self.wm.config.crt_scanlines),
            ),
            (
                "palette_visible",
                JsonValue::Bool(self.wm.palette_visible),
            ),
            ("panes", JsonValue::Array(panes)),
        ])
    }

    fn error_response(id: JsonValue, code: i64, message: &str) -> JsonValue {
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

/// Parses a human-readable key chord string (e.g. `"Ctrl+C"`, `"Alt+H"`, `"F9"`, `"Enter"`, `"Up"`) into a `KeyEvent`.
pub fn parse_key_chord_str(raw: &str) -> Option<KeyEvent> {
    let mut modifiers = KeyModifiers::empty();
    let parts: Vec<&str> = raw.split('+').collect();
    let key_part = parts.last()?.trim();
    for &mod_part in &parts[..parts.len().saturating_sub(1)] {
        match mod_part.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
            "alt" | "meta" => modifiers |= KeyModifiers::ALT,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            _ => return None,
        }
    }
    let code = match key_part.to_ascii_lowercase().as_str() {
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "backspace" => KeyCode::Backspace,
        "space" => KeyCode::Char(' '),
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "f1" => KeyCode::F(1),
        "f2" => KeyCode::F(2),
        "f3" => KeyCode::F(3),
        "f4" => KeyCode::F(4),
        "f5" => KeyCode::F(5),
        "f6" => KeyCode::F(6),
        "f7" => KeyCode::F(7),
        "f8" => KeyCode::F(8),
        "f9" => KeyCode::F(9),
        "f10" => KeyCode::F(10),
        "f11" => KeyCode::F(11),
        "f12" => KeyCode::F(12),
        s if s.chars().count() == 1 => KeyCode::Char(s.chars().next()?),
        _ => return None,
    };
    Some(KeyEvent::new(code, modifiers))
}

/// Sends a single line-delimited JSON-RPC 2.0 request to a TCP (`host:port`) or Unix domain socket path
/// and returns the JSON-RPC response string (`bilterm ctl`).
pub async fn send_rpc_request(endpoint: &str, method: &str, params: JsonValue) -> Result<String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let req = JsonValue::obj([
        ("jsonrpc", JsonValue::String("2.0".to_string())),
        ("id", JsonValue::Number(1)),
        ("method", JsonValue::String(method.to_string())),
        ("params", params),
    ])
    .to_json_string();

    if endpoint.contains(':') && !Path::new(endpoint).exists() {
        let stream = tokio::net::TcpStream::connect(endpoint).await?;
        let (rd, mut wr) = stream.into_split();
        wr.write_all(format!("{req}\n").as_bytes()).await?;
        wr.flush().await?;
        let mut lines = BufReader::new(rd).lines();
        Ok(lines.next_line().await?.unwrap_or_default())
    } else {
        #[cfg(unix)]
        {
            let path = endpoint.strip_prefix("unix://").unwrap_or(endpoint);
            let stream = tokio::net::UnixStream::connect(path).await?;
            let (rd, mut wr) = stream.into_split();
            wr.write_all(format!("{req}\n").as_bytes()).await?;
            wr.flush().await?;
            let mut lines = BufReader::new(rd).lines();
            Ok(lines.next_line().await?.unwrap_or_default())
        }
        #[cfg(not(unix))]
        {
            Err(BiltermError::Config(
                "Use host:port TCP endpoint on non-Unix platforms".to_string(),
            ))
        }
    }
}

/// Runs the Model Context Protocol (`bilterm --mcp`) JSON-RPC 2.0 server over `stdin`/`stdout`.
pub async fn run_mcp_stdio_server(rows: u16, cols: u16) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let mut plane = AutomationControlPlane::new_headless(rows, cols);
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut lines = BufReader::new(stdin).lines();

    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(resp) = plane.handle_mcp_line(trimmed) {
            stdout.write_all(format!("{resp}\n").as_bytes()).await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}

/// Runs a headless JSON-RPC 2.0 TCP automation server (`bilterm --headless --listen 127.0.0.1:PORT`).
pub async fn run_headless_tcp_server(bind_addr: &str, rows: u16, cols: u16) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    let mut plane = AutomationControlPlane::new_headless(rows, cols);

    loop {
        let (stream, _) = listener.accept().await?;
        let (rd, mut wr) = stream.into_split();
        let mut lines = BufReader::new(rd).lines();
        while let Some(line) = lines.next_line().await? {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let resp = plane.handle_jsonrpc_line(trimmed);
            wr.write_all(format!("{resp}\n").as_bytes()).await?;
            wr.flush().await?;
        }
    }
}
