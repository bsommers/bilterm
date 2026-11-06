//! Phase 51-03 (UIT-01, D-01): `:agents demo stop` undoes exactly what `:agents demo` created.
//! `primitive_` tests cover the bus and topology removal primitives.

use std::time::{Duration, Instant};

use bilterm::wm::agents_window::NodeKey;
use bilterm::wm::traffic::{TrafficBus, TrafficEndpoint, TrafficKind};

fn bus_with_mixed_events() -> (TrafficBus, Instant) {
    let now = Instant::now();
    let mut bus = TrafficBus::new();
    let pane = TrafficEndpoint::Pane;
    bus.record(TrafficEndpoint::User, pane(1), TrafficKind::Input, "ls", false, now);
    bus.record(pane(1), pane(2), TrafficKind::Message, "[demo] a", false, now);
    bus.record(pane(2), pane(1), TrafficKind::Message, "[demo] b", false, now);
    bus.record(pane(1), pane(3), TrafficKind::Message, "[demo] c", false, now);
    bus.record(TrafficEndpoint::User, pane(2), TrafficKind::Input, "pwd", false, now + Duration::from_secs(1));
    (bus, now)
}

#[test]
fn primitive_retain_events_removes_only_matching() {
    let (mut bus, _) = bus_with_mixed_events();
    assert_eq!(bus.len(), 5);
    let removed = bus.retain_events(|e| e.kind != TrafficKind::Message);
    assert_eq!(removed, 3);
    let left: Vec<&str> = bus.iter().map(|e| e.preview.as_str()).collect();
    assert_eq!(left, vec!["ls", "pwd"], "user events survive in order");
}

#[test]
fn primitive_retain_events_keeps_seq_counters() {
    let (mut bus, now) = bus_with_mixed_events();
    assert_eq!(bus.latest_seq(), 5);
    let dropped = bus.dropped();
    bus.retain_events(|e| e.kind != TrafficKind::Message);
    assert_eq!(bus.latest_seq(), 5, "latest_seq never decrements");
    assert_eq!(bus.dropped(), dropped, "retain is not ring eviction");
    let next = bus.record(TrafficEndpoint::User, TrafficEndpoint::Pane(1), TrafficKind::Input, "x", false, now);
    assert_eq!(next, 6, "sequence numbers are never reused");
}

#[test]
fn primitive_remove_pair_removes_one_pair() {
    let now = Instant::now();
    let mut bus = TrafficBus::new();
    let pane = TrafficEndpoint::Pane;
    bus.record(pane(1), pane(2), TrafficKind::Message, "m", false, now);
    bus.record(pane(2), pane(3), TrafficKind::Message, "m", false, now);
    bus.record(TrafficEndpoint::User, pane(1), TrafficKind::Input, "m", false, now);
    assert_eq!(bus.topology().len(), 3);
    let (a, b) = (NodeKey::pane(2), NodeKey::pane(1));
    let mut topo = bus.topology().clone();
    assert!(topo.remove_pair(&a, &b), "argument order does not matter");
    assert!(!topo.remove_pair(&a, &b), "second removal is a no-op");
    assert_eq!(topo.len(), 2);
    assert!(topo.pair(&NodeKey::pane(2), &NodeKey::pane(3)).is_some());
    assert!(topo.pair(&NodeKey("user".to_string()), &NodeKey::pane(1)).is_some());
}

// ---- behaviour: `:agents demo` / `:agents demo stop` on a real (headless) manager ----

use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::parse_colon_command;
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::layout::Rect;

const DEMO_EVENTS: usize = 13;

fn fixture(cols: u16, rows: u16) -> WindowManager {
    let mut wm = WindowManager::new_headless(rows, cols);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(Rect::new(0, 0, cols, rows)).unwrap();
    wm
}

fn run_colon(wm: &mut WindowManager, cmd: &str) {
    let parsed = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"));
    wm.apply(parsed).unwrap();
}

fn status(wm: &WindowManager) -> String {
    wm.active_pane().status_message().unwrap_or("").to_string()
}

fn demo_events(wm: &WindowManager) -> usize {
    wm.traffic().iter().filter(|e| e.preview.starts_with("[demo] ")).count()
}

fn pane_ids(wm: &WindowManager) -> Vec<usize> {
    wm.panes().keys().copied().collect()
}

#[test]
fn stop_closes_spawned_panes() {
    let mut wm = fixture(120, 40);
    let before = pane_ids(&wm);
    assert_eq!(before.len(), 2);
    run_colon(&mut wm, ":agents demo");
    assert_eq!(wm.pane_count(), 3);
    assert_eq!(wm.agents_demo_summary().spawned.len(), 1);
    run_colon(&mut wm, ":agents demo stop");
    assert_eq!(pane_ids(&wm), before, "only the demo's pane is closed; the original two stay");
    assert!(!wm.agents_demo_summary().running, "state cleared");
    let s = status(&wm);
    assert!(s.starts_with("Demo stopped: closed 1 pane(s), removed 13 [demo] events"), "{s}");
}

