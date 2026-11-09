//! Phase 64-03 (TMON-04, UNI-R01): every surface reaches the same `Action::TrafficMonitor` / state.
//!
//! Recorded exceptions: `:traffic reset|enable|disable|classify|oia` are refused for API-origin
//! callers unless `[api] allow_config_write = true`; geometry verbs have no palette row.

use bilterm::action::Action;
use bilterm::api::{AutomationControlPlane, JsonValue, RPC_METHODS};
use bilterm::colon_command::parse_colon_command;
use bilterm::input::handle_wm_key_event;
use bilterm::input::keymap::default_global_action;
use bilterm::input::traffic_monitor::plan_traffic_monitor_mouse;
use bilterm::input::traffic_monitor_keys::{TRAFFIC_MONITOR_BINDINGS, traffic_monitor_action};
use bilterm::traffic::{GroupKey, Origin, Role, SortKey};
use bilterm::wm::WindowManager;
use bilterm::wm::traffic_monitor::TrafficMonitorOp as Op;
use bilterm::wm::traffic_monitor_geometry::TrafficLayout;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

fn feed(wm: &mut WindowManager) {
    let m = wm.traffic_meter();
    wm.traffic_tick(0);
    let a = m.open_channel(Origin::LocalPty, "local", Some(1), Role::Both);
    let b = m.open_channel(Origin::Ssh { host: "prod-db".into(), port: 22 }, "prod-db", Some(2), Role::Both);
    let c = m.open_channel(Origin::Ws { client: "web".into() }, "web", None, Role::Wire);
    for s in 1..=4u64 {
        a.rx(1000);
        b.rx(20_000);
        b.tx(300);
        c.tx(5000);
        wm.traffic_tick(s * 1000);
    }
}

fn act(op: Op) -> Action {
    Action::TrafficMonitor(op)
}

fn colon(s: &str) -> Action {
    parse_colon_command(s).unwrap().unwrap()
}

fn key(wm: &mut WindowManager, code: KeyCode, m: KeyModifiers) {
    handle_wm_key_event(wm, KeyEvent::new(code, m)).unwrap();
}

#[test]
fn colon_forms_lower_to_actions() {
    for (s, op) in [
        (":traffic", Op::Toggle),
        (":traffic open", Op::Open),
        (":traffic group pane", Op::GroupBy(GroupKey::Pane)),
        (":traffic sort total desc", Op::Sort(SortKey::Total, Some(true))),
        (":traffic filter ssh", Op::Filter("ssh".into())),
        (":traffic pause", Op::Pause(Some(true))),
        (":traffic scale 150", Op::Scale(150)),
        (":netmon dock", Op::Dock(Some(true))),
    ] {
        assert_eq!(colon(s), act(op), "{s}");
    }
    assert!(parse_colon_command(":traffic sort").unwrap().is_err());
}

#[test]
fn toggle_chords_command_prompt_raw_passthrough_and_leader() {
    let ca = KeyModifiers::CONTROL | KeyModifiers::ALT;
    assert_eq!(default_global_action(KeyEvent::new(KeyCode::Char('t'), ca), false), Some(act(Op::Toggle)));
    let mut wm = WindowManager::new_headless(40, 120);
    key(&mut wm, KeyCode::Char('t'), ca);
    assert!(wm.traffic_monitor().active());
    key(&mut wm, KeyCode::Char('t'), ca);
    assert!(!wm.traffic_monitor().active());
    wm.active_pane_mut().toggle_manual_raw_override();
    assert_eq!(
        wm.active_pane().effective_routing_mode(),
        bilterm::pty::line_discipline::InputRoutingMode::RawPtyPassthrough
    );
    key(&mut wm, KeyCode::Char('t'), ca);
    assert!(wm.traffic_monitor().active(), "Ctrl+Alt+T works in raw passthrough");
    key(&mut wm, KeyCode::Char('t'), ca);
    assert!(!wm.traffic_monitor().active());
    let mut wm = WindowManager::new_headless(40, 120);
    key(&mut wm, KeyCode::Char('x'), KeyModifiers::CONTROL);
    key(&mut wm, KeyCode::Char('$'), KeyModifiers::NONE);
    assert!(wm.traffic_monitor().active(), "leader $ toggles");
}

