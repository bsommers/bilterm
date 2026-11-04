//! Agent traffic bus (Phase 50-05, D-01): a small in-memory ring of "who sent what to whom" events.
//!
//! Producers today: input broadcast, `:mcp broadcast`, and automation-API `pane.exec` /
//! `pane.send_keys` (RPC, HTTP and MCP all share one dispatcher). Phase 51 plugs `agent.send` into
//! the same bus through [`WindowManager::record_traffic`]. The zoom window graph and snoop view
//! read it with a sequence cursor ([`TrafficBus::events_since`]).
//!
//! The schema is the Phase 51 contract documented in ADR-008. The bus is in-memory only (nothing
//! is persisted), bounded to [`TRAFFIC_CAP`] events, and previews are sanitised, capped to
//! [`PREVIEW_CAP`] chars and replaced by `[redacted]` for password prompts (T-50-11..13).
//! Content is never written to tracing logs.

use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

use crate::pane::PaneId;
use crate::vt::sanitize_display;
use crate::wm::WindowManager;

/// Maximum number of events kept on the bus; the oldest is evicted first.
pub const TRAFFIC_CAP: usize = 512;
/// Maximum characters of a stored preview (including a trailing ellipsis when cut).
pub const PREVIEW_CAP: usize = 160;
/// Preview text stored instead of the content for password / `!ECHO` targets.
pub const REDACTED_PREVIEW: &str = "[redacted]";
/// Maximum chars of a session / agent name inside [`TrafficEndpoint::label`].
const LABEL_NAME_CAP: usize = 64;

/// One end of a traffic event.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrafficEndpoint {
    /// The local human operator (keyboard / mouse).
    User,
    /// An automation client (JSON-RPC, HTTP, or the Model Context Protocol transport).
    Api,
    /// The Master Control Program console (`:mcp ...`), not the Model Context Protocol transport.
    Mcp,
    /// A workspace pane.
    Pane(PaneId),
    /// A named session node in the session tree.
    Session(String),
    /// A named agent. Reserved for Phase 51.
    Agent(String),
}

impl TrafficEndpoint {
    /// Short human label: `you`, `api`, `mcp`, `#<id>`, `@<session>` or the agent name.
    pub fn label(&self) -> String {
        match self {
            Self::User => "you".to_string(),
            Self::Api => "api".to_string(),
            Self::Mcp => "mcp".to_string(),
            Self::Pane(id) => format!("#{id}"),
            Self::Session(name) => format!("@{}", sanitize_display(name, LABEL_NAME_CAP)),
            Self::Agent(name) => sanitize_display(name, LABEL_NAME_CAP),
        }
    }
}

/// What kind of traffic an event represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrafficKind {
    /// Operator keystrokes or a typed command.
    Input,
    /// A command submitted through `pane.exec`.
    Exec,
    /// Key chords sent through `pane.send_keys`.
    Keys,
    /// A command fanned out to several targets (input broadcast, `:mcp broadcast`).
    Broadcast,
    /// A message between agents. Reserved for Phase 51.
    Message,
}

impl TrafficKind {
    /// Stable lower-case name: `input | exec | keys | broadcast | message`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Exec => "exec",
            Self::Keys => "keys",
            Self::Broadcast => "broadcast",
            Self::Message => "message",
        }
    }
}

/// One recorded traffic event.
#[derive(Clone, Debug)]
pub struct TrafficEvent {
    /// Monotonic sequence number, starting at 1.
    pub seq: u64,
    /// When the event was recorded.
    pub at: Instant,
    /// Sender.
    pub from: TrafficEndpoint,
    /// Recipient.
    pub to: TrafficEndpoint,
    /// Event kind.
    pub kind: TrafficKind,
    /// Sanitised, capped preview, or `[redacted]`.
    pub preview: String,
    /// Byte length of the original content (kept even when redacted).
    pub bytes: usize,
    /// `true` when the preview was withheld.
    pub redacted: bool,
}

/// Bounded ring of [`TrafficEvent`]s with monotonic sequence numbers.
#[derive(Debug, Default)]
pub struct TrafficBus {
    events: VecDeque<TrafficEvent>,
    next_seq: u64,
    dropped: u64,
}

impl TrafficBus {
    /// Creates an empty bus.
    pub fn new() -> Self {
        Self { events: VecDeque::new(), next_seq: 1, dropped: 0 }
    }

