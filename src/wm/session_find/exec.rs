//! Executor for `Action::SessionFind`: search, then focus / send / beep / flash / select.
//! Output for API callers goes through `set_command_data` (same envelope everywhere).

use super::matcher::{Match, Outcome};
use super::probe::{self, ProcInfo};
use super::query::{FindAct, FindQuery, SessionFindOp};
use super::{FLASH_MS, SEND_TARGET_CAP, SEND_TEXT_CAP};
use crate::api::JsonValue;
use crate::pane::PaneId;
use crate::pane::redact::classify;
use crate::wm::WindowManager;
use crate::wm::traffic::{TrafficEndpoint, TrafficKind};
use std::time::{Duration, Instant};

fn opt_s(v: &Option<String>) -> JsonValue {
    v.clone().map_or(JsonValue::Null, JsonValue::String)
}

fn opt_n(v: Option<u32>) -> JsonValue {
    v.map_or(JsonValue::Null, |n| JsonValue::Number(i64::from(n)))
}

fn proc_json(p: &Option<ProcInfo>) -> JsonValue {
    let Some(p) = p else { return JsonValue::Null };
    // A secret-looking command line is never shown (D-U2).
    let secret = !p.cmdline.is_empty() && classify(&p.cmdline).is_some();
    JsonValue::obj([
        ("pid", JsonValue::Number(i64::from(p.pid))),
        ("ppid", opt_n(p.ppid)),
        ("pgid", opt_n(p.pgid)),
        ("sid", opt_n(p.sid)),
        ("tty", opt_s(&p.tty)),
        ("name", JsonValue::String(p.name.clone())),
        ("cmdline", if secret { JsonValue::String("[masked]".into()) } else { JsonValue::String(p.cmdline.clone()) }),
        ("cwd", opt_s(&p.cwd)),
        ("uid", opt_n(p.uid)),
        ("user", opt_s(&p.user)),
    ])
}

fn match_json(m: &Match) -> JsonValue {
    let f = &m.facts;
    JsonValue::obj([
        ("pane", JsonValue::Number(f.pane as i64)),
        ("session", JsonValue::String(m.session.clone())),
        ("coord", opt_s(&m.coord)),
        ("title", JsonValue::String(f.title.clone())),
        ("tty", opt_s(&f.tty)),
        ("cwd", opt_s(&f.pane_cwd)),
        ("fg_pgid", opt_n(f.fg_pgid)),
        ("shell", proc_json(&f.shell)),
        ("foreground", proc_json(&f.fg)),
        ("matched", JsonValue::Array(m.matched.iter().map(|s| JsonValue::String((*s).to_string())).collect())),
        ("text_hits", JsonValue::Number(m.text_hits as i64)),
    ])
}

/// `{"query","total","returned","truncated","platform","degraded","rows"}`.
pub fn outcome_json(q: &FindQuery, o: &Outcome) -> JsonValue {
    JsonValue::obj([
        ("query", JsonValue::String(q.colon_args())),
        ("total", JsonValue::Number(o.total as i64)),
        ("returned", JsonValue::Number(o.matches.len() as i64)),
        ("truncated", JsonValue::Bool(o.truncated)),
        ("platform", JsonValue::String(probe::platform().to_string())),
        ("degraded", JsonValue::Array(o.degraded.iter().cloned().map(JsonValue::String).collect())),
        ("rows", JsonValue::Array(o.matches.iter().map(match_json).collect())),
    ])
}

fn ids_json(ids: &[PaneId]) -> JsonValue {
    JsonValue::Array(ids.iter().map(|i| JsonValue::Number(*i as i64)).collect())
}

fn summary(o: &Outcome) -> String {
    let panes: Vec<String> = o.matches.iter().take(8).map(|m| format!("#{}", m.facts.pane)).collect();
    format!("Find: {} match(es){}{}", o.total, if panes.is_empty() { String::new() } else { format!(" [{}]", panes.join(" ")) }, if o.truncated { ", truncated" } else { "" })
}

impl WindowManager {
    /// Executes one session-find operation; never quits.
    pub fn session_find_apply(&mut self, op: SessionFindOp) -> bool {
        let result = match op {
            SessionFindOp::Locate(q) => Ok(self.sf_locate(&q)),
            SessionFindOp::Act { act, query } => self.sf_act(act, &query),
            SessionFindOp::Prefill => Ok(self.sf_prefill()),
            SessionFindOp::ClearSelection => {
                self.session_find.selected.clear();
                Ok(("Find: selection cleared".to_string(), JsonValue::obj([("selected", ids_json(&[]))])))
            }
        };
        let (msg, data) = match result {
            Ok(ok) => ok,
            Err(e) => (format!("Find: {e}"), JsonValue::obj([("error", JsonValue::String(e))])),
        };
        self.set_command_data(data.to_json_string());
        self.active_pane_mut().set_status_message(Some(msg));
        false
    }

    fn sf_locate(&mut self, q: &FindQuery) -> (String, JsonValue) {
        let o = self.session_find_run(q);
        self.session_find.last = o.matches.iter().map(|m| m.facts.pane).collect();
        (summary(&o), outcome_json(q, &o))
    }

