//! Phase 50.1-08: key and mouse handling of the Find window. Every gesture lowers to
//! `Action::Find(..)` and equals its `:finder` colon twin (UNI-R01, ARCH-R01).

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::config::BiltermConfig;
use bilterm::input::find_modal::{lower_find_mouse, list_viewport_start};
use bilterm::input::keymap::{FIND_MODAL_BINDINGS, find_modal_key_action};
use bilterm::input::{AppAction, handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::find_ui::{FindLayout, find_layout};
use bilterm::wm::find_cmd::{FindKind, FindOp};
use bilterm::wm::registry::{FindResults, FindRow};
use bilterm::wm::{SplitDirection, WindowManager, inset_window_frame};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

const COLS: u16 = 100;
const ROWS: u16 = 30;

fn fixture() -> WindowManager {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(Rect::new(0, 0, COLS, ROWS)).unwrap();
    wm
}

fn parse(cmd: &str) -> Action {
    parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"))
}

fn press(wm: &mut WindowManager, code: KeyCode, mods: KeyModifiers) -> AppAction {
    handle_wm_key_event(wm, KeyEvent::new(code, mods)).unwrap()
}

fn ev(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent { kind, column: col, row, modifiers: KeyModifiers::NONE }
}

fn down(col: u16, row: u16) -> MouseEvent {
    ev(MouseEventKind::Down(MouseButton::Left), col, row)
}

/// The renderer's layout for the 100x30 fixture (same rects `handle_find_modal_mouse` derives).
fn layout_of(wm: &WindowManager) -> FindLayout {
    let outer = wm.active_modal_rect(Rect::new(0, 0, COLS, ROWS)).expect("find modal rect");
    find_layout(inset_window_frame(outer), wm.config())
}

fn centre(r: Rect) -> (u16, u16) {
    (r.x + r.width / 2, r.y)
}

fn fake_results(n: usize, kind: FindKind) -> FindResults {
    let rows = (0..n).map(|i| {
        let mut r = FindRow::empty(kind);
        r.id = format!("row:{i}");
        r.label = format!("label {i}");
        r
    });
    FindResults { kind, query: String::new(), rows: rows.collect(), total: n, truncated: false, masked: 0 }
}

/// Opens Find on `Agent` with query `ab` and 5 synthetic result rows.
fn open_with_rows(rows: usize) -> WindowManager {
    let mut wm = fixture();
    wm.apply(parse(":find agent ab")).unwrap();
    wm.set_find_results(fake_results(rows, FindKind::Agent));
    wm
}

fn lower(wm: &WindowManager, mouse: MouseEvent, rows: usize) -> Option<Vec<Action>> {
    let (kind, _, selected) = wm.find_modal_state().unwrap();
    lower_find_mouse(mouse, &layout_of(wm), rows, selected, kind)
}

fn find(op: FindOp) -> Action {
    Action::Find(op)
}

// ---- Task 1: lowering --------------------------------------------------------------------------

#[test]
fn click_on_tab_chip_lowers_to_set_kind() {
    let wm = open_with_rows(5);
    let l = layout_of(&wm);
    for (i, kind) in [FindKind::Agent, FindKind::Session, FindKind::History].into_iter().enumerate() {
        let (c, r) = centre(l.tab_chips[i]);
        assert_eq!(lower(&wm, down(c, r), 5), Some(vec![find(FindOp::SetKind(kind))]));
    }
}

#[test]
fn lowering_covers_clear_rows_hover_wheel_and_footer() {
    let wm = open_with_rows(5);
    let l = layout_of(&wm);
    let (c, r) = centre(l.clear_chip);
    assert_eq!(lower(&wm, down(c, r), 5), Some(vec![find(FindOp::ClearQuery)]));

    // List row 3 (viewport 0): click selects then activates.
    let row3 = down(l.list.x + 4, l.list.y + 3);
    assert_eq!(lower(&wm, row3, 5), Some(vec![find(FindOp::Select(3)), find(FindOp::Activate)]));
    // Hover over row 2 selects it; hover over the already selected row applies nothing.
    let hover = |row: u16| ev(MouseEventKind::Moved, l.list.x + 4, l.list.y + row);
    assert_eq!(lower(&wm, hover(2), 5), Some(vec![find(FindOp::Select(2))]));
    assert_eq!(lower(&wm, hover(0), 5), Some(vec![]));
    // Wheel.
    let wheel = |k| ev(k, l.list.x + 2, l.list.y + 1);
    assert_eq!(lower(&wm, wheel(MouseEventKind::ScrollDown), 5), Some(vec![find(FindOp::Move(3))]));
    assert_eq!(lower(&wm, wheel(MouseEventKind::ScrollUp), 5), Some(vec![find(FindOp::Move(-3))]));
    // Footer chips: jump, kind, prefill, close.
    let expect = [find(FindOp::Activate), find(FindOp::CycleKind(true)), find(FindOp::Prefill), find(FindOp::Close)];
    for (chip, want) in l.footer_chips.iter().zip(expect) {
        let (c, r) = centre(*chip);
        assert_eq!(lower(&wm, down(c, r), 5), Some(vec![want]));
    }
}

#[test]
fn clicks_outside_the_interactive_rects_are_left_to_the_generic_modal_logic() {
    let wm = open_with_rows(5);
    let l = layout_of(&wm);
    let outer = wm.active_modal_rect(Rect::new(0, 0, COLS, ROWS)).unwrap();
    // Title row and border.
    assert_eq!(lower(&wm, down(outer.x + 10, outer.y), 5), None);
    assert_eq!(lower(&wm, down(outer.x, outer.y + 4), 5), None);
    // Outside the window.
    assert_eq!(lower(&wm, down(0, 0), 5), None);
    // Empty list area below the last row (5 rows, click on row 6).
    assert_eq!(lower(&wm, down(l.list.x + 2, l.list.y + 6), 5), None);
    // Query text area and the source line are not interactive.
    assert_eq!(lower(&wm, down(l.query.x + 2, l.query.y), 5), None);
    assert_eq!(lower(&wm, down(l.source_line.x + 2, l.source_line.y), 5), None);
    // Button-up and right clicks are not lowered.
    assert_eq!(lower(&wm, ev(MouseEventKind::Up(MouseButton::Left), l.list.x + 2, l.list.y), 5), None);
    assert_eq!(lower(&wm, ev(MouseEventKind::Down(MouseButton::Right), l.list.x + 2, l.list.y), 5), None);
}

#[test]
fn clicked_row_index_matches_the_rendered_viewport() {
    let mut wm = open_with_rows(40);
    let l = layout_of(&wm);
    let list_rows = usize::from(l.list.height);
    assert!(list_rows > 1 && list_rows < 40);
    wm.apply(find(FindOp::Select(35))).unwrap();
    let start = list_viewport_start(35, list_rows);
    assert_eq!(start, 35 - (list_rows - 1), "same formula as the renderer in find_ui.rs");
    for y in 0..list_rows {
        let got = lower(&wm, down(l.list.x + 1, l.list.y + y as u16), 40);
        assert_eq!(got, Some(vec![find(FindOp::Select(start + y)), find(FindOp::Activate)]), "row {y}");
    }
}

#[test]
fn handler_applies_lowered_actions_and_tracks_selection() {
    let mut wm = open_with_rows(5);
    let area = Rect::new(0, 0, COLS, ROWS);
    let l = layout_of(&wm);
    handle_wm_mouse_event(&mut wm, ev(MouseEventKind::Moved, l.list.x + 4, l.list.y + 2), area).unwrap();
    assert_eq!(wm.find_modal_state().unwrap().2, 2, "hover selects");
    handle_wm_mouse_event(&mut wm, ev(MouseEventKind::ScrollDown, l.list.x + 4, l.list.y + 2), area).unwrap();
    assert_eq!(wm.find_modal_state().unwrap().2, 4, "wheel moves 3 (clamped to the last row)");
    let (c, r) = centre(l.tab_chips[2]);
    handle_wm_mouse_event(&mut wm, down(c, r), area).unwrap();
    assert_eq!(wm.find_modal_state().unwrap().0, FindKind::History, "tab chip click switches kind");
    let (c, r) = centre(l.clear_chip);
    handle_wm_mouse_event(&mut wm, down(c, r), area).unwrap();
    assert_eq!(wm.find_modal_state().unwrap().1, "", "[clear] empties the query");
    let (c, r) = centre(l.footer_chips[3]);
    handle_wm_mouse_event(&mut wm, down(c, r), area).unwrap();
    assert!(!wm.is_find_visible(), "[Esc close] chip closes");
}

#[test]
fn title_bar_drag_still_works_through_the_generic_path() {
    let mut wm = open_with_rows(5);
    let area = Rect::new(0, 0, COLS, ROWS);
    let outer = wm.active_modal_rect(area).unwrap();
    // The first mouse event on the title row is not consumed by Find (generic move/badge logic).
    handle_wm_mouse_event(&mut wm, down(outer.x + outer.width / 2, outer.y), area).unwrap();
    assert!(wm.is_find_visible(), "a title press alone does not close the window");
}

// ---- Task 1: keys ------------------------------------------------------------------------------

#[test]
fn typing_backspace_esc_and_swallowing() {
    let mut wm = fixture();
    wm.apply(parse(":find session")).unwrap();
    for c in "abc".chars() {
        assert_eq!(press(&mut wm, KeyCode::Char(c), KeyModifiers::NONE), AppAction::Handled);
    }
    assert_eq!(wm.find_modal_state(), Some((FindKind::Session, "abc", 0)));
    press(&mut wm, KeyCode::Backspace, KeyModifiers::NONE);
    assert_eq!(wm.find_modal_state().unwrap().1, "ab");
    // T-50.1-16: unmapped keys are swallowed and change nothing.
    for (code, mods) in [
        (KeyCode::F(5), KeyModifiers::NONE),
        (KeyCode::Char('c'), KeyModifiers::CONTROL),
        (KeyCode::Char('x'), KeyModifiers::ALT),
        (KeyCode::Left, KeyModifiers::NONE),
    ] {
        assert_eq!(press(&mut wm, code, mods), AppAction::Handled, "{code:?}");
        assert_eq!(wm.find_modal_state(), Some((FindKind::Session, "ab", 0)), "{code:?}");
    }
    // The leader chord is not special-cased: `Ctrl+B` (and any follow-up key) never leaves the modal.
    press(&mut wm, KeyCode::Char('b'), KeyModifiers::CONTROL);
    assert!(wm.is_find_visible());
    press(&mut wm, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!wm.is_find_visible());
}

#[test]
fn ctrl_alt_f_closes_the_window() {
    let mut wm = fixture();
    wm.apply(parse(":find agent")).unwrap();
    press(&mut wm, KeyCode::Char('f'), KeyModifiers::CONTROL | KeyModifiers::ALT);
    assert!(!wm.is_find_visible());
}

#[test]
fn no_direct_modal_mutation_in_the_handler() {
    let src = include_str!("../src/input/find_modal.rs");
    assert!(!src.contains("modal_mut()"));
    assert!(src.contains("find_layout"));
}

// ---- Task 2: parity and entry paths ------------------------------------------------------------

fn key(code: KeyCode, mods: KeyModifiers) -> Action {
    find_modal_key_action(KeyEvent::new(code, mods)).unwrap_or_else(|| panic!("unbound {code:?}"))
}

/// Applies `actions` to a fresh Find window (Agent, query `ab`, 5 rows) and returns what is left.
fn outcome(actions: &[Action]) -> (bool, Option<(FindKind, String, usize)>) {
    let mut wm = open_with_rows(5);
    for a in actions {
        wm.apply(a.clone()).unwrap();
    }
    let state = wm.find_modal_state().map(|(k, q, s)| (k, q.to_string(), s));
    (wm.is_find_visible(), state)
}

/// Runs a gesture row: equal resulting state for all three paths, equal `Action`s when `strict`.
fn assert_parity(name: &str, keys: &[Action], mouse: &[Action], colon: &[Action], strict: bool) {
    let (k, m, c) = (outcome(keys), outcome(mouse), outcome(colon));
    assert_eq!(k, c, "{name}: key vs colon outcome");
    assert_eq!(m, c, "{name}: mouse vs colon outcome");
    if strict {
        assert_eq!(keys, colon, "{name}: key vs colon action");
        assert_eq!(mouse, colon, "{name}: mouse vs colon action");
    }
}

#[test]
fn mouse_and_key_and_colon_reach_the_same_action() {
    let wm = open_with_rows(5);
    let l = layout_of(&wm);
    let m = |ev: MouseEvent| lower(&wm, ev, 5).unwrap();
    let chip = |r: Rect| {
        let (c, y) = centre(r);
        down(c, y)
    };
    let none = KeyModifiers::NONE;
    let ctrl = KeyModifiers::CONTROL;

    // kind switch: Tab from Agent -> Session; chip 1 sets Session; `:finder kind next` cycles forward.
    assert_parity(
        "kind",
        &[key(KeyCode::Tab, none)],
        &m(chip(l.tab_chips[1])),
        &[parse(":finder kind next")],
        false,
    );
    assert_eq!(m(chip(l.footer_chips[1])), vec![parse(":finder kind next")], "[Tab kind] chip");
    // clear
    assert_parity(
        "clear",
        &[key(KeyCode::Char('u'), ctrl)],
        &m(chip(l.clear_chip)),
        &[parse(":finder query")],
        true,
    );
    // move: wheel is 3 rows; `:finder move +3`; keys Down x3.
    let down1 = key(KeyCode::Down, none);
    assert_parity(
        "move",
        &[down1.clone(), down1.clone(), down1.clone()],
        &m(ev(MouseEventKind::ScrollDown, l.list.x + 1, l.list.y)),
        &[parse(":finder move +3")],
        false,
    );
    assert_eq!(m(ev(MouseEventKind::ScrollDown, l.list.x + 1, l.list.y)), vec![parse(":finder move +3")]);
    assert_eq!(m(ev(MouseEventKind::ScrollUp, l.list.x + 1, l.list.y)), vec![parse(":finder move -3")]);
    // select: Down x2, hover row 2, `:finder select 2`.
    assert_parity(
        "select",
        &[down1.clone(), down1],
        &m(ev(MouseEventKind::Moved, l.list.x + 1, l.list.y + 2)),
        &[parse(":finder select 2")],
        false,
    );
    assert_eq!(m(ev(MouseEventKind::Moved, l.list.x + 1, l.list.y + 2)), vec![parse(":finder select 2")]);
    // jump: Enter, `[Enter jump]` chip, `:finder enter`; a row click is select + enter.
    assert_parity("jump", &[key(KeyCode::Enter, none)], &m(chip(l.footer_chips[0])), &[parse(":finder enter")], true);
    assert_eq!(
        m(down(l.list.x + 1, l.list.y + 3)),
        vec![parse(":finder select 3"), parse(":finder enter")],
        "row click"
    );
    // prefill
    assert_parity("prefill", &[key(KeyCode::Char('y'), ctrl)], &m(chip(l.footer_chips[2])), &[parse(":finder prefill")], true);
    // close
    assert_parity("close", &[key(KeyCode::Esc, none)], &m(chip(l.footer_chips[3])), &[parse(":finder close")], true);
}

/// `:finder` text for an op (the colon twin of a key-table row).
fn finder_colon_form(op: &FindOp) -> String {
    match op {
        FindOp::Close => ":finder close".into(),
        FindOp::Activate => ":finder enter".into(),
        FindOp::Prefill => ":finder prefill".into(),
        FindOp::CycleKind(true) => ":finder kind next".into(),
        FindOp::CycleKind(false) => ":finder kind prev".into(),
        FindOp::Move(i16::MIN) => ":finder move top".into(),
        FindOp::Move(i16::MAX) => ":finder move bottom".into(),
        FindOp::Move(n) => format!(":finder move {n:+}"),
        // Text editing: the committed twin is `:finder query <text>` (empty clears; Pop/Push are
        // incremental edits whose result a SetQuery reproduces).
        FindOp::ClearQuery => ":finder query".into(),
        FindOp::Pop | FindOp::Push(_) => ":finder query abc".into(),
        other => panic!("no colon form for {other:?}"),
    }
}

#[test]
fn key_table_rows_have_colon_forms() {
    for (_, _, action) in FIND_MODAL_BINDINGS {
        let Action::Find(op) = action else { panic!("{action:?} is not a Find op") };
        let text = finder_colon_form(op);
        match parse(&text) {
            Action::Find(parsed) => {
                if !matches!(op, FindOp::Pop | FindOp::Push(_)) {
                    assert_eq!(&parsed, op, "{text}");
                }
            }
            other => panic!("{text} parsed to {other:?}"),
        }
    }
    assert_eq!(find_modal_key_action(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::SHIFT)), Some(find(FindOp::Push('z'))));
}

