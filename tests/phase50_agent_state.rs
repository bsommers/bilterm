//! Phase 50-03: per-pane agent state (FLT-01, D-03). `state_` tests use synthetic `Instant`s only.

use std::time::{Duration, Instant};

use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::parse_colon_command;
use bilterm::wm::WindowManager;
use bilterm::config::BiltermConfig;
use bilterm::config::agents::AgentProfile;
use bilterm::wm::SplitDirection;
use bilterm::wm::agent_state::{AgentSignals, AgentState, FinishKind, classify};

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

fn ms(t0: Instant, n: u64) -> Instant {
    t0 + Duration::from_millis(n)
}

/// Two panes; pane 1 is unfocused (pane 2, the new split, holds focus).
fn two_panes() -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    assert_eq!(p2, 2);
    assert_eq!(wm.active_pane_id(), 2);
    wm
}

fn feed(wm: &mut WindowManager, id: usize, bytes: &[u8]) {
    wm.pane_mut(id).unwrap().process_pty_output(bytes);
}

fn lock_echo(wm: &mut WindowManager, id: usize) {
    let ld = wm.pane_mut(id).unwrap().line_discipline_mut();
    ld.echo_enabled = false;
    ld.canonical_mode = true;
}

#[test]
fn state_password_prompt_is_blocked() {
    let mut wm = two_panes();
    feed(&mut wm, 1, b"out\r\n\x1b]133;D;1\x07");
    lock_echo(&mut wm, 1);
    wm.update_agent_states(Instant::now());
    assert_eq!(wm.agent_state(1), Some(AgentState::Blocked));
}

#[test]
fn state_nonzero_exit_unseen_is_errored() {
    let mut wm = two_panes();
    let t0 = Instant::now();
    feed(&mut wm, 1, b"boom\r\n\x1b]133;D;42\x07");
    wm.update_agent_states(t0);
    assert_eq!(wm.agent_state(1), Some(AgentState::Errored));
    wm.update_agent_states(ms(t0, 5000));
    assert_eq!(wm.agent_state(1), Some(AgentState::Errored), "persists while unfocused");
    wm.focus_and_raise(1);
    wm.update_agent_states(ms(t0, 5010));
    assert_eq!(wm.agent_state(1), Some(AgentState::Idle));
}

#[test]
fn state_zero_exit_unseen_is_done() {
    let mut wm = two_panes();
    let t0 = Instant::now();
    feed(&mut wm, 1, b"ok\r\n\x1b]133;D;0\x07");
    wm.update_agent_states(t0);
    assert_eq!(wm.agent_state(1), Some(AgentState::Done));
    wm.focus_and_raise(1);
    wm.update_agent_states(ms(t0, 10));
    assert_eq!(wm.agent_state(1), Some(AgentState::Idle));
}

#[test]
fn state_exited_process_nonzero_is_errored() {
    let mut wm = two_panes();
    wm.pane_mut(1).unwrap().hold_exited_shell(Some(3));
    wm.pane_mut(2).unwrap().hold_exited_shell(Some(0));
    wm.update_agent_states(Instant::now());
    assert_eq!(wm.agent_state(1), Some(AgentState::Errored));
    assert_eq!(wm.agent_state(2), Some(AgentState::Done));
}

#[test]
fn state_unread_notification_unfocused_is_blocked() {
    let mut wm = two_panes();
    feed(&mut wm, 1, b"\x1b]9;need input\x07");
    feed(&mut wm, 2, b"\x1b]9;fyi\x07");
    wm.update_agent_states(Instant::now());
    assert_eq!(wm.agent_state(1), Some(AgentState::Blocked));
    assert_ne!(wm.agent_state(2), Some(AgentState::Blocked), "focused pane is not blocked by its own notification");
}