    /// Records an event and returns its sequence number. `redact` withholds the preview.
    pub fn record(
        &mut self,
        from: TrafficEndpoint,
        to: TrafficEndpoint,
        kind: TrafficKind,
        raw: &str,
        redact: bool,
        now: Instant,
    ) -> u64 {
        if self.next_seq == 0 {
            self.next_seq = 1;
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        let preview = if redact {
            REDACTED_PREVIEW.to_string()
        } else {
            sanitize_display(raw, PREVIEW_CAP)
        };
        self.events.push_back(TrafficEvent {
            seq,
            at: now,
            from,
            to,
            kind,
            preview,
            bytes: raw.len(),
            redacted: redact,
        });
        while self.events.len() > TRAFFIC_CAP {
            self.events.pop_front();
            self.dropped += 1;
        }
        seq
    }

    /// Events with `seq` strictly greater than the cursor, oldest first.
    pub fn events_since(&self, seq: u64) -> impl Iterator<Item = &TrafficEvent> {
        self.events.iter().filter(move |e| e.seq > seq)
    }

    /// Sequence number of the newest event, or `0` when nothing was recorded yet.
    pub fn latest_seq(&self) -> u64 {
        self.next_seq.saturating_sub(1)
    }

    /// Time of the newest event, `None` when nothing is held.
    pub fn latest_at(&self) -> Option<Instant> {
        self.events.back().map(|e| e.at)
    }

    /// Number of events currently held.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// `true` when no events are held.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Number of events evicted by the ring cap since creation.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// All held events, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &TrafficEvent> {
        self.events.iter()
    }
}

/// A communication partner of one pane within a time window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrafficPartner {
    /// The other end of the events.
    pub endpoint: TrafficEndpoint,
    /// Number of events exchanged in the window.
    pub count: usize,
    /// Time of the most recent event.
    pub last: Instant,
}

impl WindowManager {
    /// Read access to the traffic bus.
    pub fn traffic(&self) -> &TrafficBus {
        &self.traffic
    }

    /// Records an event, auto-redacting when `to` is a pane in a password / `!ECHO` prompt.
    pub fn record_traffic(
        &mut self,
        from: TrafficEndpoint,
        to: TrafficEndpoint,
        kind: TrafficKind,
        raw: &str,
        now: Instant,
    ) -> u64 {
        let redact = match &to {
            TrafficEndpoint::Pane(id) => self.panes.get(id).is_some_and(|p| p.is_password_prompt()),
            _ => false,
        };
        self.traffic.record(from, to, kind, raw, redact, now)
    }

    /// Records an event with an explicit redaction decision (evaluated by the caller beforehand).
    pub fn record_traffic_with(
        &mut self,
        from: TrafficEndpoint,
        to: TrafficEndpoint,
        kind: TrafficKind,
        raw: &str,
        redact: bool,
        now: Instant,
    ) -> u64 {
        self.traffic.record(from, to, kind, raw, redact, now)
    }

    /// Records one `Mcp -> Session(name)` broadcast event for every `broadcast_log` entry added
    /// since `before` (the log length captured before an `:mcp` directive ran).
    pub(crate) fn record_new_mcp_broadcasts(&mut self, before: usize, now: Instant) {
        let fresh: Vec<(String, String)> = self
            .daemon_state
            .broadcast_log
            .get(before..)
            .map(<[_]>::to_vec)
            .unwrap_or_default();
        for (session, command) in fresh {
            self.traffic.record(
                TrafficEndpoint::Mcp,
                TrafficEndpoint::Session(session),
                TrafficKind::Broadcast,
                &command,
                false,
                now,
            );
        }
    }

    /// Communication partners of `pane` within `window` before `now`, most recent first.
    pub fn partners_of(&self, pane: PaneId, window: Duration, now: Instant) -> Vec<TrafficPartner> {
        let me = TrafficEndpoint::Pane(pane);
        let mut by_other: BTreeMap<TrafficEndpoint, (usize, Instant)> = BTreeMap::new();
        for e in self.traffic.iter() {
            if now.saturating_duration_since(e.at) > window {
                continue;
            }
            let other = if e.from == me {
                &e.to
            } else if e.to == me {
                &e.from
            } else {
                continue;
            };
            let slot = by_other.entry(other.clone()).or_insert((0, e.at));
            slot.0 += 1;
            if e.at > slot.1 {
                slot.1 = e.at;
            }
        }
        let mut out: Vec<TrafficPartner> = by_other
            .into_iter()
            .map(|(endpoint, (count, last))| TrafficPartner { endpoint, count, last })
            .collect();
        out.sort_by(|a, b| b.last.cmp(&a.last).then_with(|| a.endpoint.cmp(&b.endpoint)));
        out
    }
}
