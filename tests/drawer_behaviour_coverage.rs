//! Behavioural coverage for the slide-out drawer: docking/size/layout/visibility actions,
//! horizontal-scroll keys, wheel/drag mouse handling and session reordering controls.

use std::sync::Once;

use bilterm::action::Action;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::wm::drawer::DrawerRowKind;
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::{DrawerEdge, DrawerLayoutMode, DrawerVisibilityMode, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

static INIT: Once = Once::new();

fn init_env() {
    INIT.call_once(|| {
        let dir = std::env::temp_dir().join(format!("bilterm-drawer-cov-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
    });
}

const AREA: Rect = Rect { x: 0, y: 0, width: 100, height: 24 };

fn wm_new(rows: u16, cols: u16) -> WindowManager {
    init_env();
    WindowManager::new_headless(rows, cols)
}

fn wm_in(tmp: &tempfile::TempDir) -> WindowManager {
    let mut wm = wm_new(24, 100);
    wm.config_mut().session_dir = Some(tmp.path().to_string_lossy().into_owned());
    wm
}

fn key(wm: &mut WindowManager, code: KeyCode) {
    key_mod(wm, code, KeyModifiers::NONE);
}

fn key_mod(wm: &mut WindowManager, code: KeyCode, m: KeyModifiers) {
    handle_wm_key_event(wm, KeyEvent::new(code, m)).unwrap();
}

fn mouse(wm: &mut WindowManager, kind: MouseEventKind, col: u16, row: u16, mods: KeyModifiers, area: Rect) {
    let ev = MouseEvent { kind, column: col, row, modifiers: mods };
    handle_wm_mouse_event(wm, ev, area).unwrap();
}

fn status(wm: &WindowManager) -> String {
    wm.active_pane().status_message().unwrap_or("").to_string()
}

fn three_pane_focused() -> WindowManager {
    let mut wm = wm_new(24, 100);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.split_focused(SplitDirection::Horizontal).unwrap();
    wm.focus_drawer();
    wm
}

#[test]
fn rows_have_session_layout_and_pane_entries_and_selection_tracks_active_pane() {
    let wm = three_pane_focused();
    let rows = wm.drawer_rows();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0].kind, DrawerRowKind::SessionHeader("default".into()));
    assert_eq!(rows[0].formatted_line(), "◈ SESSION [default]");
    assert!(rows[1].formatted_line().ends_with("WINDOW: TILED SPLITS"));
    assert_eq!(rows[4].kind, DrawerRowKind::Pane(3));
    assert!(rows[4].is_focused_pane);
    assert_eq!(wm.drawer().selected_index, 4);
    assert_eq!(wm.selected_drawer_pane_id(), Some(3));
}

#[test]
fn hscroll_by_clamps_to_longest_row_minus_twelve_and_reports_movement() {
    let mut wm = three_pane_focused();
    // longest row is the 35-char pane line -> max scroll 23
    assert!(!wm.drawer_hscroll_by(-6), "already at 0");
    assert_eq!(wm.drawer().h_scroll, 0);
    assert!(!wm.drawer_hscroll_by(0));
    assert!(wm.drawer_hscroll_by(6));
    assert_eq!(wm.drawer().h_scroll, 6);
    assert!(wm.drawer_hscroll_by(1000));
    assert_eq!(wm.drawer().h_scroll, 23);
    assert!(!wm.drawer_hscroll_by(6), "no movement at the end");
    assert_eq!(wm.drawer().h_scroll, 23);
    assert!(wm.drawer_hscroll_by(-1000));
    assert_eq!(wm.drawer().h_scroll, 0);
    assert!(!wm.drawer_hscroll_by(isize::MIN), "saturating add must not overflow");
}

#[test]
fn right_key_scrolls_while_text_overflows_then_leaves_drawer() {
    let mut wm = three_pane_focused();
    for expected in [6, 12, 18, 23] {
        key(&mut wm, KeyCode::Right);
        assert!(wm.drawer().focused, "still focused while scrolling");
        assert_eq!(wm.drawer().h_scroll, expected);
    }
    key(&mut wm, KeyCode::Right);
    assert!(!wm.drawer().focused, "Right at end of text leaves the drawer");
    assert_eq!(wm.drawer().h_scroll, 23);
}

#[test]
fn left_key_scrolls_back_and_never_leaves_the_drawer() {
    let mut wm = three_pane_focused();
    wm.drawer_hscroll_by(12);
    key(&mut wm, KeyCode::Left);
    assert_eq!(wm.drawer().h_scroll, 6);
    key(&mut wm, KeyCode::Left);
    key(&mut wm, KeyCode::Left);
    assert_eq!(wm.drawer().h_scroll, 0);
    assert!(wm.drawer().focused);
}

#[test]
fn h_and_l_keys_scroll_horizontally_by_six_and_keep_focus() {
    let mut wm = three_pane_focused();
    key(&mut wm, KeyCode::Char('l'));
    key(&mut wm, KeyCode::Char('l'));
    assert_eq!(wm.drawer().h_scroll, 12);
    key(&mut wm, KeyCode::Char('h'));
    assert_eq!(wm.drawer().h_scroll, 6);
    for _ in 0..10 {
        key(&mut wm, KeyCode::Char('l'));
    }
    assert_eq!(wm.drawer().h_scroll, 23);
    assert!(wm.drawer().focused);
}

#[test]
fn esc_and_tab_unfocus_drawer() {
    for code in [KeyCode::Esc, KeyCode::Tab] {
        let mut wm = three_pane_focused();
        key(&mut wm, code);
        assert!(!wm.drawer().focused);
    }
}

#[test]
fn vertical_navigation_keys_move_selection_with_wraparound_and_clamping() {
    let mut wm = three_pane_focused();
    assert_eq!(wm.drawer().selected_index, 4);
    key(&mut wm, KeyCode::Down);
    assert_eq!(wm.drawer().selected_index, 0, "wraps to top");
    key(&mut wm, KeyCode::Up);
    assert_eq!(wm.drawer().selected_index, 4, "wraps to bottom");
    key(&mut wm, KeyCode::Char('k'));
    assert_eq!(wm.drawer().selected_index, 3);
    key(&mut wm, KeyCode::Char('j'));
    assert_eq!(wm.drawer().selected_index, 4);
    key(&mut wm, KeyCode::Home);
    assert_eq!(wm.drawer().selected_index, 0);
    key(&mut wm, KeyCode::PageDown);
    assert_eq!(wm.drawer().selected_index, 4, "clamped to last row");
    key(&mut wm, KeyCode::PageUp);
    assert_eq!(wm.drawer().selected_index, 0, "clamped to first row");
    key(&mut wm, KeyCode::End);
    assert_eq!(wm.drawer().selected_index, 4);
    key(&mut wm, KeyCode::Char('g'));
    assert_eq!(wm.drawer().selected_index, 0);
    key(&mut wm, KeyCode::Char('G'));
    assert_eq!(wm.drawer().selected_index, 4);
}

#[test]
fn enter_hands_the_keyboard_to_the_selected_pane_and_space_keeps_drawer_focus() {
    let mut wm = three_pane_focused();
    key(&mut wm, KeyCode::Char('g'));
    key(&mut wm, KeyCode::Down);
    key(&mut wm, KeyCode::Down); // row 2 = pane 1
    assert_eq!(wm.selected_drawer_pane_id(), Some(1));
    key(&mut wm, KeyCode::Enter);
    assert_eq!(wm.active_pane_id(), 1);
    // FOCUS-01: Enter hands the keyboard to the pane; the drawer stays open.
    assert!(!wm.drawer().focused && wm.drawer().open);
    assert!(status(&wm).contains("Selected Pane #1"));
    wm.focus_drawer();
    key(&mut wm, KeyCode::Down);
    key(&mut wm, KeyCode::Char(' '));
    assert_eq!(wm.active_pane_id(), 2);
    assert!(wm.drawer().focused, "Space raises and keeps browsing");
}

#[test]
fn activating_layout_group_row_toggles_layout_mode_and_unfocus_after_works() {
    let mut wm = three_pane_focused();
    wm.drawer_mut().selected_index = 1;
    let before = wm.layout_mode();
    wm.drawer_activate_selected(true);
    assert_ne!(wm.layout_mode(), before);
    assert!(!wm.drawer().focused, "unfocus_after drops focus");
}

#[test]
fn activating_session_header_of_history_session_focuses_top_pane() {
    let mut wm = three_pane_focused();
    wm.drawer_mut().selected_index = 0;
    wm.drawer_activate_selected(false);
    assert_eq!(Some(wm.active_pane_id()), wm.z_stack().top());
    assert!(wm.drawer().focused);
}

#[test]
fn x_closes_selected_pane_and_clamps_selection() {
    let mut wm = three_pane_focused();
    assert_eq!(wm.pane_count(), 3);
    key(&mut wm, KeyCode::Char('x'));
    assert_eq!(wm.pane_count(), 2);
    assert!(wm.pane(3).is_none());
    assert_eq!(wm.drawer().selected_index, 3, "clamped to new last row");
    key(&mut wm, KeyCode::Delete);
    assert_eq!(wm.pane_count(), 1);
    // selecting a header and closing does nothing
    wm.drawer_mut().selected_index = 0;
    assert!(!wm.drawer_close_selected_pane());
    assert_eq!(wm.pane_count(), 1);
}

#[test]
fn z_zooms_selected_pane() {
    let mut wm = three_pane_focused();
    key(&mut wm, KeyCode::Char('g'));
    key(&mut wm, KeyCode::Char('j'));
    key(&mut wm, KeyCode::Char('j')); // pane 1
    key(&mut wm, KeyCode::Char('z'));
    assert_eq!(wm.zoomed_pane(), Some(1));
    // header row has no pane: zoom is a no-op on selection
    wm.drawer_mut().selected_index = 0;
    wm.drawer_zoom_selected_pane();
    assert_eq!(wm.zoomed_pane(), Some(1), "header row selects no pane, zoom unchanged");
}

#[test]
fn n_and_v_create_windows_from_drawer() {
    let mut wm = three_pane_focused();
    key(&mut wm, KeyCode::Char('n'));
    assert_eq!(wm.pane_count(), 4);
    assert!(status(&wm).starts_with("Created new window #"));
    key(&mut wm, KeyCode::Char('v'));
    assert_eq!(wm.pane_count(), 5);
    assert!(status(&wm).starts_with("Created new vertical window #"));
    assert_eq!(wm.drawer().selected_index, wm.drawer_rows().len() - 1);
}

#[test]
fn d_m_p_keys_cycle_edge_layout_and_visibility() {
    let mut wm = three_pane_focused();
    key(&mut wm, KeyCode::Char('d'));
    assert_eq!(wm.drawer_config().edge, DrawerEdge::Right);
    key(&mut wm, KeyCode::Char('d'));
    key(&mut wm, KeyCode::Char('d'));
    key(&mut wm, KeyCode::Char('d'));
    assert_eq!(wm.drawer_config().edge, DrawerEdge::Left, "four presses wrap around");

    assert_eq!(wm.drawer_config().layout, DrawerLayoutMode::Push);
    key(&mut wm, KeyCode::Char('m'));
    assert_eq!(wm.drawer_config().layout, DrawerLayoutMode::Overlay);
    assert_eq!(status(&wm), "Slide-Out Drawer Layout Mode: Float (Fly-Out overlay)");
    key(&mut wm, KeyCode::Char('m'));
    assert_eq!(status(&wm), "Slide-Out Drawer Layout Mode: Push (Push-Out reflow)");

    key(&mut wm, KeyCode::Char('p'));
    assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::Fixed);
    key(&mut wm, KeyCode::Char('p'));
    assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::AutoHide);
    key(&mut wm, KeyCode::Char('p'));
    assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::Fixed);
}

