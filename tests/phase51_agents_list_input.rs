//! Phase 51-07: keys and mouse of the AGENTS list window (UIT-03) and the demo leader chords
//! (UIT-01). Every gesture lowers to `Action::AgentsList(..)` through `WindowManager::apply`
//! (UNI-R01); the pure mouse planner lets these tests assert exactly which Action each gesture
//! produces without depending on the state machine of 51-05.

use std::sync::Mutex;

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::input::agents_list::{ListMousePlan, plan_agents_list_mouse};
use bilterm::input::agents_list_keys::{AGENTS_LIST_BINDINGS, agents_list_action};
use bilterm::input::gui_route::{GuiWheelRoute, route_gui_wheel};
use bilterm::input::keymap::{default_global_action, default_leader_action};
use bilterm::input::{
    DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS, DEFAULT_PANE_BINDINGS, handle_wm_key_event,
    handle_wm_mouse_event,
};
use bilterm::ui::shell::ShellLayout;
use bilterm::ui::typography::PanelTarget;
use bilterm::wm::agents_cmd::AgentsOp;
use bilterm::wm::agents_list::ListDrag;
use bilterm::wm::agents_list_cmd::AgentsListOp;
use bilterm::wm::agents_list_geometry::AgentsListRects;
use bilterm::wm::{SplitDirection, WindowManager, WindowResizeEdge};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

static ENV_LOCK: Mutex<()> = Mutex::new(());
const CTRL_ALT: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::ALT);
const AREA: Rect = Rect { x: 0, y: 0, width: 120, height: 40 };