#[test]
fn state_silent_running_command_goes_idle_after_hold() {
    let mut wm = WindowManager::new_headless(24, 80);
    let t0 = Instant::now();
    wm.pane_mut(1).unwrap().submit_command("sleep 100").unwrap();
    feed(&mut wm, 1, b"started\r\n");
    wm.update_agent_states(t0);
    assert_eq!(wm.agent_state(1), Some(AgentState::Working));
    wm.update_agent_states(ms(t0, 1400));
    assert_eq!(wm.agent_state(1), Some(AgentState::Working), "inside the hold");
    wm.update_agent_states(ms(t0, 1600));
    wm.update_agent_states(ms(t0, 2000));
    assert_eq!(wm.agent_state(1), Some(AgentState::Working), "dwell not yet elapsed");
    wm.update_agent_states(ms(t0, 2200));
    assert_eq!(wm.agent_state(1), Some(AgentState::Idle));
}

#[test]
fn state_working_idle_flip_respects_dwell() {
    let mut wm = WindowManager::new_headless(24, 80);
    let t0 = Instant::now();
    wm.update_agent_states(t0);
    assert_eq!(wm.agent_state(1), Some(AgentState::Idle));
    wm.pane_mut(1).unwrap().submit_command("make").unwrap();
    feed(&mut wm, 1, b"cc a.c\r\n");
    wm.update_agent_states(ms(t0, 100));
    assert_eq!(wm.agent_state(1), Some(AgentState::Idle), "candidate Working, dwell pending");
    feed(&mut wm, 1, b"\x1b]133;D;0\x07");
    wm.update_agent_states(ms(t0, 300));
    wm.pane_mut(1).unwrap().submit_command("make").unwrap();
    feed(&mut wm, 1, b"cc b.c\r\n");
    wm.update_agent_states(ms(t0, 400));
    wm.update_agent_states(ms(t0, 800));
    assert_eq!(wm.agent_state(1), Some(AgentState::Idle), "alternation restarted the dwell clock");
    wm.update_agent_states(ms(t0, 900));
    assert_eq!(wm.agent_state(1), Some(AgentState::Working));
}

#[test]
fn state_attention_states_apply_immediately() {
    let mut wm = WindowManager::new_headless(24, 80);
    let t0 = Instant::now();
    wm.pane_mut(1).unwrap().submit_command("ssh host").unwrap();
    feed(&mut wm, 1, b"connecting\r\n");
    wm.update_agent_states(t0);
    assert_eq!(wm.agent_state(1), Some(AgentState::Working));
    lock_echo(&mut wm, 1);
    wm.update_agent_states(ms(t0, 10));
    assert_eq!(wm.agent_state(1), Some(AgentState::Blocked));
}

#[test]
fn state_reported_overrides_heuristics() {
    let mut wm = WindowManager::new_headless(24, 80);
    let t0 = Instant::now();
    wm.pane_mut(1).unwrap().submit_command("agent").unwrap();
    feed(&mut wm, 1, b"thinking\r\n");
    wm.update_agent_states(t0);
    assert_eq!(wm.agent_state(1), Some(AgentState::Working));
    wm.report_agent_state(1, Some(AgentState::Blocked));
    wm.update_agent_states(ms(t0, 10));
    assert_eq!(wm.agent_state(1), Some(AgentState::Blocked));
    assert!(wm.agent_statuses()[0].reported);
    feed(&mut wm, 1, b"more\r\n");
    wm.update_agent_states(ms(t0, 20));
    assert_eq!(wm.agent_state(1), Some(AgentState::Blocked), "still reported");
    wm.report_agent_state(1, None);
    wm.update_agent_states(ms(t0, 30));
    assert_eq!(wm.agent_state(1), Some(AgentState::Working));
    assert!(!wm.agent_statuses()[0].reported);
}

#[test]
fn state_removed_pane_drops_history() {
    let mut wm = two_panes();
    let t0 = Instant::now();
    wm.update_agent_states(t0);
    assert!(wm.agent_statuses().iter().any(|s| s.pane_id == 2));
    assert!(wm.remove_pane(2));
    wm.update_agent_states(ms(t0, 10));
    assert_eq!(wm.agent_state(2), None);
    assert!(wm.agent_statuses().iter().all(|s| s.pane_id != 2));
}