#[test]
fn in_window_table_keys_drive_state() {
    let mut wm = WindowManager::new_headless(40, 120);
    feed(&mut wm);
    key(&mut wm, KeyCode::Char('t'), KeyModifiers::CONTROL | KeyModifiers::ALT);
    let n = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
    assert_eq!(traffic_monitor_action(n('p'), false), Some(act(Op::Pause(None))));
    key(&mut wm, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(wm.traffic_monitor().group, GroupKey::Session);
    key(&mut wm, KeyCode::BackTab, KeyModifiers::NONE);
    assert_eq!(wm.traffic_monitor().group, GroupKey::Connection);
    key(&mut wm, KeyCode::Char('3'), KeyModifiers::NONE);
    assert_eq!(wm.traffic_monitor().sort.key, SortKey::Rate1m);
    key(&mut wm, KeyCode::Char('p'), KeyModifiers::NONE);
    assert!(wm.traffic_monitor().paused.is_some());
    key(&mut wm, KeyCode::Char('/'), KeyModifiers::NONE);
    assert!(wm.traffic_monitor().filter_focused);
    key(&mut wm, KeyCode::Char('s'), KeyModifiers::NONE);
    key(&mut wm, KeyCode::Char('s'), KeyModifiers::NONE);
    assert_eq!(wm.traffic_monitor().filter.text, "ss");
    key(&mut wm, KeyCode::Backspace, KeyModifiers::NONE);
    assert_eq!(wm.traffic_monitor().filter.text, "s");
    key(&mut wm, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!wm.traffic_monitor().filter_focused);
    key(&mut wm, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!wm.traffic_monitor().active());
    for (_, _, op) in TRAFFIC_MONITOR_BINDINGS {
        let parsed = parse_colon_command(&op.colon_form()).unwrap().unwrap();
        assert_eq!(parsed, act(op.clone()), "table entry must round-trip through its colon form");
    }
}

fn click(col: u16, row: u16) -> MouseEvent {
    MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: col, row, modifiers: KeyModifiers::NONE }
}

#[test]
fn mouse_plans_lower_to_actions() {
    let mut wm = WindowManager::new_headless(40, 120);
    feed(&mut wm);
    wm.apply(act(Op::Open)).unwrap();
    let ws = wm.dock_base_area();
    let l = TrafficLayout::compute(wm.traffic_monitor_rect(ws).unwrap());
    let plan = |m| plan_traffic_monitor_mouse(&wm, m, ws).unwrap();
    let close = l.badges[2].1;
    assert_eq!(plan(click(close + 1, l.outer.y)).actions, vec![act(Op::Close)]);
    assert_eq!(plan(click(l.badges[0].1 + 1, l.outer.y)).actions, vec![act(Op::Pin(None))]);
    let (g, x, _) = l.tabs[3];
    assert_eq!(plan(click(x + 1, l.tabs_y)).actions, vec![act(Op::GroupBy(g))]);
    let (_, x, w) = l.columns.iter().copied().find(|(c, _, _)| c.sort_key() == Some(SortKey::Total)).unwrap();
    assert_eq!(plan(click(x + w - 1, l.colhead_y)).actions, vec![act(Op::Sort(SortKey::Total, None))]);
    let (loc, x, _) = l.chips[2];
    assert_eq!(plan(click(x, l.filter_y)).actions, vec![act(Op::Locality(loc))]);
    assert_eq!(plan(click(l.inner.x + 1, l.rows_y + 1)).actions, vec![act(Op::SelectRow(1))]);
    let wheel = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: l.inner.x + 1,
        row: l.rows_y,
        modifiers: KeyModifiers::NONE,
    };
    assert_eq!(plan(wheel).actions, vec![act(Op::Scroll(3))]);
    let title = plan(click(l.outer.x + 5, l.outer.y));
    assert!(title.drag.is_some() && title.swallow);
    let edge = plan(click(l.outer.x, l.outer.y + 5));
    assert!(matches!(edge.drag, Some(Some(_))));
    assert!(!plan(click(0, 0)).swallow);
    wm.apply(act(Op::SelectRow(0))).unwrap();
    assert_eq!(
        plan_traffic_monitor_mouse(&wm, click(l.inner.x + 1, l.rows_y), ws).unwrap().actions,
        vec![act(Op::Drill)]
    );
}

#[test]
fn palette_menu_completion_help_and_registry() {
    use bilterm::ui::modals::{HELP_LINES, PALETTE_CATALOG};
    for cmd in [":traffic toggle", ":traffic group connection", ":traffic pause", ":traffic reset"] {
        assert!(PALETTE_CATALOG.iter().any(|e| e.0 == cmd), "{cmd}");
    }
    let cwd = std::path::Path::new(".");
    let (_, _, c) = bilterm::pane::completion::compute_completions(":traff", 6, cwd);
    assert!(!c.is_empty());
    let (_, _, c) = bilterm::pane::completion::compute_completions(":traffic ", 9, cwd);
    assert!(["group", "pause", "reset"].iter().all(|v| c.iter().any(|x| x.display == *v)));
    assert!(HELP_LINES.iter().any(|l| l.0.starts_with(":traffic") && l.1.contains("Agent Traffic")));
    assert_eq!(bilterm::command_registry::lookup("netmon").unwrap().alias_of, Some(":traffic"));
    assert_eq!(colon(":traffic toggle"), act(Op::Toggle));
}

fn plane() -> AutomationControlPlane {
    let mut p = AutomationControlPlane::new_headless(24, 80);
    feed(&mut p.wm);
    p
}

fn call(p: &mut AutomationControlPlane, m: &str, params: &str) -> Result<JsonValue, String> {
    p.dispatch_method(m, &JsonValue::parse(params).unwrap())
}

