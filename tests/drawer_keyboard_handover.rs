//! `FOCUS-01` for the drawer: selecting a row hands the pane the keyboard in one step, the drawer
//! stays open, arrows browse without jumping, Esc and a pane click release it, a press on the
//! drawer takes it, and ownership is visible (`KBD:DRAWER`, inactive pane frame).

use std::sync::Mutex;

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::drawer::navigator::{navigator_footer_buttons, navigator_fold_marker_cells, navigator_row_at};
use bilterm::ui::shell::ShellLayout;
use bilterm::wm::drawer::DrawerNavOp;
use bilterm::wm::keyboard_focus::KeyboardOwner;
use bilterm::wm::{DrawerEdge, DrawerRowKind, DrawerVisibilityMode, SplitDirection, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

static ENV_LOCK: Mutex<()> = Mutex::new(());
const CTRL_ALT: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::ALT);
const NONE: KeyModifiers = KeyModifiers::NONE;
const AREA: Rect = Rect { x: 0, y: 0, width: 160, height: 44 };
const EDGES: [(&str, DrawerEdge); 4] = [
    ("right", DrawerEdge::Right),
    ("left", DrawerEdge::Left),
    ("top", DrawerEdge::Top),
    ("bottom", DrawerEdge::Bottom),
];

/// Isolates config/state directories for tests that drive real key dispatch.
fn with_temp_xdg<T>(f: impl FnOnce() -> T) -> T {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!(
        "bilterm-drawer-handover-{}-{:?}",
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

fn colon(wm: &mut WindowManager, line: &str) {
    let action = parse_colon_command(line).unwrap_or_else(|| panic!("{line} parses")).unwrap_or_else(|e| panic!("{line}: {e}"));
    wm.apply(action).unwrap();
}

/// Three panes, the drawer docked on `edge` in `style` (`navigator` / `classic`), pinned open and
/// holding the keyboard.
fn fixture(style: &str, edge: &str) -> WindowManager {
    let mut wm = WindowManager::new_headless(AREA.height, AREA.width);
    for _ in 1..3 {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    colon(&mut wm, &format!(":set drawer.style {style}"));
    colon(&mut wm, &format!(":drawer {edge}"));
    colon(&mut wm, ":drawer fixed");
    if matches!(edge, "top" | "bottom") {
        // A short horizontal drawer shows one two-line row by default: make room for every row.
        colon(&mut wm, ":drawer size 12");
        colon(&mut wm, ":drawer rows 1");
    }
    wm.resize_all(AREA).unwrap();
    colon(&mut wm, ":drawer focus");
    assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer);
    wm
}

fn press(wm: &mut WindowManager, code: KeyCode, mods: KeyModifiers) {
    handle_wm_key_event(wm, KeyEvent::new(code, mods)).unwrap();
}

fn click(wm: &mut WindowManager, col: u16, row: u16) {
    let ev = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: col, row, modifiers: NONE };
    handle_wm_mouse_event(wm, ev, AREA).unwrap();
}

fn ws_of(wm: &WindowManager) -> Rect {
    ShellLayout::compute(AREA, wm.config(), wm.pane_count()).workspace
}

fn drawer_rect(wm: &WindowManager) -> Rect {
    wm.drawer_rect(ws_of(wm)).expect("drawer on screen")
}

fn render(wm: &WindowManager) -> Buffer {
    let mut buf = Buffer::empty(AREA);
    WorkspaceWidget::new(wm).render(AREA, &mut buf);
    buf
}

fn screen(wm: &WindowManager) -> Vec<String> {
    let buf = render(wm);
    (0..AREA.height)
        .map(|y| (0..AREA.width).map(|x| buf[(x, y)].symbol().to_string()).collect())
        .collect()
}

/// Row index and id of a pane that is not the active one.
fn other_pane_row(wm: &WindowManager) -> (usize, usize) {
    wm.drawer_rows()
        .iter()
        .enumerate()
        .find_map(|(i, r)| match r.kind {
            DrawerRowKind::Pane(id) if id != wm.active_pane_id() => Some((i, id)),
            _ => None,
        })
        .expect("a row for another pane")
}

/// A screen cell of navigator row `idx` that is not its fold marker (`None` when scrolled away).
fn navigator_row_cell_opt(wm: &WindowManager, idx: usize) -> Option<(u16, u16)> {
    let ws = ws_of(wm);
    let dr = drawer_rect(wm);
    let markers = navigator_fold_marker_cells(wm, ws);
    for y in dr.y..dr.bottom() {
        for x in dr.x + 6..dr.right().saturating_sub(8) {
            if navigator_row_at(wm, ws, x, y) == Some(idx) && !markers.iter().any(|m| m.x == x && m.y == y) {
                return Some((x, y));
            }
        }
    }
    None
}

fn navigator_row_cell(wm: &WindowManager, idx: usize) -> (u16, u16) {
    navigator_row_cell_opt(wm, idx).unwrap_or_else(|| panic!("navigator row {idx} is not on screen"))
}

/// Cell and pane id of a visible navigator row of a pane that is not the active one (a short
/// top/bottom drawer shows only a few rows).
fn visible_other_pane_row(wm: &WindowManager) -> ((u16, u16), usize) {
    wm.drawer_rows()
        .iter()
        .enumerate()
        .find_map(|(i, r)| match r.kind {
            DrawerRowKind::Pane(id) if id != wm.active_pane_id() => navigator_row_cell_opt(wm, i).map(|c| (c, id)),
            _ => None,
        })
        .expect("a visible row for another pane")
}

/// A cell inside the navigator drawer that is no row, no footer button and no border.
fn navigator_empty_cell(wm: &WindowManager) -> (u16, u16) {
    let ws = ws_of(wm);
    let dr = drawer_rect(wm);
    let buttons = navigator_footer_buttons(wm, ws);
    let x = dr.x + dr.width / 2;
    (dr.y + 2..dr.bottom().saturating_sub(1))
        .rev()
        .find(|&y| navigator_row_at(wm, ws, x, y).is_none() && !buttons.iter().any(|b| b.y == y))
        .map(|y| (x, y))
        .expect("an empty drawer cell")
}

/// Screen cell of classic (vertical dock) row `idx`.
fn classic_row_cell(wm: &WindowManager, idx: usize) -> (u16, u16) {
    let dr = drawer_rect(wm);
    let rows_len = wm.drawer_rows().len();
    let (hdr, _ftr, step) = bilterm::ui::drawer::drawer_vertical_layout_metrics(wm.config(), dr.height - 2, rows_len);
    (dr.x + 5, dr.y + 1 + hdr + (idx * step) as u16)
}

/// A cell of the active pane, outside the drawer.
fn pane_cell(wm: &WindowManager) -> (u16, u16) {
    let rects = wm.compute_workspace_rects(ws_of(wm));
    let (_, r) = rects.iter().find(|(id, _)| *id == wm.active_pane_id()).expect("active pane rect");
    (r.x + r.width / 2, r.y + r.height / 2)
}

fn assert_pane_has_keyboard_and_drawer_is_open(wm: &mut WindowManager, ctx: &str) {
    assert_eq!(wm.keyboard_owner(), KeyboardOwner::Pane, "{ctx}: the pane owns the keyboard");
    assert!(!wm.drawer().focused, "{ctx}: the drawer is blurred");
    assert!(wm.drawer().open && wm.is_drawer_effectively_visible(), "{ctx}: the drawer stays open");
    press(wm, KeyCode::Char('l'), NONE);
    press(wm, KeyCode::Char('s'), NONE);
    assert_eq!(wm.active_pane().prompt().text(), "ls", "{ctx}: typing reaches the selected pane at once");
}

// ------------------------------------------------------------ 1. selecting hands over the keyboard

#[test]
fn one_click_on_a_navigator_row_gives_that_pane_the_keyboard_on_every_edge() {
    with_temp_xdg(|| {
        for (edge, expect) in EDGES {
            for drawer_had_keyboard in [true, false] {
                let ctx = format!("navigator {edge} drawer_had_keyboard={drawer_had_keyboard}");
                let mut wm = fixture("navigator", edge);
                assert_eq!(wm.drawer_config().edge, expect);
                if !drawer_had_keyboard {
                    colon(&mut wm, ":focus pane");
                }
                let ((c, r), target) = visible_other_pane_row(&wm);
                click(&mut wm, c, r);
                assert_eq!(wm.active_pane_id(), target, "{ctx}: one click jumps");
                assert_pane_has_keyboard_and_drawer_is_open(&mut wm, &ctx);
            }
        }
    });
}

#[test]
fn one_click_on_a_classic_row_gives_that_pane_the_keyboard() {
    with_temp_xdg(|| {
        for edge in ["right", "left"] {
            for drawer_had_keyboard in [true, false] {
                let ctx = format!("classic {edge} drawer_had_keyboard={drawer_had_keyboard}");
                let mut wm = fixture("classic", edge);
                if !drawer_had_keyboard {
                    colon(&mut wm, ":focus pane");
                }
                let (idx, target) = other_pane_row(&wm);
                let (c, r) = classic_row_cell(&wm, idx);
                click(&mut wm, c, r);
                assert_eq!(wm.active_pane_id(), target, "{ctx}: one click jumps");
                assert_pane_has_keyboard_and_drawer_is_open(&mut wm, &ctx);
            }
        }
        // A top/bottom classic drawer has no per-row hit test: a press on its body steps to the
        // next row and selects it, with the same hand-over.
        for edge in ["top", "bottom"] {
            let mut wm = fixture("classic", edge);
            let dr = drawer_rect(&wm);
            click(&mut wm, dr.x + dr.width / 2, dr.y + dr.height / 2);
            assert!(wm.drawer().open, "classic {edge}: the drawer stays open");
            if matches!(wm.drawer_rows()[wm.drawer().selected_index].kind, DrawerRowKind::Pane(_)) {
                assert_eq!(wm.keyboard_owner(), KeyboardOwner::Pane, "classic {edge}");
            }
        }
    });
}

#[test]
fn enter_on_the_selected_row_hands_over_the_keyboard_in_both_styles_on_every_edge() {
    with_temp_xdg(|| {
        for style in ["navigator", "classic"] {
            for (edge, _) in EDGES {
                let ctx = format!("{style} {edge}");
                let mut wm = fixture(style, edge);
                let (idx, target) = other_pane_row(&wm);
                wm.apply(Action::DrawerNav(DrawerNavOp::Row(idx))).unwrap();
                press(&mut wm, KeyCode::Enter, NONE);
                assert_eq!(wm.active_pane_id(), target, "{ctx}: Enter jumps");
                assert_pane_has_keyboard_and_drawer_is_open(&mut wm, &ctx);
            }
        }
    });
}

#[test]
fn the_colon_twin_of_enter_hands_over_too_and_an_on_demand_drawer_stays_open() {
    with_temp_xdg(|| {
        let mut wm = fixture("navigator", "right");
        colon(&mut wm, ":drawer ondemand");
        colon(&mut wm, ":drawer focus");
        assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::OnDemand);
        let (idx, target) = other_pane_row(&wm);
        colon(&mut wm, &format!(":drawer row {}", idx + 1));
        colon(&mut wm, ":drawer enter");
        assert_eq!(wm.active_pane_id(), target);
        assert_pane_has_keyboard_and_drawer_is_open(&mut wm, "on-demand :drawer enter");
    });
}