#[test]
fn stop_removes_only_demo_events() {
    let mut wm = fixture(120, 40);
    let now = Instant::now();
    wm.record_traffic(TrafficEndpoint::User, TrafficEndpoint::Pane(1), TrafficKind::Input, "before", now);
    run_colon(&mut wm, ":agents demo");
    assert_eq!(demo_events(&wm), DEMO_EVENTS);
    wm.record_traffic(TrafficEndpoint::User, TrafficEndpoint::Pane(2), TrafficKind::Input, "after", now);
    run_colon(&mut wm, ":agents demo stop");
    assert_eq!(demo_events(&wm), 0);
    let previews: Vec<&str> = wm.traffic().iter().map(|e| e.preview.as_str()).collect();
    assert_eq!(previews, vec!["before", "after"], "user Input events survive");
}

#[test]
fn stop_keeps_real_messages_that_look_similar() {
    // A `[demo] ` message that is not between the demo endpoints (or not Message) must survive.
    let mut wm = fixture(120, 40);
    run_colon(&mut wm, ":agents demo");
    let now = Instant::now();
    wm.record_traffic(TrafficEndpoint::User, TrafficEndpoint::Pane(1), TrafficKind::Message, "[demo] from the user", now);
    wm.record_traffic(TrafficEndpoint::Pane(1), TrafficEndpoint::Pane(2), TrafficKind::Input, "[demo] not a message", now);
    wm.record_traffic(TrafficEndpoint::Pane(1), TrafficEndpoint::Pane(2), TrafficKind::Message, "real message", now);
    run_colon(&mut wm, ":agents demo stop");
    assert_eq!(wm.traffic().len(), 3, "predicate needs kind, prefix and both demo endpoints");
}

#[test]
fn stop_purges_topology() {
    let mut wm = fixture(120, 40);
    let now = Instant::now();
    wm.record_traffic(TrafficEndpoint::User, TrafficEndpoint::Pane(1), TrafficKind::Input, "real", now);
    run_colon(&mut wm, ":agents demo");
    let ids = wm.agents_demo_summary().endpoints;
    assert_eq!(ids.len(), 3);
    let k = |i: usize| NodeKey::pane(ids[i]);
    for (a, b) in [(0, 1), (1, 2), (0, 2)] {
        assert!(wm.traffic().topology().pair(&k(a), &k(b)).is_some(), "demo pair {a}-{b} recorded");
    }
    run_colon(&mut wm, ":agents demo stop");
    for (a, b) in [(0, 1), (1, 2), (0, 2)] {
        assert!(wm.traffic().topology().pair(&k(a), &k(b)).is_none(), "demo pair {a}-{b} purged");
    }
    assert!(
        wm.traffic().topology().pair(&NodeKey("user".to_string()), &NodeKey::pane(1)).is_some(),
        "a real pair recorded earlier remains"
    );
}

#[test]
fn stop_without_demo_is_noop() {
    let mut wm = fixture(120, 40);
    let now = Instant::now();
    wm.record_traffic(TrafficEndpoint::User, TrafficEndpoint::Pane(1), TrafficKind::Input, "x", now);
    let (panes, ring) = (wm.pane_count(), wm.traffic().len());
    run_colon(&mut wm, ":agents demo stop");
    assert!(status(&wm).contains("No agent demo"), "{}", status(&wm));
    assert_eq!((wm.pane_count(), wm.traffic().len()), (panes, ring));
}

#[test]
fn demo_twice_replaces() {
    let mut wm = fixture(120, 40);
    run_colon(&mut wm, ":agents demo");
    let first = wm.agents_demo_summary().spawned;
    assert_eq!(first.len(), 1);
    run_colon(&mut wm, ":agents demo");
    assert_eq!(demo_events(&wm), DEMO_EVENTS, "13 stay 13, never 26");
    assert_eq!(wm.pane_count(), 3, "no extra pane on the second run");
    assert_eq!(wm.agents_demo_summary().spawned, first, "spawned is merged, not overwritten by an empty list");
    run_colon(&mut wm, ":agents demo stop");
    assert_eq!(wm.pane_count(), 2, "the first run's pane does not leak");
    assert_eq!(demo_events(&wm), 0);
}

#[test]
fn stop_skips_pane_user_closed() {
    let mut wm = fixture(120, 40);
    run_colon(&mut wm, ":agents demo");
    let spawned = wm.agents_demo_summary().spawned[0];
    assert!(wm.remove_pane(spawned));
    run_colon(&mut wm, ":agents demo stop");
    assert_eq!(wm.pane_count(), 2);
    let s = status(&wm);
    assert!(s.contains("skipped 1 already closed"), "{s}");
    assert!(!wm.agents_demo_summary().running);
}