#[test]
fn rpc_snapshot_history_validation_and_parity_with_colon_show() {
    let mut p = plane();
    for m in ["traffic.snapshot", "traffic.history", "traffic.window", "traffic.reset", "traffic.config"] {
        assert!(RPC_METHODS.contains(&m));
    }
    assert_eq!(RPC_METHODS.iter().filter(|m| m.starts_with("traffic.")).count(), 5);
    let snap = call(&mut p, "traffic.snapshot", r#"{"group_by":"origin","sort":"total","limit":9999}"#).unwrap();
    assert_eq!(snap.get("limit").and_then(JsonValue::as_usize), Some(500));
    assert!(snap.get("rows").and_then(JsonValue::as_array).unwrap().len() >= 2);
    assert!(snap.get("totals").and_then(|t| t.get("rx")).is_some());
    let err = call(&mut p, "traffic.snapshot", r#"{"group_by":"nope"}"#).unwrap_err();
    assert!(err.contains("connection|session"), "{err}");
    assert!(call(&mut p, "traffic.snapshot", r#"{"sort":"bogus"}"#).unwrap_err().contains("rate|rate10"));
    let long = "x".repeat(200);
    assert!(call(&mut p, "traffic.snapshot", &format!(r#"{{"filter":"{long}"}}"#)).is_ok());
    let first_key = snap.get("rows").and_then(JsonValue::as_array).unwrap()[0]
        .get("key")
        .and_then(JsonValue::as_str)
        .unwrap()
        .to_string();
    let h = call(&mut p, "traffic.history", &format!(r#"{{"group_by":"origin","key":"{first_key}","n":5}}"#)).unwrap();
    assert_eq!(h.get("series_rx").and_then(JsonValue::as_array).unwrap().len(), 5);
    p.wm.apply(act(Op::GroupBy(GroupKey::Origin))).unwrap();
    p.wm.apply(act(Op::Show(None))).unwrap();
    let colon_json = p.wm.take_command_data().unwrap();
    let rpc = call(&mut p, "traffic.snapshot", r#"{"group_by":"origin","limit":500}"#).unwrap();
    assert_eq!(JsonValue::parse(&colon_json).unwrap().to_json_string(), rpc.to_json_string());
    assert_eq!(JsonValue::parse(&colon_json).unwrap(), rpc);
}

#[test]
fn rpc_window_and_write_gate() {
    let mut p = plane();
    let w = call(&mut p, "traffic.window", r#"{"action":"open"}"#).unwrap();
    assert_eq!(w.get("open").and_then(JsonValue::as_bool), Some(true));
    let w = call(&mut p, "traffic.window", r#"{"action":"group","value":"pane"}"#).unwrap();
    assert_eq!(w.get("group_by").and_then(JsonValue::as_str), Some("pane"));
    let w = call(&mut p, "traffic.window", r#"{"action":"pause"}"#).unwrap();
    assert_eq!(w.get("paused").and_then(JsonValue::as_bool), Some(true));
    assert!(call(&mut p, "traffic.window", r#"{"action":"bogus"}"#).is_err());
    let e = call(&mut p, "traffic.reset", "{}").unwrap_err();
    assert!(e.contains("allow_config_write"), "{e}");
    let e = call(&mut p, "traffic.config", r#"{"set":{"classify":true}}"#).unwrap_err();
    assert!(e.contains("allow_config_write"), "{e}");
    let cfg = call(&mut p, "traffic.config", "{}").unwrap();
    assert!(cfg.get("enabled").is_some());
    assert!(call(&mut p, "traffic.config", r#"{"set":{"api":true}}"#).is_err());
    p.wm.config_mut().api.allow_config_write = true;
    assert!(call(&mut p, "traffic.reset", "{}").is_ok());
    let cfg = call(&mut p, "traffic.config", r#"{"set":{"oia_indicator":true}}"#).unwrap();
    assert_eq!(cfg.get("oia_indicator").and_then(JsonValue::as_bool), Some(true));
    let mut local = WindowManager::new_headless(24, 80);
    local.apply(act(Op::Reset)).unwrap();
}

#[test]
fn mcp_tool_listed_and_matches_rpc() {
    let mut p = plane();
    let list = p.handle_mcp_line(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap();
    assert!(list.contains("bilterm_traffic"));
    let args = r#"{"action":"snapshot","group_by":"locality"}"#;
    let line = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"bilterm_traffic","arguments":{args}}}}}"#
    );
    let resp = JsonValue::parse(&p.handle_mcp_line(&line).unwrap()).unwrap();
    let text = resp.get("result").and_then(|r| r.get("content")).and_then(JsonValue::as_array).unwrap()[0]
        .get("text")
        .and_then(JsonValue::as_str)
        .unwrap()
        .to_string();
    let rpc = call(&mut p, "traffic.snapshot", r#"{"group_by":"locality"}"#).unwrap();
    assert_eq!(text, rpc.to_json_string());
    let bad = p
        .handle_mcp_line(
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"bilterm_traffic","arguments":{"action":"zzz"}}}"#,
        )
        .unwrap();
    assert!(bad.contains("snapshot, history"));
}

#[test]
fn doc_gate_script_passes() {
    let out = std::process::Command::new("bash").arg("scripts/check_phase64_doc_coverage.sh").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
}
