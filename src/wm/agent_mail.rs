//! Agent-to-Agent Messaging & Coordination registry (`AGT-01..03`, Phase 52).
//!
//! Provides isolated in-memory mailboxes, stable naming, loop guard enforcement,
//! rate limiting, payload capping, delivery dispatch (Mailbox, Nudge, File), and
//! unread telemetry badges.

use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::pane::PaneId;

/// Maximum allowed payload size for message bodies (8 KiB).
pub const MAX_BODY_BYTES: usize = 8192;
/// Maximum messages queued per recipient inbox before rejection.
pub const MAX_INBOX_DEPTH: usize = 64;
/// Maximum hops allowed on reply chains to prevent recursive agent loops.
pub const MAX_HOPS: u8 = 8;
/// Maximum messages permitted per sender per rate window.
pub const MAX_MSGS_PER_WINDOW: usize = 32;
/// Rate-limiting window duration.
pub const RATE_WINDOW_SECS: u64 = 60;
/// Maximum length of nudge previews injected into terminal canvases.
pub const NUDGE_PREVIEW_CAP: usize = 60;

/// Delivery mode for an agent message (`AGT-02`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentDeliveryMode {
    /// Queued in the recipient's mailbox for passive API / MCP polling.
    #[default]
    Mailbox,
    /// Queued in mailbox and injected as a comment notification if pane is idle.
    Nudge,
    /// Written to a private `0600` file; path is recorded and nudged if idle.
    File,
}

impl AgentDeliveryMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mailbox => "mailbox",
            Self::Nudge => "nudge",
            Self::File => "file",
        }
    }
}

/// A structured message passed between agents (`AGT-01`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: u64,
    pub from: String,
    pub to: String,
    pub body: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<u64>,
    pub hop_count: u8,
    pub timestamp_ms: u64,
    pub acked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
}

/// Canonical operations for agent messaging (`ARCH-R01`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentMailOp {
    /// `:agent name [pane_id] <name>`
    SetName {
        pane_id: Option<usize>,
        name: String,
    },
    /// `:agent send [--nudge|--file] <to> <body...>`
    Send {
        from: Option<String>,
        to: String,
        body: String,
        kind: String,
        delivery: AgentDeliveryMode,
        reply_to: Option<u64>,
    },
    /// `:agent broadcast [--tree <subtree>] <body...>`
    Broadcast {
        from: Option<String>,
        subtree: Option<String>,
        body: String,
        kind: String,
    },
    /// `:agent inbox [name] [--unread]`
    Inbox {
        agent: Option<String>,
        unread_only: bool,
    },
    /// `:agent ack [name] [id|all]`
    Ack {
        agent: Option<String>,
        message_id: Option<u64>,
    },
    /// `:agent list`
    List,
    /// `:agent enable [on|off]`
    Enable(Option<bool>),
}

impl AgentMailOp {
    /// Reconstructs the canonical `:agent` colon command string.
    pub fn colon_form(&self) -> String {
        match self {
            Self::SetName { pane_id, name } => match pane_id {
                Some(id) => format!(":agent name {id} {name}"),
                None => format!(":agent name {name}"),
            },
            Self::Send {
                to,
                body,
                delivery,
                ..
            } => {
                let flag = match delivery {
                    AgentDeliveryMode::Mailbox => "",
                    AgentDeliveryMode::Nudge => "--nudge ",
                    AgentDeliveryMode::File => "--file ",
                };
                format!(":agent send {flag}{to} {body}")
            }
            Self::Broadcast { subtree, body, .. } => {
                if let Some(st) = subtree {
                    format!(":agent broadcast --tree {st} {body}")
                } else {
                    format!(":agent broadcast {body}")
                }
            }
            Self::Inbox { agent, unread_only } => match (agent, unread_only) {
                (Some(a), true) => format!(":agent inbox --unread {a}"),
                (Some(a), false) => format!(":agent inbox {a}"),
                (None, true) => ":agent inbox --unread".to_string(),
                (None, false) => ":agent inbox".to_string(),
            },
            Self::Ack { agent, message_id } => match (agent, message_id) {
                (Some(a), Some(id)) => format!(":agent ack {a} {id}"),
                (Some(a), None) => format!(":agent ack {a} all"),
                (None, Some(id)) => format!(":agent ack {id}"),
                (None, None) => ":agent ack all".to_string(),
            },
            Self::List => ":agent list".to_string(),
            Self::Enable(Some(true)) => ":agent enable on".to_string(),
            Self::Enable(Some(false)) => ":agent enable off".to_string(),
            Self::Enable(None) => ":agent enable".to_string(),
        }
    }
}