#[test]
fn selecting_another_session_row_switches_to_it_and_hands_over_the_keyboard() {
    with_temp_xdg(|| {
        let mut wm = fixture("navigator", "right");
        colon(&mut wm, ":drawer new-session");
        let created = wm.config().history_session.clone();
        colon(&mut wm, ":drawer focus");
        let idx = wm
            .drawer_rows()
            .iter()
            .position(|r| matches!(&r.kind, DrawerRowKind::SessionHeader(n) if *n != created))
            .expect("the first session's row");
        wm.apply(Action::DrawerNav(DrawerNavOp::Row(idx))).unwrap();
        press(&mut wm, KeyCode::Enter, NONE);
        assert_ne!(wm.config().history_session, created, "Enter switched session");
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Pane);
        assert!(wm.is_drawer_effectively_visible());
    });
}

// ------------------------------------------------------------ 2. browsing keeps the keyboard

#[test]
fn arrows_browse_without_jumping_while_the_drawer_holds_the_keyboard() {
    with_temp_xdg(|| {
        for style in ["navigator", "classic"] {
            let mut wm = fixture(style, "right");
            let start = wm.active_pane_id();
            let first = wm.drawer().selected_index;
            let mut moved = false;
            for code in [KeyCode::Up, KeyCode::Up, KeyCode::Down, KeyCode::Char('k'), KeyCode::Char('j'), KeyCode::Up] {
                press(&mut wm, code, NONE);
                moved |= wm.drawer().selected_index != first;
                assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer, "{style}: {code:?} keeps the keyboard");
                assert_eq!(wm.active_pane_id(), start, "{style}: {code:?} does not jump");
            }
            assert!(moved, "{style}: the selection moved");
            assert_eq!(wm.active_pane().prompt().text(), "", "{style}: nothing leaked into the prompt");
        }
    });
}

