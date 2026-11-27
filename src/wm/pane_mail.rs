//! Pane-mail envelope state: "another session, a tool or an agent messaged this pane".
//!
//! Per-pane `{unread, last sender, last_at}` is held privately here (`ARCH-R02`) and set only at
//! the delivery choke points: [`WindowManager::record_traffic_with`] for text injected by a
//! non-local source (`pane.send`, `pane.send_keys`, `pane.exec`, `find send` over RPC/MCP/CLI),
//! `Action::PaneInputBytes` (attach clients) and agent mail delivery. The local user's own typing
//! never calls [`WindowManager::note_pane_message`]. Unread -> closed envelope (blinking at ~1 Hz);
//! read (focus dwell, local key, `:mail ack`, `pane.mail_ack`) -> open envelope, hidden later.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::api::JsonValue;
use crate::pane::PaneId;
use crate::wm::WindowManager;

/// Half period of the unread blink (1 Hz: 500 ms on, 500 ms off).
pub const FLASH_HALF_MS: u64 = 500;
/// How long a pane must stay focused before its unread mail counts as read.
pub const FOCUS_DWELL: Duration = Duration::from_secs(1);
/// How long the open (read) envelope stays before it disappears.
pub const OPEN_HIDE: Duration = Duration::from_secs(60);
const SENDER_CAP: usize = 32;

/// Canonical pane-mail operations (`:mail ...`, `pane.mail_state`, `pane.mail_ack`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneMailOp {
    /// `:mail status [pane]`: report (all panes when `None`).
    Status { pane: Option<PaneId> },
    /// `:mail ack [pane|all]`: mark read (active pane when `pane` is `None` and `!all`).
    Ack { pane: Option<PaneId>, all: bool },
}