#[test]
fn entry_paths_open_the_window() {
    let chord = KeyModifiers::CONTROL | KeyModifiers::ALT;
    let mut wm = fixture();
    press(&mut wm, KeyCode::Char('f'), chord);
    assert_eq!(wm.find_modal_state().map(|s| s.0), Some(FindKind::Agent));
    press(&mut wm, KeyCode::Char('f'), chord);
    assert!(!wm.is_find_visible(), "second chord closes");

    // Also with the active pane in raw PTY passthrough.
    let mut wm = fixture();
    wm.active_pane_mut().toggle_manual_raw_override();
    assert_eq!(
        wm.active_pane().effective_routing_mode(),
        bilterm::pty::line_discipline::InputRoutingMode::RawPtyPassthrough,
        "fixture must be in raw passthrough"
    );
    press(&mut wm, KeyCode::Char('f'), chord);
    assert!(wm.is_find_visible(), "Ctrl+Alt+F opens over a raw pane");
    press(&mut wm, KeyCode::Char('f'), chord);
    assert!(!wm.is_find_visible(), "and closes again");
}

#[test]
fn config_default_layout_is_stable() {
    // Guards the 100x30 geometry the table above hard-codes.
    let wm = open_with_rows(5);
    let l = layout_of(&wm);
    assert!(l.list.height >= 6, "list needs at least 6 rows for the click-below-last-row case");
    let _ = BiltermConfig::default();
}
