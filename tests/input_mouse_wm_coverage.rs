//! Behavioural coverage for `src/input/mouse_wm.rs`: pane frame buttons, border drags and modal
//! move/resize/click handling (menubar, MCP, quick terminal and drawer live in the _panels file).
//! Everything is driven through `handle_wm_mouse_event`.

use bilterm::input::mouse::handle_wm_mouse_event;
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::{Modal, WindowManager, WmMouseDrag, WorkspaceLayoutMode};
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

fn two_panes(dir: SplitDirection) -> WindowManager {
    let (mut wm, area) = wm_sized(ROWS, COLS);
    wm.split_focused(dir).unwrap();
    wm.resize_all(area).unwrap();
    wm
}

fn rect_of(wm: &WindowManager, id: usize) -> Rect {
    wm.compute_workspace_rects(AREA).into_iter().find(|&(i, _)| i == id).unwrap().1
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

// ---------------------------------------------------------------- title-bar buttons

#[test]
fn minimize_badge_toggles_minimized_state_of_the_clicked_pane() {
    let mut wm = two_panes(SplitDirection::Vertical);
    let (left, _right) = ordered_ids(&wm);
    let outer = rect_of(&wm, left);
    assert!(!wm.z_stack().is_minimized(left));
    click_badge(&mut wm, outer, 0);
    assert!(wm.z_stack().is_minimized(left));
    let outer = rect_of(&wm, left);
    click_badge(&mut wm, outer, 0);
    assert!(!wm.z_stack().is_minimized(left));
}

#[test]
fn zoom_badge_zooms_then_unzooms_and_raises_the_clicked_pane() {
    let mut wm = two_panes(SplitDirection::Vertical);
    let (left, right) = ordered_ids(&wm);
    assert_eq!(wm.active_pane_id(), right);
    let outer = rect_of(&wm, left);
    click_badge(&mut wm, outer, 1);
    assert_eq!(wm.zoomed_pane(), Some(left));
    assert_eq!(wm.active_pane_id(), left);
    // zoomed pane fills the workspace; its zoom badge is now at the full-width title bar
    let outer = rect_of(&wm, left);
    assert_eq!(outer, AREA);
    click_badge(&mut wm, outer, 1);
    assert_eq!(wm.zoomed_pane(), None);
}

#[test]
fn zoom_badge_on_a_minimized_pane_restores_it_instead_of_zooming() {
    let mut wm = two_panes(SplitDirection::Vertical);
    let (left, _) = ordered_ids(&wm);
    wm.toggle_minimize_pane(left);
    assert!(wm.z_stack().is_minimized(left));
    let outer = rect_of(&wm, left);
    click_badge(&mut wm, outer, 1);
    assert!(!wm.z_stack().is_minimized(left));
    assert_eq!(wm.zoomed_pane(), None);
}

#[test]
fn close_badge_removes_the_clicked_pane() {
    let mut wm = two_panes(SplitDirection::Vertical);
    let (left, right) = ordered_ids(&wm);
    let outer = rect_of(&wm, left);
    click_badge(&mut wm, outer, 2);
    assert_eq!(wm.pane_count(), 1);
    assert!(wm.pane(left).is_none());
    assert!(wm.pane(right).is_some());
}

#[test]
fn badges_are_ignored_on_a_title_bar_too_narrow_for_them() {
    // a pane narrower than 24 columns has no badge hit-testing; clicking its right end must not close it
    let (mut wm, area) = wm_sized(ROWS, 40);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(area).unwrap();
    let mut v = wm.compute_workspace_rects(area);
    v.sort_by_key(|&(_, r)| r.x);
    let (id, outer) = v[0];
    assert!(outer.width < 24);
    fire(&mut wm, MouseEventKind::Down(MouseButton::Left), outer.x + outer.width - 3, outer.y, NONE, area);
    assert_eq!(wm.pane_count(), 2);
    assert!(!wm.z_stack().is_minimized(id));
}

// ---------------------------------------------------------------- title drag: move / swap

#[test]
fn alt_click_on_title_starts_move_and_release_without_drag_just_clears_it() {
    let mut wm = two_panes(SplitDirection::Vertical);
    let (left, _) = ordered_ids(&wm);
    let outer = rect_of(&wm, left);
    fire(&mut wm, MouseEventKind::Down(MouseButton::Left), outer.x + 5, outer.y, KeyModifiers::ALT, AREA);
    assert_eq!(
        wm.wm_mouse_drag(),
        Some(WmMouseDrag::MoveWindow { pane_id: left, grab_dx: 5, grab_dy: 0 })
    );
    up(&mut wm, outer.x + 5, outer.y);
    assert_eq!(wm.wm_mouse_drag(), None);
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Tiled, "no drag => no layout switch");
}