#[test]
fn size_keys_minimize_restore_and_maximize() {
    let mut wm = three_pane_focused();
    key(&mut wm, KeyCode::Char('M'));
    assert!(wm.drawer().maximized);
    let max_rect = wm.drawer_rect(AREA).unwrap();
    assert_eq!(max_rect.width, 66, "two thirds of 100 columns");
    // '_' while maximized restores instead of minimizing
    key(&mut wm, KeyCode::Char('_'));
    assert!(!wm.drawer().maximized);
    assert!(wm.drawer().focused);
    assert_eq!(status(&wm), "Slide-Out Drawer: RESTORED [_]");
    // '-' now minimizes: closes + forces OnDemand
    wm.set_drawer_visibility(Some(DrawerVisibilityMode::Fixed));
    key(&mut wm, KeyCode::Char('-'));
    assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::OnDemand);
    assert!(!wm.drawer().focused);
    assert!(!wm.drawer().open);
    assert_eq!(status(&wm), "Slide-Out Drawer: MINIMIZED [_]");
    // F5 toggles maximize
    wm.focus_drawer();
    key(&mut wm, KeyCode::F(5));
    assert!(wm.drawer().maximized);
    assert_eq!(status(&wm), "Slide-Out Drawer: MAXIMIZED [□]");
    key(&mut wm, KeyCode::F(5));
    assert!(!wm.drawer().maximized);
    assert_eq!(status(&wm), "Slide-Out Drawer: RESTORED");
}

