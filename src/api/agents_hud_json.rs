//! JSON views of the Phase 70 HUD, replay logs and watchdog (RPC `agent.hud|replay|watchdog`,
//! MCP `bilterm_agent_control`, `wm.command` data). Read-only; no text of masked (no-echo) panes.

use super::json::JsonValue;
use crate::pane::PaneId;
use crate::wm::WindowManager;
use crate::wm::turn_replay::Turn;

fn num(n: u64) -> JsonValue {
    JsonValue::Number(i64::try_from(n).unwrap_or(i64::MAX))
}

fn s(v: &str) -> JsonValue {
    JsonValue::String(v.to_string())
}

fn opt_num(v: Option<u64>) -> JsonValue {
    v.map_or(JsonValue::Null, num)
}

fn swarm_json(n: Option<&crate::wm::swarm_cmd::SwarmNodeInfo>) -> JsonValue {
    n.map_or(JsonValue::Null, |n| {
        JsonValue::obj([
            ("node", s(&n.node)),
            ("state", s(n.state)),
            ("wave", opt_num(n.wave.map(|w| w as u64))),
            ("retries", num(u64::from(n.retries))),
            ("worktree", s(&n.slug)),
        ])
    })
}

/// Snapshot of every HUD tile (state, generation token, rate history, queue depth, preview).
pub fn hud_snapshot(wm: &WindowManager) -> JsonValue {
    let st = wm.agents_hud();
    let hud = st.hud();
    let cards: Vec<JsonValue> = hud
        .cards()
        .map(|c| {
            JsonValue::obj([
                ("pane_id", num(c.pane as u64)),
                ("ref", JsonValue::String(format!("w1:p{}", c.pane))),
                ("title", s(&c.title)),
                ("state", s(c.state.as_str())),
                ("generation", num(c.generation)),
                ("approve_enabled", JsonValue::Bool(c.approve_enabled())),
                ("masked", JsonValue::Bool(c.masked)),
                ("queue_depth", num(c.queue_depth as u64)),
                ("swarm", swarm_json(c.swarm.as_ref())),
                ("badge", c.badge.as_deref().map_or(JsonValue::Null, s)),
                ("tool", c.tool.as_deref().map_or(JsonValue::Null, s)),
                ("file", c.file.as_deref().map_or(JsonValue::Null, s)),
                (
                    "rate_bps",
                    JsonValue::Array(c.spark.samples().map(|v| num(u64::from(v))).collect()),
                ),
                (
                    "preview",
                    JsonValue::Array(if c.masked {
                        Vec::new()
                    } else {
                        c.preview.iter().map(|l| s(l)).collect()
                    }),
                ),
            ])
        })
        .collect();
    JsonValue::obj([
        ("active", JsonValue::Bool(st.active)),
        (
            "selected",
            hud.selected().map_or(JsonValue::Null, |p| num(p as u64)),
        ),
        ("page", num(hud.page() as u64)),
        ("tile_count", num(cards.len() as u64)),
        ("cards", JsonValue::Array(cards)),
    ])
}

fn turn_summary(t: &Turn) -> JsonValue {
    JsonValue::obj([
        ("turn_id", num(t.turn_id)),
        ("prompt", s(&t.prompt)),
        ("start_ts", num(t.start_ts)),
        ("end_ts", opt_num(t.end_ts)),
        ("working_ts", opt_num(t.working_ts)),
        ("lines", num(t.snapshot.len() as u64)),
        (
            "files",
            JsonValue::Array(t.files.iter().map(|f| s(f)).collect()),
        ),
    ])
}

/// The turns recorded for `pane` (summaries; use [`replay_turn`] for the output lines).
pub fn replay_list(wm: &WindowManager, pane: PaneId) -> JsonValue {
    let log = wm.agents_hud().log(pane);
    let turns: Vec<JsonValue> = log
        .map(|l| l.turns().map(turn_summary).collect())
        .unwrap_or_default();
    JsonValue::obj([
        ("pane_id", num(pane as u64)),
        ("in_turn", JsonValue::Bool(log.is_some_and(|l| l.in_turn()))),
        (
            "current",
            opt_num(log.and_then(|l| l.current()).map(|t| t.turn_id)),
        ),
        ("turn_count", num(turns.len() as u64)),
        ("turns", JsonValue::Array(turns)),
    ])
}

/// One recorded turn of `pane` including its output lines.
pub fn replay_turn(wm: &WindowManager, pane: PaneId, turn_id: u64) -> Option<JsonValue> {
    let t = wm
        .agents_hud()
        .log(pane)?
        .turns()
        .find(|t| t.turn_id == turn_id)?;
    let JsonValue::Object(mut map) = turn_summary(t) else {
        return None;
    };
    map.insert("pane_id".to_string(), num(pane as u64));
    map.insert(
        "output".to_string(),
        JsonValue::Array(t.snapshot.iter().map(|l| s(l)).collect()),
    );
    Some(JsonValue::Object(map))
}

/// Watchdog configuration, watched pane count and recent alerts.
pub fn watchdog_status(wm: &WindowManager) -> JsonValue {
    let cfg = &wm.config().agents.watchdog;
    let st = wm.agents_hud();
    let alerts: Vec<JsonValue> = st
        .alerts()
        .map(|a| {
            JsonValue::obj([
                ("pane_id", num(a.pane as u64)),
                ("kind", s(a.kind)),
                ("detail", s(&a.detail)),
                ("count", num(a.count as u64)),
                ("at_secs", num(a.at_secs)),
                ("target", opt_num(a.target.map(|t| t as u64))),
            ])
        })
        .collect();
    JsonValue::obj([
        ("enabled", JsonValue::Bool(cfg.enabled)),
        ("stall_secs", num(cfg.stall_secs)),
        ("loop_window", num(cfg.loop_window as u64)),
        ("loop_repeat", num(cfg.loop_repeat as u64)),
        ("cooldown_secs", num(cfg.cooldown_secs)),
        (
            "alert",
            JsonValue::Array(cfg.alert.iter().map(|a| s(a)).collect()),
        ),
        ("valid", JsonValue::Bool(cfg.validate().is_ok())),
        ("watched", num(st.watchdog().watched() as u64)),
        ("alerts", JsonValue::Array(alerts)),
    ])
}
