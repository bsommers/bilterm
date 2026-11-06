//! Phase 51-01: tracer slice and frozen command contract for the AGENTS list window (UIT-03) and
//! `:agents demo stop` (UIT-01). `list_tracer` = parse -> Action -> apply -> state -> render,
//! `list_grammar` = the frozen colon grammar of 51-CONTRACT.md section 3.

use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::parse_colon_command;
use bilterm::ui::WorkspaceWidget;
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