#[test]
fn alt_drag_of_a_tiled_pane_switches_to_stacked_and_moves_the_window() {
    let mut wm = two_panes(SplitDirection::Vertical);
    let (left, _) = ordered_ids(&wm);
    let outer = rect_of(&wm, left);
    fire(&mut wm, MouseEventKind::Down(MouseButton::Left), outer.x + 5, outer.y, KeyModifiers::ALT, AREA);
    fire(&mut wm, MouseEventKind::Drag(MouseButton::Left), outer.x + 15, outer.y + 3, KeyModifiers::ALT, AREA);
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked);
    let moved = rect_of(&wm, left);
    assert_eq!((moved.x, moved.y), (outer.x + 10, outer.y + 3));
    assert!(moved.width > 0 && moved.height > 0);
    assert!(wm.active_pane().status_message().unwrap_or("").contains(&format!("Window #{left} moved to")));
    up(&mut wm, outer.x + 15, outer.y + 3);
    assert_eq!(wm.wm_mouse_drag(), None);
}

#[test]
fn dragging_a_window_left_of_the_workspace_clamps_at_the_origin() {
    let mut wm = two_panes(SplitDirection::Vertical);
    let (_, right) = ordered_ids(&wm);
    let outer = rect_of(&wm, right);
    fire(&mut wm, MouseEventKind::Down(MouseButton::Left), outer.x + 8, outer.y, KeyModifiers::ALT, AREA);
    fire(&mut wm, MouseEventKind::Drag(MouseButton::Left), 1, 0, KeyModifiers::ALT, AREA);
    let moved = rect_of(&wm, right);
    assert_eq!(moved.x, 0);
    assert_eq!(moved.y, 0);
}

#[test]
fn stacked_title_grip_drag_moves_without_alt() {
    let mut wm = two_panes(SplitDirection::Vertical);
    wm.set_layout_mode(WorkspaceLayoutMode::Stacked);
    let (id, outer) = {
        let id = wm.active_pane_id();
        (id, rect_of(&wm, id))
    };
    // grip = title text region: col in (x+1, x+18) and not on a badge
    down(&mut wm, outer.x + 4, outer.y);
    match wm.wm_mouse_drag() {
        Some(WmMouseDrag::MoveWindow { pane_id, grab_dx, .. }) => {
            assert_eq!(pane_id, id);
            assert_eq!(grab_dx, 4);
        }
        other => panic!("expected MoveWindow, got {other:?}"),
    }
    drag(&mut wm, outer.x + 4 + 6, outer.y + 2);
    let moved = rect_of(&wm, id);
    assert_eq!((moved.x, moved.y), (outer.x + 6, outer.y + 2));
}

#[test]
fn tiled_title_drag_without_alt_is_not_a_move() {
    let mut wm = two_panes(SplitDirection::Vertical);
    let (left, _) = ordered_ids(&wm);
    let outer = rect_of(&wm, left);
    // tiled + no ALT + column off the grip region and off the corner: falls through to border resize
    down(&mut wm, outer.x + 25, outer.y);
    assert!(
        !matches!(wm.wm_mouse_drag(), Some(WmMouseDrag::MoveWindow { .. })),
        "tiled title click must not start a window move: {:?}",
        wm.wm_mouse_drag()
    );
}

#[test]
fn tiled_move_drag_over_another_pane_swaps_the_two_leaves() {
    let mut wm = two_panes(SplitDirection::Vertical);
    let (left, right) = ordered_ids(&wm);
    let (lo, ro) = (rect_of(&wm, left), rect_of(&wm, right));
    // start the move with ALT, then drag without ALT while still tiled => swap branch
    fire(&mut wm, MouseEventKind::Down(MouseButton::Left), lo.x + 5, lo.y, KeyModifiers::ALT, AREA);
    drag(&mut wm, ro.x + 3, ro.y + 3);
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Tiled);
    assert_eq!(rect_of(&wm, left).x, ro.x, "left pane now occupies the right slot");
    assert_eq!(rect_of(&wm, right).x, lo.x);
    assert!(wm.active_pane().status_message().unwrap_or("").contains("Swapped Pane"));
}

#[test]
fn tiled_move_drag_over_own_rect_or_outside_does_not_swap() {
    let mut wm = two_panes(SplitDirection::Vertical);
    let (left, right) = ordered_ids(&wm);
    let (lo, ro) = (rect_of(&wm, left), rect_of(&wm, right));
    fire(&mut wm, MouseEventKind::Down(MouseButton::Left), lo.x + 5, lo.y, KeyModifiers::ALT, AREA);
    drag(&mut wm, lo.x + 7, lo.y + 4); // still inside own rect
    assert_eq!((rect_of(&wm, left), rect_of(&wm, right)), (lo, ro));
    drag(&mut wm, COLS + 50, ROWS + 50); // way outside any pane
    assert_eq!((rect_of(&wm, left), rect_of(&wm, right)), (lo, ro));
    assert!(wm.wm_mouse_drag().is_some());
}

#[test]
fn single_pane_move_drag_uses_the_stacked_path() {
    let (mut wm, _) = wm_sized(ROWS, COLS);
    let id = wm.active_pane_id();
    wm.set_wm_mouse_drag(Some(WmMouseDrag::MoveWindow { pane_id: id, grab_dx: 0, grab_dy: 0 }));
    drag(&mut wm, 10, 5);
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked);
    let r = rect_of(&wm, id);
    assert_eq!((r.x, r.y), (10, 5));
}

