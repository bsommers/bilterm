//! Second half of the `mouse_wm.rs` coverage: menubar drop-down, MCP console panel, quick terminal
//! border and drawer buttons, all driven through `handle_wm_mouse_event`.

use bilterm::action::Action;
use bilterm::input::mouse::handle_wm_mouse_event;
use bilterm::mcp_console::McpRowAction;
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::{DrawerEdge, Modal, WindowManager, WmMouseDrag};
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

const COLS: u16 = 100;
const ROWS: u16 = 24;
const AREA: Rect = Rect { x: 0, y: 0, width: COLS, height: ROWS };
const NONE: KeyModifiers = KeyModifiers::NONE;

fn wm_sized(rows: u16, cols: u16) -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(rows, cols);
    let area = Rect::new(0, 0, cols, rows);
    wm.resize_all(area).unwrap();
    (wm, area)
}

fn fire(wm: &mut WindowManager, kind: MouseEventKind, col: u16, row: u16, mods: KeyModifiers, area: Rect) {
    let ev = MouseEvent { kind, column: col, row, modifiers: mods };
    handle_wm_mouse_event(wm, ev, area).unwrap();
}

fn down(wm: &mut WindowManager, col: u16, row: u16) {
    fire(wm, MouseEventKind::Down(MouseButton::Left), col, row, NONE, AREA);
}
fn drag(wm: &mut WindowManager, col: u16, row: u16) {
    fire(wm, MouseEventKind::Drag(MouseButton::Left), col, row, NONE, AREA);
}
fn up(wm: &mut WindowManager, col: u16, row: u16) {
    fire(wm, MouseEventKind::Up(MouseButton::Left), col, row, NONE, AREA);
}

/// Returns `(first_id, second_id)` ordered by on-screen position (x then y).
fn ordered_ids(wm: &WindowManager) -> (usize, usize) {
    let mut v = wm.compute_workspace_rects(AREA);
    v.sort_by_key(|&(_, r)| (r.x, r.y));
    (v[0].0, v[1].0)
}

/// Column of the minimize / zoom / close badge on a pane's title bar, derived from the live glyph set.
fn badge_col(wm: &WindowManager, outer: Rect, which: u8) -> u16 {
    let g = wm.config().glyph_set();
    let (m, z, c) = (
        g.minimize_badge.chars().count() as u16,
        g.zoom_badge.chars().count() as u16,
        g.close_badge.chars().count() as u16,
    );
    let start = (outer.x + outer.width).saturating_sub(m + z + c + 2);
    match which {
        0 => start,
        1 => start + m,
        _ => start + m + z,
    }
}

fn click_badge(wm: &mut WindowManager, outer: Rect, which: u8) {
    let col = badge_col(wm, outer, which);
    down(wm, col, outer.y);
}


// ---------------------------------------------------------------- menubar drop-down

fn menubar_wm() -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.config_mut().show_top_menubar = true;
    wm.resize_all(AREA).unwrap();
    // the menubar takes row 0; the drop-down lives in the workspace below it
    (wm, Rect::new(0, 1, COLS, ROWS - 1))
}

fn menubar_item_pos(cat: usize, item: usize, ws: Rect) -> (u16, u16) {
    let dd = bilterm::ui::modals::menubar_dropdown_rect(cat, ws);
    let inner = bilterm::wm::inset_window_frame(dd);
    (inner.x + 2, inner.y + item as u16)
}

#[test]
fn menubar_title_click_opens_toggles_and_switches_categories() {
    let (mut wm, _ws) = menubar_wm();
    let x1 = bilterm::ui::modals::menubar_category_x_offset(1) + 1;
    let x2 = bilterm::ui::modals::menubar_category_x_offset(2) + 1;
    down(&mut wm, x1, 0);
    assert_eq!(wm.menubar_open_category(), Some(1));
    // hovering another title with a menu open switches to it
    fire(&mut wm, MouseEventKind::Moved, x2, 0, NONE, AREA);
    assert_eq!(wm.menubar_open_category(), Some(2));
    fire(&mut wm, MouseEventKind::Drag(MouseButton::Left), x1, 0, NONE, AREA);
    assert_eq!(wm.menubar_open_category(), Some(1));
    // clicking the open title closes it
    down(&mut wm, x1, 0);
    assert_eq!(wm.menubar_open_category(), None);
    // hovering with nothing open does not open anything
    fire(&mut wm, MouseEventKind::Moved, x2, 0, NONE, AREA);
    assert_eq!(wm.menubar_open_category(), None);
    // clicking dead space on the bar closes an open menu
    down(&mut wm, x1, 0);
    down(&mut wm, COLS - 1, 0);
    assert_eq!(wm.menubar_open_category(), None);
}

