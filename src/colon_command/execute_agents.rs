//! Executor for `:agents` (Phase 50-03): structured per-pane agent state via `set_command_data`.
//!
//! Data carries pane id, sanitised identity token, state, timing and profile name only; never
//! screen text or command arguments (T-50-06).

use std::time::{Duration, Instant};

use crate::api::JsonValue;
use crate::wm::WindowManager;
use crate::wm::agents_cmd::{AgentsOp, SnoopFilterOp};
use crate::wm::agents_window::{ComposeGate, WindowPane, key_label};

/// Window over which `:agents list` reports a pane's recent communication partners.
const PARTNER_WINDOW: Duration = Duration::from_secs(60);

/// Executes one `:agents` operation; always returns `false` (never quits).
pub(super) fn execute_agents_command(wm: &mut WindowManager, op: AgentsOp) -> bool {
    match op {
        AgentsOp::List => {
            wm.open_agents_list();
            list(wm);
        }
        AgentsOp::Zoom(focus) => zoom(wm, focus),
        AgentsOp::Close => wm.close_agents_window(),
        AgentsOp::Toggle(focus) => {
            if wm.agents_window().active {
                wm.close_agents_window();
            } else {
                zoom(wm, focus);
            }
        }
        AgentsOp::Demo => super::execute_agents_topology::demo(wm),
        AgentsOp::DemoStop => super::execute_agents_topology::demo_stop(wm),
        AgentsOp::Dismiss => dismiss(wm),
        AgentsOp::Blur => wm.agents_window_mut().blur(),
        AgentsOp::Fade(secs) => super::execute_agents_topology::fade(wm, secs),
        AgentsOp::Topology(op) => super::execute_agents_topology::topology(wm, op),
        AgentsOp::Select(dir) => {
            wm.agents_select_direction(dir);
        }
        AgentsOp::CycleEdge(forward) => {
            wm.agents_cycle_edge(forward);
        }
        AgentsOp::Activate => activate(wm),
        AgentsOp::FocusPane(pane) => wm.focus_agents_pane(pane),
        AgentsOp::SnoopStep(n) => {
            if !wm.step_snoop_selection(n) {
                status(wm, "No conversation lines".to_string());
            }
        }
        AgentsOp::SnoopSelect(seq) => {
            if !wm.select_snoop_seq(seq) {
                status(wm, format!("No conversation line {seq}"));
            }
        }
        AgentsOp::Copy => match wm.copy_selected_snoop() {
            Some(_) => status(wm, "Copied the selected conversation line".to_string()),
            None => status(wm, "Nothing to copy".to_string()),
        },
        // No text, no pane write, no bus event: the gate is closed in Phase 50 (D-09).
        AgentsOp::ComposeSubmit => match wm.compose_gate() {
            ComposeGate::Unavailable(_) => status(wm, "Agent messaging is not enabled (Phase 52)".to_string()),
        },
        AgentsOp::SnoopScroll(n) => wm.scroll_snoop(n),
        AgentsOp::Follow(mode) => wm.set_snoop_follow(mode),
        AgentsOp::KeywordEdit(edit) => wm.edit_snoop_keyword(edit),
        AgentsOp::Filter(f) => filter(wm, f),
        other => execute_window_op(wm, other),
    }
    false
}

/// Conversation filter operations.
fn filter(wm: &mut WindowManager, op: SnoopFilterOp) {
    match op {
        SnoopFilterOp::Clear => wm.clear_snoop_filters(),
        SnoopFilterOp::Remove(kind) => wm.remove_snoop_filter(kind),
        SnoopFilterOp::Text(t) => wm.apply_text_filter(&t),
        SnoopFilterOp::AgentFromSelection => match wm.snoop_selected_endpoints() {
            Some((from, _)) => wm.apply_agent_filter(&from),
            None => status(wm, "No conversation line selected".to_string()),
        },
        SnoopFilterOp::PairFromSelection => match wm.snoop_selected_endpoints() {
            Some((from, to)) if from != to => wm.apply_pair_filter(&from, &to),
            _ => status(wm, "No conversation line selected".to_string()),
        },
        SnoopFilterOp::Agent(raw) => match wm.resolve_snoop_node(&raw) {
            Some(key) => wm.apply_agent_filter(&key),
            None => status(wm, format!("No node {raw}")),
        },
        SnoopFilterOp::Pair(a, b) => match (wm.resolve_snoop_node(&a), wm.resolve_snoop_node(&b)) {
            (Some(a), Some(b)) if a != b => wm.apply_pair_filter(&a, &b),
            (Some(_), Some(_)) => status(wm, "A pair needs two different nodes".to_string()),
            (None, _) => status(wm, format!("No node {a}")),
            (_, None) => status(wm, format!("No node {b}")),
        },
    }
}

