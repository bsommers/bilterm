//! Pane selectors for agent/pane RPC (`HERDR-02`).
//!
//! Grammar (case-insensitive):
//! - `w<N>:p<M>` hierarchical: workspace `N` (1 is the live workspace), pane id `M`
//! - `p<M>` or a bare integer: pane id in the live workspace
//! - `@idle|@working|@blocked|@done|@errored`: first pane in that agent state
//! - `@focused` / `@active`: the focused pane
//! - `@<name>`: first pane whose agent profile (`claude`, `agy`, ...), agent-mail name or title
//!   contains `<name>`
//!
//! Semantic selectors skip `exclude` (normally the caller's own `BILTERM_PANE_ID`) so an agent
//! never prompts itself, and pick the lowest pane id when several match.

use super::rpc::AutomationControlPlane;
use crate::pane::PaneId;
use crate::wm::agent_state::{AgentState, AgentStatus};

/// A parsed selector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selector {
    Pane { workspace: usize, pane: PaneId },
    State(AgentState),
    Focused,
    Named(String),
}

/// Parses a selector string.
pub fn parse_selector(raw: &str) -> Result<Selector, String> {
    let s = raw.trim().to_ascii_lowercase();
    if s.is_empty() || s.len() > 64 {
        return Err("empty or oversized selector".to_string());
    }
    if let Some(name) = s.strip_prefix('@') {
        return Ok(match name {
            "idle" => Selector::State(AgentState::Idle),
            "working" => Selector::State(AgentState::Working),
            "blocked" => Selector::State(AgentState::Blocked),
            "done" => Selector::State(AgentState::Done),
            "errored" | "error" => Selector::State(AgentState::Errored),
            "focused" | "active" => Selector::Focused,
            "" => return Err("empty @selector".to_string()),
            n if n.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) => {
                Selector::Named(n.to_string())
            }
            _ => return Err(format!("invalid selector '{raw}'")),
        });
    }
    let bad = || format!("invalid selector '{raw}' (expected w<N>:p<M>, p<M>, <id>, or @claude/@agy/@idle/@working/@blocked)");
    let num = |t: &str, prefix: char| -> Option<usize> {
        t.strip_prefix(prefix).unwrap_or(t).parse::<usize>().ok()
    };
    if let Some((w, p)) = s.split_once(':') {
        if !w.starts_with('w') || !p.starts_with('p') {
            return Err(bad());
        }
        let workspace = num(w, 'w').ok_or_else(bad)?;
        let pane = num(p, 'p').ok_or_else(bad)?;
        return Ok(Selector::Pane { workspace, pane });
    }
    let pane = num(&s, 'p').ok_or_else(bad)?;
    Ok(Selector::Pane { workspace: 1, pane })
}

/// Pure resolution over a status table (testable without a window manager).
pub fn resolve_in(
    sel: &Selector,
    statuses: &[AgentStatus],
    titles: &dyn Fn(PaneId) -> String,
    focused: PaneId,
    exclude: Option<PaneId>,
) -> Result<PaneId, String> {
    let ok = |id: PaneId| Some(id) != exclude;
    let found = match sel {
        Selector::Pane { workspace, pane } => {
            if *workspace != 1 {
                return Err(format!(
                    "workspace w{workspace} is not live in this instance (only w1 is; see `workspace list`)"
                ));
            }
            return statuses
                .iter()
                .any(|s| s.pane_id == *pane)
                .then_some(*pane)
                .ok_or_else(|| format!("pane p{pane} not found in w1"));
        }
        Selector::Focused => Some(focused),
        Selector::State(st) => statuses.iter().find(|s| s.state == *st && ok(s.pane_id)).map(|s| s.pane_id),
        Selector::Named(n) => statuses
            .iter()
            .find(|s| {
                ok(s.pane_id)
                    && (s.profile.as_deref().is_some_and(|p| p.to_ascii_lowercase().contains(n.as_str()))
                        || s.name.to_ascii_lowercase().contains(n.as_str())
                        || titles(s.pane_id).to_ascii_lowercase().contains(n.as_str()))
            })
            .map(|s| s.pane_id),
    };
    found.ok_or_else(|| "selector matched no pane".to_string())
}

impl AutomationControlPlane {
    /// Resolves `selector` (and falls back to `pane_id` / the active pane) to a live pane id.
    pub(super) fn resolve_selector(
        &mut self,
        selector: &str,
        exclude: Option<PaneId>,
    ) -> Result<PaneId, String> {
        let sel = parse_selector(selector)?;
        self.wm.update_agent_states(std::time::Instant::now());
        let statuses = self.wm.agent_statuses();
        let titles = |id: PaneId| self.wm.pane(id).map(|p| p.title().to_string()).unwrap_or_default();
        let id = resolve_in(&sel, &statuses, &titles, self.wm.active_pane_id(), exclude)?;
        self.wm.pane(id).map(|_| id).ok_or_else(|| format!("pane #{id} not found"))
    }

    /// Target pane for an RPC: `selector` first, then `pane_id`, then the active pane.
    pub(super) fn target_pane(&mut self, params: &super::json::JsonValue) -> Result<PaneId, String> {
        let exclude = params.get("from_pane").and_then(super::json::JsonValue::as_usize);
        if let Some(sel) = params.get("selector").and_then(super::json::JsonValue::as_str) {
            return self.resolve_selector(sel, exclude);
        }
        Ok(params
            .get("pane_id")
            .and_then(super::json::JsonValue::as_usize)
            .unwrap_or_else(|| self.wm.active_pane_id()))
    }
}