/// Validates that an agent name is safe, non-empty, and single-component.
pub fn validate_agent_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("agent name cannot be empty".to_string());
    }
    if name.len() > 64 {
        return Err("agent name too long (max 64 chars)".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "invalid agent name '{name}': only ASCII alphanumeric, '-', and '_' are allowed"
        ));
    }
    Ok(())
}

/// Sanitizes message preview text for terminal canvas nudge injection.
pub fn sanitize_preview(body: &str, cap: usize) -> String {
    let mut out = String::new();
    for c in body.chars() {
        if out.len() >= cap {
            out.push_str("...");
            break;
        }
        if c == '\n' || c == '\r' || c == '\t' {
            out.push(' ');
        } else if !c.is_control() {
            out.push(c);
        }
    }
    out
}

/// In-memory mailbox registry and coordination engine.
#[derive(Clone, Debug)]
pub struct AgentMailRegistry {
    enabled: bool,
    names: BTreeMap<String, PaneId>,
    pane_names: BTreeMap<PaneId, String>,
    inboxes: BTreeMap<String, VecDeque<AgentMessage>>,
    rate_limits: BTreeMap<String, Vec<Instant>>,
    next_id: u64,
}

impl AgentMailRegistry {
    /// Creates a new registry with initial enabled status.
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            names: BTreeMap::new(),
            pane_names: BTreeMap::new(),
            inboxes: BTreeMap::new(),
            rate_limits: BTreeMap::new(),
            next_id: 1,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Registers a stable name for a pane.
    pub fn register_name(&mut self, pane_id: PaneId, name: &str) -> Result<(), String> {
        validate_agent_name(name)?;
        if let Some(old_name) = self.pane_names.remove(&pane_id) {
            self.names.remove(&old_name);
        }
        if let Some(prev_pane) = self.names.remove(name) {
            self.pane_names.remove(&prev_pane);
        }
        self.names.insert(name.to_string(), pane_id);
        self.pane_names.insert(pane_id, name.to_string());
        self.inboxes.entry(name.to_string()).or_default();
        Ok(())
    }

    pub fn name_for_pane(&self, pane_id: PaneId) -> Option<&str> {
        self.pane_names.get(&pane_id).map(String::as_str)
    }

    pub fn pane_for_name(&self, name: &str) -> Option<PaneId> {
        self.names.get(name).copied()
    }

    /// Resolves recipient string to `(recipient_name, Option<PaneId>)`.
    pub fn resolve_recipient(&self, to: &str) -> Option<(String, PaneId)> {
        if let Some(&pane_id) = self.names.get(to) {
            return Some((to.to_string(), pane_id));
        }
        if let Some(stripped) = to.strip_prefix('#')
            && let Ok(id) = stripped.parse::<usize>()
            && let Some(name) = self.pane_names.get(&id)
        {
            return Some((name.clone(), id));
        }
        None
    }

    pub fn remove_pane(&mut self, pane_id: PaneId) {
        if let Some(name) = self.pane_names.remove(&pane_id) {
            self.names.remove(&name);
        }
    }

    pub fn unread_count_by_name(&self, name: &str) -> usize {
        self.inboxes
            .get(name)
            .map(|q| q.iter().filter(|m| !m.acked).count())
            .unwrap_or(0)
    }