#[test]
fn q_and_f7_close_drawer_and_fixed_visibility_demotes_to_on_demand() {
    let mut wm = three_pane_focused();
    wm.set_drawer_visibility(Some(DrawerVisibilityMode::Fixed));
    key(&mut wm, KeyCode::Char('q'));
    assert!(!wm.drawer().open);
    assert!(!wm.drawer().focused);
    assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::OnDemand);
    assert!(!wm.is_drawer_effectively_visible());
    wm.focus_drawer();
    key(&mut wm, KeyCode::F(7));
    assert!(!wm.is_drawer_effectively_visible());
}

#[test]
fn prompt_opening_keys_set_command_text_and_unfocus() {
    let mut wm = three_pane_focused();
    key(&mut wm, KeyCode::Char(':'));
    assert!(!wm.drawer().focused);
    assert_eq!(wm.active_pane().prompt().text(), ":");

    let mut wm = three_pane_focused();
    key(&mut wm, KeyCode::Char('g')); // session header
    key(&mut wm, KeyCode::Char('P'));
    assert!(!wm.drawer().focused);
    assert_eq!(wm.active_pane().prompt().text(), ":session graft default ");

    let mut wm = three_pane_focused();
    key(&mut wm, KeyCode::Char('g'));
    key(&mut wm, KeyCode::Char('r'));
    assert_eq!(wm.active_pane().prompt().text(), ":session rename default ");
}

#[test]
fn layout_keys_arrange_panes_in_place() {
    let mut wm = three_pane_focused();
    key(&mut wm, KeyCode::Char('A'));
    assert_eq!(wm.layout_mode(), bilterm::wm::WorkspaceLayoutMode::Stacked);
    assert!(wm.z_stack().has_any_custom_geometry());
    key(&mut wm, KeyCode::Char('t')); // not tiled -> arrange grid
    assert_eq!(wm.layout_mode(), bilterm::wm::WorkspaceLayoutMode::Tiled);
    key(&mut wm, KeyCode::Char('T')); // tiled -> cascade
    assert_eq!(wm.layout_mode(), bilterm::wm::WorkspaceLayoutMode::Stacked);
    assert!(!wm.z_stack().has_any_custom_geometry());
    key(&mut wm, KeyCode::Char('a'));
    assert_eq!(wm.layout_mode(), bilterm::wm::WorkspaceLayoutMode::Tiled);
    key(&mut wm, KeyCode::Char('L'));
    assert!(status(&wm).starts_with("Saved mid-flight window positions"));
    assert_eq!(wm.pane_count(), 3);
}

#[test]
fn session_branch_and_reparent_keys_update_session_tree() {
    let mut wm = three_pane_focused();
    key(&mut wm, KeyCode::Char('g'));
    key(&mut wm, KeyCode::Char('b'));
    assert!(status(&wm).starts_with("Branched child session 'default/sub-"), "{}", status(&wm));
    let child = wm
        .drawer_rows()
        .into_iter()
        .find_map(|r| match r.kind {
            DrawerRowKind::SessionHeader(n) if n.starts_with("default/sub-") => Some(n),
            _ => None,
        })
        .expect("child session row");
    assert_eq!(wm.selected_drawer_session_name(), child);

    // '<' promotes the child to a root
    key(&mut wm, KeyCode::Char('<'));
    assert!(status(&wm).starts_with("Promoted session"), "{}", status(&wm));
    // '>' grafts it back under the preceding session
    key(&mut wm, KeyCode::Char('>'));
    assert!(status(&wm).starts_with("Reparented session"), "{}", status(&wm));
    // Alt+Left promotes, Alt+Right re-indents
    key_mod(&mut wm, KeyCode::Left, KeyModifiers::ALT);
    assert!(status(&wm).starts_with("Promoted session"));
    key_mod(&mut wm, KeyCode::Right, KeyModifiers::ALT);
    assert!(status(&wm).starts_with("Reparented session"));
    // 'u' also promotes
    key(&mut wm, KeyCode::Char('u'));
    assert!(status(&wm).starts_with("Promoted session"));
}

