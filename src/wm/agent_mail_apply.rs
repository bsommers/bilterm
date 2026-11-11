//! `WindowManager::agent_mail_apply` implementation (`AGT-01..03`, Phase 52).

use std::time::Instant;

use crate::api::JsonValue;
use crate::pane::PaneId;
use crate::wm::agent_mail::{
    AgentDeliveryMode, AgentMailOp, AgentMessage, NUDGE_PREVIEW_CAP, sanitize_preview,
};
use crate::wm::traffic::{TrafficEndpoint, TrafficKind};
use crate::wm::WindowManager;

impl WindowManager {
    /// Applies an [`AgentMailOp`] against the workspace, mailboxes, and PTY canvases.
    pub fn agent_mail_apply(&mut self, op: &AgentMailOp) -> Result<String, String> {
        match op {
            AgentMailOp::SetName { pane_id, name } => {
                let target_id = pane_id.unwrap_or_else(|| self.active_pane_id());
                if !self.panes.contains_key(&target_id) {
                    return Err(format!("Pane #{target_id} not found"));
                }
                self.agent_mail.register_name(target_id, name)?;
                let data = JsonValue::obj([
                    ("status", JsonValue::String("ok".to_string())),
                    ("pane_id", JsonValue::Number(target_id as i64)),
                    ("name", JsonValue::String(name.clone())),
                ]);
                self.set_command_data(data.to_json_string());
                Ok(format!("Agent name for pane #{target_id} set to '{name}'"))
            }

            AgentMailOp::Enable(opt) => {
                let new_state = opt.unwrap_or_else(|| self.agent_mail.is_enabled());
                if let Some(val) = opt {
                    self.set_agent_messaging_enabled(*val);
                }
                let data = JsonValue::obj([
                    ("status", JsonValue::String("ok".to_string())),
                    ("enabled", JsonValue::Bool(self.agent_mail.is_enabled())),
                ]);
                self.set_command_data(data.to_json_string());
                if opt.is_some() {
                    Ok(format!(
                        "Agent messaging {}",
                        if new_state { "enabled" } else { "disabled" }
                    ))
                } else {
                    Ok(format!(
                        "Agent messaging is {}",
                        if new_state { "enabled" } else { "disabled" }
                    ))
                }
            }

            AgentMailOp::Send {
                from,
                to,
                body,
                kind,
                delivery,
                reply_to,
            } => {
                if !self.agent_mail.is_enabled() {
                    return Err(
                        "Agent messaging is disabled (enable via :agent enable on or [api] allow_agent_messaging = true)"
                            .to_string(),
                    );
                }
                let (recipient_name, target_pane_id) = self
                    .agent_mail
                    .resolve_recipient(to)
                    .ok_or_else(|| format!("Unknown agent '{to}'"))?;

                let sender_name = from
                    .clone()
                    .or_else(|| {
                        self.agent_mail
                            .name_for_pane(self.active_pane_id())
                            .map(String::from)
                    })
                    .unwrap_or_else(|| "operator".to_string());

                let now = Instant::now();
                self.agent_mail.check_rate_limit(&sender_name, now)?;
                let hop_count = self.agent_mail.hop_count_for_reply(*reply_to)?;

                let mut file_path = None;
                if *delivery == AgentDeliveryMode::File {
                    let next_id = self.agent_mail.get_inbox(&recipient_name, false).len() as u64 + 1;
                    let path = self.write_agent_mail_file(&recipient_name, next_id, body)?;
                    file_path = Some(path);
                }

                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);

                let msg = AgentMessage {
                    id: 0,
                    from: sender_name.clone(),
                    to: recipient_name.clone(),
                    body: body.clone(),
                    kind: kind.clone(),
                    reply_to: *reply_to,
                    hop_count,
                    timestamp_ms: now_ms,
                    acked: false,
                    file_path: file_path.clone(),
                };

                let enqueued = self.agent_mail.enqueue(msg)?;
                let msg_id = enqueued.id;

                let sender_endpoint = self
                    .agent_mail
                    .pane_for_name(&sender_name)
                    .map(TrafficEndpoint::Pane)
                    .unwrap_or(TrafficEndpoint::Api);
                self.record_traffic_with(
                    sender_endpoint,
                    TrafficEndpoint::Pane(target_pane_id),
                    TrafficKind::Message,
                    body,
                    false,
                    now,
                );

                let (status_text, is_nudged) = match delivery {
                    AgentDeliveryMode::Mailbox => ("queued in mailbox".to_string(), false),
                    AgentDeliveryMode::Nudge | AgentDeliveryMode::File => {
                        let is_busy = self
                            .panes
                            .get(&target_pane_id)
                            .and_then(|p| p.resolve_running_command())
                            .is_some()
                            || self.agent_state(target_pane_id)
                                == Some(crate::wm::agent_state::AgentState::Working);
                        let Some(pane) = self.panes.get_mut(&target_pane_id) else {
                            return Err(format!("Agent '{recipient_name}' pane #{target_pane_id} no longer exists"));
                        };
                        let is_alt = pane.vt().is_alternate_screen();
                        let is_pw =
                            pane.is_password_prompt() || pane.is_active_canvas_password_prompt();
                        if !is_alt && !is_pw && !is_busy {
                            let preview = if *delivery == AgentDeliveryMode::File {
                                format!("file {}", file_path.as_deref().unwrap_or(""))
                            } else {
                                sanitize_preview(body, NUDGE_PREVIEW_CAP)
                            };
                            let line = format!("# [mail from {sender_name} #{msg_id}]: {preview}\n");
                            let _ = pane.write_to_pty(line.as_bytes());
                            if *delivery == AgentDeliveryMode::File {
                                (
                                    format!("file delivered: {}", file_path.as_deref().unwrap_or("")),
                                    true,
                                )
                            } else {
                                ("nudged".to_string(), true)
                            }
                        } else {
                            (
                                "queued in mailbox (nudge suppressed: pane busy/locked)".to_string(),
                                false,
                            )
                        }
                    }
                };

                let data = JsonValue::obj([
                    ("status", JsonValue::String(status_text.clone())),
                    ("message_id", JsonValue::Number(msg_id as i64)),
                    ("from", JsonValue::String(sender_name)),
                    ("to", JsonValue::String(recipient_name.clone())),
                    ("delivery", JsonValue::String(delivery.as_str().to_string())),
                    ("hop_count", JsonValue::Number(hop_count as i64)),
                    ("nudged", JsonValue::Bool(is_nudged)),
                    (
                        "file_path",
                        file_path.map_or(JsonValue::Null, JsonValue::String),
                    ),
                ]);
                self.set_command_data(data.to_json_string());
                Ok(format!(
                    "Message #{msg_id} sent to '{recipient_name}' ({status_text})"
                ))
            }

            AgentMailOp::Broadcast {
                from,
                subtree: _,
                body,
                kind,
            } => {
                if !self.agent_mail.is_enabled() {
                    return Err(
                        "Agent messaging is disabled (enable via :agent enable on or [api] allow_agent_messaging = true)"
                            .to_string(),
                    );
                }
                let sender_name = from
                    .clone()
                    .or_else(|| {
                        self.agent_mail
                            .name_for_pane(self.active_pane_id())
                            .map(String::from)
                    })
                    .unwrap_or_else(|| "operator".to_string());

                let now = Instant::now();
                self.agent_mail.check_rate_limit(&sender_name, now)?;

                let targets: Vec<(String, PaneId)> = self
                    .agent_mail
                    .list()
                    .into_iter()
                    .filter(|(name, _, _)| name != &sender_name)
                    .map(|(name, pid, _)| (name, pid))
                    .collect();

                if targets.is_empty() {
                    return Err("No recipient agents registered for broadcast".to_string());
                }

                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);

                let mut recipient_names = Vec::new();
                for (name, pane_id) in &targets {
                    let msg = AgentMessage {
                        id: 0,
                        from: sender_name.clone(),
                        to: name.clone(),
                        body: body.clone(),
                        kind: kind.clone(),
                        reply_to: None,
                        hop_count: 1,
                        timestamp_ms: now_ms,
                        acked: false,
                        file_path: None,
                    };
                    if self.agent_mail.enqueue(msg).is_ok() {
                        recipient_names.push(JsonValue::String(name.clone()));
                        self.record_traffic_with(
                            TrafficEndpoint::Pane(self.active_pane_id()),
                            TrafficEndpoint::Pane(*pane_id),
                            TrafficKind::Broadcast,
                            body,
                            false,
                            now,
                        );
                    }
                }

                let count = recipient_names.len();
                let data = JsonValue::obj([
                    ("status", JsonValue::String("broadcast".to_string())),
                    ("count", JsonValue::Number(count as i64)),
                    ("recipients", JsonValue::Array(recipient_names)),
                ]);
                self.set_command_data(data.to_json_string());
                Ok(format!("Broadcast message sent to {count} agent(s)"))
            }