    pub fn unread_count_by_pane(&self, pane_id: PaneId) -> usize {
        self.pane_names
            .get(&pane_id)
            .map(|name| self.unread_count_by_name(name))
            .unwrap_or(0)
    }

    /// Enforces per-sender rate limit (`MAX_MSGS_PER_WINDOW` per 60s).
    pub fn check_rate_limit(&mut self, from: &str, now: Instant) -> Result<(), String> {
        let window = Duration::from_secs(RATE_WINDOW_SECS);
        let timestamps = self.rate_limits.entry(from.to_string()).or_default();
        timestamps.retain(|t| now.saturating_duration_since(*t) < window);
        if timestamps.len() >= MAX_MSGS_PER_WINDOW {
            return Err(format!(
                "Rate limit exceeded for '{from}' (max {MAX_MSGS_PER_WINDOW} msgs per minute)"
            ));
        }
        timestamps.push(now);
        Ok(())
    }

    /// Inspects existing messages across all inboxes to compute hop count.
    pub fn hop_count_for_reply(&self, reply_to: Option<u64>) -> Result<u8, String> {
        let Some(prev_id) = reply_to else {
            return Ok(1);
        };
        for queue in self.inboxes.values() {
            if let Some(prev) = queue.iter().find(|m| m.id == prev_id) {
                let next_hop = prev.hop_count.saturating_add(1);
                if next_hop > MAX_HOPS {
                    return Err(format!(
                        "Agent message loop guard triggered (hop_count {next_hop} > {MAX_HOPS})"
                    ));
                }
                return Ok(next_hop);
            }
        }
        Ok(1)
    }

    /// Enqueues a message into the recipient's mailbox.
    pub fn enqueue(&mut self, mut msg: AgentMessage) -> Result<AgentMessage, String> {
        if msg.body.len() > MAX_BODY_BYTES {
            return Err(format!(
                "Message body exceeds {MAX_BODY_BYTES} bytes (was {})",
                msg.body.len()
            ));
        }
        let queue = self.inboxes.entry(msg.to.clone()).or_default();
        if queue.len() >= MAX_INBOX_DEPTH {
            return Err(format!(
                "Inbox for '{}' is full (max {MAX_INBOX_DEPTH} messages)",
                msg.to
            ));
        }
        msg.id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        queue.push_back(msg.clone());
        Ok(msg)
    }

