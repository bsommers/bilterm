//! Phase 51-01: tracer slice and frozen command contract for the AGENTS list window (UIT-03) and
//! `:agents demo stop` (UIT-01). `list_tracer` = parse -> Action -> apply -> state -> render,
//! `list_grammar` = the frozen colon grammar of 51-CONTRACT.md section 3.

use bilterm::action::Action;
use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::parse_colon_command;
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::typography::PanelTarget;
use bilterm::wm::agents_cmd::AgentsOp;
use bilterm::wm::agents_list_cmd::{AgentsListOp, ListFilter};
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

fn fixture(cols: u16, rows: u16) -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(rows, cols);
    let area = Rect::new(0, 0, cols, rows);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(area).unwrap();
    (wm, area)
}

fn render(wm: &WindowManager, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(wm).render(area, &mut buf);
    buf
}

fn text(buf: &Buffer) -> String {
    let a = buf.area;
    (a.y..a.y + a.height)
        .map(|y| (a.x..a.x + a.width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn run_colon(wm: &mut WindowManager, cmd: &str) {
    let parsed = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"));
    wm.apply(parsed).unwrap();
}

#[test]
fn list_tracer_colon_opens_renders_closes() {
    let (mut wm, area) = fixture(120, 40);
    assert!(!text(&render(&wm, area)).contains("AGENTS"), "closed list renders nothing");

    run_colon(&mut wm, ":agents list");
    let state = wm.agents_list();
    assert!(state.active && state.focused, "`:agents list` opens and focuses the window");
    assert!(!wm.agents_window().captures_input(), "only one window owns the keyboard");
    let shown = text(&render(&wm, area));
    assert!(shown.contains("AGENTS"), "missing AGENTS frame:\n{shown}");
    assert!(!shown.contains("AGENT TRAFFIC"));
    let rows = wm.agents_list_rows();
    assert_eq!(rows.len(), 2, "one row per tracked pane");
    for row in &rows {
        assert!(shown.contains(&format!("#{}", row.pane_id)), "missing row for pane {}", row.pane_id);
        assert!(shown.contains(&row.name), "missing name {}", row.name);
    }

    run_colon(&mut wm, ":agents list close");
    assert!(!wm.agents_list().active && !wm.agents_list().focused);
    assert!(!text(&render(&wm, area)).contains("AGENTS"), "window disappears when closed");
}

#[test]
fn list_tracer_rpc_wm_command_matches_colon() {
    let mut plane = AutomationControlPlane::new_headless(24, 120);
    plane.wm.split_focused(SplitDirection::Vertical).unwrap();
    let params = JsonValue::parse(r#"{"command":":agents list"}"#).unwrap();
    plane.dispatch_method("wm.command", &params).unwrap();
    assert!(plane.wm.agents_list().active && plane.wm.agents_list().focused);
    let params = JsonValue::parse(r#"{"command":":agents list close"}"#).unwrap();
    plane.dispatch_method("wm.command", &params).unwrap();
    assert!(!plane.wm.agents_list().active);
}

#[test]
fn list_opening_either_window_blurs_the_other() {
    let (mut wm, _) = fixture(120, 40);
    run_colon(&mut wm, ":agents zoom");
    assert!(wm.agents_window().captures_input());
    run_colon(&mut wm, ":agents list open");
    assert!(wm.agents_list().captures_input() && !wm.agents_window().captures_input());
    run_colon(&mut wm, ":agents zoom");
    assert!(wm.agents_window().captures_input() && !wm.agents_list().captures_input());
    assert!(wm.agents_list().active, "blur does not close");
    wm.focus_and_raise(1);
    assert!(!wm.agents_window().focused && !wm.agents_list().focused);
}

#[test]
fn list_dismiss_closes_unpinned_only() {
    let (mut wm, _) = fixture(120, 40);
    run_colon(&mut wm, ":agents list open");
    run_colon(&mut wm, ":agents list dismiss");
    assert!(!wm.agents_list().active);
    run_colon(&mut wm, ":agents list toggle");
    assert!(wm.agents_list().active);
    wm.agents_list_mut().pinned = true;
    run_colon(&mut wm, ":agents list dismiss");
    assert!(wm.agents_list().active && !wm.agents_list().focused, "pinned: only the keyboard is released");
    run_colon(&mut wm, ":agents list toggle");
    assert!(!wm.agents_list().active, "toggle closes an open window");
}

/// One value of every `AgentsListOp` variant (both Option forms where the grammar has two).
fn sample_list_ops() -> Vec<AgentsListOp> {
    use AgentsListOp::*;
    vec![
        Open, Close, Toggle, Dismiss, Blur,
        Step(-1), Step(1), First, Last, Page(-1), Page(1), SelectPane(3),
        Jump(None), Jump(Some(2)), Zoom(None), Zoom(Some(2)),
        Pin(Some(true)), Pin(Some(false)), Pin(None),
        ScaleStep(10), ScaleStep(-10),
        Filter(Some(ListFilter::All)), Filter(Some(ListFilter::Agents)), Filter(None),
        Dock(Some(true)), Dock(Some(false)), Dock(None),
        Move { dx: -1, dy: 2 }, Resize { dw: 3, dh: -4 }, SetRect { x: 1, y: 2, w: 30, h: 8 },
        Size(None), Size(Some((40, 12))), Scroll(-3), Scroll(3),
    ]
}

fn parsed(cmd: &str) -> Result<Action, String> {
    parse_colon_command(cmd).unwrap_or_else(|| panic!("not a colon command: {cmd}"))
}

#[test]
fn list_grammar_round_trip() {
    for op in sample_list_ops() {
        let form = op.colon_form();
        assert_eq!(parsed(&form), Ok(Action::AgentsList(op.clone())), "round trip of {form}");
    }
}

#[test]
fn list_grammar_scale_absolute() {
    for cmd in [":agents list scale 150", ":agents list scale 150%"] {
        assert_eq!(parsed(cmd), Ok(Action::PanelScaleOf(PanelTarget::AgentsList, 150)), "{cmd}");
    }
    assert_eq!(parsed(":agents list scale +10"), Ok(Action::AgentsList(AgentsListOp::ScaleStep(10))));
    assert_eq!(parsed(":agents list scale -10"), Ok(Action::AgentsList(AgentsListOp::ScaleStep(-10))));
    for bad in ["scale 74", "scale 201", "scale x", "scale", "scale 100 100", "scale +x"] {
        assert!(parsed(&format!(":agents list {bad}")).is_err(), "{bad} must be a usage error");
    }
}

#[test]
fn list_grammar_select_forms() {
    use AgentsListOp::*;
    for (arg, op) in [
        ("up", Step(-1)), ("down", Step(1)), ("first", First), ("last", Last),
        ("page-up", Page(-1)), ("page-down", Page(1)), ("3", SelectPane(3)), ("#3", SelectPane(3)),
    ] {
        assert_eq!(parsed(&format!(":agents list select {arg}")), Ok(Action::AgentsList(op)), "select {arg}");
    }
    for bad in ["select", "select sideways", "select 99999999999", "select 1 2", "select #"] {
        assert!(parsed(&format!(":agents list {bad}")).is_err(), "{bad} must be a usage error");
    }
}

#[test]
fn list_grammar_rejects_garbage() {
    for bad in [
        ":agents list bogus", ":agents list close now", ":agents list move 1", ":agents list move a b",
        ":agents list rect 1 2 3", ":agents list rect 1 2 3 -4", ":agents list size 0x5",
        ":agents list size 9999x9999", ":agents list size 40", ":agents list resize 1",
        ":agents list scroll", ":agents list scroll x", ":agents list scroll 99999",
        ":agents list filter", ":agents list filter bogus", ":agents list dock now",
        ":agents list pin maybe", ":agents list jump x", ":agents list zoom 1 2",
        ":agents list open extra", ":agents list \u{202e}", ":agents demo x", ":agents demo stop x",
    ] {
        let res = parsed(bad);
        let err = res.err().unwrap_or_else(|| panic!("{bad} must be a usage error"));
        assert!(err.starts_with("Usage:"), "{bad}: {err}");
    }
}

#[test]
fn list_aliases_and_bare_list_unchanged() {
    for cmd in [":agents", ":agents list", ":agents ls", ":agents status"] {
        assert_eq!(parsed(cmd), Ok(Action::Agents(AgentsOp::List)), "{cmd}");
    }
    assert_eq!(parsed(":agents ls close"), parsed(":agents list close"));
    assert_eq!(parsed(":agents ls close"), Ok(Action::AgentsList(AgentsListOp::Close)));
}

#[test]
fn demo_stop_parses_and_stub_reports() {
    assert_eq!(parsed(":agents demo stop"), Ok(Action::Agents(AgentsOp::DemoStop)));
    assert_eq!(parsed(":agents demo"), Ok(Action::Agents(AgentsOp::Demo)));
    let (mut wm, _) = fixture(120, 40);
    let before = wm.pane_count();
    run_colon(&mut wm, ":agents demo stop");
    let msg = wm.active_pane().status_message().unwrap_or("").to_string();
    assert!(msg.contains("No agent demo"), "status: {msg}");
    assert_eq!(wm.pane_count(), before, "the seam stub changes nothing");
}