#[test]
fn menubar_help_title_toggles_the_help_modal() {
    let (mut wm, _) = menubar_wm();
    let x6 = bilterm::ui::modals::menubar_category_x_offset(6) + 1;
    down(&mut wm, x6, 0);
    assert_eq!(wm.menubar_open_category(), Some(6));
    down(&mut wm, x6, 0); // open Help category => toggles Help modal
    assert!(wm.is_help_visible());
    down(&mut wm, x6, 0); // Help visible => toggled off
    assert!(matches!(wm.modal(), Modal::None));
}

#[test]
fn menubar_dropdown_hover_tracks_the_item_under_the_pointer() {
    let (mut wm, ws) = menubar_wm();
    wm.set_menubar_open_category(Some(1));
    let (c, r) = menubar_item_pos(1, 3, ws);
    fire(&mut wm, MouseEventKind::Moved, c, r, NONE, AREA);
    assert_eq!(wm.menubar_state(), Some((1, 3)));
    let (c, r) = menubar_item_pos(1, 5, ws);
    fire(&mut wm, MouseEventKind::Drag(MouseButton::Left), c, r, NONE, AREA);
    assert_eq!(wm.menubar_state(), Some((1, 5)));
    // border row of the dropdown has no item: hover leaves the selection untouched
    let dd = bilterm::ui::modals::menubar_dropdown_rect(1, ws);
    fire(&mut wm, MouseEventKind::Moved, dd.x + 3, dd.y, NONE, AREA);
    assert_eq!(wm.menubar_state(), Some((1, 5)));
}

#[test]
fn menubar_dropdown_click_runs_the_item_and_closes_the_menu() {
    let (mut wm, ws) = menubar_wm();
    wm.set_menubar_open_category(Some(1));
    let (c, r) = menubar_item_pos(1, 0, ws); // ":split h"
    let before = wm.pane_count();
    down(&mut wm, c, r);
    assert!(matches!(wm.modal(), Modal::None));
    assert_eq!(wm.pane_count(), before + 1);
}

#[test]
fn menubar_dropdown_release_also_activates_but_border_click_does_not() {
    let (mut wm, ws) = menubar_wm();
    wm.set_menubar_open_category(Some(1));
    let dd = bilterm::ui::modals::menubar_dropdown_rect(1, ws);
    down(&mut wm, dd.x + 3, dd.y); // frame row: swallowed, menu stays open
    assert_eq!(wm.menubar_open_category(), Some(1));
    let (c, r) = menubar_item_pos(1, 0, ws);
    let before = wm.pane_count();
    up(&mut wm, c, r);
    assert_eq!(wm.pane_count(), before + 1);
    assert_eq!(wm.menubar_open_category(), None);
}

#[test]
fn menubar_dropdown_item_that_quits_returns_quit_or_opens_guard() {
    let (mut wm, ws) = menubar_wm();
    wm.set_menubar_open_category(Some(0));
    let (c, r) = menubar_item_pos(0, 6, ws); // ":quit"
    let ev = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: c, row: r, modifiers: NONE };
    let res = handle_wm_mouse_event(&mut wm, ev, AREA).unwrap();
    assert!(res == bilterm::input::AppAction::Quit || matches!(wm.modal(), Modal::ExitGuard(_)), "{res:?}");
}

#[test]
fn menubar_click_outside_dropdown_closes_it_but_other_events_pass_through() {
    let (mut wm, _ws) = menubar_wm();
    wm.set_menubar_open_category(Some(1));
    // wheel outside the dropdown does not close it
    fire(&mut wm, MouseEventKind::ScrollDown, COLS - 2, ROWS - 3, NONE, AREA);
    assert_eq!(wm.menubar_open_category(), Some(1));
    // a click far from the dropdown closes it
    down(&mut wm, COLS - 2, ROWS - 3);
    assert_eq!(wm.menubar_open_category(), None);
}