/// Isolates config/state directories for tests that drive real key dispatch.
fn with_temp_xdg<T>(f: impl FnOnce() -> T) -> T {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!(
        "bilterm-agents-list-input-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Serialized by ENV_LOCK.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("XDG_STATE_HOME", &dir);
        std::env::set_var("XDG_DATA_HOME", &dir);
    }
    let out = f();
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// A headless 120x40 manager with `panes` panes (list closed).
fn fixture(panes: usize) -> WindowManager {
    let mut wm = WindowManager::new_headless(AREA.height, AREA.width);
    for _ in 1..panes {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.resize_all(AREA).unwrap();
    wm
}

fn press(wm: &mut WindowManager, code: KeyCode, mods: KeyModifiers) {
    handle_wm_key_event(wm, KeyEvent::new(code, mods)).unwrap();
}

fn prompts(wm: &WindowManager) -> Vec<String> {
    wm.panes().values().map(|p| p.prompt().text()).collect()
}

fn list_action(op: AgentsListOp) -> Action {
    Action::AgentsList(op)
}

// ---------------------------------------------------------------- keys

#[test]
fn key_global_chord_toggles_list() {
    with_temp_xdg(|| {
        let mut wm = fixture(2);
        for code in [KeyCode::Char('n'), KeyCode::Char('N')] {
            assert_eq!(
                default_global_action(KeyEvent::new(code, CTRL_ALT), false),
                Some(list_action(AgentsListOp::Toggle))
            );
        }
        assert_eq!(
            default_global_action(KeyEvent::new(KeyCode::Char('N'), CTRL_ALT | KeyModifiers::SHIFT), false),
            Some(list_action(AgentsListOp::Toggle)),
            "the Shift form resolves too"
        );
        press(&mut wm, KeyCode::Char('n'), CTRL_ALT);
        assert!(wm.agents_list().active && wm.agents_list().focused, "chord opens the list");
        press(&mut wm, KeyCode::Char('n'), CTRL_ALT);
        assert!(!wm.agents_list().active, "chord closes it again while focused");
        press(&mut wm, KeyCode::Char('N'), CTRL_ALT | KeyModifiers::SHIFT);
        assert!(wm.agents_list().active, "Ctrl+Alt+Shift+N opens it");
        press(&mut wm, KeyCode::Char('N'), CTRL_ALT | KeyModifiers::SHIFT);
        assert!(!wm.agents_list().active);
    });
}

#[test]
fn key_leader_chords() {
    with_temp_xdg(|| {
        let mut wm = fixture(2);
        press(&mut wm, KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(wm.is_leader_pending());
        press(&mut wm, KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert!(!wm.is_leader_pending());
        assert!(wm.agents_list().active, "leader A opens the list");
        // Focused list: the leader key is not consumed, so the chord still toggles it closed.
        press(&mut wm, KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(wm.is_leader_pending(), "the focused list never eats the leader key");
        press(&mut wm, KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert!(!wm.agents_list().active, "leader A closes it again");

        let colon = |s: &str| parse_colon_command(s).unwrap().unwrap();
        let leader = |c: char| default_leader_action(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        assert_eq!(leader('A'), Some(list_action(AgentsListOp::Toggle)));
        assert_eq!(leader('y'), Some(Action::Agents(AgentsOp::Demo)));
        assert_eq!(leader('Y'), Some(Action::Agents(AgentsOp::DemoStop)));
        assert_eq!(leader('y'), Some(colon(":agents demo")));
        assert_eq!(leader('Y'), Some(colon(":agents demo stop")));
        assert_eq!(leader('A'), Some(colon(":agents list toggle")));
    });
}

#[test]
fn key_chord_works_in_raw_passthrough() {
    with_temp_xdg(|| {
        let mut wm = fixture(1);
        wm.active_pane_mut().toggle_manual_raw_override();
        assert_eq!(
            wm.active_pane().effective_routing_mode(),
            bilterm::pty::line_discipline::InputRoutingMode::RawPtyPassthrough,
            "fixture must be in raw passthrough"
        );
        let before = prompts(&wm);
        press(&mut wm, KeyCode::Char('n'), CTRL_ALT);
        assert!(wm.agents_list().active, "Ctrl+Alt+N reaches the window manager over a raw pane");
        assert_eq!(prompts(&wm), before, "nothing was typed into the pane");
        press(&mut wm, KeyCode::Char('n'), CTRL_ALT);
        assert!(!wm.agents_list().active);
    });
}

#[test]
fn key_table_maps_every_row() {
    let key = |mods, code| agents_list_action(KeyEvent::new(code, mods));
    let n = KeyModifiers::NONE;
    let op = AgentsListOp::clone;
    let expect: Vec<(KeyModifiers, KeyCode, AgentsListOp)> = vec![
        (n, KeyCode::Up, AgentsListOp::Step(-1)),
        (n, KeyCode::Char('k'), AgentsListOp::Step(-1)),
        (n, KeyCode::Down, AgentsListOp::Step(1)),
        (n, KeyCode::Char('j'), AgentsListOp::Step(1)),
        (n, KeyCode::PageUp, AgentsListOp::Page(-1)),
        (n, KeyCode::PageDown, AgentsListOp::Page(1)),
        (n, KeyCode::Home, AgentsListOp::First),
        (n, KeyCode::End, AgentsListOp::Last),
        (n, KeyCode::Enter, AgentsListOp::Jump(None)),
        (n, KeyCode::Char('z'), AgentsListOp::Zoom(None)),
        (n, KeyCode::Char('f'), AgentsListOp::Filter(None)),
        (n, KeyCode::Char('*'), AgentsListOp::Pin(None)),
        (n, KeyCode::Char('['), AgentsListOp::ScaleStep(-10)),
        (n, KeyCode::Char(']'), AgentsListOp::ScaleStep(10)),
        (KeyModifiers::SHIFT, KeyCode::Left, AgentsListOp::Move { dx: -1, dy: 0 }),
        (KeyModifiers::SHIFT, KeyCode::Right, AgentsListOp::Move { dx: 1, dy: 0 }),
        (KeyModifiers::SHIFT, KeyCode::Up, AgentsListOp::Move { dx: 0, dy: -1 }),
        (KeyModifiers::SHIFT, KeyCode::Down, AgentsListOp::Move { dx: 0, dy: 1 }),
        (KeyModifiers::ALT, KeyCode::Left, AgentsListOp::Resize { dw: -1, dh: 0 }),
        (KeyModifiers::ALT, KeyCode::Right, AgentsListOp::Resize { dw: 1, dh: 0 }),
        (KeyModifiers::ALT, KeyCode::Up, AgentsListOp::Resize { dw: 0, dh: -1 }),
        (KeyModifiers::ALT, KeyCode::Down, AgentsListOp::Resize { dw: 0, dh: 1 }),
        (n, KeyCode::Char('d'), AgentsListOp::Dock(None)),
        (n, KeyCode::Esc, AgentsListOp::Dismiss),
    ];
    for (mods, code, want) in &expect {
        assert_eq!(key(*mods, *code), Some(list_action(op(want))), "row {mods:?} {code:?}");
    }
    assert_eq!(AGENTS_LIST_BINDINGS.len(), expect.len(), "the table has exactly the documented rows");
    assert_eq!(key(n, KeyCode::Char('x')), None);
    assert_eq!(key(KeyModifiers::CONTROL, KeyCode::Char('d')), None, "modifiers are exact");
}

#[test]
fn key_focused_swallows_and_leader_passes() {
    with_temp_xdg(|| {
        let mut wm = fixture(2);
        wm.open_agents_list();
        assert!(wm.agents_list().captures_input());
        let before = prompts(&wm);
        press(&mut wm, KeyCode::Char('x'), KeyModifiers::NONE);
        press(&mut wm, KeyCode::Char('q'), KeyModifiers::NONE);
        press(&mut wm, KeyCode::F(12), KeyModifiers::NONE);
        assert_eq!(prompts(&wm), before, "unknown keys are swallowed, nothing reaches the pane");
        assert!(wm.agents_list().active);
        press(&mut wm, KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(wm.is_leader_pending(), "the leader key is not consumed");
        press(&mut wm, KeyCode::Esc, KeyModifiers::NONE);
        assert!(!wm.is_leader_pending());

        // Not focused (blurred): the window intercepts nothing and typing reaches the pane.
        wm.agents_list_mut().blur();
        assert!(wm.agents_list().active && !wm.agents_list().captures_input());
        press(&mut wm, KeyCode::Char('x'), KeyModifiers::NONE);
        press(&mut wm, KeyCode::Char('z'), KeyModifiers::NONE);
        assert_ne!(prompts(&wm), before, "typing reaches the pane when the list is not focused");
        assert!(wm.agents_list().active, "Esc/keys of the window did not run");
    });
}

#[test]
fn key_esc_dismiss_semantics_plan() {
    with_temp_xdg(|| {
        assert_eq!(
            agents_list_action(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(list_action(AgentsListOp::Dismiss))
        );
        // The toggle chord is handled before the table, so the focused window closes like Ctrl+Alt+A.
        let mut wm = fixture(2);
        wm.open_agents_list();
        press(&mut wm, KeyCode::Char('n'), CTRL_ALT);
        assert!(!wm.agents_list().active, "Ctrl+Alt+N while focused toggles (closes)");
        wm.open_agents_list();
        press(&mut wm, KeyCode::Esc, KeyModifiers::NONE);
        assert!(!wm.agents_list().active, "Esc dismisses an unpinned window");
    });
}

#[test]
fn key_drift_no_duplicates_no_shadow() {
    for (i, (m, c, _)) in AGENTS_LIST_BINDINGS.iter().enumerate() {
        for (m2, c2, _) in &AGENTS_LIST_BINDINGS[i + 1..] {
            assert!(!(m == m2 && c == c2), "duplicate row {m:?} {c:?}");
        }
    }
    assert!(AGENTS_LIST_BINDINGS.iter().all(|(_, _, a)| matches!(a, Action::AgentsList(_))));
    // Every row has a colon twin that parses back to the same Action.
    for (_, _, a) in AGENTS_LIST_BINDINGS {
        let Action::AgentsList(op) = a else { unreachable!() };
        let back = parse_colon_command(&op.colon_form()).expect("colon").expect("parses");
        assert_eq!(&back, a, "{} does not round-trip", op.colon_form());
    }
    // The new global rows: exactly Ctrl+Alt+n / Ctrl+Alt+N, no other global row shares the chord.
    let toggles: Vec<_> = DEFAULT_GLOBAL_BINDINGS
        .iter()
        .filter(|(_, _, a)| *a == list_action(AgentsListOp::Toggle))
        .collect();
    assert_eq!(toggles.len(), 2, "Ctrl+Alt+n and Ctrl+Alt+N");
    for (m, c, _) in &toggles {
        assert_eq!(*m, CTRL_ALT);
        let same = DEFAULT_GLOBAL_BINDINGS.iter().filter(|(gm, gc, _)| gm == m && gc == c).count();
        assert_eq!(same, 1, "global chord {m:?} {c:?} is unique");
        assert!(!DEFAULT_PANE_BINDINGS.iter().any(|(pm, pc, _)| pm == m && pc == c), "no pane collision");
    }
    // Leader rows are read live: A, y, Y each appear exactly once and `?` stays HelpToggle.
    for (ch, want) in [
        ('A', list_action(AgentsListOp::Toggle)),
        ('y', Action::Agents(AgentsOp::Demo)),
        ('Y', Action::Agents(AgentsOp::DemoStop)),
        ('?', Action::HelpToggle),
    ] {
        let rows: Vec<_> = DEFAULT_LEADER_BINDINGS.iter().filter(|(c, _)| *c == KeyCode::Char(ch)).collect();
        assert_eq!(rows.len(), 1, "leader {ch} appears once");
        assert_eq!(rows[0].1, want, "leader {ch}");
    }
}

#[test]
fn key_input_calls_only_allow_listed_wm_methods() {
    const ALLOW: &[&str] = &[
        "apply",
        "agents_list",
        "agents_list_mut",
        "agents_list_hit",
        "agents_list_rect",
        "agents_list_layout",
        "panel_space",
        "config",
        "is_leader_pending",
        "agents_window",
        "modal",
    ];
    for (name, src) in [
        ("agents_list_keys.rs", include_str!("../src/input/agents_list_keys.rs")),
        ("agents_list.rs", include_str!("../src/input/agents_list.rs")),
    ] {
        let mut rest = src;
        let mut seen = 0;
        while let Some(at) = rest.find("wm.") {
            let after = &rest[at + 3..];
            let ident: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            let is_call = after[ident.len()..].starts_with('(');
            if is_call {
                seen += 1;
                assert!(ALLOW.contains(&ident.as_str()), "{name} calls non-allow-listed wm.{ident}(..)");
            }
            rest = &after[ident.len()..];
        }
        assert!(seen > 0, "{name} should drive the manager through wm.apply");
    }
}

// ---------------------------------------------------------------- mouse

fn ws_of(wm: &WindowManager) -> Rect {
    ShellLayout::compute(AREA, wm.config(), wm.pane_count()).workspace
}

fn lay(wm: &WindowManager) -> AgentsListRects {
    wm.agents_list_layout(ws_of(wm)).expect("list layout")
}

fn ev(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent { kind, column: col, row, modifiers: KeyModifiers::NONE }
}

fn down(col: u16, row: u16) -> MouseEvent {
    ev(MouseEventKind::Down(MouseButton::Left), col, row)
}

fn plan(wm: &WindowManager, e: MouseEvent) -> ListMousePlan {
    plan_agents_list_mouse(wm, e, ws_of(wm)).expect("plan for an open list")
}

fn actions(p: &ListMousePlan) -> Vec<Action> {
    p.actions.clone()
}

fn mouse(wm: &mut WindowManager, e: MouseEvent) {
    handle_wm_mouse_event(wm, e, AREA).unwrap();
}

/// Open list with `panes` panes; returns the manager.
fn open_list(panes: usize) -> WindowManager {
    let mut wm = fixture(panes);
    wm.open_agents_list();
    wm
}

fn row_cell(wm: &WindowManager, i: usize) -> (u16, u16) {
    let l = lay(wm);
    (l.rows_area.x + 2, l.rows_area.y + i as u16)
}

fn row_id(wm: &WindowManager, i: usize) -> usize {
    wm.agents_list_rows()[i].pane_id
}

fn badge_cell(wm: &WindowManager, which: bilterm::wm::agents_list_geometry::ListBadge) -> (u16, u16) {
    let l = lay(wm);
    let (_, r) = l.badges.iter().find(|(b, _)| *b == which).expect("badge");
    (r.x, r.y)
}

#[test]
fn mouse_row_click_plans() {
    let mut wm = open_list(3);
    // A fresh open selects the active pane's row (51-05); click a different row.
    let active = wm.active_pane_id();
    assert_eq!(wm.agents_list().selected, Some(active));
    let other = (0..3).find(|&i| row_id(&wm, i) != active).expect("a row besides the active pane");
    let (c, r) = row_cell(&wm, other);
    let id = row_id(&wm, other);
    // Focused, not selected: select.
    let p = plan(&wm, down(c, r));
    assert_eq!(actions(&p), vec![list_action(AgentsListOp::SelectPane(id))]);
    assert!(p.swallow);
    // Selected row again: jump.
    wm.agents_list_mut().selected = Some(id);
    assert_eq!(actions(&plan(&wm, down(c, r))), vec![list_action(AgentsListOp::Jump(Some(id)))]);
    // Unfocused: retake first, never jump, even on the selected row.
    wm.agents_list_mut().blur();
    assert_eq!(
        actions(&plan(&wm, down(c, r))),
        vec![list_action(AgentsListOp::Open), list_action(AgentsListOp::SelectPane(id))]
    );
    // Right-click zooms that row.
    wm.open_agents_list();
    let right = ev(MouseEventKind::Down(MouseButton::Right), c, r);
    assert_eq!(actions(&plan(&wm, right)), vec![list_action(AgentsListOp::Zoom(Some(id)))]);
    // The real handler swallows the click and lowers it through apply: the selected row jumps to
    // its pane and (unpinned) the list closes (51-05 semantics).
    wm.agents_list_mut().selected = Some(id);
    mouse(&mut wm, down(c, r));
    assert_eq!(wm.active_pane_id(), id);
    assert!(!wm.agents_list().active);
}

#[test]
fn mouse_badges_and_chips_plan() {
    use bilterm::wm::agents_list_geometry::{ListBadge, ListChip};
    let wm = open_list(3);
    for (badge, want) in [
        (ListBadge::Pin, AgentsListOp::Pin(None)),
        (ListBadge::Minimize, AgentsListOp::Dismiss),
        (ListBadge::Dock, AgentsListOp::Dock(None)),
        (ListBadge::Close, AgentsListOp::Close),
    ] {
        let (c, r) = badge_cell(&wm, badge);
        let p = plan(&wm, down(c, r));
        assert_eq!(actions(&p), vec![list_action(want)], "{badge:?}");
        assert!(p.swallow && p.drag.is_none(), "{badge:?} does not start a drag");
    }
    let l = lay(&wm);
    for (chip, want) in [
        (ListChip::Filter, AgentsListOp::Filter(None)),
        (ListChip::Dock, AgentsListOp::Dock(None)),
        (ListChip::Zoom, AgentsListOp::Zoom(None)),
    ] {
        let (_, rect) = l.chips.iter().find(|(c, _)| *c == chip).expect("chip");
        assert_eq!(actions(&plan(&wm, down(rect.x, rect.y))), vec![list_action(want)], "{chip:?}");
    }
    // An unfocused click on the plain footer or body only retakes the keyboard.
    let mut wm = wm;
    wm.agents_list_mut().blur();
    assert_eq!(actions(&plan(&wm, down(l.header.x + 1, l.header.y))), vec![list_action(AgentsListOp::Open)]);
}

#[test]
fn mouse_drag_title_and_border() {
    let mut wm = open_list(3);
    let ws = ws_of(&wm);
    let outer = lay(&wm).outer;
    // Press on a plain title cell (left of the badges, away from the corner).
    let (tc, tr) = (outer.x + 3, outer.y);
    assert_eq!(wm.agents_list_hit(ws, tc, tr), bilterm::wm::agents_list_geometry::ListHit::Title);
    let p = plan(&wm, down(tc, tr));
    assert!(p.actions.is_empty());
    assert_eq!(p.drag, Some(Some(ListDrag::Move { grab_dx: 3, grab_dy: 0 })));
    // Motions keep the grab offset and the size.
    wm.agents_list_mut().drag = Some(ListDrag::Move { grab_dx: 3, grab_dy: 0 });
    let p = plan(&wm, ev(MouseEventKind::Drag(MouseButton::Left), 50, 12));
    assert_eq!(
        actions(&p),
        vec![list_action(AgentsListOp::SetRect { x: 47, y: 12, w: outer.width, h: outer.height })]
    );
    assert!(p.swallow);
    // Extreme coordinates saturate instead of overflowing (T-51-19), and apply never panics.
    let p = plan(&wm, ev(MouseEventKind::Drag(MouseButton::Left), u16::MAX, u16::MAX));
    assert_eq!(p.actions.len(), 1);
    for a in p.actions {
        let _ = wm.apply(a);
    }
    // Release clears the drag.
    let up = plan(&wm, ev(MouseEventKind::Up(MouseButton::Left), 50, 12));
    assert_eq!(up.drag, Some(None));
    assert!(up.swallow);

    // Border drag: bottom-right corner resizes through `apply_rect_edge_resize`.
    let mut wm = open_list(3);
    let outer = lay(&wm).outer;
    let (bc, br) = (outer.x + outer.width - 1, outer.y + outer.height - 1);
    let p = plan(&wm, down(bc, br));
    assert_eq!(
        p.drag,
        Some(Some(ListDrag::Resize { edge: WindowResizeEdge::BottomRight, initial: outer }))
    );
    wm.agents_list_mut().drag = Some(ListDrag::Resize { edge: WindowResizeEdge::BottomRight, initial: outer });
    let p = plan(&wm, ev(MouseEventKind::Drag(MouseButton::Left), bc + 4, br + 3));
    assert_eq!(
        actions(&p),
        vec![list_action(AgentsListOp::SetRect {
            x: outer.x,
            y: outer.y,
            w: outer.width + 4,
            h: outer.height + 3,
        })]
    );
}

#[test]
fn mouse_outside_blurs_and_passes() {
    with_temp_xdg(|| {
        let mut wm = open_list(2);
        wm.focus_and_raise(1);
        wm.open_agents_list();
        assert!(wm.agents_list().captures_input());
        // A cell inside pane 2 (left of the list, far from it).
        let ws = ws_of(&wm);
        let (oc, or) = (ws.x + ws.width - 1, ws.y + ws.height - 1);
        assert_eq!(wm.agents_list_hit(ws, oc, or), bilterm::wm::agents_list_geometry::ListHit::Outside);
        let p = plan(&wm, down(oc, or));
        assert_eq!(actions(&p), vec![list_action(AgentsListOp::Blur)]);
        assert!(p.pass_through && !p.swallow);
        mouse(&mut wm, down(oc, or));
        assert!(wm.agents_list().active && !wm.agents_list().focused, "blurred, still open");
        assert_eq!(wm.active_pane_id(), 2, "the press reached the pane under the pointer");

        // Unfocused window: an outside press plans nothing; a press inside never blurs.
        let p = plan(&wm, down(oc, or));
        assert!(p.actions.is_empty() && !p.swallow);
        wm.open_agents_list();
        let (c, r) = row_cell(&wm, 0);
        assert!(!actions(&plan(&wm, down(c, r))).contains(&list_action(AgentsListOp::Blur)));
        // Moves outside the window are not presses.
        assert!(plan(&wm, ev(MouseEventKind::Moved, oc, or)).actions.is_empty());
    });
}

#[test]
fn mouse_inside_is_swallowed() {
    with_temp_xdg(|| {
        let mut wm = open_list(2);
        let before = prompts(&wm);
        let active = wm.active_pane_id();
        let l = lay(&wm);
        for (c, r) in [(l.inner.x + 1, l.inner.y + 1), (l.outer.x + 3, l.outer.y), (l.footer.x, l.footer.y)] {
            for kind in [
                MouseEventKind::Down(MouseButton::Left),
                MouseEventKind::Up(MouseButton::Left),
                MouseEventKind::Moved,
                MouseEventKind::ScrollDown,
            ] {
                assert!(plan(&wm, ev(kind, c, r)).swallow, "{kind:?} at {c},{r} is swallowed");
                mouse(&mut wm, ev(kind, c, r));
            }
        }
        assert_eq!(prompts(&wm), before);
        assert_eq!(wm.active_pane_id(), active, "no event reached a pane beneath the list");
        // Outside both windows nothing is swallowed.
        let ws = ws_of(&wm);
        assert!(!plan(&wm, ev(MouseEventKind::Moved, ws.x, ws.y + ws.height - 1)).swallow);
        // A closed list plans nothing at all.
        wm.close_agents_list();
        assert!(plan_agents_list_mouse(&wm, down(l.inner.x + 1, l.inner.y + 1), ws_of(&wm)).is_none());
    });
}

#[test]
fn mouse_wheel_scroll_and_scale() {
    let mut wm = open_list(3);
    let (c, r) = row_cell(&wm, 0);
    assert_eq!(actions(&plan(&wm, ev(MouseEventKind::ScrollUp, c, r))), vec![list_action(AgentsListOp::Scroll(-3))]);
    assert_eq!(actions(&plan(&wm, ev(MouseEventKind::ScrollDown, c, r))), vec![list_action(AgentsListOp::Scroll(3))]);
    let ctrl = |kind| MouseEvent { kind, column: c, row: r, modifiers: KeyModifiers::CONTROL };
    assert_eq!(
        actions(&plan(&wm, ctrl(MouseEventKind::ScrollUp))),
        vec![list_action(AgentsListOp::ScaleStep(10))]
    );
    assert_eq!(
        actions(&plan(&wm, ctrl(MouseEventKind::ScrollDown))),
        vec![list_action(AgentsListOp::ScaleStep(-10))]
    );

    // At a GUI surface and scale 150 the hit-test uses the panel's own scaled space: the real cell
    // that maps onto a virtual row cell hits the same row.
    wm.set_gui_surface(true);
    wm.set_scale(PanelTarget::AgentsList, 150);
    let ws = ws_of(&wm);
    let (_, vws) = wm.panel_space(down(0, 0), ws, PanelTarget::AgentsList);
    let rows_area = wm.agents_list_layout(vws).expect("scaled layout").rows_area;
    let (vc, vr) = (rows_area.x + 2, rows_area.y + 1);
    let id = wm.agents_list_rows()[1].pane_id;
    // The real cell for the virtual cell is `ws.x + v * 150 / 100` (the inverse of `panel_space`).
    let (rc, rr) = (ws.x + vc * 3 / 2, ws.y + vr * 3 / 2);
    let p = plan(&wm, down(rc, rr));
    assert_eq!(actions(&p), vec![list_action(AgentsListOp::SelectPane(id))], "scaled click hits the same row");
}

#[test]
fn mouse_list_above_traffic_window() {
    with_temp_xdg(|| {
        let mut wm = fixture(3);
        wm.broadcast_command("echo hi").unwrap();
        wm.open_agents_window(None);
        wm.open_agents_list();
        let ws = ws_of(&wm);
        let list = wm.agents_list_rect(ws).unwrap();
        let traffic = wm.agents_window_rect(ws).unwrap();
        let overlap = list.intersection(traffic);
        assert!(overlap.area() > 0, "list {list:?} must overlap traffic {traffic:?}");
        // A cell of the overlap (inside both windows): the list takes the click.
        let cell = (overlap.x + overlap.width / 2, overlap.y + overlap.height / 2);
        assert!(traffic.contains(cell.into()) && list.contains(cell.into()));
        mouse(&mut wm, down(cell.0, cell.1));
        assert!(wm.agents_list().captures_input(), "the list got the click");
        assert!(!wm.agents_window().captures_input(), "the traffic window did not");
        // A cell of the traffic window outside the list goes to the traffic window.
        let t = (traffic.x + 1, traffic.y + traffic.height / 2);
        assert!(!list.contains(t.into()));
        mouse(&mut wm, down(t.0, t.1));
        assert!(wm.agents_window().captures_input(), "traffic-only part reaches the traffic window");
        assert!(!wm.agents_list().captures_input(), "the list was blurred by the press outside");
    });
}

#[test]
fn gui_wheel_over_list_forwards() {
    let mut wm = fixture(3);
    wm.set_gui_surface(true);
    wm.open_agents_list();
    let ws = ws_of(&wm);
    let l = wm.agents_list_rect(ws).unwrap();
    let (c, r) = (l.x + l.width / 2, l.y + l.height / 2);
    let route = route_gui_wheel(&wm, c, r, true, KeyModifiers::CONTROL, AREA);
    let GuiWheelRoute::Forward(e) = route else { panic!("expected Forward, got {route:?}") };
    assert!(e.modifiers.contains(KeyModifiers::CONTROL));
    // Elsewhere (a pane cell away from every panel) Ctrl+wheel is a font zoom.
    assert_eq!(
        route_gui_wheel(&wm, ws.x, ws.y + ws.height - 1, true, KeyModifiers::CONTROL, AREA),
        GuiWheelRoute::FontZoom { up: true }
    );
    // The forwarded event is consumed by the list handler (51-05 owns what ScaleStep does).
    handle_wm_mouse_event(&mut wm, e, AREA).unwrap();
}