#[test]
fn reparent_with_single_session_cannot_indent() {
    let mut wm = three_pane_focused();
    wm.drawer_mut().selected_index = 0;
    assert!(!wm.drawer_reparent_selected_session(true), "no sibling to graft under");
    assert!(wm.drawer_reparent_selected_session(false), "promote of a root succeeds");
}

#[test]
fn branch_selected_session_returns_child_name() {
    let mut wm = three_pane_focused();
    wm.drawer_mut().selected_index = 0;
    let child = wm.drawer_branch_selected_session().expect("branch");
    assert!(child.starts_with("default/sub-"));
}

#[test]
fn move_selected_pane_reorders_windows_in_split_order() {
    let mut wm = three_pane_focused();
    assert_eq!(wm.ordered_pane_ids(), vec![1, 2, 3]);
    // selected is pane 3 (last row): move up swaps with pane 2
    wm.drawer_move_selected_session(true);
    assert_eq!(wm.ordered_pane_ids(), vec![1, 3, 2]);
    assert_eq!(wm.selected_drawer_pane_id(), Some(3));
    assert!(status(&wm).starts_with("Moved Pane #3 up (Window order: #1, #3, #2)"));
    // Shift+Down and J move it back down; K moves up
    key_mod(&mut wm, KeyCode::Down, KeyModifiers::SHIFT);
    assert_eq!(wm.ordered_pane_ids(), vec![1, 2, 3]);
    assert!(status(&wm).contains("down"));
    key(&mut wm, KeyCode::Char('K'));
    assert_eq!(wm.ordered_pane_ids(), vec![1, 3, 2]);
    key(&mut wm, KeyCode::Char('J'));
    assert_eq!(wm.ordered_pane_ids(), vec![1, 2, 3]);
    key_mod(&mut wm, KeyCode::Up, KeyModifiers::ALT);
    assert_eq!(wm.ordered_pane_ids(), vec![1, 3, 2]);
    // moving the first pane up has no partner: falls through to the session ordering path
    wm.drawer_mut().selected_index = 2;
    assert_eq!(wm.selected_drawer_pane_id(), Some(1));
    wm.drawer_move_selected_session(true);
    assert_eq!(wm.ordered_pane_ids(), vec![1, 3, 2], "order unchanged");
}

#[test]
fn n_upper_creates_new_session_and_s_saves_selected_session() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.focus_drawer();
    key(&mut wm, KeyCode::Char('N'));
    assert!(status(&wm).starts_with("Created new session '"), "{}", status(&wm));
    let sessions: Vec<_> = wm
        .drawer_rows()
        .into_iter()
        .filter(|r| matches!(r.kind, DrawerRowKind::SessionHeader(_)))
        .collect();
    assert!(sessions.len() >= 2, "new session shows up as a header row");

    key(&mut wm, KeyCode::Char('g'));
    key(&mut wm, KeyCode::Char('s'));
    assert!(status(&wm).starts_with("Saved session '"), "{}", status(&wm));
    let tomls = std::fs::read_dir(tmp.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "toml"))
        .count();
    assert!(tomls >= 1);
}

#[test]
fn execute_drawer_action_clamps_sizes_per_edge_and_rejects_foreign_actions() {
    let mut wm = wm_new(24, 100);
    assert!(wm.execute_drawer_action(&Action::DrawerSize(5)));
    assert_eq!(wm.drawer_config().width_cols, 16, "vertical min clamp");
    assert_eq!(status(&wm), "Slide-Out Drawer Size: 5");
    wm.execute_drawer_action(&Action::DrawerSize(500));
    assert_eq!(wm.drawer_config().width_cols, 100, "vertical max clamp");
    wm.execute_drawer_action(&Action::DrawerSize(40));
    assert_eq!(wm.drawer_config().width_cols, 40);

    assert!(wm.execute_drawer_action(&Action::DrawerDock(Some(DrawerEdge::Top))));
    assert_eq!(status(&wm), "Slide-Out Drawer Dock Edge: Top");
    wm.execute_drawer_action(&Action::DrawerSize(1));
    assert_eq!(wm.drawer_config().height_rows, 3, "horizontal min clamp");
    wm.execute_drawer_action(&Action::DrawerSize(200));
    assert_eq!(wm.drawer_config().height_rows, 14, "horizontal max clamp");
    assert_eq!(wm.drawer_config().width_cols, 40, "width untouched by horizontal resize");

    assert!(wm.execute_drawer_action(&Action::DrawerDock(None)));
    assert_eq!(wm.drawer_config().edge, DrawerEdge::Bottom, "None cycles Top -> Bottom");
    assert!(wm.execute_drawer_action(&Action::DrawerLayout(Some(DrawerLayoutMode::Overlay))));
    assert_eq!(status(&wm), "Slide-Out Drawer Layout Mode: Float");
    assert!(wm.execute_drawer_action(&Action::DrawerLayout(None)));
    assert_eq!(wm.drawer_config().layout, DrawerLayoutMode::Push);
    assert!(wm.execute_drawer_action(&Action::DrawerVisibility(Some(DrawerVisibilityMode::AutoHide))));
    assert_eq!(status(&wm), "Slide-Out Drawer Visibility: AutoHide");

    assert!(wm.execute_drawer_action(&Action::DrawerToggle));
    assert_eq!(status(&wm), "Slide-Out Drawer: OPEN");
    assert!(wm.execute_drawer_action(&Action::DrawerToggle));
    assert_eq!(status(&wm), "Slide-Out Drawer: HIDDEN");
    assert!(wm.execute_drawer_action(&Action::DrawerFocus));
    assert!(wm.drawer().focused && wm.drawer().open);
    assert_eq!(status(&wm), "Slide-Out Drawer: FOCUSED");

    assert!(!wm.execute_drawer_action(&Action::Quit), "non drawer action is rejected");
}

