//! Native Model Context Protocol (MCP) Server (`bilterm --mcp` over `stdio` JSON-RPC 2.0).

use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::json::JsonValue;
use super::rpc::AutomationControlPlane;
use crate::error::Result;

impl AutomationControlPlane {
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
                    "bilterm_workspace_fontsize" => Some("workspace.fontsize"),
                    "bilterm_canvas" => Some("canvas.command"),
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
            Self::mcp_tool_schema(
                "bilterm_workspace_fontsize",
                "Query or dynamically adjust terminal font size and scale percentage (in, out, reset, size pt, or scale_pct).",
                &[
                    ("action", "string", "Optional action: in, out, reset, or size/percentage string"),
                    ("size", "integer", "Optional font point size (6..49) or scale percentage (50..300)"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_canvas",
                "Control the Tektronix 4010/4014 storage-tube vector engine and modern retained graphical canvas (vectors, 2D sprites, plots, images).",
                &[(
                    "command",
                    "string",
                    "Optional canvas command (e.g. 'tek demo', 'line 0 0 20 10', 'sprite 1 4 4 2 0', 'plot candlestick', 'image 24 16', 'clear')",
                )],
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
}

/// Runs the Model Context Protocol (`bilterm --mcp`) JSON-RPC 2.0 server over `stdin`/`stdout`
/// with the provided `AutomationControlPlane`.
pub async fn run_mcp_stdio_server_with_plane(mut plane: AutomationControlPlane) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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

/// Runs the Model Context Protocol (`bilterm --mcp`) JSON-RPC 2.0 server over `stdin`/`stdout`.
pub async fn run_mcp_stdio_server(rows: u16, cols: u16) -> Result<()> {
    run_mcp_stdio_server_with_plane(AutomationControlPlane::new_headless(rows, cols)).await
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