/// Window geometry, splitter and node operations (Phase 50-09).
fn execute_window_op(wm: &mut WindowManager, op: AgentsOp) {
    use ratatui::layout::Rect;
    match op {
        AgentsOp::Move { dx, dy } => wm.move_agents_window(dx, dy),
        AgentsOp::Resize { dw, dh } => wm.resize_agents_window(dw, dh),
        AgentsOp::SetRect { x, y, w, h } => {
            wm.set_agents_rect(Rect::new(x, y, w, h));
        }
        AgentsOp::Size(size) => wm.set_agents_size(size),
        AgentsOp::Maximize => wm.toggle_agents_maximize(),
        AgentsOp::Split(pct) => wm.set_agents_splitter_pct(pct),
        AgentsOp::SplitStep(n) => {
            let pct = (i32::from(wm.agents_window().splitter_pct) + i32::from(n))
                .clamp(0, i32::from(u16::MAX)) as u16;
            wm.set_agents_splitter_pct(pct);
        }
        AgentsOp::SelectNode(key) => {
            if !wm.agents_select_node(&key) {
                status(wm, format!("No node {}", key.as_str()));
            }
        }
        AgentsOp::SelectEdge(a, b) => {
            if !wm.agents_select_edge(&a, &b) {
                status(wm, format!("No edge {} - {}", a.as_str(), b.as_str()));
            }
        }
        AgentsOp::Grab(mode) => wm.set_agents_grab(mode),
        AgentsOp::NudgeNode(dir) => {
            if !wm.nudge_agents_node(dir) {
                status(wm, "No node selected".to_string());
            }
        }
        AgentsOp::PinNode { key, x_pm, y_pm } => {
            if !wm.pin_agents_node(&key, x_pm, y_pm) {
                status(wm, format!("No node {}", key.as_str()));
            }
        }
        AgentsOp::UnpinNode(key) => {
            let Some(key) = key.or_else(|| wm.agents_window().selection.node.clone()) else {
                return status(wm, "No node selected".to_string());
            };
            if !wm.unpin_agents_node(&key) {
                status(wm, format!("{} is not pinned", key.as_str()));
            }
        }
        AgentsOp::Pin(v) => {
            let pin = v.unwrap_or(!wm.agents_window().pinned);
            wm.agents_window_mut().pinned = pin;
            let msg = if pin {
                "Agents window pinned (Esc returns focus to panes; :agents unpin unpins)"
            } else {
                "Agents window unpinned"
            };
            status(wm, msg.to_string());
        }
        AgentsOp::ScaleStep(delta) => {
            let n = wm.step_scale(crate::ui::typography::PanelTarget::Agents, delta);
            let note = if wm.gui_surface { "" } else { " (applies in the GUI only)" };
            status(wm, format!("Panel scale: {n}%{note}"));
        }
        // Handled by the caller.
        AgentsOp::List
        | AgentsOp::Zoom(_)
        | AgentsOp::Toggle(_)
        | AgentsOp::Demo
        | AgentsOp::DemoStop
        | AgentsOp::Close
        | AgentsOp::Dismiss
        | AgentsOp::Blur
        | AgentsOp::Fade(_)
        | AgentsOp::Topology(_)
        | AgentsOp::Select(_)
        | AgentsOp::CycleEdge(_)
        | AgentsOp::Activate
        | AgentsOp::FocusPane(_)
        | AgentsOp::SnoopStep(_)
        | AgentsOp::SnoopSelect(_)
        | AgentsOp::Copy
        | AgentsOp::SnoopScroll(_)
        | AgentsOp::Follow(_)
        | AgentsOp::Filter(_)
        | AgentsOp::KeywordEdit(_)
        | AgentsOp::ComposeSubmit => {}
    }
}

fn status(wm: &mut WindowManager, msg: String) {
    wm.active_pane_mut().set_status_message(Some(msg));
}

fn zoom(wm: &mut WindowManager, focus: Option<crate::pane::PaneId>) {
    wm.open_agents_window(focus);
    if let Some(id) = focus
        && wm.pane(id).is_none()
    {
        wm.active_pane_mut().set_status_message(Some(format!("No pane #{id}")));
    }
}

