//! Phase 50.1-12: result-aware `FindOp` execution (open, re-rank, kind switching, jump/prefill).
//! Headless only; saved and log rows are injected with `set_find_results` because the disk
//! providers belong to plan 11.

use std::time::Instant;

use bilterm::action::Action;
use bilterm::api::JsonValue;
use bilterm::colon_command::parse_colon_command;
use bilterm::wm::find_cmd::{FindKind, FindOp};
use bilterm::wm::registry::{
    FindResults, FindRow, HistoryRow, Origin, PaneRow, Recency, ResultSource, RowData, SessionRow,
};
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::layout::Rect;

fn fixture(panes: usize) -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    for _ in 1..panes {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.resize_all(Rect::new(0, 0, 80, 24)).unwrap();
    wm.update_agent_states(Instant::now());
    wm
}

fn parse(cmd: &str) -> Action {
    parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"))
}

fn run_colon(wm: &mut WindowManager, cmd: &str) {
    wm.apply(parse(cmd)).unwrap();
}

fn status(wm: &WindowManager) -> String {
    wm.pane(wm.active_pane_id()).unwrap().status_message().unwrap_or_default().to_string()
}

#[test]
fn find_tracer_agent_enter_focuses_pane() {
    let mut wm = fixture(3);
    assert!(wm.focus_and_raise(1) || wm.active_pane_id() == 1);
    assert_eq!(wm.active_pane_id(), 1);
    run_colon(&mut wm, ":find agent");
    let data = wm.take_command_data().expect("find stashes command data");
    let json = JsonValue::parse(&data).expect("valid json");
    let JsonValue::Array(rows) = json.get("rows").unwrap() else { panic!("rows") };
    assert_eq!(rows.len(), 3);
    let src = rows[0].get("source").unwrap();
    assert_eq!(src.get("kind").and_then(JsonValue::as_str), Some("live"));
    assert!(wm.is_find_visible());

    run_colon(&mut wm, ":finder select 2");
    let target = wm.find_results().rows[2].pane.expect("agent row has a pane");
    assert_ne!(target, 1);
    run_colon(&mut wm, ":finder enter");
    assert_eq!(wm.active_pane_id(), target);
    assert!(!wm.is_find_visible());
    assert!(status(&wm).contains("live"), "status: {}", status(&wm));
}