// ---------------------------------------------------------------- border resize (tiled split dividers)

#[test]
fn dragging_the_right_border_of_the_left_pane_moves_the_split_divider() {
    let mut wm = two_panes(SplitDirection::Vertical);
    let (left, right) = ordered_ids(&wm);
    let lo = rect_of(&wm, left);
    let right_x = lo.x + lo.width - 1;
    let mid_row = lo.y + lo.height / 2;
    down(&mut wm, right_x, mid_row);
    assert!(matches!(
        wm.wm_mouse_drag(),
        Some(WmMouseDrag::ResizeWindow { pane_id, edge: bilterm::wm::WindowResizeEdge::Right, .. }) if pane_id == left
    ));
    drag(&mut wm, right_x + 20, mid_row);
    let (l2, r2) = (rect_of(&wm, left), rect_of(&wm, right));
    assert!(l2.width > lo.width, "left pane grew: {lo:?} -> {l2:?}");
    assert!(r2.width < rect_of_width_before(&lo, COLS));
    up(&mut wm, right_x + 20, mid_row);
    assert_eq!(wm.wm_mouse_drag(), None);
    // dragging back shrinks it again
    wm.set_wm_mouse_drag(Some(WmMouseDrag::ResizeWindow {
        pane_id: left,
        edge: bilterm::wm::WindowResizeEdge::Right,
        initial_rect: l2,
    }));
    drag(&mut wm, l2.x + 10, mid_row);
    assert!(rect_of(&wm, left).width < l2.width);
}

fn rect_of_width_before(left: &Rect, total: u16) -> u16 {
    total - left.width
}

#[test]
fn dragging_the_left_border_of_the_right_pane_moves_the_same_divider() {
    let mut wm = two_panes(SplitDirection::Vertical);
    let (left, right) = ordered_ids(&wm);
    let ro = rect_of(&wm, right);
    let mid_row = ro.y + ro.height / 2;
    down(&mut wm, ro.x, mid_row);
    assert!(matches!(
        wm.wm_mouse_drag(),
        Some(WmMouseDrag::ResizeWindow { pane_id, edge: bilterm::wm::WindowResizeEdge::Left, .. }) if pane_id == right
    ));
    let before = rect_of(&wm, left).width;
    drag(&mut wm, ro.x + 15, mid_row);
    assert!(rect_of(&wm, left).width > before);
}

#[test]
fn dragging_bottom_and_top_borders_resizes_a_horizontal_split() {
    let mut wm = two_panes(SplitDirection::Horizontal);
    let mut v = wm.compute_workspace_rects(AREA);
    v.sort_by_key(|&(_, r)| r.y);
    let (top, bottom) = (v[0].0, v[1].0);
    let to = rect_of(&wm, top);
    let bottom_y = to.y + to.height - 1;
    down(&mut wm, to.x + 10, bottom_y);
    assert!(matches!(
        wm.wm_mouse_drag(),
        Some(WmMouseDrag::ResizeWindow { edge: bilterm::wm::WindowResizeEdge::Bottom, .. })
    ));
    drag(&mut wm, to.x + 10, bottom_y + 5);
    let grown = rect_of(&wm, top);
    assert!(grown.height > to.height);
    up(&mut wm, to.x + 10, bottom_y + 5);

    // now grab the bottom pane's top border and pull it back up
    let bo = rect_of(&wm, bottom);
    down(&mut wm, bo.x + 10, bo.y);
    assert!(matches!(
        wm.wm_mouse_drag(),
        Some(WmMouseDrag::ResizeWindow { edge: bilterm::wm::WindowResizeEdge::Top, .. })
    ));
    drag(&mut wm, bo.x + 10, bo.y - 6);
    assert!(rect_of(&wm, top).height < grown.height);
}

#[test]
fn corner_drag_resizes_in_both_axes_when_both_dividers_exist() {
    // left | (top / bottom): bottom-right corner of the top-right pane touches both dividers
    let (mut wm, area) = wm_sized(ROWS, COLS);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.split_focused(SplitDirection::Horizontal).unwrap();
    wm.resize_all(area).unwrap();
    let mut v = wm.compute_workspace_rects(area);
    v.sort_by_key(|&(_, r)| (r.x, r.y));
    let (tl, _, _) = (v[0].0, v[1].0, v[2].0);
    // the left pane is full height; its right border drag controls the vertical divider
    let lo = rect_of(&wm, tl);
    let corner = (lo.x + lo.width - 1, lo.y + lo.height - 1);
    down(&mut wm, corner.0, corner.1);
    assert!(matches!(
        wm.wm_mouse_drag(),
        Some(WmMouseDrag::ResizeWindow { edge: bilterm::wm::WindowResizeEdge::BottomRight, .. })
    ));
    drag(&mut wm, corner.0 + 10, corner.1);
    assert!(rect_of(&wm, tl).width > lo.width);
}