#[test]
fn drawer_size_zero_and_max_actions_via_apply() {
    let mut wm = wm_new(24, 100);
    let _ = wm.apply(Action::DrawerSize(u16::MAX));
    assert!(wm.drawer().maximized);
    assert!(wm.drawer().open);
    let _ = wm.apply(Action::DrawerSize(0));
    assert!(!wm.drawer().maximized, "first 0 restores");
    assert!(wm.drawer().open);
    let _ = wm.apply(Action::DrawerSize(0));
    assert!(!wm.drawer().open, "second 0 minimizes");
    // resizing clears maximize
    wm.toggle_maximize_drawer();
    wm.resize_drawer(20);
    assert!(!wm.drawer().maximized);
}

#[test]
fn visibility_toggle_pin_and_open_state_side_effects() {
    let mut wm = wm_new(24, 100);
    assert_eq!(wm.set_drawer_visibility(None), DrawerVisibilityMode::Fixed);
    assert!(wm.drawer().open);
    assert_eq!(wm.set_drawer_visibility(None), DrawerVisibilityMode::AutoHide);
    assert_eq!(wm.set_drawer_visibility(Some(DrawerVisibilityMode::OnDemand)), DrawerVisibilityMode::OnDemand);
    assert!(!wm.drawer().open, "OnDemand without focus closes the drawer");
    wm.focus_drawer();
    wm.set_drawer_visibility(Some(DrawerVisibilityMode::OnDemand));
    assert!(wm.drawer().open, "focused drawer stays open");
    assert_eq!(DrawerVisibilityMode::OnDemand.cycle(), DrawerVisibilityMode::Fixed);
    assert_eq!(DrawerVisibilityMode::Fixed.cycle(), DrawerVisibilityMode::AutoHide);
    assert_eq!(DrawerVisibilityMode::AutoHide.cycle(), DrawerVisibilityMode::OnDemand);
}

#[test]
fn drawer_rect_clamps_and_disappears_when_workspace_too_small() {
    let mut wm = wm_new(24, 100);
    wm.set_drawer_visibility(Some(DrawerVisibilityMode::Fixed));
    assert!(wm.drawer_rect(Rect::new(0, 0, 11, 24)).is_none(), "too narrow");
    assert!(wm.drawer_rect(Rect::new(0, 0, 100, 3)).is_none(), "too short");
    // width_cols = 34 but half of a 40-col area is 20
    let r = wm.drawer_rect(Rect::new(0, 0, 40, 24)).unwrap();
    assert_eq!(r.width, 20);
    // offset workspace respected for Right edge
    wm.set_drawer_edge(Some(DrawerEdge::Right));
    let r = wm.drawer_rect(Rect::new(5, 2, 100, 20)).unwrap();
    assert_eq!((r.x, r.y, r.width, r.height), (71, 2, 34, 20));
    // Top/Bottom: height 6 clamped by half the area (10 rows -> 5)
    wm.set_drawer_edge(Some(DrawerEdge::Top));
    let r = wm.drawer_rect(Rect::new(0, 0, 100, 10)).unwrap();
    assert_eq!((r.y, r.height, r.width), (0, 5, 100));
    wm.set_drawer_edge(Some(DrawerEdge::Bottom));
    let r = wm.drawer_rect(Rect::new(0, 0, 100, 10)).unwrap();
    assert_eq!((r.y, r.height), (5, 5));
    // maximized uses two thirds, min 44 cols / 10 rows
    wm.toggle_maximize_drawer();
    let r = wm.drawer_rect(Rect::new(0, 0, 90, 30)).unwrap();
    assert_eq!(r.height, 20);
    assert_eq!(r.width, 90);
}

#[test]
fn effective_workspace_area_subtracts_dock_only_in_push_mode() {
    let mut wm = wm_new(24, 100);
    wm.set_drawer_visibility(Some(DrawerVisibilityMode::Fixed));
    let area = Rect::new(0, 0, 100, 24);
    wm.set_drawer_edge(Some(DrawerEdge::Left));
    assert_eq!(wm.effective_workspace_area(area), Rect::new(34, 0, 66, 24));
    wm.set_drawer_edge(Some(DrawerEdge::Right));
    assert_eq!(wm.effective_workspace_area(area), Rect::new(0, 0, 66, 24));
    wm.set_drawer_edge(Some(DrawerEdge::Top));
    assert_eq!(wm.effective_workspace_area(area), Rect::new(0, 6, 100, 18));
    wm.set_drawer_edge(Some(DrawerEdge::Bottom));
    assert_eq!(wm.effective_workspace_area(area), Rect::new(0, 0, 100, 18));
    wm.set_drawer_layout(Some(DrawerLayoutMode::Overlay));
    assert_eq!(wm.effective_workspace_area(area), area, "overlay never shrinks the workspace");
}

#[test]
fn push_mode_reflows_pty_overlay_does_not() {
    let mut wm = wm_new(24, 100);
    wm.resize_all(AREA).unwrap();
    let base = wm.active_pane().vt().size();
    wm.set_drawer_layout(Some(DrawerLayoutMode::Push));
    wm.set_drawer_visibility(Some(DrawerVisibilityMode::Fixed));
    wm.resize_all(AREA).unwrap();
    let pushed = wm.active_pane().vt().size();
    assert!(pushed.1 < base.1, "push mode narrows panes: {base:?} -> {pushed:?}");
    wm.set_drawer_layout(Some(DrawerLayoutMode::Overlay));
    wm.resize_all(AREA).unwrap();
    assert_eq!(wm.active_pane().vt().size(), base);
}