#[test]
fn menubar_dropdown_wheel_inside_is_swallowed() {
    let (mut wm, ws) = menubar_wm();
    wm.set_menubar_open_category(Some(2));
    let (c, r) = menubar_item_pos(2, 1, ws);
    fire(&mut wm, MouseEventKind::ScrollDown, c, r, NONE, AREA);
    assert_eq!(wm.menubar_open_category(), Some(2));
    assert_eq!(wm.menubar_state(), Some((2, 0)));
}

// ---------------------------------------------------------------- MCP console

fn mcp_wm(modern: bool) -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.config_mut().modern_mode = modern;
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(AREA).unwrap();
    let ids: Vec<usize> = wm.panes().keys().copied().collect();
    let mcp = wm.mcp_console_mut();
    mcp.active = true;
    mcp.view_lines = (0..40)
        .map(|i| match i {
            3 => format!("Pane #{} {}", ids[0], "x".repeat(70)),
            4 => format!("Pane #{} {}", ids[1], "y".repeat(70)),
            _ => format!("row {i:02} {}", "z".repeat(70)),
        })
        .collect();
    mcp.row_actions = (0..40)
        .map(|i| match i {
            3 => McpRowAction::FocusPane(ids[0]),
            4 => McpRowAction::FocusPane(ids[1]),
            _ => McpRowAction::None,
        })
        .collect();
    let rect = wm.mcp_console().panel_rect(AREA).unwrap();
    (wm, rect)
}

#[test]
fn mcp_title_row_right_edge_closes_the_console_without_touching_panes() {
    let (mut wm, rect) = mcp_wm(false);
    let panes = wm.pane_count();
    down(&mut wm, rect.x + rect.width - 2, rect.y);
    assert!(!wm.mcp_console().active);
    assert_eq!(wm.pane_count(), panes);
}

#[test]
fn mcp_title_row_middle_toggles_overlay_push_and_left_part_only_reflows() {
    let (mut wm, rect) = mcp_wm(false);
    let start = wm.mcp_console().layout_mode;
    down(&mut wm, rect.x + 1, rect.y);
    assert_eq!(wm.mcp_console().layout_mode, start);
    assert!(wm.mcp_console().active);
    down(&mut wm, rect.x + rect.width / 2 + 1, rect.y);
    assert_ne!(wm.mcp_console().layout_mode, start);
    assert!(wm.mcp_console().active);
}

#[test]
fn mcp_footer_row_regions_toggle_layout_cycle_edge_and_close() {
    let (mut wm, rect) = mcp_wm(false);
    assert!(rect.width > 56, "panel wide enough for the footer buttons: {rect:?}");
    let y = rect.y + rect.height - 1;
    let layout = wm.mcp_console().layout_mode;
    down(&mut wm, rect.x + 20, y);
    assert_ne!(wm.mcp_console().layout_mode, layout);
    let rect = wm.mcp_console().panel_rect(AREA).unwrap();
    let y = rect.y + rect.height - 1;
    let edge = wm.mcp_console().edge;
    down(&mut wm, rect.x + 35, y);
    assert_ne!(wm.mcp_console().edge, edge);
    // gap between buttons does nothing
    let rect = wm.mcp_console().panel_rect(AREA).unwrap();
    let y = rect.y + rect.height - 1;
    down(&mut wm, rect.x + 3, y);
    assert!(wm.mcp_console().active);
    down(&mut wm, rect.x + 45, y);
    assert!(!wm.mcp_console().active);
}

#[test]
fn mcp_modern_header_row_regions_toggle_cycle_and_close() {
    let (mut wm, rect) = mcp_wm(true);
    assert!(rect.height >= 8 && rect.width > 73, "{rect:?}");
    let hdr_y = rect.y + 1;
    let layout = wm.mcp_console().layout_mode;
    down(&mut wm, rect.x + 5, hdr_y);
    assert_ne!(wm.mcp_console().layout_mode, layout);
    let rect = wm.mcp_console().panel_rect(AREA).unwrap();
    let edge = wm.mcp_console().edge;
    down(&mut wm, rect.x + 45, rect.y + 1);
    assert_ne!(wm.mcp_console().edge, edge);
    let rect = wm.mcp_console().panel_rect(AREA).unwrap();
    down(&mut wm, rect.x + 65, rect.y + 1); // 58..72 is a dead zone
    assert!(wm.mcp_console().active);
    assert!(rect.width > 74, "{rect:?}");
    down(&mut wm, rect.x + 73, rect.y + 1);
    assert!(!wm.mcp_console().active);
}