#[test]
fn resize_drag_fallback_mirrors_when_the_pane_has_no_border_on_that_side() {
    // the left-most pane has no divider on its LEFT edge: the drag falls back to the opposite
    // divider with a mirrored coordinate instead of doing nothing.
    let mut wm = two_panes(SplitDirection::Vertical);
    let (left, _) = ordered_ids(&wm);
    let lo = rect_of(&wm, left);
    wm.set_wm_mouse_drag(Some(WmMouseDrag::ResizeWindow {
        pane_id: left,
        edge: bilterm::wm::WindowResizeEdge::Left,
        initial_rect: lo,
    }));
    drag(&mut wm, lo.x + 12, lo.y + 3);
    assert_ne!(rect_of(&wm, left).width, lo.width, "mirrored fallback must change the split");
}

#[test]
fn resize_drag_with_no_movement_changes_nothing() {
    let mut wm = two_panes(SplitDirection::Vertical);
    let (left, right) = ordered_ids(&wm);
    let lo = rect_of(&wm, left);
    wm.set_wm_mouse_drag(Some(WmMouseDrag::ResizeWindow {
        pane_id: left,
        edge: bilterm::wm::WindowResizeEdge::Left,
        initial_rect: lo,
    }));
    let ro = rect_of(&wm, right);
    drag(&mut wm, lo.x, lo.y + 3); // column == initial_rect.x: fallback guard is false
    assert_eq!((rect_of(&wm, left), rect_of(&wm, right)), (lo, ro));
}

#[test]
fn stacked_resize_uses_rect_edges_and_reports_status() {
    let mut wm = two_panes(SplitDirection::Vertical);
    wm.set_layout_mode(WorkspaceLayoutMode::Stacked);
    let id = wm.active_pane_id();
    let outer = rect_of(&wm, id);
    // bottom-right corner drag grows the window
    down(&mut wm, outer.x + outer.width - 1, outer.y + outer.height - 1);
    assert!(matches!(
        wm.wm_mouse_drag(),
        Some(WmMouseDrag::ResizeWindow { edge: bilterm::wm::WindowResizeEdge::BottomRight, .. })
    ));
    drag(&mut wm, outer.x + outer.width + 4, outer.y + outer.height + 2);
    let grown = rect_of(&wm, id);
    assert!(grown.width >= outer.width && grown.height >= outer.height);
    assert!(wm.active_pane().status_message().unwrap_or("").contains("resized to"));
    up(&mut wm, 0, 0);
    // top-left corner shrinks from the other side
    let o2 = rect_of(&wm, id);
    wm.set_wm_mouse_drag(Some(WmMouseDrag::ResizeWindow {
        pane_id: id,
        edge: bilterm::wm::WindowResizeEdge::TopLeft,
        initial_rect: o2,
    }));
    drag(&mut wm, o2.x + 6, o2.y + 2);
    let shrunk = rect_of(&wm, id);
    assert!(shrunk.width < o2.width && shrunk.height < o2.height);
    assert_eq!((shrunk.x, shrunk.y), (o2.x + 6, o2.y + 2));
}

#[test]
fn single_pane_resize_drag_converts_to_stacked() {
    let (mut wm, _) = wm_sized(ROWS, COLS);
    let id = wm.active_pane_id();
    let initial = rect_of(&wm, id);
    wm.set_wm_mouse_drag(Some(WmMouseDrag::ResizeWindow {
        pane_id: id,
        edge: bilterm::wm::WindowResizeEdge::BottomRight,
        initial_rect: initial,
    }));
    drag(&mut wm, 60, 12);
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked);
    let r = rect_of(&wm, id);
    assert_eq!((r.width, r.height), (61, 13));
}

#[test]
fn release_without_press_and_non_left_events_are_harmless() {
    let mut wm = two_panes(SplitDirection::Vertical);
    let before = wm.compute_workspace_rects(AREA);
    up(&mut wm, 10, 10);
    assert_eq!(wm.wm_mouse_drag(), None);
    drag(&mut wm, 30, 10);
    fire(&mut wm, MouseEventKind::Drag(MouseButton::Right), 30, 10, NONE, AREA);
    assert_eq!(wm.compute_workspace_rects(AREA), before);
    // an active drag ignores events that are neither left-drag nor left-up (falls through to the pane)
    wm.set_wm_mouse_drag(Some(WmMouseDrag::ResizeQuickTerminal));
    fire(&mut wm, MouseEventKind::Moved, 30, 10, NONE, AREA);
    assert_eq!(wm.wm_mouse_drag(), Some(WmMouseDrag::ResizeQuickTerminal));
    up(&mut wm, 30, 10);
    assert_eq!(wm.wm_mouse_drag(), None);
}

#[test]
fn click_outside_every_pane_is_continue_not_an_error() {
    let (mut wm, area) = wm_sized(ROWS, COLS);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(area).unwrap();
    let active = wm.active_pane_id();
    let ev = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: COLS + 10, row: ROWS + 10, modifiers: NONE };
    let r = handle_wm_mouse_event(&mut wm, ev, area).unwrap();
    assert_eq!(r, bilterm::input::AppAction::Continue);
    assert_eq!(wm.active_pane_id(), active);
}