    fn sf_prefill(&mut self) -> (String, JsonValue) {
        const SEED: &str = ":find --text ";
        let shown = self.active_pane().is_prompt_visible();
        if shown {
            self.active_pane_mut().prompt_mut().set_text(SEED);
        }
        let msg = if shown { "Find: edit the filters (--pid --pty --cwd --cmd --text ...) and press Enter" } else { "Find: the prompt is hidden (raw mode), nothing was placed" };
        (msg.to_string(), JsonValue::obj([("prefilled", JsonValue::Bool(shown))]))
    }

    /// Targets: the explicit query's matches, or the last search when the query is empty.
    fn sf_targets(&mut self, query: &FindQuery) -> (Vec<PaneId>, Option<JsonValue>) {
        if query.is_empty() {
            let live: Vec<PaneId> = self.session_find.last.iter().copied().filter(|i| self.pane(*i).is_some()).collect();
            return (live, None);
        }
        let o = self.session_find_run(query);
        let ids: Vec<PaneId> = o.matches.iter().map(|m| m.facts.pane).collect();
        self.session_find.last = ids.clone();
        (ids, Some(outcome_json(query, &o)))
    }

    fn sf_act(&mut self, act: FindAct, query: &FindQuery) -> Result<(String, JsonValue), String> {
        let (ids, rows) = self.sf_targets(query);
        if ids.is_empty() {
            return Err("no matches (run :find --... first, or give filters)".to_string());
        }
        let mut skipped: Vec<(PaneId, &'static str)> = Vec::new();
        let (acted, msg): (Vec<PaneId>, String) = match &act {
            FindAct::Focus => {
                let id = ids[0];
                if !self.focus_and_raise(id) {
                    return Err(format!("pane #{id} could not be focused"));
                }
                (vec![id], format!("Find: focused #{id} ({} match(es))", ids.len()))
            }
            FindAct::Send { text, enter } => {
                let done = self.sf_send(&ids, text, *enter, &mut skipped)?;
                let n = done.len();
                (done, format!("Find: sent to {n} pane(s), {} skipped", skipped.len()))
            }
            FindAct::Beep => {
                for id in &ids {
                    if let Some(p) = self.pane_mut(*id) {
                        p.vt_mut().ring_bell();
                    }
                }
                self.session_find.host_bell = true;
                self.trigger_tron_beep();
                (ids.clone(), format!("Find: bell on {} pane(s)", ids.len()))
            }
            FindAct::Flash => {
                let until = Instant::now() + Duration::from_millis(FLASH_MS);
                for id in &ids {
                    self.session_find.flash.insert(*id, until);
                }
                (ids.clone(), format!("Find: flashing {} pane(s)", ids.len()))
            }
            FindAct::Select => {
                self.session_find.selected = ids.iter().copied().collect();
                (ids.clone(), format!("Find: selected {} pane(s)", ids.len()))
            }
        };
        let mut entries = vec![
            ("action", JsonValue::String(act.verb().to_string())),
            ("targets", ids_json(&ids)),
            ("acted", ids_json(&acted)),
            (
                "skipped",
                JsonValue::Array(
                    skipped
                        .iter()
                        .map(|(id, why)| JsonValue::obj([("pane", JsonValue::Number(*id as i64)), ("reason", JsonValue::String((*why).to_string()))]))
                        .collect(),
                ),
            ),
        ];
        if let Some(rows) = rows {
            entries.push(("result", rows));
        }
        Ok((msg, JsonValue::obj(entries)))
    }

    /// Types `text` into each target with the `pane.send` gates: read-only (PROT) and `!ECHO`
    /// password panes are skipped (never overridden here), the target count and text size are capped.
    fn sf_send(&mut self, ids: &[PaneId], text: &str, enter: bool, skipped: &mut Vec<(PaneId, &'static str)>) -> Result<Vec<PaneId>, String> {
        if ids.len() > SEND_TARGET_CAP {
            return Err(format!("{} matches exceed the send limit of {SEND_TARGET_CAP}; narrow the filters", ids.len()));
        }
        if text.len() > SEND_TEXT_CAP {
            return Err(format!("text exceeds {SEND_TEXT_CAP} bytes"));
        }
        let mut bytes = text.as_bytes().to_vec();
        if enter {
            bytes.push(b'\r');
        }
        let from = if self.is_api_origin() { TrafficEndpoint::Api } else { TrafficEndpoint::User };
        let mut done = Vec::new();
        for &id in ids {
            let Some(pane) = self.pane_mut(id) else { continue };
            if pane.is_read_only() {
                skipped.push((id, "read_only"));
                continue;
            }
            if pane.is_injection_password_locked() {
                skipped.push((id, "echo_locked"));
                continue;
            }
            if pane.write_input(&bytes).is_err() {
                skipped.push((id, "write_failed"));
                continue;
            }
            self.record_traffic_with(from.clone(), TrafficEndpoint::Pane(id), TrafficKind::Keys, text, false, Instant::now());
            done.push(id);
        }
        Ok(done)
    }
}