/// Esc: close an unpinned window; a pinned window only releases the keyboard.
fn dismiss(wm: &mut WindowManager) {
    if wm.agents_window().pinned {
        let state = wm.agents_window_mut();
        state.focused = false;
        state.grab_mode = false;
        state.drag = None;
    } else {
        wm.close_agents_window();
    }
}

/// Enter: focus the selected live pane node, then close (unpinned) or release the keyboard (pinned).
fn activate(wm: &mut WindowManager) {
    let sel = wm.agents_window().selection.clone();
    // A selected edge opens its conversation: filter to the pair and move to the conversation pane.
    if let Some((a, b)) = &sel.edge {
        wm.apply_pair_filter(a, b);
        wm.focus_agents_pane(Some(WindowPane::Snoop));
        status(wm, format!("Conversation filtered to {} - {}", key_label(a), key_label(b)));
        return;
    }
    let pane = match (&sel.edge, &sel.node) {
        (None, Some(node)) => node.pane_id().filter(|id| wm.pane(*id).is_some()),
        _ => None,
    };
    let Some(id) = pane else {
        let what = match (&sel.edge, &sel.node) {
            (Some((a, b)), _) => format!("{} - {}", a.as_str(), b.as_str()),
            (None, Some(node)) => node.as_str().to_string(),
            (None, None) => "nothing selected".to_string(),
        };
        wm.active_pane_mut().set_status_message(Some(format!("Nothing to activate for {what}")));
        return;
    };
    wm.focus_and_raise(id);
    dismiss(wm);
}

fn list(wm: &mut WindowManager) {
    if !wm.agents.has_updated() {
        wm.update_agent_states(Instant::now());
    }
    let now = Instant::now();
    let statuses = wm.agent_statuses();
    // One visible line per pane for the status bar; the full structure goes over RPC below.
    let per_pane: Vec<String> = statuses
        .iter()
        .map(|s| format!("#{} {} {}", s.pane_id, s.name, s.state.as_str()))
        .collect();
    let agents: Vec<JsonValue> = statuses
        .into_iter()
        .map(|s| {
            let partners: Vec<JsonValue> = wm
                .partners_of(s.pane_id, PARTNER_WINDOW, now)
                .into_iter()
                .map(|p| {
                    JsonValue::obj([
                        ("label", JsonValue::String(p.endpoint.label())),
                        ("count", JsonValue::Number(p.count as i64)),
                    ])
                })
                .collect();
            JsonValue::obj([
                ("pane_id", JsonValue::Number(s.pane_id as i64)),
                ("name", JsonValue::String(s.name)),
                ("state", JsonValue::String(s.state.as_str().to_string())),
                ("since_ms", JsonValue::Number(s.since_ms as i64)),
                ("profile", s.profile.map_or(JsonValue::Null, JsonValue::String)),
                ("reported", JsonValue::Bool(s.reported)),
                ("partners", JsonValue::Array(partners)),
            ])
        })
        .collect();
    let c = wm.agent_counts();
    let counts = JsonValue::obj([
        ("working", JsonValue::Number(c.working as i64)),
        ("blocked", JsonValue::Number(c.blocked as i64)),
        ("idle", JsonValue::Number(c.idle as i64)),
        ("done", JsonValue::Number(c.done as i64)),
        ("errored", JsonValue::Number(c.errored as i64)),
    ]);
    let bus = wm.traffic();
    let traffic = JsonValue::obj([
        ("latest_seq", JsonValue::Number(bus.latest_seq() as i64)),
        ("len", JsonValue::Number(bus.len() as i64)),
        ("dropped", JsonValue::Number(bus.dropped() as i64)),
    ]);
    let data = JsonValue::obj([
        ("agents", JsonValue::Array(agents)),
        ("counts", counts),
        ("traffic", traffic),
        ("agents_list", super::execute_agents_list::summary_json(wm)),
    ]);
    wm.set_command_data(data.to_json_string());
    let mut msg = format!(
        "Agents: {} working · {} blocked · {} idle · {} done · {} errored",
        c.working, c.blocked, c.idle, c.done, c.errored
    );
    if !per_pane.is_empty() {
        msg.push_str(" | ");
        msg.push_str(&per_pane.join(" · "));
    }
    wm.active_pane_mut().set_status_message(Some(msg));
}