#[test]
fn mcp_list_click_selects_the_row_and_activates_its_pane_action() {
    let (mut wm, rect) = mcp_wm(false);
    let ids: Vec<usize> = wm.panes().keys().copied().collect();
    // plain (non-modern) layout: list starts directly under the title border
    down(&mut wm, rect.x + 4, rect.y + 1 + 3);
    assert_eq!(wm.active_pane_id(), ids[0]);
    assert_eq!(wm.mcp_console().selected_row, 3);
    down(&mut wm, rect.x + 4, rect.y + 1 + 4);
    assert_eq!(wm.active_pane_id(), ids[1]);
    assert_eq!(wm.mcp_console().selected_row, 4);
}

#[test]
fn mcp_list_click_honours_scroll_offset_and_ignores_rows_past_the_end() {
    let (mut wm, rect) = mcp_wm(false);
    wm.mcp_console_mut().scroll_offset = 2;
    // visible row 1 is line index 3
    let ids: Vec<usize> = wm.panes().keys().copied().collect();
    down(&mut wm, rect.x + 4, rect.y + 1 + 1);
    assert_eq!(wm.mcp_console().selected_row, 3);
    assert_eq!(wm.active_pane_id(), ids[0]);
    // beyond the last line: selection unchanged
    wm.mcp_console_mut().view_lines.truncate(6);
    wm.mcp_console_mut().row_actions.truncate(6);
    wm.mcp_console_mut().scroll_offset = 30;
    down(&mut wm, rect.x + 4, rect.y + 1 + 2);
    assert_eq!(wm.mcp_console().selected_row, 3);
}

#[test]
fn mcp_modern_list_click_skips_the_header_rows() {
    let (mut wm, rect) = mcp_wm(true);
    let (hdr, _ftr) = bilterm::ui::mcp_ui::mcp_console_layout_metrics(wm.config(), rect.height);
    assert_eq!(hdr, 2);
    let ids: Vec<usize> = wm.panes().keys().copied().collect();
    down(&mut wm, rect.x + 4, rect.y + 1 + hdr + 4);
    assert_eq!(wm.mcp_console().selected_row, 4);
    assert_eq!(wm.active_pane_id(), ids[1]);
}

#[test]
fn mcp_wheel_scrolls_two_rows_and_horizontal_wheel_scrolls_six_columns() {
    let (mut wm, rect) = mcp_wm(false);
    let (c, r) = (rect.x + 4, rect.y + 3);
    fire(&mut wm, MouseEventKind::ScrollDown, c, r, NONE, AREA);
    assert_eq!(wm.mcp_console().scroll_offset, 2);
    fire(&mut wm, MouseEventKind::ScrollDown, c, r, NONE, AREA);
    assert_eq!(wm.mcp_console().scroll_offset, 4);
    fire(&mut wm, MouseEventKind::ScrollUp, c, r, NONE, AREA);
    assert_eq!(wm.mcp_console().scroll_offset, 2);
    fire(&mut wm, MouseEventKind::ScrollRight, c, r, NONE, AREA);
    assert_eq!(wm.mcp_console().h_scroll, 6);
    fire(&mut wm, MouseEventKind::ScrollLeft, c, r, NONE, AREA);
    fire(&mut wm, MouseEventKind::ScrollLeft, c, r, NONE, AREA);
    assert_eq!(wm.mcp_console().h_scroll, 0);
}

#[test]
fn mcp_swallows_other_events_inside_but_passes_clicks_outside_to_panes() {
    let (mut wm, rect) = mcp_wm(false);
    let before = wm.active_pane_id();
    fire(&mut wm, MouseEventKind::Moved, rect.x + 3, rect.y + 3, NONE, AREA);
    fire(&mut wm, MouseEventKind::Down(MouseButton::Right), rect.x + 3, rect.y + 3, NONE, AREA);
    assert_eq!(wm.active_pane_id(), before);
    assert!(wm.mcp_console().active);
    // find a workspace cell outside the panel and inside the non-active pane
    let (first, _) = ordered_ids(&wm);
    let target = wm
        .compute_workspace_rects(AREA)
        .into_iter()
        .find(|&(id, _)| id == first)
        .unwrap()
        .1;
    let cell = (target.x..target.x + target.width)
        .flat_map(|x| (target.y..target.y + target.height).map(move |y| (x, y)))
        .find(|&(x, y)| !(x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height));
    if let Some((x, y)) = cell {
        down(&mut wm, x, y);
        assert_eq!(wm.active_pane_id(), first);
    }
}