#[test]
fn update_drawer_hover_triggers_on_each_edge_and_sticks_while_inside() {
    let mut wm = wm_new(24, 100);
    wm.set_drawer_visibility(Some(DrawerVisibilityMode::AutoHide));
    // Left edge
    assert!(wm.update_drawer_hover(0, 5, AREA));
    assert!(wm.drawer().hover_active);
    assert!(wm.is_drawer_effectively_visible());
    assert!(!wm.update_drawer_hover(10, 5, AREA), "inside the open drawer: unchanged");
    assert!(wm.drawer().hover_active);
    assert!(wm.update_drawer_hover(60, 5, AREA), "moving away flips state");
    assert!(!wm.drawer().hover_active);
    // Right edge
    wm.set_drawer_edge(Some(DrawerEdge::Right));
    assert!(!wm.update_drawer_hover(0, 5, AREA));
    assert!(wm.update_drawer_hover(98, 5, AREA));
    assert!(wm.update_drawer_hover(10, 5, AREA));
    // Top edge
    wm.set_drawer_edge(Some(DrawerEdge::Top));
    assert!(wm.update_drawer_hover(50, 0, AREA));
    assert!(wm.update_drawer_hover(50, 20, AREA));
    // Bottom edge
    wm.set_drawer_edge(Some(DrawerEdge::Bottom));
    assert!(wm.update_drawer_hover(50, 22, AREA));
    assert!(!wm.update_drawer_hover(50, 23, AREA));
    assert!(wm.drawer().hover_active);
}

#[test]
fn autohide_hover_in_push_mode_reflows_workspace() {
    let mut wm = wm_new(24, 100);
    wm.resize_all(AREA).unwrap();
    let base = wm.active_pane().vt().size();
    wm.set_drawer_visibility(Some(DrawerVisibilityMode::AutoHide));
    wm.set_drawer_layout(Some(DrawerLayoutMode::Push));
    mouse(&mut wm, MouseEventKind::Moved, 0, 5, KeyModifiers::NONE, AREA);
    assert!(wm.drawer().hover_active);
    assert!(wm.active_pane().vt().size().1 < base.1);
    mouse(&mut wm, MouseEventKind::Moved, 80, 5, KeyModifiers::NONE, AREA);
    assert!(!wm.drawer().hover_active);
    assert_eq!(wm.active_pane().vt().size(), base);
}

#[test]
fn effective_scroll_and_wheel_scroll_clamp_and_override() {
    use bilterm::wm::drawer::WorkspaceDrawer;
    let mut d = WorkspaceDrawer::new();
    // selection below viewport pins it
    assert_eq!(d.effective_scroll(9, 4, 12), 6);
    // selection above viewport
    d.scroll_offset = 5;
    assert_eq!(d.effective_scroll(2, 4, 12), 2);
    // never beyond len - cap
    d.scroll_offset = 100;
    assert_eq!(d.effective_scroll(3, 4, 12), 3, "selection above stale offset re-pins");
    assert_eq!(d.effective_scroll(11, 4, 12), 8, "clamped to len - cap");
    d.scroll_offset = 9;
    assert_eq!(d.effective_scroll(10, 4, 12), 8, "stale offset beyond max is clamped");
    // fits entirely
    assert_eq!(d.effective_scroll(0, 10, 3), 0);
    d.scroll_offset = 0;
    d.wheel_scroll(0, 4, 12, 3);
    assert_eq!(d.view_scroll, Some((0, 3)));
    assert_eq!(d.effective_scroll(0, 4, 12), 3, "wheel viewport wins while selection unchanged");
    d.wheel_scroll(0, 4, 12, 100);
    assert_eq!(d.view_scroll, Some((0, 8)));
    d.wheel_scroll(0, 4, 12, -100);
    assert_eq!(d.view_scroll, Some((0, 0)));
    d.wheel_scroll(0, 4, 12, isize::MIN);
    assert_eq!(d.view_scroll, Some((0, 0)));
    // selection changed -> view_scroll ignored
    d.view_scroll = Some((0, 7));
    assert_eq!(d.effective_scroll(1, 4, 12), 0);
    // short list: viewport can't scroll
    d.wheel_scroll(0, 10, 3, 3);
    assert_eq!(d.view_scroll, Some((0, 0)));
}

#[test]
fn sparkline_and_telemetry_track_output_buckets() {
    let mut wm = wm_new(24, 100);
    assert_eq!(wm.drawer().sparkline_for(1, 0), "▁▁▁▁▁▁");
    assert_eq!(wm.drawer().sparkline_for(1, 16), "▁▁▁▂▃▂");
    assert_eq!(wm.drawer().sparkline_for(1, 16 * 100), "▁▁▁▂▃▇", "level clamps at 5");
    wm.update_drawer_telemetry();
    assert_eq!(wm.drawer().sparkline_for(1, 0), "▁▁▁▁▁▁", "no output yet");
    wm.pane_mut(1).unwrap().process_pty_output(b"hello world\r\n");
    wm.update_drawer_telemetry();
    let s = wm.drawer().sparkline_for(1, 0);
    assert!(s.ends_with('▂'), "1..=32 bytes lands in bucket 1: {s}");
    wm.pane_mut(1).unwrap().process_pty_output(&vec![b'x'; 3000]);
    wm.update_drawer_telemetry();
    let s = wm.drawer().sparkline_for(1, 0);
    assert!(s.ends_with('▇'), "huge burst is the top bucket: {s}");
    assert!(s.chars().nth(4) == Some('▂'), "history rotates left: {s}");
    let pane = wm.pane(1).unwrap();
    let sp = wm.drawer().spinner_for(pane);
    assert!(bilterm::wm::drawer::BRAILLE_SPINNER_FRAMES.contains(&sp));
}