// ---------------------------------------------------------------- modal move / resize / buttons

fn open_help() -> (WindowManager, Rect) {
    let (mut wm, area) = wm_sized(ROWS, COLS);
    wm.toggle_help();
    let outer = wm.active_modal_rect(area).unwrap();
    (wm, outer)
}

#[test]
fn modal_title_drag_moves_the_modal_and_release_ends_the_drag() {
    let (mut wm, outer) = open_help();
    down(&mut wm, outer.x + 2, outer.y);
    assert_eq!(wm.wm_mouse_drag(), Some(WmMouseDrag::MoveModal { grab_dx: 2, grab_dy: 0 }));
    drag(&mut wm, outer.x + 2 + 4, outer.y + 1);
    let moved = wm.active_modal_rect(AREA).unwrap();
    assert_eq!((moved.x, moved.y), (outer.x + 4, outer.y + 1));
    assert_eq!((moved.width, moved.height), (outer.width, outer.height));
    up(&mut wm, 0, 0);
    assert_eq!(wm.wm_mouse_drag(), None);
    assert!(wm.is_help_visible());
}

#[test]
fn modal_drag_past_the_workspace_edge_is_clamped_to_bounds() {
    let (mut wm, outer) = open_help();
    down(&mut wm, outer.x + 2, outer.y);
    drag(&mut wm, COLS - 1, ROWS - 1);
    let moved = wm.active_modal_rect(AREA).unwrap();
    assert_eq!(moved.x + moved.width, COLS, "right edge pinned to the workspace");
    assert_eq!(moved.y + moved.height, ROWS, "bottom edge pinned to the workspace");
    drag(&mut wm, 0, 0);
    let moved = wm.active_modal_rect(AREA).unwrap();
    assert_eq!((moved.x, moved.y), (0, 0));
}

#[test]
fn modal_move_drag_without_an_open_modal_is_a_noop() {
    let (mut wm, _) = wm_sized(ROWS, COLS);
    wm.set_wm_mouse_drag(Some(WmMouseDrag::MoveModal { grab_dx: 0, grab_dy: 0 }));
    drag(&mut wm, 30, 10);
    assert_eq!(wm.active_modal_rect(AREA), None);
    assert!(wm.wm_mouse_drag().is_some());
    up(&mut wm, 30, 10);
    assert_eq!(wm.wm_mouse_drag(), None);
}

#[test]
fn modal_right_and_bottom_border_drags_resize_and_clamp_to_the_minimum() {
    let (mut wm, outer) = open_help();
    let mid_row = outer.y + outer.height / 2;
    down(&mut wm, outer.x + outer.width - 1, mid_row);
    assert!(matches!(wm.wm_mouse_drag(), Some(WmMouseDrag::ResizeModal { .. })));
    drag(&mut wm, outer.x + outer.width + 5, mid_row);
    let grown = wm.active_modal_rect(AREA).unwrap();
    assert_eq!(grown.width, outer.width + 6);
    drag(&mut wm, outer.x + 2, mid_row); // far too narrow: clamped to 20 columns
    assert_eq!(wm.active_modal_rect(AREA).unwrap().width, 20);
    up(&mut wm, 0, 0);

    let o = wm.active_modal_rect(AREA).unwrap();
    down(&mut wm, o.x + 6, o.y + o.height - 1);
    drag(&mut wm, o.x + 6, o.y + 2); // collapses toward the 5-row minimum
    assert_eq!(wm.active_modal_rect(AREA).unwrap().height, 5);
}

#[test]
fn modal_top_left_corner_drag_resizes_from_the_origin_side() {
    let (mut wm, outer) = open_help();
    down(&mut wm, outer.x, outer.y + 1); // left border just under the title bar
    assert!(matches!(
        wm.wm_mouse_drag(),
        Some(WmMouseDrag::ResizeModal { edge: bilterm::wm::WindowResizeEdge::TopLeft, .. })
    ));
    drag(&mut wm, outer.x + 6, outer.y + 3);
    let r = wm.active_modal_rect(AREA).unwrap();
    assert_eq!(r.x, outer.x + 6);
    assert_eq!(r.width, outer.width - 6);
    assert_eq!(r.y, outer.y + 3);
    assert_eq!(r.height, outer.height - 3);
}

fn modal_badge_col(wm: &WindowManager, outer: Rect, which: u8) -> u16 {
    badge_col(wm, outer, which)
}

#[test]
fn modal_maximize_badge_toggles_full_workspace_and_close_badge_closes() {
    let (mut wm, outer) = open_help();
    let col = modal_badge_col(&wm, outer, 1);
    down(&mut wm, col, outer.y);
    assert_eq!(wm.active_modal_rect(AREA), Some(AREA));
    let full_col = col_for_full(&wm);
    down(&mut wm, full_col, 0);
    assert_ne!(wm.active_modal_rect(AREA), Some(AREA), "second click restores");
    let outer = wm.active_modal_rect(AREA).unwrap();
    let col = modal_badge_col(&wm, outer, 2);
    down(&mut wm, col, outer.y);
    assert!(matches!(wm.modal(), Modal::None));
}