#[test]
fn mcp_inactive_console_does_not_capture_clicks() {
    let (mut wm, rect) = mcp_wm(false);
    wm.mcp_console_mut().active = false;
    let layout = wm.mcp_console().layout_mode;
    down(&mut wm, rect.x + rect.width / 2 + 1, rect.y);
    assert_eq!(wm.mcp_console().layout_mode, layout);
}

// ---------------------------------------------------------------- quick terminal border

fn quick_wm() -> (WindowManager, Rect) {
    let (mut wm, _) = wm_sized(ROWS, COLS);
    wm.toggle_quick_terminal();
    assert!(wm.quick_terminal().visible);
    let outer = wm.quick_terminal().overlay_rect(AREA);
    (wm, outer)
}

#[test]
fn quick_terminal_close_and_minimize_badges_hide_it() {
    for which in [0u8, 2] {
        let (mut wm, outer) = quick_wm();
        click_badge(&mut wm, outer, which);
        assert!(!wm.quick_terminal().visible, "badge {which}");
    }
}

#[test]
fn quick_terminal_zoom_badge_toggles_between_tall_and_default_height() {
    let (mut wm, outer) = quick_wm();
    let initial = wm.quick_terminal().height_pct;
    click_badge(&mut wm, outer, 1);
    let tall = wm.quick_terminal().height_pct;
    assert_eq!(tall, 90);
    assert!(wm.quick_terminal().visible);
    let outer = wm.quick_terminal().overlay_rect(AREA);
    click_badge(&mut wm, outer, 1);
    assert_eq!(wm.quick_terminal().height_pct, 40);
    let _ = initial;
}

#[test]
fn quick_terminal_title_click_off_the_badges_is_swallowed() {
    let (mut wm, outer) = quick_wm();
    let pct = wm.quick_terminal().height_pct;
    down(&mut wm, outer.x + 3, outer.y);
    assert_eq!(wm.wm_mouse_drag(), None);
    assert!(wm.quick_terminal().visible);
    assert_eq!(wm.quick_terminal().height_pct, pct);
}

#[test]
fn quick_terminal_bottom_border_drag_resizes_by_percentage_with_clamps() {
    let (mut wm, outer) = quick_wm();
    let bottom = outer.y + outer.height - 1;
    down(&mut wm, outer.x + 30, bottom);
    assert_eq!(wm.wm_mouse_drag(), Some(WmMouseDrag::ResizeQuickTerminal));
    drag(&mut wm, 30, 11);
    assert_eq!(wm.quick_terminal().height_pct, ((11 + 1) * 100 / u32::from(ROWS)) as u16);
    drag(&mut wm, 30, 0);
    assert_eq!(wm.quick_terminal().height_pct, 15, "lower clamp");
    drag(&mut wm, 30, ROWS + 20);
    assert_eq!(wm.quick_terminal().height_pct, 95, "upper clamp");
    up(&mut wm, 30, 5);
    assert_eq!(wm.wm_mouse_drag(), None);
}

#[test]
fn quick_terminal_side_border_also_starts_a_resize() {
    let (mut wm, outer) = quick_wm();
    down(&mut wm, outer.x, outer.y + 2);
    assert_eq!(wm.wm_mouse_drag(), Some(WmMouseDrag::ResizeQuickTerminal));
    up(&mut wm, 0, 0);
    down(&mut wm, outer.x + outer.width - 1, outer.y + 2);
    assert_eq!(wm.wm_mouse_drag(), Some(WmMouseDrag::ResizeQuickTerminal));
}

// ---------------------------------------------------------------- drawer buttons / extra branches

fn three_pane_drawer() -> WindowManager {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.split_focused(SplitDirection::Horizontal).unwrap();
    wm.focus_drawer();
    wm
}