            AgentMailOp::Inbox { agent, unread_only } => {
                let target_agent = agent
                    .clone()
                    .or_else(|| {
                        self.agent_mail
                            .name_for_pane(self.active_pane_id())
                            .map(String::from)
                    })
                    .ok_or_else(|| {
                        "No agent specified and active pane is not named".to_string()
                    })?;

                let msgs = self.agent_mail.get_inbox(&target_agent, *unread_only);
                let total = self.agent_mail.get_inbox(&target_agent, false).len();
                let unread = self.agent_mail.unread_count_by_name(&target_agent);

                let msg_objs: Vec<JsonValue> = msgs
                    .iter()
                    .map(|m| {
                        JsonValue::obj([
                            ("id", JsonValue::Number(m.id as i64)),
                            ("from", JsonValue::String(m.from.clone())),
                            ("to", JsonValue::String(m.to.clone())),
                            ("body", JsonValue::String(m.body.clone())),
                            ("kind", JsonValue::String(m.kind.clone())),
                            (
                                "reply_to",
                                m.reply_to
                                    .map_or(JsonValue::Null, |id| JsonValue::Number(id as i64)),
                            ),
                            ("hop_count", JsonValue::Number(m.hop_count as i64)),
                            ("timestamp_ms", JsonValue::Number(m.timestamp_ms as i64)),
                            ("acked", JsonValue::Bool(m.acked)),
                            (
                                "file_path",
                                m.file_path
                                    .clone()
                                    .map_or(JsonValue::Null, JsonValue::String),
                            ),
                        ])
                    })
                    .collect();

                let data = JsonValue::obj([
                    ("agent", JsonValue::String(target_agent.clone())),
                    ("total", JsonValue::Number(total as i64)),
                    ("unread", JsonValue::Number(unread as i64)),
                    ("messages", JsonValue::Array(msg_objs)),
                ]);
                self.set_command_data(data.to_json_string());
                Ok(format!(
                    "Inbox for '{target_agent}': {} message(s) ({unread} unread)",
                    msgs.len()
                ))
            }