fn col_for_full(wm: &WindowManager) -> u16 {
    badge_col(wm, AREA, 1)
}

#[test]
fn modal_minimize_badge_also_closes_the_modal() {
    let (mut wm, outer) = open_help();
    let col = modal_badge_col(&wm, outer, 0);
    down(&mut wm, col, outer.y);
    assert!(matches!(wm.modal(), Modal::None));
}

#[test]
fn click_outside_the_modal_falls_through_to_the_workspace() {
    let (mut wm, area) = wm_sized(ROWS, COLS);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(area).unwrap();
    wm.toggle_help();
    let outer = wm.active_modal_rect(area).unwrap();
    let (first, _) = ordered_ids(&wm);
    assert_ne!(wm.active_pane_id(), first);
    // top-left workspace corner is outside the centred modal and inside the left pane
    assert!(outer.x > 1 && outer.y > 0);
    down(&mut wm, 0, 0);
    assert_eq!(wm.active_pane_id(), first);
    assert!(wm.is_help_visible(), "modal is not dismissed by an outside click");
}

#[test]
fn modal_right_click_and_scroll_left_are_swallowed() {
    let (mut wm, outer) = open_help();
    fire(&mut wm, MouseEventKind::Down(MouseButton::Right), outer.x + 5, outer.y + 5, NONE, AREA);
    fire(&mut wm, MouseEventKind::ScrollLeft, outer.x + 5, outer.y + 5, NONE, AREA);
    assert!(wm.is_help_visible());
    assert_eq!(wm.wm_mouse_drag(), None);
}

// ---------------------------------------------------------------- modal wheel / hover / click

#[test]
fn wheel_scrolls_help_and_inspector_by_two_and_saturates_at_zero() {
    let (mut wm, outer) = open_help();
    let (c, r) = (outer.x + 5, outer.y + 5);
    fire(&mut wm, MouseEventKind::ScrollDown, c, r, NONE, AREA);
    fire(&mut wm, MouseEventKind::ScrollDown, c, r, NONE, AREA);
    assert!(matches!(wm.modal(), Modal::Help { scroll: 4 }));
    for _ in 0..5 {
        fire(&mut wm, MouseEventKind::ScrollUp, c, r, NONE, AREA);
    }
    assert!(matches!(wm.modal(), Modal::Help { scroll: 0 }));

    *wm.modal_mut() = Modal::Inspector { scroll: 0 };
    let outer = wm.active_modal_rect(AREA).unwrap();
    fire(&mut wm, MouseEventKind::ScrollDown, outer.x + 3, outer.y + 3, NONE, AREA);
    assert!(matches!(wm.modal(), Modal::Inspector { scroll: 2 }));
    fire(&mut wm, MouseEventKind::ScrollUp, outer.x + 3, outer.y + 3, NONE, AREA);
    assert!(matches!(wm.modal(), Modal::Inspector { scroll: 0 }));
}

#[test]
fn wheel_moves_the_palette_selection_by_one() {
    let (mut wm, area) = wm_sized(ROWS, COLS);
    wm.open_palette_with_query("");
    let outer = wm.active_modal_rect(area).unwrap();
    let (c, r) = (outer.x + 5, outer.y + 5);
    fire(&mut wm, MouseEventKind::ScrollDown, c, r, NONE, AREA);
    fire(&mut wm, MouseEventKind::ScrollDown, c, r, NONE, AREA);
    assert_eq!(wm.palette_state().unwrap().1, 2);
    fire(&mut wm, MouseEventKind::ScrollUp, c, r, NONE, AREA);
    assert_eq!(wm.palette_state().unwrap().1, 1);
    for _ in 0..4 {
        fire(&mut wm, MouseEventKind::ScrollUp, c, r, NONE, AREA);
    }
    assert_eq!(wm.palette_state().unwrap().1, 0);
}

#[test]
fn hovering_a_palette_row_selects_it_and_header_hover_does_not() {
    let (mut wm, area) = wm_sized(ROWS, COLS);
    wm.open_palette_with_query("");
    let outer = wm.active_modal_rect(area).unwrap();
    let inner = bilterm::wm::inset_window_frame(outer);
    let hdr = bilterm::ui::modals::palette_header_rows(wm.config(), inner.height);
    fire(&mut wm, MouseEventKind::Moved, inner.x + 3, inner.y + hdr + 3, NONE, AREA);
    assert_eq!(wm.palette_state().unwrap().1, 3);
    fire(&mut wm, MouseEventKind::Moved, inner.x + 3, inner.y, NONE, AREA);
    assert_eq!(wm.palette_state().unwrap().1, 3, "header row hover is ignored");
    // a row beyond the end of a filtered list is ignored
    wm.open_palette_with_query("zzzzqqqq-no-match");
    assert!(wm.filtered_palette_items().is_empty());
    fire(&mut wm, MouseEventKind::Moved, inner.x + 3, inner.y + hdr + 2, NONE, AREA);
    assert_eq!(wm.palette_state().unwrap().1, 0);
}