#[test]
fn mouse_wheel_in_vertical_drawer_scrolls_viewport_without_moving_selection() {
    let mut wm = wm_new(30, 100);
    for _ in 0..9 {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.set_drawer_visibility(Some(DrawerVisibilityMode::Fixed));
    wm.focus_drawer();
    let area = Rect::new(0, 0, 100, 12);
    let rows_len = wm.drawer_rows().len();
    assert_eq!(rows_len, 12);
    let sel = wm.drawer().selected_index;
    let dr = wm.drawer_rect(area).unwrap();
    let inner_h = dr.height - 2;
    let (hdr, ftr, step) = bilterm::ui::drawer::drawer_vertical_layout_metrics(wm.config(), inner_h, rows_len);
    let cap = (usize::from(inner_h - hdr - ftr).max(1) / step).max(1);
    assert!(cap < rows_len);
    let max = rows_len - cap;

    mouse(&mut wm, MouseEventKind::ScrollUp, 5, 5, KeyModifiers::NONE, area);
    // selection is last row; view starts pinned at max, scrolling up by 3
    assert_eq!(wm.drawer().view_scroll, Some((sel, max.saturating_sub(3))));
    assert_eq!(wm.drawer().selected_index, sel, "wheel does not move selection");
    for _ in 0..10 {
        mouse(&mut wm, MouseEventKind::ScrollUp, 5, 5, KeyModifiers::NONE, area);
    }
    assert_eq!(wm.drawer().view_scroll, Some((sel, 0)));
    for _ in 0..10 {
        mouse(&mut wm, MouseEventKind::ScrollDown, 5, 5, KeyModifiers::NONE, area);
    }
    assert_eq!(wm.drawer().view_scroll, Some((sel, max)));
}

#[test]
fn mouse_wheel_in_horizontal_drawer_moves_selection() {
    let mut wm = three_pane_focused();
    wm.set_drawer_edge(Some(DrawerEdge::Bottom));
    wm.drawer_mut().selected_index = 2;
    let y = AREA.height - 2;
    mouse(&mut wm, MouseEventKind::ScrollUp, 10, y, KeyModifiers::NONE, AREA);
    assert_eq!(wm.drawer().selected_index, 1);
    mouse(&mut wm, MouseEventKind::ScrollDown, 10, y, KeyModifiers::NONE, AREA);
    mouse(&mut wm, MouseEventKind::ScrollDown, 10, y, KeyModifiers::NONE, AREA);
    assert_eq!(wm.drawer().selected_index, 3);
}

#[test]
fn shift_wheel_and_scroll_left_right_scroll_horizontally() {
    let mut wm = three_pane_focused();
    mouse(&mut wm, MouseEventKind::ScrollDown, 5, 5, KeyModifiers::SHIFT, AREA);
    assert_eq!(wm.drawer().h_scroll, 6, "delta 3 * 2");
    mouse(&mut wm, MouseEventKind::ScrollDown, 5, 5, KeyModifiers::SHIFT, AREA);
    assert_eq!(wm.drawer().h_scroll, 12);
    mouse(&mut wm, MouseEventKind::ScrollUp, 5, 5, KeyModifiers::SHIFT, AREA);
    assert_eq!(wm.drawer().h_scroll, 6);
    assert!(wm.drawer().view_scroll.is_none(), "shift wheel doesn't touch vertical viewport");
    mouse(&mut wm, MouseEventKind::ScrollRight, 5, 5, KeyModifiers::NONE, AREA);
    assert_eq!(wm.drawer().h_scroll, 12);
    mouse(&mut wm, MouseEventKind::ScrollLeft, 5, 5, KeyModifiers::NONE, AREA);
    mouse(&mut wm, MouseEventKind::ScrollLeft, 5, 5, KeyModifiers::NONE, AREA);
    assert_eq!(wm.drawer().h_scroll, 0);
    // outside the drawer nothing scrolls
    mouse(&mut wm, MouseEventKind::ScrollRight, 80, 5, KeyModifiers::NONE, AREA);
    assert_eq!(wm.drawer().h_scroll, 0);
}

#[test]
fn drag_on_resize_border_resizes_each_edge_and_mouse_up_ends_drag() {
    let left = Rect::new(0, 0, 100, 24);
    // Left: border column is x + width - 1 = 33
    let mut wm = three_pane_focused();
    mouse(&mut wm, MouseEventKind::Down(MouseButton::Left), 33, 10, KeyModifiers::NONE, left);
    assert!(wm.drawer().resizing_drag);
    mouse(&mut wm, MouseEventKind::Drag(MouseButton::Left), 40, 10, KeyModifiers::NONE, left);
    assert_eq!(wm.drawer_config().width_cols, 41);
    mouse(&mut wm, MouseEventKind::Drag(MouseButton::Left), 2, 10, KeyModifiers::NONE, left);
    assert_eq!(wm.drawer_config().width_cols, 16, "clamped to minimum");
    mouse(&mut wm, MouseEventKind::Up(MouseButton::Left), 2, 10, KeyModifiers::NONE, left);
    assert!(!wm.drawer().resizing_drag);

    // Right: border column is dr.x
    let mut wm = three_pane_focused();
    wm.set_drawer_edge(Some(DrawerEdge::Right));
    mouse(&mut wm, MouseEventKind::Down(MouseButton::Left), 66, 10, KeyModifiers::NONE, left);
    assert!(wm.drawer().resizing_drag);
    mouse(&mut wm, MouseEventKind::Drag(MouseButton::Left), 50, 10, KeyModifiers::NONE, left);
    assert_eq!(wm.drawer_config().width_cols, 50);

    // Top: border row is y + height - 1 = 5
    let mut wm = three_pane_focused();
    wm.set_drawer_edge(Some(DrawerEdge::Top));
    mouse(&mut wm, MouseEventKind::Down(MouseButton::Left), 50, 5, KeyModifiers::NONE, left);
    assert!(wm.drawer().resizing_drag);
    mouse(&mut wm, MouseEventKind::Drag(MouseButton::Left), 50, 9, KeyModifiers::NONE, left);
    assert_eq!(wm.drawer_config().height_rows, 10);
    mouse(&mut wm, MouseEventKind::Drag(MouseButton::Left), 50, 20, KeyModifiers::NONE, left);
    assert_eq!(wm.drawer_config().height_rows, 14, "clamped to max");

    // Bottom: border row is dr.y = 18
    let mut wm = three_pane_focused();
    wm.set_drawer_edge(Some(DrawerEdge::Bottom));
    mouse(&mut wm, MouseEventKind::Down(MouseButton::Left), 50, 18, KeyModifiers::NONE, left);
    assert!(wm.drawer().resizing_drag);
    mouse(&mut wm, MouseEventKind::Drag(MouseButton::Left), 50, 14, KeyModifiers::NONE, left);
    assert_eq!(wm.drawer_config().height_rows, 10);
}

#[test]
fn click_in_list_selects_and_activates_row_and_click_outside_unfocuses() {
    let mut wm = three_pane_focused();
    let dr = wm.drawer_rect(AREA).unwrap();
    let inner_h = dr.height - 2;
    let rows_len = wm.drawer_rows().len();
    let (hdr, _ftr, step) = bilterm::ui::drawer::drawer_vertical_layout_metrics(wm.config(), inner_h, rows_len);
    let list_start = dr.y + 1 + hdr;
    // click first pane row (row index 2)
    let y = list_start + 2 * step as u16;
    mouse(&mut wm, MouseEventKind::Down(MouseButton::Left), 5, y, KeyModifiers::NONE, AREA);
    assert_eq!(wm.drawer().selected_index, 2);
    assert_eq!(wm.active_pane_id(), 1);
    // FOCUS-01: one click on a pane row hands that pane the keyboard; the drawer stays open.
    assert!(!wm.drawer().focused && wm.drawer().open);
    // click outside drawer unfocuses
    wm.focus_drawer();
    assert!(wm.drawer().focused);
    mouse(&mut wm, MouseEventKind::Down(MouseButton::Left), 80, 10, KeyModifiers::NONE, AREA);
    assert!(!wm.drawer().focused);
}

#[test]
fn dragging_across_rows_reorders_selected_pane() {
    let mut wm = three_pane_focused();
    let dr = wm.drawer_rect(AREA).unwrap();
    let inner_h = dr.height - 2;
    let rows_len = wm.drawer_rows().len();
    let (hdr, _ftr, step) = bilterm::ui::drawer::drawer_vertical_layout_metrics(wm.config(), inner_h, rows_len);
    let list_start = dr.y + 1 + hdr;
    assert_eq!(wm.ordered_pane_ids(), vec![1, 2, 3]);
    // selection is row 4 (pane 3); drag to row 3 (pane 2) -> moves pane 3 up
    let y = list_start + 3 * step as u16;
    mouse(&mut wm, MouseEventKind::Drag(MouseButton::Left), 5, y, KeyModifiers::NONE, AREA);
    assert_eq!(wm.ordered_pane_ids(), vec![1, 3, 2]);
    // dragging onto the same row is a no-op
    let sel = wm.drawer().selected_index;
    let y = list_start + sel as u16 * step as u16;
    mouse(&mut wm, MouseEventKind::Drag(MouseButton::Left), 5, y, KeyModifiers::NONE, AREA);
    assert_eq!(wm.ordered_pane_ids(), vec![1, 3, 2]);
}

#[test]
fn title_bar_clicks_cycle_edge_layout_and_visibility() {
    let mut wm = three_pane_focused();
    wm.resize_drawer(60); // wide enough that the badge zone does not shadow the visibility zone
    // rel_x < 16 on the title row cycles the dock edge
    mouse(&mut wm, MouseEventKind::Down(MouseButton::Left), 8, 0, KeyModifiers::NONE, AREA);
    assert_eq!(wm.drawer_config().edge, DrawerEdge::Right);
    // drawer is now on the right: its title row starts at x=50 (width capped at half the area)
    mouse(&mut wm, MouseEventKind::Down(MouseButton::Left), 50 + 18, 0, KeyModifiers::NONE, AREA);
    assert_eq!(wm.drawer_config().layout, DrawerLayoutMode::Overlay);
    mouse(&mut wm, MouseEventKind::Down(MouseButton::Left), 50 + 30, 0, KeyModifiers::NONE, AREA);
    assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::Fixed);
    assert!(wm.drawer().open);
    mouse(&mut wm, MouseEventKind::Down(MouseButton::Left), 50 + 30, 0, KeyModifiers::NONE, AREA);
    assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::OnDemand);
}