#[test]
fn rows_and_keys_that_are_not_a_jump_keep_the_keyboard_in_the_drawer() {
    with_temp_xdg(|| {
        // Classic: the layout-group header row toggles the layout and is no jump target.
        let mut wm = fixture("classic", "right");
        let idx = wm
            .drawer_rows()
            .iter()
            .position(|r| matches!(r.kind, DrawerRowKind::LayoutGroup(_)))
            .expect("layout group row");
        let before = wm.layout_mode();
        wm.apply(Action::DrawerNav(DrawerNavOp::Row(idx))).unwrap();
        press(&mut wm, KeyCode::Enter, NONE);
        assert_ne!(wm.layout_mode(), before);
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer, "a header row is not a jump target");
        let (c, r) = classic_row_cell(&wm, idx);
        click(&mut wm, c, r);
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer, "clicking the header row keeps the keyboard");

        // Space raises the pane to look at it and keeps browsing (both styles' peek).
        let mut wm = fixture("classic", "right");
        let (idx, target) = other_pane_row(&wm);
        wm.apply(Action::DrawerNav(DrawerNavOp::Row(idx))).unwrap();
        press(&mut wm, KeyCode::Char(' '), NONE);
        assert_eq!(wm.active_pane_id(), target);
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer, "Space keeps the keyboard");
        let mut wm = fixture("navigator", "right");
        let (idx, target) = other_pane_row(&wm);
        colon(&mut wm, &format!(":drawer row {}", idx + 1));
        colon(&mut wm, ":drawer raise");
        assert_eq!(wm.active_pane_id(), target);
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer, ":drawer raise keeps the keyboard");

        // Navigator: expand/collapse (key and marker click) and Ctrl+click selection are no jump.
        let mut wm = fixture("navigator", "right");
        let active = wm.active_pane_id();
        for code in [KeyCode::Char('e'), KeyCode::Char('e'), KeyCode::Char('C'), KeyCode::Char('E')] {
            press(&mut wm, code, NONE);
            assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer, "{code:?} keeps the keyboard");
        }
        let marker = navigator_fold_marker_cells(&wm, ws_of(&wm)).into_iter().next().expect("a fold marker");
        click(&mut wm, marker.x, marker.y);
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer, "the fold marker keeps the keyboard");
        click(&mut wm, marker.x, marker.y);
        let (idx, _) = other_pane_row(&wm);
        let (c, r) = navigator_row_cell(&wm, idx);
        let ev = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: c, row: r, modifiers: KeyModifiers::CONTROL };
        handle_wm_mouse_event(&mut wm, ev, AREA).unwrap();
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer, "Ctrl+click selects without jumping");
        assert_eq!(wm.active_pane_id(), active);

        // Rename needs follow-up input: no pane jump; the keyboard goes to the rename dialog.
        let mut wm = fixture("navigator", "right");
        let active = wm.active_pane_id();
        press(&mut wm, KeyCode::Char('r'), NONE);
        assert_eq!(wm.active_pane_id(), active, "rename does not jump");
        assert!(wm.rename_dialog_open(), "the rename dialog has the keyboard");
        assert!(wm.is_drawer_effectively_visible());
    });
}