#[test]
fn hovering_a_palette_row_when_the_list_is_scrolled_keeps_the_selection() {
    let (mut wm, area) = wm_sized(ROWS, COLS);
    wm.open_palette_with_query("");
    let outer = wm.active_modal_rect(area).unwrap();
    let inner = bilterm::wm::inset_window_frame(outer);
    let hdr = bilterm::ui::modals::palette_header_rows(wm.config(), inner.height);
    let list_rows = usize::from(inner.height - hdr);
    if let Modal::Palette { selected, .. } = wm.modal_mut() {
        *selected = list_rows + 2; // list is scrolled: start > 0
    }
    fire(&mut wm, MouseEventKind::Moved, inner.x + 3, inner.y + hdr + 1, NONE, AREA);
    assert_eq!(wm.palette_state().unwrap().1, list_rows + 2);
}

#[test]
fn clicking_a_palette_entry_closes_the_palette_and_runs_its_command() {
    let (mut wm, area) = wm_sized(ROWS, COLS);
    wm.open_palette_with_query("");
    let items = wm.filtered_palette_items();
    assert_eq!(items[0].0, ":split h");
    let outer = wm.active_modal_rect(area).unwrap();
    let inner = bilterm::wm::inset_window_frame(outer);
    let hdr = bilterm::ui::modals::palette_header_rows(wm.config(), inner.height);
    let before = wm.pane_count();
    down(&mut wm, inner.x + 4, inner.y + hdr);
    assert!(matches!(wm.modal(), Modal::None));
    assert_eq!(wm.pane_count(), before + 1);
}

#[test]
fn clicking_a_palette_entry_that_needs_an_argument_leaves_the_palette_open() {
    let (mut wm, area) = wm_sized(ROWS, COLS);
    wm.open_palette_with_query("rename");
    let items = wm.filtered_palette_items();
    let idx = items.iter().position(|i| i.0 == ":rename ").unwrap();
    let outer = wm.active_modal_rect(area).unwrap();
    let inner = bilterm::wm::inset_window_frame(outer);
    let hdr = bilterm::ui::modals::palette_header_rows(wm.config(), inner.height);
    assert!(idx < usize::from(inner.height - hdr), "entry must be visible");
    down(&mut wm, inner.x + 4, inner.y + hdr + idx as u16);
    // `:rename ` has no name => parse error => the click is ignored and the palette stays up
    assert!(matches!(wm.modal(), Modal::Palette { .. }), "{:?}", wm.modal());
}

#[test]
fn clicking_the_palette_header_or_blank_space_below_the_list_does_nothing() {
    let (mut wm, area) = wm_sized(ROWS, COLS);
    wm.open_palette_with_query("no-such-command-xyz");
    let outer = wm.active_modal_rect(area).unwrap();
    let inner = bilterm::wm::inset_window_frame(outer);
    let hdr = bilterm::ui::modals::palette_header_rows(wm.config(), inner.height);
    let before = wm.pane_count();
    down(&mut wm, inner.x + 4, inner.y); // query line
    down(&mut wm, inner.x + 4, inner.y + hdr + 1); // empty list
    assert!(matches!(wm.modal(), Modal::Palette { .. }));
    assert_eq!(wm.pane_count(), before);
}

fn help_row_for(needle: &str) -> usize {
    bilterm::ui::modals::HELP_LINES.iter().position(|&(l, r)| !r.is_empty() && l.contains(needle)).unwrap()
}

#[test]
fn clicking_a_help_line_that_names_a_command_executes_it() {
    let (mut wm, outer) = open_help();
    let inner = bilterm::wm::inset_window_frame(outer);
    let idx = help_row_for(":palette");
    assert!(idx < usize::from(inner.height));
    down(&mut wm, inner.x + 3, inner.y + idx as u16);
    assert!(matches!(wm.modal(), Modal::Palette { .. }), "help closed, :palette executed");
}

#[test]
fn clicking_the_help_line_for_help_toggles_help_off() {
    let (mut wm, outer) = open_help();
    let inner = bilterm::wm::inset_window_frame(outer);
    let idx = help_row_for("F1 / :help");
    down(&mut wm, inner.x + 3, inner.y + idx as u16);
    assert!(matches!(wm.modal(), Modal::None));
}

#[test]
fn clicking_help_headers_and_command_less_lines_changes_nothing() {
    let (mut wm, outer) = open_help();
    let inner = bilterm::wm::inset_window_frame(outer);
    down(&mut wm, inner.x + 3, inner.y); // section header (empty rhs)
    assert!(wm.is_help_visible());
    let idx = help_row_for("Ctrl+X <key>"); // no ':' command
    down(&mut wm, inner.x + 3, inner.y + idx as u16);
    assert!(wm.is_help_visible());
    // the inspector has no clickable rows at all
    *wm.modal_mut() = Modal::Inspector { scroll: 0 };
    let o = wm.active_modal_rect(AREA).unwrap();
    let i = bilterm::wm::inset_window_frame(o);
    down(&mut wm, i.x + 3, i.y + 2);
    assert!(matches!(wm.modal(), Modal::Inspector { .. }));
}