#[test]
fn state_classify_table() {
    let base = AgentSignals::default();
    let ms = |n| Duration::from_millis(n);
    let cases: Vec<(AgentSignals, AgentState)> = vec![
        (AgentSignals { echo_locked: true, unseen_finish: Some(FinishKind::Failure), ..base.clone() }, AgentState::Blocked),
        (AgentSignals { exited: true, exit_code: Some(2), ..base.clone() }, AgentState::Errored),
        (AgentSignals { exited: true, exit_code: Some(0), ..base.clone() }, AgentState::Done),
        (AgentSignals { unseen_finish: Some(FinishKind::Failure), running: true, quiet_for: ms(0), ..base.clone() }, AgentState::Errored),
        (AgentSignals { unseen_finish: Some(FinishKind::Success), ..base.clone() }, AgentState::Done),
        (AgentSignals { unread_notification: true, focused: false, ..base.clone() }, AgentState::Blocked),
        (AgentSignals { unread_notification: true, focused: true, ..base.clone() }, AgentState::Idle),
        (AgentSignals { running: true, quiet_for: ms(1500), ..base.clone() }, AgentState::Working),
        (AgentSignals { running: true, quiet_for: ms(1501), ..base.clone() }, AgentState::Idle),
        (AgentSignals { running: false, quiet_for: ms(0), ..base.clone() }, AgentState::Idle),
        (base.clone(), AgentState::Idle),
    ];
    for (i, (sig, want)) in cases.iter().enumerate() {
        assert_eq!(classify(sig, None), *want, "case {i}: {sig:?}");
    }
    assert_eq!(AgentState::Working.as_str(), "working");
    assert!(AgentState::Blocked.is_attention() && AgentState::Errored.is_attention() && AgentState::Done.is_attention());
    assert!(!AgentState::Working.is_attention() && !AgentState::Idle.is_attention());
}

fn profile_wm(name: &str, p: AgentProfile) -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().agents.insert(name.to_string(), p);
    wm
}