// ------------------------------------------------------------ 3. taking and releasing with the pointer / Esc

#[test]
fn a_press_on_an_empty_part_of_the_drawer_takes_the_keyboard_and_a_pane_click_releases_it() {
    with_temp_xdg(|| {
        for (edge, _) in EDGES {
            let mut wm = fixture("navigator", edge);
            colon(&mut wm, ":focus pane");
            let active = wm.active_pane_id();
            let (c, r) = navigator_empty_cell(&wm);
            click(&mut wm, c, r);
            assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer, "navigator {edge}: an empty-area press takes the keyboard");
            assert_eq!(wm.active_pane_id(), active, "navigator {edge}: nothing jumped");
            let (c, r) = pane_cell(&wm);
            click(&mut wm, c, r);
            assert_pane_has_keyboard_and_drawer_is_open(&mut wm, &format!("navigator {edge} pane click"));
        }
        for edge in ["right", "left"] {
            let mut wm = fixture("classic", edge);
            colon(&mut wm, ":focus pane");
            let selected = wm.drawer().selected_index;
            let dr = drawer_rect(&wm);
            // Below the last row, above the footer.
            click(&mut wm, dr.x + 5, dr.bottom() - 6);
            assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer, "classic {edge}: a body press takes the keyboard");
            assert_eq!(wm.drawer().selected_index, selected, "classic {edge}: the selection is kept");
            let (c, r) = pane_cell(&wm);
            click(&mut wm, c, r);
            assert_pane_has_keyboard_and_drawer_is_open(&mut wm, &format!("classic {edge} pane click"));
            // The footer row (new window) and a header chip take the keyboard as well.
            let dr = drawer_rect(&wm);
            click(&mut wm, dr.x + 2, dr.bottom() - 2);
            assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer, "classic {edge}: a footer press takes the keyboard");
            colon(&mut wm, ":focus pane");
            click(&mut wm, dr.x + dr.width / 2, dr.y + 1);
            assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer, "classic {edge}: a header-chip press takes the keyboard");
        }
    });
}

