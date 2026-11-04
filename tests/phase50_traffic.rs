//! Phase 50-05: agent traffic bus (FLT-02, D-01). `bus_` tests use synthetic `Instant`s only.

use std::time::{Duration, Instant};

use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::parse_colon_command;
use bilterm::wm::WindowManager;
use bilterm::wm::SplitDirection;
use bilterm::wm::traffic::{PREVIEW_CAP, TRAFFIC_CAP, TrafficBus, TrafficEndpoint, TrafficKind};

fn run(wm: &mut WindowManager, cmd: &str) {
    let action = parse_colon_command(cmd).expect("colon").expect("parse");
    let _ = wm.apply(action);
}

fn data(wm: &mut WindowManager) -> JsonValue {
    JsonValue::parse(&wm.take_command_data().expect("command data")).expect("json")
}

fn two_pane_wm() -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.split_focused(SplitDirection::Vertical).expect("split");
    wm
}

fn partner_labels(d: &JsonValue, pane_id: usize) -> Vec<String> {
    let JsonValue::Array(items) = d.get("agents").expect("agents") else { panic!("agents not array") };
    let entry = items
        .iter()
        .find(|a| a.get("pane_id").and_then(JsonValue::as_usize) == Some(pane_id))
        .unwrap_or_else(|| panic!("no agent entry for pane {pane_id}: {d:?}"));
    let JsonValue::Array(ps) = entry.get("partners").expect("partners") else { panic!("partners not array") };
    ps.iter()
        .filter_map(|p| p.get("label").and_then(JsonValue::as_str).map(str::to_string))
        .collect()
}

#[test]
fn bus_tracer_broadcast_records_and_lists_partner() {
    let mut wm = two_pane_wm();
    let delivered = wm.broadcast_command("echo hi").expect("broadcast");
    assert_eq!(delivered, 2);
    assert_eq!(wm.traffic().len(), 2);
    for e in wm.traffic().iter() {
        assert_eq!(e.from, TrafficEndpoint::User);
        assert!(matches!(e.to, TrafficEndpoint::Pane(_)));
        assert_eq!(e.kind, TrafficKind::Broadcast);
        assert_eq!(e.preview, "echo hi");
        assert!(!e.redacted);
    }
    run(&mut wm, ":agents list");
    let d = data(&mut wm);
    assert!(partner_labels(&d, 1).contains(&"you".to_string()), "partners: {d:?}");
    assert_eq!(d.get("traffic").and_then(|t| t.get("len")).and_then(JsonValue::as_usize), Some(2));
}

fn enter_password_prompt(wm: &mut WindowManager, id: usize) {
    let pane = wm.pane_mut(id).expect("pane");
    pane.process_pty_output(b"[sudo] password for operator: ");
    pane.line_discipline_mut().echo_enabled = false;
    assert!(pane.is_password_prompt(), "fixture must enter the password prompt state");
}

fn rpc_ok(plane: &mut AutomationControlPlane, method: &str, params: &str) {
    let line = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{params}}}"#);
    let v = JsonValue::parse(&plane.handle_jsonrpc_line(&line)).expect("json");
    assert!(v.get("result").is_some(), "{method} failed: {v:?}");
}

#[test]
fn bus_ring_cap_evicts_oldest() {
    let mut bus = TrafficBus::new();
    let t = Instant::now();
    for i in 0..600 {
        bus.record(TrafficEndpoint::User, TrafficEndpoint::Pane(1), TrafficKind::Input, &format!("c{i}"), false, t);
    }
    assert_eq!(bus.len(), TRAFFIC_CAP);
    assert_eq!(bus.len(), 512);
    assert_eq!(bus.iter().next().map(|e| e.seq), Some(89));
    assert_eq!(bus.dropped(), 88);
    assert_eq!(bus.latest_seq(), 600);
    let seqs: Vec<u64> = bus.iter().map(|e| e.seq).collect();
    assert!(seqs.windows(2).all(|w| w[0] < w[1]), "seq must be strictly increasing");
}