#[test]
fn stop_last_pane_guard() {
    let mut wm = WindowManager::new_headless(40, 120);
    run_colon(&mut wm, ":agents demo");
    assert_eq!(wm.pane_count(), 3);
    let spawned = wm.agents_demo_summary().spawned;
    assert_eq!(spawned.len(), 2, "one-pane start spawns two");
    let keep = *spawned.last().unwrap();
    for id in pane_ids(&wm) {
        if id != keep {
            assert!(wm.remove_pane(id));
        }
    }
    assert_eq!(wm.pane_count(), 1);
    run_colon(&mut wm, ":agents demo stop");
    assert_eq!(pane_ids(&wm), vec![keep], "remove_pane refuses the last pane");
    let s = status(&wm);
    assert!(s.contains(&format!("kept pane {keep} (last pane)")), "{s}");
}

#[test]
fn demo_unredacted_with_password_endpoint() {
    let mut wm = fixture(120, 40);
    let pane = wm.pane_mut(2).expect("pane");
    pane.process_pty_output(b"[sudo] password for operator: ");
    pane.line_discipline_mut().echo_enabled = false;
    assert!(wm.pane(2).unwrap().is_password_prompt(), "fixture must be in the password state");
    run_colon(&mut wm, ":agents demo");
    assert_eq!(demo_events(&wm), DEMO_EVENTS, "prefix survives although an endpoint is in a password prompt");
    assert!(wm.traffic().iter().all(|e| !e.redacted));
    run_colon(&mut wm, ":agents demo stop");
    assert_eq!(demo_events(&wm), 0);
    assert!(wm.traffic().is_empty(), "stop removed every demo event");
}

#[test]
fn stop_resets_stale_ui_refs() {
    let mut wm = fixture(120, 40);
    run_colon(&mut wm, ":agents demo");
    let spawned = wm.agents_demo_summary().spawned[0];
    let gone = NodeKey::pane(spawned);
    let keep = NodeKey::pane(2);
    // A snoop view scrolled back onto a demo line, filtered on the doomed pane and on a pair with it.
    wm.apply_agent_filter(&gone);
    wm.apply_pair_filter(&gone, &keep);
    assert!(wm.snoop_lines().len() > 1);
    wm.set_snoop_follow(Some(false));
    let first = wm.snoop_lines()[0].seq;
    assert!(wm.select_snoop_seq(first));
    wm.agents_window_mut().selection.node = Some(gone.clone());
    run_colon(&mut wm, ":agents demo stop");
    let w = wm.agents_window();
    assert_eq!(w.selection, Default::default(), "selection pointing at a removed node is reset");
    assert!(w.snoop.filter_agent.is_none() && w.snoop.filter_pair.is_none());
    assert!(wm.snoop_lines().is_empty());
    assert_eq!(wm.snoop_selected_seq(), None, "no panic; falls back to the newest line, of which there is none");
}

#[test]
fn stop_keeps_ui_refs_to_surviving_nodes() {
    let mut wm = fixture(120, 40);
    run_colon(&mut wm, ":agents demo");
    let keep = NodeKey::pane(1);
    wm.apply_agent_filter(&keep);
    wm.agents_window_mut().selection.node = Some(keep.clone());
    run_colon(&mut wm, ":agents demo stop");
    assert_eq!(wm.agents_window().selection.node, Some(keep.clone()));
    assert_eq!(wm.agents_window().snoop.filter_agent, Some(keep));
}

fn rpc_command(plane: &mut AutomationControlPlane, cmd: &str) -> JsonValue {
    let params = JsonValue::obj([("command", JsonValue::String(cmd.to_string()))]);
    plane.dispatch_method("wm.command", &params).unwrap_or_else(|e| panic!("{cmd}: {e}"))
}

#[test]
fn demo_stop_over_rpc() {
    let mut plane = AutomationControlPlane::new_headless(40, 120);
    plane.wm.split_focused(SplitDirection::Vertical).unwrap();
    let before = plane.wm.pane_count();
    let r = rpc_command(&mut plane, ":agents demo");
    assert_eq!(r.get("status").and_then(JsonValue::as_str), Some("handled"));
    assert_eq!(plane.wm.pane_count(), before + 1);
    let r = rpc_command(&mut plane, ":agents demo stop");
    assert_eq!(r.get("status").and_then(JsonValue::as_str), Some("handled"), "no new gate (D-01)");
    assert_eq!(plane.wm.pane_count(), before);
    assert_eq!(demo_events(&plane.wm), 0);
}

#[test]
fn session_reset_clears_demo() {
    let mut wm = fixture(120, 40);
    run_colon(&mut wm, ":agents demo");
    assert!(wm.agents_demo_summary().running);
    run_colon(&mut wm, ":session new reset-a");
    assert!(!wm.agents_demo_summary().running, "new session forgets the demo");
    let panes = wm.pane_count();
    run_colon(&mut wm, ":agents demo stop");
    assert_eq!(wm.pane_count(), panes, "stale ids are never closed");
    assert!(status(&wm).contains("No agent demo"), "{}", status(&wm));
}