#[test]
fn a_press_on_the_drawer_takes_the_keyboard_from_another_side_window() {
    with_temp_xdg(|| {
        let mut wm = fixture("navigator", "left");
        colon(&mut wm, ":agents list open");
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::AgentsList);
        let (c, r) = navigator_empty_cell(&wm);
        click(&mut wm, c, r);
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer);
        assert!(wm.agents_list().active && !wm.agents_list().captures_input(), "one owner; the list stays open");
    });
}

#[test]
fn esc_releases_the_keyboard_and_leaves_the_drawer_open() {
    with_temp_xdg(|| {
        for style in ["navigator", "classic"] {
            for visibility in ["fixed", "ondemand"] {
                let ctx = format!("{style} {visibility}");
                let mut wm = fixture(style, "right");
                colon(&mut wm, &format!(":drawer {visibility}"));
                colon(&mut wm, ":drawer focus");
                let active = wm.active_pane_id();
                press(&mut wm, KeyCode::Esc, NONE);
                assert_eq!(wm.active_pane_id(), active, "{ctx}: Esc does not jump");
                // An on-demand drawer that was opened also stays on screen after Esc; `q`/F7 hide it.
                assert_pane_has_keyboard_and_drawer_is_open(&mut wm, &ctx);
            }
        }
    });
}

