//! Phase 51-05: behavior of the AGENTS list window (D-03): rows, filter, selection, scroll, jump,
//! zoom, pin, scale, dock flag, geometry ops and live refresh. Every test applies an
//! `AgentsListOp` directly: the key, mouse, colon and RPC paths all lower to it.

use std::time::{Duration, Instant};

use bilterm::ui::typography::PanelTarget;
use bilterm::wm::agent_state::AgentState;
use bilterm::wm::agents_list_cmd::{AgentsListOp, ListFilter};
use bilterm::wm::agents_window::NodeKey;
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::layout::Rect;

const COLS: u16 = 120;
const ROWS: u16 = 40;

fn area() -> Rect {
    Rect::new(0, 0, COLS, ROWS)
}

fn fixture(panes: usize) -> WindowManager {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    for _ in 1..panes {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.resize_all(area()).unwrap();
    wm.update_agent_states(Instant::now());
    wm
}

fn ids(wm: &WindowManager) -> Vec<usize> {
    wm.agents_list_rows().iter().map(|r| r.pane_id).collect()
}

fn apply(wm: &mut WindowManager, op: AgentsListOp) -> Option<String> {
    wm.agents_list_apply(&op)
}

fn tick(wm: &mut WindowManager, ms: u64) -> bool {
    wm.tick_agent_ui(Instant::now() + Duration::from_millis(ms))
}

#[test]
fn rows_list_all_panes_in_id_order_and_filter() {
    let mut wm = fixture(4);
    wm.open_agents_list();
    assert_eq!(ids(&wm), vec![1, 2, 3, 4]);
    for r in wm.agents_list_rows() {
        assert!(r.name.chars().all(|c| !c.is_control()), "name {:?}", r.name);
        assert!(!r.is_agent);
    }
    // A pane title with escape bytes never reaches a row name.
    wm.pane_mut(2).unwrap().process_pty_output(b"\x1b]0;ev\x1b[31mil\x07\r\n");
    wm.update_agent_states(Instant::now());
    for r in wm.agents_list_rows() {
        assert!(r.name.chars().all(|c| !c.is_control() && c != '\u{202e}'), "name {:?}", r.name);
    }
    wm.report_agent_state(3, Some(AgentState::Blocked));
    assert_eq!(ids(&wm), vec![1, 2, 3, 4], "all is the default");
    let msg = apply(&mut wm, AgentsListOp::Filter(Some(ListFilter::Agents)));
    assert_eq!(msg.as_deref(), Some("Agents list: detected agents only"));
    assert_eq!(ids(&wm), vec![3]);
    let msg = apply(&mut wm, AgentsListOp::Filter(None));
    assert_eq!(msg.as_deref(), Some("Agents list: all panes"));
    assert_eq!(ids(&wm), vec![1, 2, 3, 4]);
    apply(&mut wm, AgentsListOp::Filter(None));
    assert_eq!(wm.agents_list().filter, ListFilter::Agents);
}

#[test]
fn selection_keyed_by_pane_id() {
    let mut wm = fixture(4);
    wm.open_agents_list();
    apply(&mut wm, AgentsListOp::SelectPane(3));
    assert_eq!(wm.agents_list().selected, Some(3));
    // A state change and a new pane do not move the selection.
    wm.report_agent_state(1, Some(AgentState::Errored));
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.update_agent_states(Instant::now());
    assert_eq!(ids(&wm), vec![1, 2, 3, 4, 5]);
    assert_eq!(wm.agents_list().selected, Some(3));
    // Closing the selected pane falls back to the nearest remaining row, never dangling.
    assert!(wm.remove_pane(3));
    wm.update_agent_states(Instant::now());
    assert!(tick(&mut wm, 5), "rows changed while the list is open");
    let sel = wm.agents_list().selected.expect("a row is selected");
    assert!(ids(&wm).contains(&sel));
    assert_eq!(sel, 4, "the row that took its place");
    // Closing every other pane leaves the sole row; filtering everything out clears it.
    apply(&mut wm, AgentsListOp::Filter(Some(ListFilter::Agents)));
    assert_eq!(ids(&wm), vec![1]);
    assert_eq!(wm.agents_list().selected, Some(1));
    wm.report_agent_state(1, None);
    wm.update_agent_states(Instant::now());
    apply(&mut wm, AgentsListOp::Filter(Some(ListFilter::Agents)));
    assert_eq!(wm.agents_list().selected, None);
}

#[test]
fn step_first_last_page_scroll() {
    let mut wm = fixture(30);
    wm.open_agents_list();
    // 10 rows high -> 6 visible rows (title, header, footer, bottom frame).
    apply(&mut wm, AgentsListOp::Size(Some((36, 10))));
    let vis = 6usize;
    let inside = |wm: &WindowManager| {
        let s = wm.agents_list();
        let idx = ids(wm).iter().position(|&i| Some(i) == s.selected).unwrap();
        idx >= s.scroll && idx < s.scroll + vis
    };
    apply(&mut wm, AgentsListOp::First);
    let first = ids(&wm)[0];
    assert_eq!(wm.agents_list().selected, Some(first));
    apply(&mut wm, AgentsListOp::Step(-1));
    assert_eq!(wm.agents_list().selected, Some(first), "clamps at the top");
    for _ in 0..8 {
        apply(&mut wm, AgentsListOp::Step(1));
        assert!(inside(&wm));
    }
    assert_eq!(wm.agents_list().selected, Some(first + 8));
    apply(&mut wm, AgentsListOp::Page(1));
    assert_eq!(wm.agents_list().selected, Some(first + 8 + (vis - 1)));
    assert!(inside(&wm));
    apply(&mut wm, AgentsListOp::Page(-1));
    assert_eq!(wm.agents_list().selected, Some(first + 8));
    apply(&mut wm, AgentsListOp::Last);
    let last = *ids(&wm).last().unwrap();
    assert_eq!(wm.agents_list().selected, Some(last));
    assert!(inside(&wm));
    apply(&mut wm, AgentsListOp::Step(1));
    assert_eq!(wm.agents_list().selected, Some(last), "clamps at the bottom");
    // Scroll clamps to [0, rows - visible] and leaves the selection alone.
    apply(&mut wm, AgentsListOp::Scroll(-1000));
    assert_eq!(wm.agents_list().scroll, 0);
    assert_eq!(wm.agents_list().selected, Some(last));
    apply(&mut wm, AgentsListOp::Scroll(1000));
    assert_eq!(wm.agents_list().scroll, 30 - vis);
    apply(&mut wm, AgentsListOp::Scroll(-3));
    assert_eq!(wm.agents_list().scroll, 30 - vis - 3);
    // Stepping afterwards brings the selection back into view.
    apply(&mut wm, AgentsListOp::Step(-1));
    assert!(inside(&wm));
}

#[test]
fn select_pane_unknown_reports() {
    let mut wm = fixture(3);
    // A closed list opens on a known id.
    assert!(!wm.agents_list().active);
    assert_eq!(apply(&mut wm, AgentsListOp::SelectPane(2)), None);
    assert!(wm.agents_list().active);
    assert_eq!(wm.agents_list().selected, Some(2));
    let msg = apply(&mut wm, AgentsListOp::SelectPane(99));
    assert!(msg.as_deref().is_some_and(|m| m.contains("No agent in pane 99")), "{msg:?}");
    assert_eq!(wm.agents_list().selected, Some(2));
}

#[test]
fn tick_reports_change_only_when_rows_change() {
    let mut wm = fixture(3);
    assert!(!wm.agents_list().active);
    wm.report_agent_state(1, Some(AgentState::Blocked));
    assert!(!tick(&mut wm, 10), "closed list adds nothing (drawer hidden)");
    wm.open_agents_list();
    let _ = tick(&mut wm, 20);
    assert!(!tick(&mut wm, 30), "nothing changed");
    assert!(!tick(&mut wm, 40), "still nothing changed");
    wm.report_agent_state(2, Some(AgentState::Errored));
    assert!(tick(&mut wm, 50), "a state changed");
    assert!(!tick(&mut wm, 60));
    wm.split_focused(SplitDirection::Vertical).unwrap();
    assert!(tick(&mut wm, 70), "a pane was added");
    assert!(!tick(&mut wm, 80));
    wm.close_agents_list();
    wm.report_agent_state(1, Some(AgentState::Done));
    assert!(!tick(&mut wm, 90), "closed again");
}

#[test]
fn open_selects_active_pane() {
    let mut wm = fixture(3);
    let active = wm.active_pane_id();
    wm.open_agents_list();
    assert_eq!(wm.agents_list().selected, Some(active));
    wm.close_agents_list();
    // Active pane filtered out: the first row.
    wm.report_agent_state(1, Some(AgentState::Working));
    apply(&mut wm, AgentsListOp::Filter(Some(ListFilter::Agents)));
    assert_ne!(active, 1);
    wm.open_agents_list();
    assert_eq!(wm.agents_list().selected, Some(1));
}

#[test]
fn jump_focuses_pane_and_releases_keyboard() {
    let mut wm = fixture(3);
    wm.open_agents_list();
    apply(&mut wm, AgentsListOp::SelectPane(1));
    assert_ne!(wm.active_pane_id(), 1);
    assert_eq!(apply(&mut wm, AgentsListOp::Jump(None)), Some("Focused pane 1".to_string()));
    assert_eq!(wm.active_pane_id(), 1);
    assert!(!wm.agents_list().captures_input());
    assert!(wm.agents_list().active, "the list stays open, blurred, pinned or not (FOCUS-01)");
    // Pinned: stays open, keyboard released; explicit id works without a selection.
    wm.open_agents_list();
    apply(&mut wm, AgentsListOp::Pin(Some(true)));
    wm.agents_list_mut().selected = None;
    assert_eq!(apply(&mut wm, AgentsListOp::Jump(Some(2))), Some("Focused pane 2".to_string()));
    assert_eq!(wm.active_pane_id(), 2);
    assert!(wm.agents_list().active && wm.agents_list().pinned);
    assert!(!wm.agents_list().captures_input());
    // Unknown id: message, nothing changes.
    wm.open_agents_list();
    let msg = apply(&mut wm, AgentsListOp::Jump(Some(77)));
    assert!(msg.as_deref().is_some_and(|m| m.contains("No agent in pane 77")), "{msg:?}");
    assert_eq!(wm.active_pane_id(), 2);
    assert!(wm.agents_list().captures_input());
}

#[test]
fn zoom_opens_traffic_window_on_pane() {
    let mut wm = fixture(3);
    wm.open_agents_list();
    apply(&mut wm, AgentsListOp::SelectPane(2));
    assert_eq!(apply(&mut wm, AgentsListOp::Zoom(None)), None);
    assert!(wm.agents_window().active && wm.agents_window().focused);
    assert_eq!(wm.agents_window().selection.node, Some(NodeKey::pane(2)));
    assert!(wm.agents_list().active, "the list stays open");
    assert!(!wm.agents_list().captures_input());
    wm.open_agents_list();
    assert_eq!(apply(&mut wm, AgentsListOp::Zoom(Some(3))), None);
    assert_eq!(wm.agents_window().selection.node, Some(NodeKey::pane(3)));
    assert!(wm.agents_window().captures_input() && !wm.agents_list().captures_input());
    let msg = apply(&mut wm, AgentsListOp::Zoom(Some(55)));
    assert!(msg.as_deref().is_some_and(|m| m.contains("No agent in pane 55")), "{msg:?}");
}

#[test]
fn pin_dismiss_blur_semantics() {
    let mut wm = fixture(2);
    wm.open_agents_list();
    let msg = apply(&mut wm, AgentsListOp::Pin(Some(true)));
    assert_eq!(
        msg.as_deref(),
        Some("Agents list pinned (Esc returns focus to panes; :agents list unpin unpins)")
    );
    assert!(wm.agents_list().pinned);
    apply(&mut wm, AgentsListOp::Dismiss);
    assert!(wm.agents_list().active && !wm.agents_list().focused, "pinned: only blurs");
    // Open retakes the keyboard.
    apply(&mut wm, AgentsListOp::Open);
    assert!(wm.agents_list().captures_input());
    apply(&mut wm, AgentsListOp::Blur);
    assert!(wm.agents_list().active && !wm.agents_list().focused);
    apply(&mut wm, AgentsListOp::Open);
    assert!(wm.agents_list().captures_input());
    // Toggle form and unpin.
    assert_eq!(apply(&mut wm, AgentsListOp::Pin(None)).as_deref(), Some("Agents list unpinned"));
    assert!(!wm.agents_list().pinned);
    apply(&mut wm, AgentsListOp::Dismiss);
    assert!(!wm.agents_list().active, "unpinned: closes");
    apply(&mut wm, AgentsListOp::Pin(None));
    assert!(!wm.agents_list().pinned, "closed list cannot be pinned");
}

#[test]
fn only_one_window_owns_keyboard() {
    let mut wm = fixture(3);
    wm.open_agents_window(None);
    assert!(wm.agents_window().captures_input());
    wm.open_agents_list();
    assert!(wm.agents_list().captures_input() && !wm.agents_window().captures_input());
    wm.open_agents_window(None);
    assert!(wm.agents_window().captures_input() && !wm.agents_list().captures_input());
    wm.open_agents_list();
    wm.focus_and_raise(2);
    assert!(!wm.agents_list().captures_input() && !wm.agents_window().captures_input());
    wm.open_agents_list();
    wm.shuffle_deck_next();
    assert!(!wm.agents_list().captures_input() && !wm.agents_window().captures_input());
}

#[test]
fn scale_step_is_independent() {
    let mut wm = fixture(2);
    wm.open_agents_list();
    let before: Vec<u16> = [PanelTarget::Drawer, PanelTarget::Mcp, PanelTarget::Agents]
        .iter()
        .map(|t| wm.scale_setting(*t))
        .collect();
    let msg = apply(&mut wm, AgentsListOp::ScaleStep(10));
    assert_eq!(msg.as_deref(), Some("Panel scale: 110% (applies in the GUI only)"));
    wm.set_gui_surface(true);
    let msg = apply(&mut wm, AgentsListOp::ScaleStep(10));
    assert_eq!(msg.as_deref(), Some("Panel scale: 120%"));
    assert_eq!(wm.scale_setting(PanelTarget::AgentsList), 120);
    let after: Vec<u16> = [PanelTarget::Drawer, PanelTarget::Mcp, PanelTarget::Agents]
        .iter()
        .map(|t| wm.scale_setting(*t))
        .collect();
    assert_eq!(before, after);
    apply(&mut wm, AgentsListOp::ScaleStep(1000));
    assert_eq!(wm.scale_setting(PanelTarget::AgentsList), 200);
    apply(&mut wm, AgentsListOp::ScaleStep(-1000));
    assert_eq!(wm.scale_setting(PanelTarget::AgentsList), 75);
}

#[test]
fn geometry_ops_clamp_and_undock() {
    let mut wm = fixture(3);
    wm.open_agents_list();
    let ws = area();
    let check = |wm: &WindowManager| {
        let r = wm.agents_list_rect(ws).expect("rect");
        assert!(r.x + r.width <= ws.width && r.y + r.height <= ws.height, "{r:?}");
        assert!(r.width >= 20 && r.height >= 6, "{r:?}");
        r
    };
    let base = check(&wm);
    apply(&mut wm, AgentsListOp::Move { dx: -5, dy: 2 });
    let r = check(&wm);
    assert_eq!((r.x, r.y), (base.x - 5, base.y + 2));
    for (dx, dy) in [(i16::MAX, i16::MAX), (i16::MIN, i16::MIN)] {
        apply(&mut wm, AgentsListOp::Move { dx, dy });
        check(&wm);
    }
    for (dw, dh) in [(i16::MIN, i16::MIN), (i16::MAX, i16::MAX), (3, -1)] {
        apply(&mut wm, AgentsListOp::Resize { dw, dh });
        check(&wm);
    }
    apply(&mut wm, AgentsListOp::SetRect { x: 500, y: 500, w: 3, h: 1 });
    check(&wm);
    apply(&mut wm, AgentsListOp::SetRect { x: 0, y: 0, w: u16::MAX, h: u16::MAX });
    let r = check(&wm);
    assert!(r.width <= ws.width && r.height <= ws.height);
    apply(&mut wm, AgentsListOp::Size(Some((50, 12))));
    let r = check(&wm);
    assert_eq!((r.width, r.height), (50, 12));
    assert!(wm.agents_list().rect.is_some());
    apply(&mut wm, AgentsListOp::Size(None));
    assert!(wm.agents_list().rect.is_none());
    assert_eq!(check(&wm), base);
    // Every manual geometry op clears docked.
    let ops = [
        AgentsListOp::Move { dx: 1, dy: 0 },
        AgentsListOp::Resize { dw: 1, dh: 0 },
        AgentsListOp::SetRect { x: 1, y: 1, w: 30, h: 8 },
        AgentsListOp::Size(Some((30, 8))),
        AgentsListOp::Size(None),
    ];
    for op in ops {
        apply(&mut wm, AgentsListOp::Dock(Some(true)));
        assert!(wm.agents_list().docked, "{op:?}");
        apply(&mut wm, op.clone());
        assert!(!wm.agents_list().docked, "{op:?} must undock");
    }
}

#[test]
fn dock_op_uses_seam_api() {
    let mut wm = fixture(3);
    // Closed list: a message, no change.
    let msg = apply(&mut wm, AgentsListOp::Dock(Some(true)));
    assert_eq!(msg.as_deref(), Some("Agents list is not open"));
    assert!(!wm.agents_list().docked);
    wm.open_agents_list();
    apply(&mut wm, AgentsListOp::Dock(Some(true)));
    assert!(wm.agents_list().docked);
    apply(&mut wm, AgentsListOp::Dock(None));
    assert!(!wm.agents_list().docked, "toggle off");
    apply(&mut wm, AgentsListOp::Dock(None));
    assert!(wm.agents_list().docked, "toggle on");
    apply(&mut wm, AgentsListOp::Dock(Some(false)));
    assert!(!wm.agents_list().docked);
    // Closing releases the dock.
    apply(&mut wm, AgentsListOp::Dock(Some(true)));
    apply(&mut wm, AgentsListOp::Close);
    assert!(!wm.agents_list().docked);
    assert!(!wm.agents_list_summary().docked);
}

#[test]
fn closed_list_ops_report() {
    let mut wm = fixture(2);
    let ops = [
        AgentsListOp::Jump(None),
        AgentsListOp::Jump(Some(1)),
        AgentsListOp::Move { dx: 1, dy: 1 },
        AgentsListOp::Resize { dw: 1, dh: 1 },
        AgentsListOp::SetRect { x: 0, y: 0, w: 30, h: 8 },
        AgentsListOp::Size(Some((30, 8))),
        AgentsListOp::Step(1),
        AgentsListOp::Scroll(1),
        AgentsListOp::Zoom(None),
    ];
    for op in ops {
        let msg = apply(&mut wm, op.clone());
        assert_eq!(msg.as_deref(), Some("Agents list is not open"), "{op:?}");
        assert!(!wm.agents_list().active);
    }
}