#[test]
fn clicking_a_help_line_that_quits_reports_quit_or_opens_the_exit_guard() {
    let (mut wm, outer) = open_help();
    let inner = bilterm::wm::inset_window_frame(outer);
    let idx = help_row_for(":quit");
    let max_s = bilterm::ui::modals::HELP_LINES.len().saturating_sub(usize::from(inner.height));
    *wm.modal_mut() = Modal::Help { scroll: idx };
    let row = inner.y + (idx - idx.min(max_s)) as u16;
    let ev = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: inner.x + 3, row, modifiers: NONE };
    let r = handle_wm_mouse_event(&mut wm, ev, AREA).unwrap();
    // `:quit` honours the exit guard on a live headless pane: either quit now or the guard modal opens
    assert!(r == bilterm::input::AppAction::Quit || matches!(wm.modal(), Modal::ExitGuard(_)), "{r:?} {:?}", wm.modal());
}


// ---------------------------------------------------------------- remaining edges and mirrored fallbacks

#[test]
fn outer_edge_drags_fall_back_to_the_opposite_divider_for_every_edge() {
    use bilterm::wm::WindowResizeEdge as E;
    // vertical split: right pane's RIGHT edge has no divider; horizontal split: TOP pane's TOP and
    // BOTTOM pane's BOTTOM edges have none. Each drag must still move the only divider (mirrored).
    let wm = two_panes(SplitDirection::Vertical);
    let (left, right) = ordered_ids(&wm);
    let ro = rect_of(&wm, right);
    let mut wm = wm;
    wm.set_wm_mouse_drag(Some(WmMouseDrag::ResizeWindow { pane_id: right, edge: E::Right, initial_rect: ro }));
    drag(&mut wm, ro.x + ro.width - 1 - 10, ro.y + 2);
    assert_ne!(rect_of(&wm, left).width, rect_of(&two_panes(SplitDirection::Vertical), left).width);

    for (edge, pick_top) in [(E::Top, true), (E::Bottom, false)] {
        let mut wm = two_panes(SplitDirection::Horizontal);
        let mut v = wm.compute_workspace_rects(AREA);
        v.sort_by_key(|&(_, r)| r.y);
        let (id, r) = if pick_top { v[0] } else { v[1] };
        let h0 = rect_of(&wm, id).height;
        wm.set_wm_mouse_drag(Some(WmMouseDrag::ResizeWindow { pane_id: id, edge, initial_rect: r }));
        let target_row = if pick_top { r.y + 4 } else { r.y + r.height - 1 - 4 };
        drag(&mut wm, r.x + 5, target_row);
        assert_ne!(rect_of(&wm, id).height, h0, "{edge:?}");
    }
}

#[test]
fn top_corner_presses_pick_the_corner_edges() {
    use bilterm::wm::WindowResizeEdge as E;
    let mut wm = two_panes(SplitDirection::Vertical);
    let (left, right) = ordered_ids(&wm);
    let lo = rect_of(&wm, left);
    let ro = rect_of(&wm, right);
    // top-left corner of the left pane
    down(&mut wm, lo.x, lo.y);
    assert!(matches!(wm.wm_mouse_drag(), Some(WmMouseDrag::ResizeWindow { edge: E::TopLeft, .. })), "{:?}", wm.wm_mouse_drag());
    up(&mut wm, 0, 0);
    // top-right corner of the right pane
    down(&mut wm, ro.x + ro.width - 1, ro.y);
    assert!(matches!(wm.wm_mouse_drag(), Some(WmMouseDrag::ResizeWindow { edge: E::TopRight, .. })), "{:?}", wm.wm_mouse_drag());
    up(&mut wm, 0, 0);
    // bottom-left corner of the right pane, and plain top edge mid-title (off the grip)
    down(&mut wm, ro.x, ro.y + ro.height - 1);
    assert!(matches!(wm.wm_mouse_drag(), Some(WmMouseDrag::ResizeWindow { edge: E::BottomLeft, .. })));
    up(&mut wm, 0, 0);
    down(&mut wm, ro.x + 25, ro.y);
    assert!(matches!(wm.wm_mouse_drag(), Some(WmMouseDrag::ResizeWindow { edge: E::Top, .. })));
}

#[test]
fn press_on_the_interior_of_a_framed_pane_starts_no_drag() {
    let mut wm = two_panes(SplitDirection::Vertical);
    let (left, _) = ordered_ids(&wm);
    let lo = rect_of(&wm, left);
    down(&mut wm, lo.x + 5, lo.y + 5);
    assert_eq!(wm.wm_mouse_drag(), None);
    assert_eq!(wm.active_pane_id(), left, "the click still focuses the pane");
}
