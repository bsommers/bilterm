//! Phase 50-03: per-pane agent state (FLT-01, D-03). `state_` tests use synthetic `Instant`s only.

use std::time::Instant;

use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::parse_colon_command;
use bilterm::wm::WindowManager;
use bilterm::wm::agent_state::AgentState;

fn run(wm: &mut WindowManager, cmd: &str) {
    let action = parse_colon_command(cmd).expect("colon").expect("parse");
    let _ = wm.apply(action);
}

fn data(wm: &mut WindowManager) -> JsonValue {
    JsonValue::parse(&wm.take_command_data().expect("command data")).expect("json")
}

fn state_of(d: &JsonValue, pane_id: usize) -> String {
    let JsonValue::Array(items) = d.get("agents").expect("agents") else { panic!("agents not array") };
    items
        .iter()
        .find(|a| a.get("pane_id").and_then(JsonValue::as_usize) == Some(pane_id))
        .and_then(|a| a.get("state").and_then(JsonValue::as_str).map(str::to_string))
        .unwrap_or_else(|| panic!("no agent entry for pane {pane_id}: {d:?}"))
}

#[test]
fn state_tracer_running_command_is_working() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.pane_mut(1).unwrap().submit_command("sleep 100").unwrap();
    wm.pane_mut(1).unwrap().process_pty_output(b"building...\r\n");
    let t0 = Instant::now();
    wm.update_agent_states(t0);
    run(&mut wm, ":agents list");
    let d = data(&mut wm);
    assert_eq!(state_of(&d, 1), "working");
    assert_eq!(d.get("counts").and_then(|c| c.get("working")).and_then(JsonValue::as_usize), Some(1));
    assert_eq!(wm.agent_state(1), Some(AgentState::Working));
}

#[test]
fn state_tracer_list_over_rpc() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let line = r#"{"jsonrpc":"2.0","id":1,"method":"wm.command","params":{"command":":agents list"}}"#;
    let v = JsonValue::parse(&plane.handle_jsonrpc_line(line)).expect("json");
    let result = v.get("result").unwrap_or_else(|| panic!("no result: {v:?}"));
    let data = result.get("data").unwrap_or_else(|| panic!("no data: {result:?}"));
    assert!(data.get("counts").is_some(), "{data:?}");
    assert!(data.get("agents").is_some(), "{data:?}");
}
