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

        let result: std::result::Result<JsonValue, (i64, String)> = match method {
            "initialize" => Ok(JsonValue::obj([
                (
                    "protocolVersion",
                    JsonValue::String("2024-11-05".to_string()),
                ),
                (
                    "capabilities",
                    JsonValue::obj([
                        (
                            "tools",
                            JsonValue::obj([("listChanged", JsonValue::Bool(false))]),
                        ),
                        (
                            "resources",
                            JsonValue::obj([
                                ("subscribe", JsonValue::Bool(false)),
                                ("listChanged", JsonValue::Bool(false)),
                            ]),
                        ),
                    ]),
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
                ("instructions", JsonValue::String(crate::agent_guide::MCP_INSTRUCTIONS.to_string())),
            ])),
            "ping" => Ok(JsonValue::Object(BTreeMap::new())),
            "tools/list" => Ok(Self::mcp_tools_list()),
            "resources/list" => Ok(self.mcp_resources_list()),
            "resources/read" => self.mcp_resources_read(&params),
            "tools/call" => {
                if !matches!(params.get("arguments"), None | Some(JsonValue::Object(_))) {
                    return Some(
                        Self::error_response(
                            id,
                            -32602,
                            "Invalid params: 'arguments' must be an object",
                        )
                        .to_json_string(),
                    );
                }
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
                    "bilterm_find" => Some(Self::find_method_for_kind(&args).unwrap_or("")),
                    "bilterm_agents_list" => Some("agents.list"),
                    "bilterm_agents_demo" => Some("agents.demo"),
                    "bilterm_session_map" => Some("session.map"),
                    "bilterm_remote_link" => Some("remote.link"),
                    "bilterm_agent_mail" => Some(Self::agent_mail_method(&args).unwrap_or("")),
                    "bilterm_window_manage" => Some(Self::window_manage_method(&args).unwrap_or("")),
                    "bilterm_daemon" => Some(Self::daemon_method(&args).unwrap_or("")),
                    "bilterm_web_surface" => Some(Self::web_surface_method(&args).unwrap_or("")),
                    "bilterm_drawer_preview" => Some("drawer.preview"),
                    "bilterm_clipboard" => Some(Self::clipboard_method(&args).unwrap_or("")),
                    "bilterm_traffic" => Some(Self::traffic_method(&args).unwrap_or("")),
                    "bilterm_voice" => Some(Self::voice_method(&args).unwrap_or("")),
                    "bilterm_browser" => Some(Self::browser_method(&args).unwrap_or("")),
                    "bilterm_video" => Some(Self::video_method(&args).unwrap_or("")),
                    "bilterm_keys" => Some(Self::keys_method(&args).unwrap_or("")),
                    "bilterm_agent_control" => Some(Self::agent_control_method(&args).unwrap_or("")),
                    "bilterm_coords_resolve" => Some("coords.resolve"),
                    "bilterm_coords_format" => Some("coords.format"),
                    _ => None,
                };
                match mapped_method {
                    Some(rpc_method) => match if rpc_method.is_empty() {
                        if tool_name == "bilterm_agent_mail" {
                            Err("missing or invalid 'action' parameter: send, broadcast, inbox, ack, list, name or wait".to_string())
                        } else if tool_name == "bilterm_window_manage" {
                            Err("missing or invalid 'action' parameter: move, rename, rename_session, select, bulk, or undo".to_string())
                        } else if tool_name == "bilterm_daemon" {
                            Err("missing or invalid 'action' parameter: list, status, start, stop, switch".to_string())
                        } else if tool_name == "bilterm_web_surface" {
                            Err("invalid 'action' parameter: status, open, port or stop".to_string())
                        } else if tool_name == "bilterm_clipboard" {
                            Err("missing or invalid 'action' parameter: list, get, copy, paste, pin, delete, clear, or window".to_string())
                        } else if tool_name == "bilterm_agent_control" {
                            Err("missing or invalid 'action' parameter: hud, replay, watchdog, approve, prompt, last_turn, wait, guide, pane_list, pane_split, pane_focus, pane_zoom, pane_send, workspace_list, workspace_select, workspace_new or workspace_rename".to_string())
                        } else if tool_name == "bilterm_keys" {
                            Err("invalid 'action' parameter: list, check, explain, profile, window_mode or menu_open".to_string())
                        } else if tool_name == "bilterm_traffic" {
                            Err("missing or invalid 'action' parameter: snapshot, history, window, reset, or config".to_string())
                        } else if tool_name == "bilterm_voice" {
                            Err("missing or invalid 'action' parameter: toggle, start, stop, cancel, status, provider, or inject_wav".to_string())
                        } else {
                            Err("missing 'kind' parameter: agent, session or history".to_string())
                        }
                    } else {
                        let normalized_args = Self::normalize_window_manage_args(rpc_method, args);
                        self.dispatch_method(rpc_method, &normalized_args)
                    } {
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
                        Err(err_msg) if err_msg.starts_with("missing ") => {
                            Err((-32602, format!("Invalid params: {err_msg}")))
                        }
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
                    None => Err((-32602, format!("Invalid params: unknown MCP tool: {tool_name}"))),
                }
            }
            _ => Err((-32601, format!("Method not found: {method}"))),
        };

        Some(match result {
            Ok(res) => JsonValue::obj([
                ("jsonrpc", JsonValue::String("2.0".to_string())),
                ("id", id),
                ("result", res),
            ])
            .to_json_string(),
            Err((code, msg)) => Self::error_response(id, code, &msg).to_json_string(),
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
            Self::mcp_tool_schema(
                "bilterm_find",
                "Search the unified registry: live agents, sessions (live first, then saved on disk) or command history. Every row carries its source (live, saved-session or history-log) and the response a count of masked secret-looking entries.",
                &[
                    ("kind", "string", "What to search: agent, session or history (required)"),
                    ("query", "string", "Optional fuzzy query (empty lists everything, capped at 128 chars)"),
                    ("limit", "integer", "Maximum rows (default 50, max 200)"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_agents_list",
                "Open, close, pin, select, jump, zoom, scale, filter, dock or position the floating agents-list window, or read the agent state (no op).",
                &[
                    ("op", "string", "Optional operation: open, close, toggle, dismiss, blur, select, jump, zoom, pin, unpin, scale, filter, dock, float, move, resize, rect, size or scroll (absent or list reads the state)"),
                    ("args", "string", "Space-separated arguments, for example `down` or `110`"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_agents_demo",
                "Start the agent traffic demo (default) or stop it and close the panes it spawned.",
                &[("stop", "boolean", "true stops the demo")],
            ),
            Self::mcp_tool_schema(
                "bilterm_remote_link",
                "Show, reconnect or stop the persistent remote tmux link over SSH.",
                &[("op", "string", "status | reconnect | disconnect (default status)")],
            ),
            Self::mcp_tool_schema(
                "bilterm_agent_mail",
                "Agent-to-agent messaging and coordination: send structured messages, check inboxes, acknowledge mail, broadcast, register names, and wait for replies.",
                &[
                    ("action", "string", "Operation: send | broadcast | inbox | ack | list | name | wait (default 'inbox')"),
                    ("to", "string", "Recipient agent name (for send)"),
                    ("from", "string", "Optional sender agent name"),
                    ("body", "string", "Message body text (for send and broadcast, max 8192 bytes)"),
                    ("kind", "string", "Optional message kind (default 'message')"),
                    ("delivery", "string", "Delivery mode: mailbox | nudge | file (default 'mailbox')"),
                    ("reply_to", "integer", "Optional previous message id being replied to"),
                    ("agent", "string", "Target agent name (for inbox and ack)"),
                    ("message_id", "integer", "Specific message id to ack (for ack) or wait for reply (for wait)"),
                    ("unread_only", "boolean", "Only return unacknowledged messages (for inbox)"),
                    ("timeout_ms", "integer", "Timeout in milliseconds (for wait, default 5000)"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_window_manage",
                "Manage Bilterm windows and sessions: move, rename, select, bulk operations (close, move, rename-prefix, tile, pin), and undo.",
                &[
                    ("action", "string", "Operation: move | rename | rename_session | select | bulk | undo (required)"),
                    ("window_id", "integer", "Optional window (pane) ID (defaults to active window)"),
                    ("target_session", "string", "Target session for move or bulk move"),
                    ("new_name", "string", "New window or session name"),
                    ("old_name", "string", "Old session name for rename_session"),
                    ("switch", "boolean", "Switch active session after window move"),
                    ("select_action", "string", "Selection action for select: toggle | range | all | none | invert | explicit"),
                    ("bulk_action", "string", "Bulk action: move | close | rename_prefix | tile | pin"),
                    ("prefix", "string", "Prefix string for bulk rename_prefix"),
                    ("force", "boolean", "Force bulk close bypassing exit guard"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_session_map",
                "Inspect or manage the session packing viewer, pick up windows, and drag-and-drop between sessions.",
                &[
                    ("op", "string", "Optional operation: open, close, toggle, pin, dock, pickup, target, drop, cancel, scale"),
                    ("pane_id", "integer", "Pane ID for pickup or drop"),
                    ("session", "string", "Target session name"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_daemon",
                "Supervise and inspect Bilterm background daemons (list, status, start, stop, switch).",
                &[
                    ("action", "string", "Operation: list | status | start | stop | switch"),
                    ("name", "string", "Optional daemon name (defaults to 'default')"),
                    ("force", "boolean", "Optional flag to force stop without graceful shutdown"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_web_surface",
                "Inspect or control the Web UI surface server (status, open browser, set port, stop). Returns enabled, port, url, active_clients, web_dir.",
                &[
                    ("action", "string", "Operation: status | open | port | stop (default 'status')"),
                    ("port", "integer", "TCP port when action='port'"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_drawer_preview",
                "Open, close, toggle, commit, or inspect the floating Drawer Window Peek & Live Preview Pane for active, occluded, minimized, background-session, or saved-session windows.",
                &[
                    ("op", "string", "Operation: status | on | off | toggle | commit | auto | row | pane | session (default 'status')"),
                    ("row", "integer", "0-based drawer row index when op='row'"),
                    ("pane_id", "integer", "Target window/pane ID when op='pane'"),
                    ("session", "string", "Target session name when op='session'"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_clipboard",
                "Inspect, filter, copy, paste, pin, delete, clear, or manage the Bilterm CLIPBOARD viewer and registry.",
                &[
                    ("action", "string", "Operation: list | get | copy | paste | pin | delete | clear | window (default 'list')"),
                    ("scope", "string", "Optional filter scope for list: all | session | window | pinned | media"),
                    ("session", "string", "Optional session name filter for list"),
                    ("window", "integer", "Optional pane/window ID filter for list"),
                    ("query", "string", "Optional substring filter for list or window search"),
                    ("id", "integer", "Optional entry ID for get, paste, pin, or delete"),
                    ("index", "integer", "Optional 1-based entry index for paste"),
                    ("text", "string", "Text to copy into the clipboard registry (for copy)"),
                    ("pinned", "boolean", "Pin state for pin"),
                    ("op", "string", "Window operation for action='window': open | close | toggle | pin | dock | float | all | session | window | pinned | media"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_traffic",
                "Read terminal traffic (bytes per connection, session, window, pane, origin, locality, direction) or drive the TRAFFIC MONITOR window and counters.",
                &[
                    ("action", "string", "Operation: snapshot | history | window | reset | config (default 'snapshot')"),
                    ("group_by", "string", "connection | session | window | pane | origin | locality | direction"),
                    ("sort", "string", "rate | rate10 | rate1m | total | peak | name"),
                    ("descending", "boolean", "Sort direction"),
                    ("locality", "string", "all | local | loopback | network"),
                    ("filter", "string", "Text filter (max 64 chars)"),
                    ("limit", "integer", "Row limit 1..500"),
                    ("key", "string", "Row key for history"),
                    ("n", "integer", "Seconds of history (max 60)"),
                    ("op", "string", "Window operation for action='window': open | close | toggle | pin | dock | float | group | sort | filter | pause | resume | drill | up"),
                    ("set", "object", "For action='config': {enabled|classify|oia_indicator: boolean} (write-gated)"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_voice",
                "Control the Speech-to-Text (STT) voice dictation subsystem: toggle, start recording, stop and transcribe, cancel, check status, switch provider, or inject audio.",
                &[
                    ("action", "string", "Operation: toggle | start | stop | cancel | status | provider | inject_wav (default 'status')"),
                    ("pane_id", "integer", "Optional target pane ID"),
                    ("provider", "string", "Optional provider name for action='provider': openai | ollama | cli | mock"),
                    ("wav_base64", "string", "Base64-encoded WAV audio data for action='inject_wav'"),
                    ("wav_hex", "string", "Hex-encoded WAV audio data for action='inject_wav'"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_browser",
                "Open, navigate, or control the Chrome Engine Web Browser Canvas in the active or target pane.",
                &[
                    ("action", "string", "Operation: open | back | forward | reload | stop | focus | zoom | close | state | toggle (default 'state')"),
                    ("url", "string", "URL to navigate to for action='open'"),
                    ("pane_id", "integer", "Optional target pane ID (defaults to active pane)"),
                    ("delta_pct", "integer", "Zoom delta percentage for action='zoom' (default 10)"),
                    ("focused", "boolean", "Focus state for action='focus'"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_video",
                "Open, play, pause, seek, adjust volume, or inspect the Video Player Canvas in the active or target pane.",
                &[
                    ("action", "string", "Operation: open | play | pause | toggle | seek | seek_pct | volume | mute | loop | speed | close | state (default 'state')"),
                    ("source", "string", "Video file path, URL, or demo URI (e.g. 'demo://smpte', 'demo://plasma') for action='open'"),
                    ("pane_id", "integer", "Optional target pane ID (defaults to active pane)"),
                    ("delta_secs", "number", "Seek relative delta in seconds for action='seek' (default 5.0)"),
                    ("pct", "integer", "Seek absolute percentage (0..=100) for action='seek_pct'"),
                    ("volume_delta", "integer", "Volume delta percent (+/-5) for action='volume'"),
                    ("speed", "integer", "Speed percentage (100 = 1.0x, 150 = 1.5x) for action='speed'"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_keys",
                "Inspect and drive the keyboard model: list the active key profile, check key conflicts (OS, desktop, shell, editor and multiplexer chords), explain a chord, switch profile, enter Window mode, or open the menu bar. Mutating actions need [api] allow_config_write.",
                &[
                    ("action", "string", "Operation: list | check | explain | profile | window_mode | menu_open (default 'list')"),
                    ("chord", "string", "Chord to explain for action='explain' (e.g. 'Primary+L', 'Alt+F4')"),
                    ("name", "string", "Profile name for action='profile' (auto|mac|windows|linux|portable|legacy); omit to query"),
                    ("op", "string", "Window mode operation for action='window_mode': on | off | toggle | focus | resize | move"),
                    ("category", "integer", "Optional 0-based menu category for action='menu_open'"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_agent_control",
                "Live agent control: the Mission Control HUD snapshot (hud), per-turn replay (replay), the stall/loop watchdog (watchdog), answer or prompt an agent pane (approve, prompt), read its last turn, and pane/workspace control. Each action is the RPC method of the same name (agent.hud, agent.replay, agent.approve, pane.split, ...).",
                &[
                    ("action", "string", "Operation: hud | replay | watchdog | approve | prompt | last_turn | wait | guide | pane_list | pane_split | pane_focus | pane_zoom | pane_send | workspace_list | workspace_select | workspace_new | workspace_rename (required)"),
                    ("selector", "string", "Pane selector (p3, @blocked, @focused, ...)"),
                    ("pane_id", "integer", "Pane id (alternative to selector)"),
                    ("op", "string", "Sub-operation for hud (open|close|toggle|next|prev|select|approve|prompt|diff|zoom) and replay (open|close|prev|next|first|last|turn|scroll); guide: get (default, returns the agent guide text) | inject (hand it to the selected pane, or target=all) | print"),
                    ("target", "string", "guide inject: 'all' = every detected agent pane"),
                    ("turn_id", "integer", "Replay: return this turn with its output"),
                    ("text", "string", "Text for prompt / pane_send"),
                    ("answer", "string", "approve answer: yes | y | n | no | esc"),
                    ("force", "boolean", "approve even if the pane is not blocked"),
                    ("name", "string", "Workspace name for workspace_select/new/rename"),
                    ("direction", "string", "pane_split direction: h | v"),
                ],
            ),
            Self::mcp_tool_schema(
                "bilterm_coords_resolve",
                "Resolve a hierarchical coordinate (e.g. d0:w1:main:y2, wildcards allowed) to the pane ids and canonical coordinate paths it addresses.",
                &[("coord", "string", "Target coordinate: dD:wW:session:yY / pane path, `*` wildcards")],
            ),
            Self::mcp_tool_schema(
                "bilterm_coords_format",
                "Format a pane's hierarchical coordinate as badge text in a display style, plus its canonical path.",
                &[
                    ("pane_id", "integer", "Optional pane ID (defaults to the active pane)"),
                    ("style", "string", "verbose | compact | daemon | off (default 'compact')"),
                ],
            ),
        ];
        JsonValue::obj([("tools", JsonValue::Array(tools))])
    }

    /// `bilterm_voice` `action` argument -> the `voice.*` RPC method; `None` when unknown.
    fn voice_method(args: &JsonValue) -> Option<&'static str> {
        let action = args.get("action").and_then(JsonValue::as_str).unwrap_or("status");
        match action {
            "toggle" => Some("voice.toggle"),
            "start" => Some("voice.start"),
            "stop" => Some("voice.stop"),
            "cancel" => Some("voice.cancel"),
            "status" => Some("voice.status"),
            "provider" => Some("voice.provider"),
            "inject_wav" => Some("voice.inject_wav"),
            _ => None,
        }
    }

    /// `bilterm_browser` `action` argument -> the `browser.*` RPC method.
    fn browser_method(args: &JsonValue) -> Option<&'static str> {
        let action = args.get("action").and_then(JsonValue::as_str).unwrap_or("state");
        match action {
            "open" => Some("browser.open"),
            "state" => Some("browser.state"),
            _ => Some("browser.control"),
        }
    }

    /// `bilterm_agent_control` `action` argument -> the `agent.*` / `pane.*` / `workspace.*` RPC method.
    pub fn agent_control_method(args: &JsonValue) -> Option<&'static str> {
        Some(match args.get("action").and_then(JsonValue::as_str)? {
            "hud" => "agent.hud",
            "replay" => "agent.replay",
            "watchdog" => "agent.watchdog",
            "approve" => "agent.approve",
            "prompt" => "agent.prompt",
            "last_turn" => "agent.last_turn",
            "wait" => "agent.wait",
            "guide" => "agent.guide",
            "pane_list" => "pane.list",
            "pane_split" => "pane.split",
            "pane_focus" => "pane.focus",
            "pane_zoom" => "pane.zoom",
            "pane_send" => "pane.send",
            "workspace_list" => "workspace.list",
            "workspace_select" => "workspace.select",
            "workspace_new" => "workspace.new",
            "workspace_rename" => "workspace.rename",
            _ => return None,
        })
    }

    /// `bilterm_keys` `action` argument -> the `keys.*` / `menu.open` RPC method; `None` when unknown.
    fn keys_method(args: &JsonValue) -> Option<&'static str> {
        match args.get("action").and_then(JsonValue::as_str).unwrap_or("list") {
            "list" => Some("keys.list"),
            "check" => Some("keys.check"),
            "explain" => Some("keys.explain"),
            "profile" => Some("keys.profile"),
            "window_mode" => Some("keys.window_mode"),
            "menu_open" => Some("menu.open"),
            _ => None,
        }
    }

    /// `bilterm_video` `action` argument -> the `video.*` RPC method.
    fn video_method(args: &JsonValue) -> Option<&'static str> {
        let action = args.get("action").and_then(JsonValue::as_str).unwrap_or("state");
        match action {
            "open" => Some("video.open"),
            "state" => Some("video.state"),
            _ => Some("video.control"),
        }
    }

    /// `bilterm_clipboard` `action` argument -> the `clipboard.*` RPC method; `None` when unknown.
    fn clipboard_method(args: &JsonValue) -> Option<&'static str> {
        let action = args.get("action").and_then(JsonValue::as_str).unwrap_or("list");
        match action {
            "list" => Some("clipboard.list"),
            "get" => Some("clipboard.get"),
            "copy" => Some("clipboard.copy"),
            "paste" => Some("clipboard.paste"),
            "pin" => Some("clipboard.pin"),
            "delete" => Some("clipboard.delete"),
            "clear" => Some("clipboard.clear"),
            "window" => Some("clipboard.window"),
            _ => None,
        }
    }

    /// `bilterm_traffic` `action` argument -> the `traffic.*` RPC method; `None` when unknown.
    fn traffic_method(args: &JsonValue) -> Option<&'static str> {
        match args.get("action").and_then(JsonValue::as_str).unwrap_or("snapshot") {
            "snapshot" => Some("traffic.snapshot"),
            "history" => Some("traffic.history"),
            "window" => Some("traffic.window"),
            "reset" => Some("traffic.reset"),
            "config" => Some("traffic.config"),
            _ => None,
        }
    }

    /// `bilterm_web_surface` `action` argument -> the `web.*` RPC method; `None` when unknown.
    fn web_surface_method(args: &JsonValue) -> Option<&'static str> {
        match args.get("action").and_then(JsonValue::as_str).unwrap_or("status") {
            "status" => Some("web.status"),
            "open" => Some("web.open"),
            "port" => Some("web.port"),
            "stop" => Some("web.stop"),
            _ => None,
        }
    }

    /// `bilterm_daemon` `action` argument -> the `daemon.*` RPC method; `None` when missing or unknown.
    fn daemon_method(args: &JsonValue) -> Option<&'static str> {
        let action = args.get("action").and_then(JsonValue::as_str)?;
        match action {
            "list" => Some("daemon.list"),
            "status" => Some("daemon.status"),
            "start" => Some("daemon.start"),
            "stop" => Some("daemon.stop"),
            "switch" => Some("daemon.switch"),
            _ => None,
        }
    }

    /// `bilterm_agent_mail` `action` argument -> the `agent.*` RPC method; `None` when missing or unknown.
    fn agent_mail_method(args: &JsonValue) -> Option<&'static str> {
        let action = args.get("action").and_then(JsonValue::as_str).unwrap_or("inbox");
        match action {
            "name" => Some("agent.name"),
            "send" => Some("agent.send"),
            "broadcast" => Some("agent.broadcast"),
            "inbox" => Some("agent.inbox"),
            "ack" => Some("agent.ack"),
            "list" => Some("agent.list"),
            "wait" => Some("agent.wait"),
            _ => None,
        }
    }

    /// `bilterm_window_manage` `action` argument -> the `window.*` / `session.rename` RPC method.
    fn window_manage_method(args: &JsonValue) -> Option<&'static str> {
        let action = args.get("action").and_then(JsonValue::as_str)?;
        match action {
            "move" => Some("window.move"),
            "rename" => Some("window.rename"),
            "rename_session" => Some("session.rename"),
            "select" | "toggle" | "range" | "all" | "none" | "invert" | "explicit" => {
                Some("window.select")
            }
            "bulk" | "close" | "tile" | "pin" | "rename_prefix" => Some("window.bulk"),
            "undo" => Some("window.undo"),
            _ => None,
        }
    }

    /// Normalizes arguments from `bilterm_window_manage` to match the target RPC method schema.
    fn normalize_window_manage_args(method: &str, mut args: JsonValue) -> JsonValue {
        if let JsonValue::Object(ref mut map) = args {
            if method == "window.select" {
                if let Some(sub) = map.get("select_action").cloned() {
                    map.insert("action".to_string(), sub);
                } else if map.get("action").and_then(JsonValue::as_str) == Some("select")
                    && let Some(sub) = map.get("subaction").cloned()
                {
                    map.insert("action".to_string(), sub);
                }
            } else if method == "window.bulk" {
                if let Some(sub) = map.get("bulk_action").cloned() {
                    map.insert("action".to_string(), sub);
                } else if map.get("action").and_then(JsonValue::as_str) == Some("bulk")
                    && let Some(sub) = map.get("subaction").cloned()
                {
                    map.insert("action".to_string(), sub);
                }
            }
        }
        args
    }

    /// `bilterm_find` `kind` argument -> the `find.*` RPC method; `None` when missing or unknown.
    fn find_method_for_kind(args: &JsonValue) -> Option<&'static str> {
        match args.get("kind").and_then(JsonValue::as_str)? {
            "agent" | "agents" => Some("find.agents"),
            "session" | "sessions" => Some("find.sessions"),
            "history" => Some("find.history"),
            _ => None,
        }
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
    // H26/H27: the stdio loop counts its own bytes on an `Origin::Mcp` wire channel.
    let tap = plane.wm.traffic_meter().open_channel(
        crate::traffic::Origin::Mcp,
        "mcp",
        None,
        crate::traffic::Role::Wire,
    );

    while let Some(line) = lines.next_line().await? {
        tap.rx(line.len() + 1);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(resp) = plane.handle_mcp_line(trimmed) {
            tap.tx(resp.len() + 1);
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

