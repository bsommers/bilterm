//! JSON-RPC 2.0 dispatch and method handlers for AutomationControlPlane.

use std::collections::BTreeMap;
use std::time::Instant;

use super::json::JsonValue;
use super::mcp::parse_key_chord_str;
use super::playbook::{AutomationEvent, Playbook};
use super::rpc::{AutomationControlPlane, SnapshotPolicy, extract_vt_row_text};
use crate::colon_command::execute_find::find_results_json;
use crate::colon_command::parse_colon_command;
use crate::input::{AppAction, handle_wm_key_event};
use crate::remote::{FleetSelector, RemoteHostProfile};
use crate::wm::find_cmd::{FindKind, cap_query};
use crate::wm::registry::FIND_DEFAULT_LIMIT;
use crate::wm::traffic::{TrafficEndpoint, TrafficKind};

impl AutomationControlPlane {
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

                // `from_pane` attributes the call to a pane (attribution only; the API can already
                // type into any pane), so a script can draw pane-to-pane edges in the traffic graph.
                let source = match params.get("from_pane").and_then(JsonValue::as_usize) {
                    Some(from) if self.wm.pane_mut(from).is_some() => TrafficEndpoint::Pane(from),
                    Some(from) => return Err(format!("from_pane #{from} not found")),
                    None => TrafficEndpoint::Api,
                };
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
                let redact = pane.is_password_prompt();
                pane.submit_command(cmd).map_err(|e| e.to_string())?;
                self.wm.record_traffic_with(
                    source,
                    TrafficEndpoint::Pane(target_id),
                    TrafficKind::Exec,
                    cmd,
                    redact,
                    Instant::now(),
                );

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
                // Redaction is decided before dispatch: the chords may leave the prompt state.
                let target = self.wm.active_pane_id();
                let redact = self.wm.pane_mut(target).is_some_and(|p| p.is_password_prompt());
                self.wm.set_api_origin(true);
                let sent = (|| {
                    for chord in &chords {
                        let key_ev = parse_key_chord_str(chord)
                            .ok_or_else(|| format!("unrecognized key chord: {chord}"))?;
                        handle_wm_key_event(&mut self.wm, key_ev)
                            .map_err(|e| format!("key dispatch error: {e}"))?;
                    }
                    Ok::<(), String>(())
                })();
                self.wm.set_api_origin(false);
                sent?;
                self.wm.record_traffic_with(
                    TrafficEndpoint::Api,
                    TrafficEndpoint::Pane(target),
                    TrafficKind::Keys,
                    &chords.join(" "),
                    redact,
                    Instant::now(),
                );
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
                    .apply_with_policy(action, SnapshotPolicy::Confined)
                    .map_err(|e| format!("action execution error: {e}"))?;
                let outcome_str = if app_action == AppAction::Quit {
                    "quit"
                } else {
                    "handled"
                };
                let mut out = vec![
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
                ];
                if let Some(data) = self.wm.take_command_data().and_then(|d| JsonValue::parse(&d)) {
                    out.push(("data", data));
                }
                Ok(JsonValue::obj(out))
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
                    return Ok(self.connect_tmux_result(pane_id, target, tmux_sess));
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
                Ok(self.connect_tmux_result(pane_id, target, session))
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
            "agents.list" => self.rpc_agents_list(params),
            "agents.demo" => self.rpc_agents_demo(params),
            "session.map" => self.rpc_session_map(params),
            "remote.link" => self.rpc_remote_link(params),
            "find.agents" => self.find_rpc(FindKind::Agent, params),
            "find.sessions" => self.find_rpc(FindKind::Session, params),
            "find.history" => self.find_rpc(FindKind::History, params),
            "workspace.fontsize" => self.handle_rpc_fontsize(params),
            "canvas.command" | "canvas.status" => self.handle_rpc_canvas(params),
            "session.save" => self.handle_rpc_session_save(params),
            "session.restore" => self.handle_rpc_session_restore(params),
            "window.move" => self.handle_window_move(params),
            "window.rename" => self.handle_window_rename(params),
            "session.rename" => self.handle_session_rename(params),
            "window.select" => self.handle_window_select(params),
            "window.bulk" => self.handle_window_bulk(params),
            "window.undo" => self.handle_window_undo(params),
            "agent.name" => self.rpc_agent_name(params),
            "agent.send" => self.rpc_agent_send(params),
            "agent.broadcast" => self.rpc_agent_broadcast(params),
            "agent.inbox" => self.rpc_agent_inbox(params),
            "agent.ack" => self.rpc_agent_ack(params),
            "agent.list" => self.rpc_agent_list(params),
            "agent.wait" => self.rpc_agent_wait(params),
            "daemon.list" => super::rpc_daemon::handle_daemon_list(),
            "daemon.status" => super::rpc_daemon::handle_daemon_status(params),
            "daemon.start" => super::rpc_daemon::handle_daemon_start(params),
            "daemon.stop" => super::rpc_daemon::handle_daemon_stop(params),
            "daemon.switch" => super::rpc_daemon::handle_daemon_switch(params),
            "drawer.preview" => self.handle_rpc_drawer_preview(params),
            "workspace.view_model" | "web.status" | "web.open" | "web.port" | "web.stop" | "web.pump" | "web.agents" | "web.agent" | "web.logs" | "web.search" | "web.input" | "web.create" | "web.kill" | "web.update" => self.dispatch_web(method, params),
            "keys.list" | "keys.check" | "keys.explain" | "keys.profile" | "keys.window_mode" | "menu.open" => self.dispatch_keys(method, params),
            "clipboard.list" => self.rpc_clipboard_list(params),
            "clipboard.get" => self.rpc_clipboard_get(params),
            "clipboard.copy" => self.rpc_clipboard_copy(params),
            "clipboard.paste" => self.rpc_clipboard_paste(params),
            "clipboard.pin" => self.rpc_clipboard_pin(params),
            "clipboard.delete" => self.rpc_clipboard_delete(params),
            "clipboard.clear" => self.rpc_clipboard_clear(params),
            "clipboard.window" => self.rpc_clipboard_window(params),
            "coords.resolve" => self.rpc_coords_resolve(params),
            "coords.format" => self.rpc_coords_format(params),
            "traffic.snapshot" => self.rpc_traffic_snapshot(params),
            "traffic.history" => self.rpc_traffic_history(params),
            "traffic.window" => self.rpc_traffic_window(params),
            "traffic.reset" => self.rpc_traffic_reset(params),
            "traffic.config" => self.rpc_traffic_config(params),
            "voice.toggle" => self.rpc_voice_toggle(params),
            "voice.start" => self.rpc_voice_start(params),
            "voice.stop" => self.rpc_voice_stop(params),
            "voice.cancel" => self.rpc_voice_cancel(params),
            "voice.status" => self.rpc_voice_status(params),
            "voice.provider" => self.rpc_voice_provider(params),
            "voice.inject_wav" => self.rpc_voice_inject_wav(params),
            "browser.open" => self.rpc_browser_open(params),
            "browser.control" => self.rpc_browser_control(params),
            "browser.state" => self.rpc_browser_state(params),
            "video.open" => self.rpc_video_open(params),
            "video.control" => self.rpc_video_control(params),
            "video.state" => self.rpc_video_state(params),
            _ => Err(format!("Method not found: {method}")),
        }
    }

    /// `find.agents|sessions|history`: read-only registry search (FND-R04). Same JSON as the
    /// `:find` data; no modal, no focus change, no command data (same exposure class as `pane.read`).
    fn find_rpc(&mut self, kind: FindKind, params: &JsonValue) -> std::result::Result<JsonValue, String> {
        let query = match params.get("query") {
            None | Some(JsonValue::Null) => String::new(),
            Some(v) => v.as_str().ok_or_else(|| "invalid 'query' parameter".to_string())?.to_string(),
        };
        let limit = match params.get("limit") {
            None | Some(JsonValue::Null) => FIND_DEFAULT_LIMIT,
            Some(v) => v.as_usize().ok_or_else(|| "invalid 'limit' parameter".to_string())?,
        };
        let results = self.wm.run_find(kind, &cap_query(&query), limit);
        Ok(find_results_json(&results))
    }

    fn rpc_agent_name(&mut self, params: &JsonValue) -> std::result::Result<JsonValue, String> {
        let name = params
            .get("name")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "missing 'name' parameter".to_string())?;
        let pane_id = params.get("pane_id").and_then(JsonValue::as_usize);
        self.wm.agent_mail_apply(&crate::wm::agent_mail::AgentMailOp::SetName {
            pane_id,
            name: name.to_string(),
        })?;
        let data = self
            .wm
            .take_command_data()
            .and_then(|d| JsonValue::parse(&d))
            .unwrap_or_else(|| JsonValue::Object(BTreeMap::new()));
        Ok(data)
    }

    fn rpc_agent_send(&mut self, params: &JsonValue) -> std::result::Result<JsonValue, String> {
        let to = params
            .get("to")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "missing 'to' parameter".to_string())?;
        let body = params
            .get("body")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "missing 'body' parameter".to_string())?;
        let from = params.get("from").and_then(JsonValue::as_str).map(String::from);
        let kind = params
            .get("kind")
            .and_then(JsonValue::as_str)
            .unwrap_or("message")
            .to_string();
        let delivery_str = params
            .get("delivery")
            .and_then(JsonValue::as_str)
            .unwrap_or("mailbox");
        let delivery = match delivery_str {
            "nudge" => crate::wm::agent_mail::AgentDeliveryMode::Nudge,
            "file" => crate::wm::agent_mail::AgentDeliveryMode::File,
            _ => crate::wm::agent_mail::AgentDeliveryMode::Mailbox,
        };
        let reply_to = params
            .get("reply_to")
            .and_then(JsonValue::as_i64)
            .and_then(|n| u64::try_from(n).ok());

        self.wm.agent_mail_apply(&crate::wm::agent_mail::AgentMailOp::Send {
            from: from.clone(),
            to: to.to_string(),
            body: body.to_string(),
            kind: kind.clone(),
            delivery,
            reply_to,
        })?;

        let data = self
            .wm
            .take_command_data()
            .and_then(|d| JsonValue::parse(&d))
            .unwrap_or_else(|| JsonValue::Object(BTreeMap::new()));

        if let Some(msg_id) = data
            .get("message_id")
            .and_then(JsonValue::as_i64)
            .and_then(|n| u64::try_from(n).ok())
        {
            let sender = data
                .get("from")
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .to_string();
            let recipient = data
                .get("to")
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .to_string();
            self.push_event(AutomationEvent::AgentMessage {
                message_id: msg_id,
                from: sender,
                to: recipient,
                kind,
                body: body.to_string(),
            });
        }
        Ok(data)
    }

    fn rpc_agent_broadcast(&mut self, params: &JsonValue) -> std::result::Result<JsonValue, String> {
        let body = params
            .get("body")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "missing 'body' parameter".to_string())?;
        let from = params.get("from").and_then(JsonValue::as_str).map(String::from);
        let subtree = params.get("subtree").and_then(JsonValue::as_str).map(String::from);
        let kind = params
            .get("kind")
            .and_then(JsonValue::as_str)
            .unwrap_or("broadcast")
            .to_string();

        self.wm.agent_mail_apply(&crate::wm::agent_mail::AgentMailOp::Broadcast {
            from: from.clone(),
            subtree,
            body: body.to_string(),
            kind: kind.clone(),
        })?;

        let data = self
            .wm
            .take_command_data()
            .and_then(|d| JsonValue::parse(&d))
            .unwrap_or_else(|| JsonValue::Object(BTreeMap::new()));

        if let Some(recipients) = data.get("recipients").and_then(JsonValue::as_array) {
            for r in recipients {
                if let Some(to_name) = r.as_str() {
                    self.push_event(AutomationEvent::AgentMessage {
                        message_id: 0,
                        from: from.clone().unwrap_or_else(|| "operator".to_string()),
                        to: to_name.to_string(),
                        kind: kind.clone(),
                        body: body.to_string(),
                    });
                }
            }
        }
        Ok(data)
    }

    fn rpc_agent_inbox(&mut self, params: &JsonValue) -> std::result::Result<JsonValue, String> {
        let agent = params.get("agent").and_then(JsonValue::as_str).map(String::from);
        let unread_only = params
            .get("unread_only")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);

        self.wm.agent_mail_apply(&crate::wm::agent_mail::AgentMailOp::Inbox {
            agent,
            unread_only,
        })?;

        let data = self
            .wm
            .take_command_data()
            .and_then(|d| JsonValue::parse(&d))
            .unwrap_or_else(|| JsonValue::Object(BTreeMap::new()));
        Ok(data)
    }

    fn rpc_agent_ack(&mut self, params: &JsonValue) -> std::result::Result<JsonValue, String> {
        let agent = params.get("agent").and_then(JsonValue::as_str).map(String::from);
        let message_id = params
            .get("message_id")
            .and_then(JsonValue::as_i64)
            .and_then(|n| u64::try_from(n).ok());

        self.wm.agent_mail_apply(&crate::wm::agent_mail::AgentMailOp::Ack {
            agent,
            message_id,
        })?;

        let data = self
            .wm
            .take_command_data()
            .and_then(|d| JsonValue::parse(&d))
            .unwrap_or_else(|| JsonValue::Object(BTreeMap::new()));
        Ok(data)
    }

    fn rpc_agent_list(&mut self, _params: &JsonValue) -> std::result::Result<JsonValue, String> {
        self.wm
            .agent_mail_apply(&crate::wm::agent_mail::AgentMailOp::List)?;

        let data = self
            .wm
            .take_command_data()
            .and_then(|d| JsonValue::parse(&d))
            .unwrap_or_else(|| JsonValue::Object(BTreeMap::new()));
        Ok(data)
    }

    fn handle_rpc_drawer_preview(
        &mut self,
        params: &JsonValue,
    ) -> std::result::Result<JsonValue, String> {
        use crate::action::{Action, DrawerPreviewOp};
        use crate::wm::DrawerPreviewTarget;

        let op_str = params
            .get("op")
            .or_else(|| params.get("action"))
            .and_then(JsonValue::as_str)
            .unwrap_or("status");

        if op_str != "status" {
            let op = match op_str {
                "on" | "open" | "show" => DrawerPreviewOp::On,
                "off" | "close" | "hide" => DrawerPreviewOp::Off,
                "toggle" => DrawerPreviewOp::Toggle,
                "commit" | "focus" | "enter" => DrawerPreviewOp::Commit,
                "auto" | "auto-scroll" => DrawerPreviewOp::AutoToggle,
                "row" => {
                    let idx = params
                        .get("row")
                        .or_else(|| params.get("index"))
                        .and_then(JsonValue::as_usize)
                        .ok_or_else(|| "missing 'row' parameter for drawer.preview".to_string())?;
                    DrawerPreviewOp::Row(idx)
                }
                "pane" | "window" => {
                    let pid = params
                        .get("pane_id")
                        .or_else(|| params.get("window_id"))
                        .and_then(JsonValue::as_usize)
                        .ok_or_else(|| {
                            "missing 'pane_id' parameter for drawer.preview".to_string()
                        })?;
                    DrawerPreviewOp::Pane(pid)
                }
                "session" => {
                    let name = params
                        .get("session")
                        .or_else(|| params.get("name"))
                        .and_then(JsonValue::as_str)
                        .filter(|s| !s.trim().is_empty())
                        .ok_or_else(|| {
                            "missing 'session' parameter for drawer.preview".to_string()
                        })?;
                    DrawerPreviewOp::Session(name.to_string())
                }
                other => {
                    return Err(format!(
                        "invalid 'op' parameter: {other} (expected status, on, off, toggle, commit, auto, row, pane, or session)"
                    ));
                }
            };
            self.wm
                .apply(Action::DrawerPreview(op))
                .map_err(|e| e.to_string())?;
        }

        let target_json = match self.wm.resolve_drawer_preview_target() {
            Some(DrawerPreviewTarget::ActivePane(id)) => JsonValue::obj([
                ("kind", JsonValue::String("active_pane".to_string())),
                ("pane_id", JsonValue::Number(id as i64)),
            ]),
            Some(DrawerPreviewTarget::QuickTerminal) => JsonValue::obj([(
                "kind",
                JsonValue::String("quick_terminal".to_string()),
            )]),
            Some(DrawerPreviewTarget::BackgroundPane { session, pane_id }) => JsonValue::obj([
                ("kind", JsonValue::String("background_pane".to_string())),
                ("session", JsonValue::String(session)),
                ("pane_id", JsonValue::Number(pane_id as i64)),
            ]),
            Some(DrawerPreviewTarget::SavedSession { name }) => JsonValue::obj([
                ("kind", JsonValue::String("saved_session".to_string())),
                ("name", JsonValue::String(name)),
            ]),
            None => JsonValue::Null,
        };

        Ok(JsonValue::obj([
            ("active", JsonValue::Bool(self.wm.drawer().preview.active)),
            (
                "auto_preview_on_scroll",
                JsonValue::Bool(self.wm.drawer().preview.auto_preview_on_scroll),
            ),
            (
                "selected_index",
                JsonValue::Number(self.wm.drawer().selected_index as i64),
            ),
            (
                "active_pane_id",
                JsonValue::Number(self.wm.active_pane_id() as i64),
            ),
            ("target", target_json),
        ]))
    }
}