#[test]
fn the_focus_cycle_reaches_a_right_docked_drawer_and_returns_to_the_pane() {
    with_temp_xdg(|| {
        let mut wm = fixture("navigator", "right");
        colon(&mut wm, ":focus pane");
        assert_eq!(wm.keyboard_focus_ring(), vec![KeyboardOwner::Drawer]);
        press(&mut wm, KeyCode::Char('o'), CTRL_ALT);
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer);
        press(&mut wm, KeyCode::Char('o'), CTRL_ALT);
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Pane);
        colon(&mut wm, ":focus cycle");
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer);
        colon(&mut wm, ":focus pane");
        assert!(wm.is_drawer_effectively_visible(), "cycling closes nothing");
    });
}

// ------------------------------------------------------------ 4. ownership is visible

/// Symbol of the top-left frame corner of the active pane.
fn pane_corner(wm: &WindowManager) -> String {
    let rects = wm.compute_workspace_rects(ws_of(wm));
    let (_, outer) = rects.iter().find(|(id, _)| *id == wm.active_pane_id()).expect("active pane rect");
    render(wm)[(outer.x, outer.y)].symbol().to_string()
}

/// Symbols and styles of the drawer's top border row.
fn drawer_title_row(wm: &WindowManager) -> Vec<String> {
    let dr = drawer_rect(wm);
    let buf = render(wm);
    (dr.x..dr.right()).map(|x| format!("{:?}", buf[(x, dr.y)])).collect()
}

#[test]
fn the_navigator_drawer_shows_the_badge_and_an_inactive_pane_while_it_owns_the_keyboard() {
    with_temp_xdg(|| {
        for (edge, _) in EDGES {
            let mut wm = fixture("navigator", edge);
            assert_eq!(wm.keyboard_owner_shown(), KeyboardOwner::Drawer);
            assert!(screen(&wm).iter().any(|l| l.contains("KBD:DRAWER")), "{edge}: badge while the drawer owns the keyboard");
            let owned = pane_corner(&wm);
            let (idx, _) = other_pane_row(&wm);
            wm.apply(Action::DrawerNav(DrawerNavOp::Row(idx))).unwrap();
            press(&mut wm, KeyCode::Enter, NONE);
            assert!(!screen(&wm).iter().any(|l| l.contains("KBD:")), "{edge}: badge gone after the hand-over");
            assert_ne!(pane_corner(&wm), owned, "{edge}: the selected pane's frame is drawn active again");
        }
    });
}

#[test]
fn a_modern_look_shows_drawer_ownership_with_the_classic_drawer_style_too() {
    with_temp_xdg(|| {
        let mut wm = fixture("classic", "right");
        colon(&mut wm, ":set appearance.profile midnight");
        assert!(wm.config().explicit_new_look());
        assert!(!wm.drawer_focus_chrome_pinned());
        assert_eq!(wm.keyboard_owner_shown(), KeyboardOwner::Drawer);
        assert!(screen(&wm).iter().any(|l| l.contains("KBD:DRAWER")));
        press(&mut wm, KeyCode::Esc, NONE);
        assert!(!screen(&wm).iter().any(|l| l.contains("KBD:")));
    });
}

#[test]
fn the_classic_drawer_in_the_retro_looks_shows_focus_on_its_own_frame_only() {
    with_temp_xdg(|| {
        for look in ["", ":modern on", ":liquid on"] {
            let mut wm = fixture("classic", "right");
            if !look.is_empty() {
                colon(&mut wm, look);
            }
            assert!(wm.drawer_focus_chrome_pinned(), "{look:?}: pinned by the classic goldens");
            assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer);
            assert_eq!(wm.keyboard_owner_shown(), KeyboardOwner::Pane);
            assert!(!screen(&wm).iter().any(|l| l.contains("KBD:")), "{look:?}: no badge on the golden-pinned chrome");
            let focused = drawer_title_row(&wm);
            press(&mut wm, KeyCode::Esc, NONE);
            assert_ne!(drawer_title_row(&wm), focused, "{look:?}: the drawer frame shows focused vs unfocused");
        }
    });
}