#[test]
fn bus_events_since_cursor() {
    let mut bus = TrafficBus::new();
    let t = Instant::now();
    for _ in 0..20 {
        bus.record(TrafficEndpoint::Api, TrafficEndpoint::Pane(1), TrafficKind::Exec, "x", false, t);
    }
    let got: Vec<u64> = bus.events_since(10).map(|e| e.seq).collect();
    assert_eq!(got, (11..=20).collect::<Vec<u64>>());
    assert_eq!(bus.events_since(20).count(), 0);
}

#[test]
fn bus_preview_sanitized_and_capped() {
    let mut bus = TrafficBus::new();
    let raw = format!("a\u{202E}b\u{1b}c\u{0}d{}", "x".repeat(1000));
    bus.record(TrafficEndpoint::User, TrafficEndpoint::Pane(1), TrafficKind::Input, &raw, false, Instant::now());
    let e = bus.iter().next().unwrap();
    assert!(!e.preview.contains('\u{202E}') && !e.preview.contains('\u{1b}') && !e.preview.contains('\u{0}'));
    assert!(e.preview.starts_with("abcd"));
    assert!(e.preview.chars().count() <= PREVIEW_CAP, "len {}", e.preview.chars().count());
    assert_eq!(e.bytes, raw.len());
    assert!(!e.redacted);
}

#[test]
fn bus_password_target_redacted() {
    let mut wm = two_pane_wm();
    enter_password_prompt(&mut wm, 1);
    let raw = "hunter2";
    wm.record_traffic(TrafficEndpoint::Api, TrafficEndpoint::Pane(1), TrafficKind::Keys, raw, Instant::now());
    wm.record_traffic(TrafficEndpoint::Api, TrafficEndpoint::Pane(2), TrafficKind::Keys, raw, Instant::now());
    let evs: Vec<_> = wm.traffic().iter().collect();
    assert_eq!(evs[0].preview, "[redacted]");
    assert!(evs[0].redacted);
    assert_eq!(evs[0].bytes, raw.len());
    assert_eq!(evs[1].preview, "hunter2");
    assert!(!evs[1].redacted);
}

#[test]
fn bus_api_origin_broadcast_labels_api() {
    let mut wm = two_pane_wm();
    wm.set_api_origin(true);
    wm.broadcast_command("uptime").expect("broadcast");
    wm.set_api_origin(false);
    assert_eq!(wm.traffic().len(), 2);
    assert!(wm.traffic().iter().all(|e| e.from == TrafficEndpoint::Api));
}

#[test]
fn bus_mcp_broadcast_records_session_targets() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.execute_mcp_command("spawn agents/w1");
    wm.execute_mcp_command("spawn agents/w2");
    let before = wm.traffic().len();
    wm.execute_mcp_command("broadcast agents uptime");
    let log_len = wm.daemon_state().broadcast_log.len();
    assert!(log_len >= 2, "daemon must have delivered to at least two sessions");
    let evs: Vec<_> = wm.traffic().iter().skip(before).collect();
    assert_eq!(evs.len(), log_len);
    for e in &evs {
        assert_eq!(e.from, TrafficEndpoint::Mcp);
        assert!(matches!(&e.to, TrafficEndpoint::Session(_)));
        assert_eq!(e.kind, TrafficKind::Broadcast);
        assert_eq!(e.preview, "uptime");
    }
    let labels: Vec<String> = evs.iter().map(|e| e.to.label()).collect();
    assert!(labels.contains(&"@w1".to_string()) && labels.contains(&"@w2".to_string()), "{labels:?}");
}