impl PaneMailOp {
    /// Canonical `:mail` colon form.
    pub fn colon_form(&self) -> String {
        match self {
            Self::Status { pane: Some(p) } => format!(":mail status {p}"),
            Self::Status { pane: None } => ":mail status".to_string(),
            Self::Ack { all: true, .. } => ":mail ack all".to_string(),
            Self::Ack { pane: Some(p), .. } => format!(":mail ack {p}"),
            Self::Ack { pane: None, .. } => ":mail ack".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
struct PaneMail {
    unread: u32,
    sender: String,
    last_at: Instant,
    read_at: Option<Instant>,
}

/// Renderer-facing view of one pane's envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaneMailView {
    /// Unread count; `0` means read (open envelope).
    pub unread: u32,
    pub sender: String,
    /// Blink phase: `false` while an unread envelope is in its "off" half period.
    pub visible: bool,
}

/// All panes' mail state plus the focus-dwell clock.
#[derive(Debug, Default)]
pub struct PaneMailTracker {
    map: BTreeMap<PaneId, PaneMail>,
    focus: Option<(PaneId, Instant)>,
    last_phase: bool,
}

impl PaneMailTracker {
    pub(crate) fn remove(&mut self, id: PaneId) {
        self.map.remove(&id);
    }

    pub(crate) fn has_unread(&self, id: PaneId) -> bool {
        self.map.get(&id).is_some_and(|e| e.unread > 0)
    }
}

fn phase_on(since: Instant, now: Instant) -> bool {
    (now.saturating_duration_since(since).as_millis() as u64 / FLASH_HALF_MS).is_multiple_of(2)
}

impl WindowManager {
    /// Records a message to `id` from `sender` (a short label). Choke-point API; never call for the
    /// local user's own typing.
    pub fn note_pane_message(&mut self, id: PaneId, sender: &str, now: Instant) {
        if !self.panes.contains_key(&id) {
            return;
        }
        let label: String = sender.chars().filter(|c| !c.is_control()).take(SENDER_CAP).collect();
        let e = self.pane_mail.map.entry(id).or_insert(PaneMail { unread: 0, sender: String::new(), last_at: now, read_at: None });
        e.unread = e.unread.saturating_add(1);
        e.sender = label;
        e.last_at = now;
        e.read_at = None;
    }

    /// Marks mail read: `Some(id)` one pane, `None` every pane. Returns how many panes had unread mail.
    pub fn pane_mail_ack(&mut self, id: Option<PaneId>, now: Instant) -> usize {
        let mut n = 0;
        for (pid, e) in self.pane_mail.map.iter_mut() {
            if id.is_none_or(|i| i == *pid) && e.unread > 0 {
                e.unread = 0;
                e.read_at = Some(now);
                n += 1;
            }
        }
        n
    }

    /// A key typed by the local user: whatever is unread on the active pane counts as read.
    pub fn pane_mail_on_local_key(&mut self) {
        if !self.pane_mail.map.is_empty() && !self.is_api_origin() {
            let id = self.active_pane_id();
            self.pane_mail_ack(Some(id), Instant::now());
        }
    }

    /// Envelope view for `id` at `now`; `None` when there is nothing to show or the indicator is off.
    pub fn pane_mail_view(&self, id: PaneId, now: Instant) -> Option<PaneMailView> {
        if !self.config().ui.pane_mail_indicator {
            return None;
        }
        let e = self.pane_mail.map.get(&id)?;
        let visible = e.unread == 0 || !self.config().ui.pane_mail_flash || phase_on(e.last_at, now);
        Some(PaneMailView { unread: e.unread, sender: e.sender.clone(), visible })
    }

    /// Advances dwell/expiry/blink. `true` when something visible changed (the caller redraws).
    pub fn pane_mail_tick(&mut self, now: Instant) -> bool {
        if self.pane_mail.map.is_empty() {
            self.pane_mail.focus = None;
            return false;
        }
        let mut changed = false;
        let panes = &self.panes;
        let before = self.pane_mail.map.len();
        self.pane_mail.map.retain(|id, e| panes.contains_key(id) && e.read_at.is_none_or(|r| now.saturating_duration_since(r) < OPEN_HIDE));
        changed |= before != self.pane_mail.map.len();
        let active = self.active_pane_id();
        match self.pane_mail.focus {
            Some((id, since)) if id == active => {
                if now.saturating_duration_since(since) >= FOCUS_DWELL {
                    changed |= self.pane_mail_ack(Some(active), now) > 0;
                }
            }
            _ => self.pane_mail.focus = Some((active, now)),
        }
        if self.config().ui.pane_mail_indicator && self.config().ui.pane_mail_flash {
            let phase = self.pane_mail.map.values().filter(|e| e.unread > 0).fold(0u8, |acc, e| acc ^ u8::from(phase_on(e.last_at, now)));
            let cur = phase == 1;
            if self.pane_mail.map.values().any(|e| e.unread > 0) && cur != self.pane_mail.last_phase {
                changed = true;
            }
            self.pane_mail.last_phase = cur;
        }
        changed
    }

    /// `{unread, sender, age_ms}` for listings; unread `0` and null sender when no mail.
    pub fn pane_mail_json(&self, id: PaneId, now: Instant) -> JsonValue {
        match self.pane_mail.map.get(&id) {
            Some(e) => JsonValue::obj([
                ("pane_id", JsonValue::Number(id as i64)),
                ("unread", JsonValue::Number(i64::from(e.unread))),
                ("read", JsonValue::Bool(e.unread == 0)),
                ("sender", JsonValue::String(e.sender.clone())),
                ("age_ms", JsonValue::Number(now.saturating_duration_since(e.last_at).as_millis() as i64)),
            ]),
            None => JsonValue::obj([
                ("pane_id", JsonValue::Number(id as i64)),
                ("unread", JsonValue::Number(0)),
                ("read", JsonValue::Bool(true)),
                ("sender", JsonValue::Null),
                ("age_ms", JsonValue::Null),
            ]),
        }
    }

    /// Applies a [`PaneMailOp`]; the JSON result is left in `command_data`.
    pub fn pane_mail_apply(&mut self, op: &PaneMailOp) -> Result<String, String> {
        let now = Instant::now();
        let check = |wm: &Self, p: PaneId| if wm.panes.contains_key(&p) { Ok(()) } else { Err(format!("Pane #{p} not found")) };
        let msg = match op {
            PaneMailOp::Status { pane } => {
                let ids: Vec<PaneId> = match pane {
                    Some(p) => {
                        check(self, *p)?;
                        vec![*p]
                    }
                    None => self.pane_mail.map.keys().copied().collect(),
                };
                let rows: Vec<JsonValue> = ids.iter().map(|i| self.pane_mail_json(*i, now)).collect();
                let unread: u32 = ids.iter().filter_map(|i| self.pane_mail.map.get(i)).map(|e| e.unread).sum();
                self.set_command_data(JsonValue::obj([("panes", JsonValue::Array(rows)), ("unread", JsonValue::Number(i64::from(unread)))]).to_json_string());
                format!("Pane mail: {unread} unread in {} pane(s)", ids.len())
            }
            PaneMailOp::Ack { pane, all } => {
                let target = if *all {
                    None
                } else {
                    let p = pane.unwrap_or_else(|| self.active_pane_id());
                    check(self, p)?;
                    Some(p)
                };
                let n = self.pane_mail_ack(target, now);
                let acked: Vec<JsonValue> = match target {
                    Some(p) => vec![self.pane_mail_json(p, now)],
                    None => self.pane_mail.map.keys().map(|i| self.pane_mail_json(*i, now)).collect(),
                };
                self.set_command_data(JsonValue::obj([("status", JsonValue::String("acked".into())), ("count", JsonValue::Number(n as i64)), ("panes", JsonValue::Array(acked))]).to_json_string());
                format!("Pane mail: marked {n} pane(s) read")
            }
        };
        Ok(msg)
    }
}