fn tool_profile(working: &[&str], blocked: &[&str]) -> AgentProfile {
    AgentProfile {
        matches: vec![],
        working: working.iter().map(|s| s.to_string()).collect(),
        blocked: blocked.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn state_profile_blocked_pattern_marks_blocked() {
    let mut wm = profile_wm("tool", tool_profile(&[], &["proceed?"]));
    wm.pane_mut(1).unwrap().submit_command("tool --go").unwrap();
    feed(&mut wm, 1, b"Do you want to PROCEED?\r\n");
    wm.update_agent_states(Instant::now());
    assert_eq!(wm.agent_state(1), Some(AgentState::Blocked));
}

#[test]
fn state_profile_working_pattern_keeps_working_while_silent() {
    let mut wm = profile_wm("tool", tool_profile(&["esc to interrupt"], &[]));
    let t0 = Instant::now();
    wm.pane_mut(1).unwrap().submit_command("tool").unwrap();
    feed(&mut wm, 1, b"thinking (Esc to interrupt)\r\n");
    wm.update_agent_states(t0);
    wm.update_agent_states(ms(t0, 5000));
    wm.update_agent_states(ms(t0, 9000));
    assert_eq!(wm.agent_state(1), Some(AgentState::Working));
    // Without the profile the same silence goes idle.
    let mut plain = WindowManager::new_headless(24, 80);
    plain.pane_mut(1).unwrap().submit_command("tool").unwrap();
    feed(&mut plain, 1, b"thinking (Esc to interrupt)\r\n");
    plain.update_agent_states(t0);
    plain.update_agent_states(ms(t0, 5000));
    plain.update_agent_states(ms(t0, 9000));
    assert_eq!(plain.agent_state(1), Some(AgentState::Idle));
}

#[test]
fn state_profile_matches_defaults_to_name() {
    let mut wm = profile_wm("claude", tool_profile(&[], &["allow?"]));
    wm.pane_mut(1).unwrap().submit_command("/usr/local/bin/claude --resume").unwrap();
    feed(&mut wm, 1, b"Allow? (y/n)\r\n");
    wm.update_agent_states(Instant::now());
    assert_eq!(wm.agent_state(1), Some(AgentState::Blocked));
    // A different program does not pick the profile up.
    let mut other = profile_wm("claude", tool_profile(&[], &["allow?"]));
    other.pane_mut(1).unwrap().submit_command("vim notes.txt").unwrap();
    feed(&mut other, 1, b"Allow? (y/n)\r\n");
    other.update_agent_states(Instant::now());
    assert_ne!(other.agent_state(1), Some(AgentState::Blocked));
    // Explicit `matches` wins over the profile name.
    let mut custom = profile_wm("x", AgentProfile { matches: vec!["cod".into()], ..tool_profile(&[], &["allow?"]) });
    custom.pane_mut(1).unwrap().submit_command("codex").unwrap();
    feed(&mut custom, 1, b"allow?\r\n");
    custom.update_agent_states(Instant::now());
    assert_eq!(custom.agent_state(1), Some(AgentState::Blocked));
}

#[test]
fn state_profile_patterns_are_capped() {
    let many: Vec<String> = (0..40).map(|i| format!("zz{i:02}x")).collect();
    let long = "x".repeat(500);
    let p = AgentProfile { matches: vec![], working: many.clone(), blocked: vec![long, "  ".into(), " Ok ".into()] }.sanitized();
    assert_eq!(p.working.len(), 16);
    assert_eq!(p.blocked.len(), 2, "empty entries dropped");
    assert_eq!(p.blocked[0].chars().count(), 80);
    assert_eq!(p.blocked[1], "ok", "trimmed and lower-cased");
    // The tracker applies the same caps: a 40th pattern never matches.
    let mut wm = profile_wm("tool", AgentProfile { matches: vec![], working: vec![], blocked: many });
    wm.pane_mut(1).unwrap().submit_command("tool").unwrap();
    feed(&mut wm, 1, b"zz39x\r\n");
    wm.update_agent_states(Instant::now());
    assert_ne!(wm.agent_state(1), Some(AgentState::Blocked));
}

#[test]
fn state_empty_agents_table_not_serialized() {
    let text = toml::to_string(&BiltermConfig::default()).unwrap();
    // `[agents_window]` (window settings) is a different table from the `[agents.<name>]` profiles.
    assert!(!text.contains("[agents]") && !text.contains("[agents."), "{text}");
    let cfg: BiltermConfig = toml::from_str("[agents.tool]\nblocked = [\"ok?\"]\n").unwrap();
    assert_eq!(cfg.agents["tool"].blocked, ["ok?"]);
    assert!(toml::to_string(&cfg).unwrap().contains("[agents.tool]"));
}

#[test]
fn state_list_reports_profile_name() {
    let mut wm = profile_wm("tool", tool_profile(&[], &["proceed?"]));
    wm.pane_mut(1).unwrap().submit_command("tool run").unwrap();
    feed(&mut wm, 1, b"working\r\n");
    run(&mut wm, ":agents list");
    let d = data(&mut wm);
    let JsonValue::Array(items) = d.get("agents").unwrap() else { panic!() };
    assert_eq!(items[0].get("profile").and_then(JsonValue::as_str), Some("tool"));
    assert_eq!(items[0].get("name").and_then(JsonValue::as_str), Some("tool"));
    // Panes without a matching profile report null.
    let mut bare = WindowManager::new_headless(24, 80);
    run(&mut bare, ":agents list");
    let d = data(&mut bare);
    let JsonValue::Array(items) = d.get("agents").unwrap() else { panic!() };
    assert!(matches!(items[0].get("profile"), Some(JsonValue::Null)));
}