    /// Returns messages for an agent.
    pub fn get_inbox(&self, agent: &str, unread_only: bool) -> Vec<AgentMessage> {
        self.inboxes
            .get(agent)
            .map(|q| {
                q.iter()
                    .filter(|m| !unread_only || !m.acked)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Acknowledges message(s) for an agent. Returns count of acked messages.
    pub fn ack(&mut self, agent: &str, message_id: Option<u64>) -> usize {
        let Some(queue) = self.inboxes.get_mut(agent) else {
            return 0;
        };
        let mut count = 0;
        for msg in queue.iter_mut() {
            if match message_id {
                Some(id) => msg.id == id,
                None => true,
            } && !msg.acked
            {
                msg.acked = true;
                count += 1;
            }
        }
        count
    }

    /// Lists all registered agents and their statuses.
    pub fn list(&self) -> Vec<(String, PaneId, usize)> {
        self.names
            .iter()
            .map(|(name, &pane_id)| (name.clone(), pane_id, self.unread_count_by_name(name)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_validation_rules() {
        assert!(validate_agent_name("alice").is_ok());
        assert!(validate_agent_name("agent-1").is_ok());
        assert!(validate_agent_name("worker_99").is_ok());
        assert!(validate_agent_name("").is_err());
        assert!(validate_agent_name("bad/name").is_err());
        assert!(validate_agent_name("../traversal").is_err());
        assert!(validate_agent_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn registration_and_resolution() {
        let mut reg = AgentMailRegistry::new(true);
        assert!(reg.register_name(1, "alice").is_ok());
        assert_eq!(reg.name_for_pane(1), Some("alice"));
        assert_eq!(reg.pane_for_name("alice"), Some(1));
        assert_eq!(reg.resolve_recipient("alice"), Some(("alice".to_string(), 1)));
        assert_eq!(reg.resolve_recipient("#1"), Some(("alice".to_string(), 1)));
        assert_eq!(reg.resolve_recipient("bob"), None);
    }

    #[test]
    fn enqueue_ack_and_unread_counts() {
        let mut reg = AgentMailRegistry::new(true);
        reg.register_name(1, "alice").unwrap();

        let msg1 = AgentMessage {
            id: 0,
            from: "bob".to_string(),
            to: "alice".to_string(),
            body: "hello 1".to_string(),
            kind: "test".to_string(),
            reply_to: None,
            hop_count: 1,
            timestamp_ms: 1000,
            acked: false,
            file_path: None,
        };
        let m1 = reg.enqueue(msg1).unwrap();
        assert_eq!(m1.id, 1);
        assert_eq!(reg.unread_count_by_name("alice"), 1);

        let msg2 = AgentMessage {
            id: 0,
            from: "bob".to_string(),
            to: "alice".to_string(),
            body: "hello 2".to_string(),
            kind: "test".to_string(),
            reply_to: None,
            hop_count: 1,
            timestamp_ms: 1001,
            acked: false,
            file_path: None,
        };
        reg.enqueue(msg2).unwrap();
        assert_eq!(reg.unread_count_by_name("alice"), 2);

        // Ack message 1
        assert_eq!(reg.ack("alice", Some(1)), 1);
        assert_eq!(reg.unread_count_by_name("alice"), 1);

        // Ack remaining
        assert_eq!(reg.ack("alice", None), 1);
        assert_eq!(reg.unread_count_by_name("alice"), 0);
    }

    #[test]
    fn loop_guard_stops_excessive_reply_chains() {
        let mut reg = AgentMailRegistry::new(true);
        reg.register_name(1, "alice").unwrap();
        reg.register_name(2, "bob").unwrap();

        let mut last_id = 0;
        let mut hop = 1;
        for i in 1..=MAX_HOPS {
            let msg = AgentMessage {
                id: 0,
                from: if i % 2 == 1 { "alice" } else { "bob" }.to_string(),
                to: if i % 2 == 1 { "bob" } else { "alice" }.to_string(),
                body: format!("ping {i}"),
                kind: "ping".to_string(),
                reply_to: if last_id > 0 { Some(last_id) } else { None },
                hop_count: hop,
                timestamp_ms: 1000 + i as u64,
                acked: false,
                file_path: None,
            };
            let sent = reg.enqueue(msg).unwrap();
            last_id = sent.id;
            hop = reg.hop_count_for_reply(Some(last_id)).unwrap_or(hop + 1);
        }

        // 9th hop must fail
        let err = reg.hop_count_for_reply(Some(last_id));
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("loop guard triggered"));
    }

    #[test]
    fn rate_limiting_enforcement() {
        let mut reg = AgentMailRegistry::new(true);
        let now = Instant::now();
        for _ in 0..MAX_MSGS_PER_WINDOW {
            assert!(reg.check_rate_limit("spammer", now).is_ok());
        }
        let err = reg.check_rate_limit("spammer", now);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("Rate limit exceeded"));

        // After window expires, succeeds again
        let later = now + Duration::from_secs(RATE_WINDOW_SECS + 1);
        assert!(reg.check_rate_limit("spammer", later).is_ok());
    }

    #[test]
    fn body_size_cap_enforcement() {
        let mut reg = AgentMailRegistry::new(true);
        reg.register_name(1, "alice").unwrap();
        let huge_body = "x".repeat(MAX_BODY_BYTES + 1);
        let msg = AgentMessage {
            id: 0,
            from: "bob".to_string(),
            to: "alice".to_string(),
            body: huge_body,
            kind: "test".to_string(),
            reply_to: None,
            hop_count: 1,
            timestamp_ms: 1000,
            acked: false,
            file_path: None,
        };
        let err = reg.enqueue(msg);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("Message body exceeds"));
    }
}