#[test]
fn clicks_on_title_row_two_create_window_and_session() {
    let mut wm = three_pane_focused();
    wm.set_drawer_edge(Some(DrawerEdge::Left));
    wm.resize_drawer(60);
    let n = wm.pane_count();
    // row y=1, rel_x >= 38 -> new session
    mouse(&mut wm, MouseEventKind::Down(MouseButton::Left), 45, 1, KeyModifiers::NONE, AREA);
    assert!(status(&wm).starts_with("Created new session '"), "{}", status(&wm));
    // row y=1, 31 <= rel_x < 38 -> new window
    let n2 = wm.pane_count();
    mouse(&mut wm, MouseEventKind::Down(MouseButton::Left), 33, 1, KeyModifiers::NONE, AREA);
    assert_eq!(wm.pane_count(), n2 + 1);
    assert!(status(&wm).starts_with("Created new window #"));
    let _ = n;
}

#[test]
fn footer_button_row_actions() {
    let mut wm = three_pane_focused();
    wm.drawer_mut().focused = false;
    let dr = wm.drawer_rect(AREA).unwrap();
    let y = dr.y + dr.height - 2;
    let before = wm.pane_count();
    // rel_x < win_end -> new window
    mouse(&mut wm, MouseEventKind::Down(MouseButton::Left), 2, y, KeyModifiers::NONE, AREA);
    assert_eq!(wm.pane_count(), before + 1);
    assert!(wm.drawer().focused);
    // far right of the footer -> close drawer
    mouse(&mut wm, MouseEventKind::Down(MouseButton::Left), 33 - 1, y, KeyModifiers::NONE, AREA);
    assert!(!wm.drawer().focused, "close_drawer drops focus: {}", status(&wm));
}