            AgentMailOp::Ack { agent, message_id } => {
                let target_agent = agent
                    .clone()
                    .or_else(|| {
                        self.agent_mail
                            .name_for_pane(self.active_pane_id())
                            .map(String::from)
                    })
                    .ok_or_else(|| {
                        "No agent specified and active pane is not named".to_string()
                    })?;

                let count = self.agent_mail.ack(&target_agent, *message_id);
                let data = JsonValue::obj([
                    ("status", JsonValue::String("acked".to_string())),
                    ("agent", JsonValue::String(target_agent.clone())),
                    ("count", JsonValue::Number(count as i64)),
                ]);
                self.set_command_data(data.to_json_string());
                Ok(format!(
                    "Acknowledged {count} message(s) for '{target_agent}'"
                ))
            }

            AgentMailOp::List => {
                let list = self.agent_mail.list();
                let agent_objs: Vec<JsonValue> = list
                    .iter()
                    .map(|(name, pid, unread)| {
                        let state_str =
                            self.agent_state(*pid).map(|s| s.as_str()).unwrap_or("idle");
                        JsonValue::obj([
                            ("name", JsonValue::String(name.clone())),
                            ("pane_id", JsonValue::Number(*pid as i64)),
                            ("unread", JsonValue::Number(*unread as i64)),
                            ("state", JsonValue::String(state_str.to_string())),
                        ])
                    })
                    .collect();

                let data = JsonValue::obj([
                    ("enabled", JsonValue::Bool(self.agent_mail.is_enabled())),
                    ("agents", JsonValue::Array(agent_objs)),
                ]);
                self.set_command_data(data.to_json_string());
                let summary = if list.is_empty() {
                    "No agents registered".to_string()
                } else {
                    list.iter()
                        .map(|(n, p, u)| format!("{n}(#{p})[{u}]"))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                Ok(format!("Agents ({}): {}", list.len(), summary))
            }
        }
    }

    /// Writes an agent message body to a private `0600` file under the runtime dir.
    pub fn write_agent_mail_file(&self, to: &str, id: u64, body: &str) -> Result<String, String> {
        let base = crate::secure_fs::private_runtime_dir()
            .map_err(|e| format!("cannot resolve private runtime dir: {e}"))?;
        let mail_base = base.join("agent-mail");
        crate::secure_fs::ensure_private_dir(&mail_base)
            .map_err(|e| format!("cannot create agent-mail base dir: {e}"))?;
        let dir = mail_base.join(to);
        crate::secure_fs::ensure_private_dir(&dir)
            .map_err(|e| format!("cannot create agent-mail dir: {e}"))?;
        let file_path = dir.join(format!("msg-{id}.txt"));
        crate::secure_fs::write_private_file(&file_path, body.as_bytes())
            .map_err(|e| format!("cannot write agent message file: {e}"))?;
        Ok(file_path.to_string_lossy().into_owned())
    }
}