#[test]
fn bus_rpc_pane_exec_records_api_event() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    rpc_ok(&mut plane, "pane.exec", r#"{"command":"echo hi","pane_id":1}"#);
    let evs: Vec<_> = plane.wm.traffic().iter().collect();
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].from, TrafficEndpoint::Api);
    assert_eq!(evs[0].to, TrafficEndpoint::Pane(1));
    assert_eq!(evs[0].kind, TrafficKind::Exec);
    assert_eq!(evs[0].preview, "echo hi");
    // Error path records nothing.
    let line = r#"{"jsonrpc":"2.0","id":2,"method":"pane.exec","params":{"command":"x","pane_id":99}}"#;
    let v = JsonValue::parse(&plane.handle_jsonrpc_line(line)).unwrap();
    assert!(v.get("error").is_some());
    assert_eq!(plane.wm.traffic().len(), 1);
}

#[test]
fn bus_rpc_send_keys_records_keys_event() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    rpc_ok(&mut plane, "pane.send_keys", r#"{"keys":["a","b"]}"#);
    {
        let e = plane.wm.traffic().iter().next().expect("event");
        assert_eq!(e.from, TrafficEndpoint::Api);
        assert_eq!(e.to, TrafficEndpoint::Pane(plane.wm.active_pane_id()));
        assert_eq!(e.kind, TrafficKind::Keys);
        assert_eq!(e.preview, "a b");
        assert!(!e.redacted);
    }
    enter_password_prompt(&mut plane.wm, 1);
    rpc_ok(&mut plane, "pane.send_keys", r#"{"keys":["h","u","n"]}"#);
    let last = plane.wm.traffic().iter().last().unwrap();
    assert!(last.redacted);
    assert_eq!(last.preview, "[redacted]");
    assert_eq!(last.bytes, "h u n".len());
    // A bad chord records nothing.
    let n = plane.wm.traffic().len();
    let line = r#"{"jsonrpc":"2.0","id":3,"method":"pane.send_keys","params":{"keys":["Nope+Nope+Nope"]}}"#;
    let _ = plane.handle_jsonrpc_line(line);
    assert_eq!(plane.wm.traffic().len(), n);
}

#[test]
fn bus_agent_message_kind_reserved_for_phase51() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.record_traffic(
        TrafficEndpoint::Agent("a".into()),
        TrafficEndpoint::Agent("b".into()),
        TrafficKind::Message,
        "hello",
        Instant::now(),
    );
    let e = wm.traffic().iter().next().unwrap();
    assert_eq!(e.from.label(), "a");
    assert_eq!(e.to.label(), "b");
    assert_eq!(e.kind.as_str(), "message");
    assert_eq!(TrafficEndpoint::User.label(), "you");
    assert_eq!(TrafficEndpoint::Api.label(), "api");
    assert_eq!(TrafficEndpoint::Mcp.label(), "mcp");
    assert_eq!(TrafficEndpoint::Pane(7).label(), "#7");
    assert_eq!(TrafficEndpoint::Session("s".into()).label(), "@s");
}

#[test]
fn bus_partners_window_excludes_old_events() {
    let mut wm = two_pane_wm();
    let t0 = Instant::now();
    wm.record_traffic(TrafficEndpoint::User, TrafficEndpoint::Pane(1), TrafficKind::Input, "old", t0);
    let now = t0 + Duration::from_secs(120);
    wm.record_traffic(TrafficEndpoint::Api, TrafficEndpoint::Pane(1), TrafficKind::Exec, "new", now - Duration::from_secs(5));
    wm.record_traffic(TrafficEndpoint::Api, TrafficEndpoint::Pane(1), TrafficKind::Exec, "new2", now - Duration::from_secs(1));
    let partners = wm.partners_of(1, Duration::from_secs(60), now);
    assert_eq!(partners.len(), 1, "{partners:?}");
    assert_eq!(partners[0].endpoint, TrafficEndpoint::Api);
    assert_eq!(partners[0].count, 2);
    assert!(wm.partners_of(2, Duration::from_secs(60), now).is_empty());
    let wide = wm.partners_of(1, Duration::from_secs(600), now);
    assert_eq!(wide.len(), 2);
    assert_eq!(wide[0].endpoint, TrafficEndpoint::Api, "most recent partner first");
}