#[test]
fn drawer_title_badges_shrink_maximise_and_close() {
    let mut wm = three_pane_drawer();
    let dr = wm.drawer_rect(AREA).unwrap();
    let base_w = dr.width;
    click_badge(&mut wm, dr, 1); // maximise
    let big = wm.drawer_rect(AREA).unwrap();
    assert!(big.width > base_w || big.height > dr.height, "{dr:?} -> {big:?}");
    click_badge(&mut wm, big, 0); // minimise (DrawerSize(0)) restores
    let small = wm.drawer_rect(AREA).unwrap();
    assert!(small.width <= big.width);
    let dr = wm.drawer_rect(AREA).unwrap();
    click_badge(&mut wm, dr, 2); // close
    assert_eq!(wm.drawer_rect(AREA), None);
}

#[test]
fn drawer_footer_buttons_new_window_session_move_reparent_and_close() {
    let mut wm = three_pane_drawer();
    let dr = wm.drawer_rect(AREA).unwrap();
    let y = dr.y + dr.height - 2;
    let rel = |x: u16| dr.x + 1 + x;
    // [Win]: creates a window
    let panes = wm.pane_count();
    down(&mut wm, rel(2), y);
    assert_eq!(wm.pane_count(), panes + 1);
    // [Move]: reorders the selected pane (or at least is handled and keeps drawer focus)
    wm.drawer_mut().selected_index = 3;
    let before = wm.ordered_pane_ids();
    down(&mut wm, rel(17), y);
    assert_ne!(wm.ordered_pane_ids(), before, "selected pane moved down by the footer button");
    // [Parent]: handled
    down(&mut wm, rel(25), y);
    assert!(wm.drawer().focused);
    // anything right of the last button closes the drawer
    assert!(wm.drawer().focused);
    down(&mut wm, rel(40), y);
    assert!(!wm.drawer().focused, "close_drawer drops focus (a Fixed drawer stays visible)");
}

#[test]
fn drawer_footer_session_button_creates_a_session() {
    let mut wm = three_pane_drawer();
    let tmp = tempfile::tempdir().unwrap();
    wm.config_mut().session_dir = Some(tmp.path().to_string_lossy().into_owned());
    let dr = wm.drawer_rect(AREA).unwrap();
    let y = dr.y + dr.height - 2;
    down(&mut wm, dr.x + 1 + 9, y);
    assert_eq!(wm.pane_count(), 1, "a fresh session starts with a single pane");
    assert!(wm.active_pane().status_message().unwrap_or("").contains("Created new session"));
}

#[test]
fn horizontal_drawer_click_selects_next_and_activates() {
    let mut wm = three_pane_drawer();
    wm.set_drawer_edge(Some(DrawerEdge::Bottom));
    let dr = wm.drawer_rect(AREA).unwrap();
    wm.drawer_mut().selected_index = 0;
    // a click on the body (not title / border rows) advances the selection
    down(&mut wm, dr.x + 5, dr.y + 2);
    assert!(wm.drawer().focused);
    assert!(wm.drawer().selected_index >= 1, "selection advanced: {}", wm.drawer().selected_index);
}

#[test]
fn drawer_ignores_other_buttons_and_drag_outside_the_list() {
    let mut wm = three_pane_drawer();
    let dr = wm.drawer_rect(AREA).unwrap();
    let sel = wm.drawer().selected_index;
    fire(&mut wm, MouseEventKind::Down(MouseButton::Right), dr.x + 4, dr.y + 4, NONE, AREA);
    fire(&mut wm, MouseEventKind::Up(MouseButton::Left), dr.x + 4, dr.y + 4, NONE, AREA);
    drag(&mut wm, dr.x + 4, dr.y); // title row: outside list range
    assert_eq!(wm.drawer().selected_index, sel);
    // Moved outside the drawer is not consumed by it
    fire(&mut wm, MouseEventKind::Moved, COLS - 2, 3, NONE, AREA);
}

#[test]
fn drawer_drag_to_a_lower_row_moves_the_session_row_down() {
    let mut wm = three_pane_drawer();
    let dr = wm.drawer_rect(AREA).unwrap();
    let inner_h = dr.height - 2;
    let rows_len = wm.drawer_rows().len();
    let (hdr, _f, step) = bilterm::ui::drawer::drawer_vertical_layout_metrics(wm.config(), inner_h, rows_len);
    let list_start = dr.y + 1 + hdr;
    wm.drawer_mut().selected_index = 3;
    let y = list_start + 4 * step as u16; // row below selection
    let before = wm.ordered_pane_ids();
    drag(&mut wm, dr.x + 5, y);
    assert_ne!(wm.ordered_pane_ids(), before);
}

#[allow(dead_code)]
fn _keep(_: Action) {}
