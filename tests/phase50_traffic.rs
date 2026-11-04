//! Phase 50-05: agent traffic bus (FLT-02, D-01). `bus_` tests use synthetic `Instant`s only.

use bilterm::api::JsonValue;
use bilterm::colon_command::parse_colon_command;
use bilterm::wm::WindowManager;
use bilterm::wm::SplitDirection;
use bilterm::wm::traffic::{TrafficEndpoint, TrafficKind};

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
