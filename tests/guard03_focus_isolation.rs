//! Phase 65 GUARD-03: a click on an inactive pane is consumed purely as focus+raise.

use bilterm::input::mouse::handle_wm_mouse_event;
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::{WindowManager, WmMouseDrag};
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

const COLS: u16 = 100;
const ROWS: u16 = 24;
const AREA: Rect = Rect { x: 0, y: 0, width: COLS, height: ROWS };
const SGR_SETUP: &[u8] = b"\x1b[?1049h\x1b[?1000h\x1b[?1002h\x1b[?1006h";

fn fire(wm: &mut WindowManager, kind: MouseEventKind, col: u16, row: u16) {
    let ev = MouseEvent { kind, column: col, row, modifiers: KeyModifiers::NONE };
    handle_wm_mouse_event(wm, ev, AREA).unwrap();
}
fn down(wm: &mut WindowManager, c: u16, r: u16) {
    fire(wm, MouseEventKind::Down(MouseButton::Left), c, r);
}

fn rect_of(wm: &WindowManager, id: usize) -> Rect {
    wm.compute_workspace_rects(AREA).into_iter().find(|&(i, _)| i == id).unwrap().1
}

/// Two side-by-side panes; returns (wm, inactive_left, active_right).
fn setup(sgr_left: bool) -> (WindowManager, usize, usize) {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.resize_all(AREA).unwrap();
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(AREA).unwrap();
    let mut v = wm.compute_workspace_rects(AREA);
    v.sort_by_key(|&(_, r)| (r.x, r.y));
    let (left, right) = (v[0].0, v[1].0);
    wm.focus_and_raise(right);
    if sgr_left {
        wm.pane_mut(left).unwrap().process_pty_output(SGR_SETUP);
    }
    assert_eq!(wm.active_pane_id(), right);
    (wm, left, right)
}

fn left_canvas(wm: &WindowManager, left: usize) -> (u16, u16) {
    let o = rect_of(wm, left);
    (o.x + 6, o.y + 3)
}

fn assert_pristine(wm: &WindowManager, id: usize) {
    let p = wm.pane(id).unwrap();
    assert!(p.last_pty_write().is_empty(), "bytes leaked: {:?}", p.last_pty_write());
    assert!(p.mouse_drag().is_none(), "drag state left behind");
    assert!(!p.copy_mode().has_selection(), "selection created");
    assert!(!p.copy_mode().active);
}

#[test]
fn inactive_canvas_click_only_focuses_and_sends_no_sgr() {
    let (mut wm, left, _) = setup(true);
    assert!(wm.pane(left).unwrap().vt().is_mouse_sgr_or_tracking());
    let (c, r) = left_canvas(&wm, left);
    down(&mut wm, c, r);
    assert_eq!(wm.active_pane_id(), left);
    assert_pristine(&wm, left);
    fire(&mut wm, MouseEventKind::Drag(MouseButton::Left), c + 8, r);
    fire(&mut wm, MouseEventKind::Up(MouseButton::Left), c + 8, r);
    assert_pristine(&wm, left);
}

#[test]
fn inactive_canvas_click_starts_no_selection_on_plain_pane() {
    let (mut wm, left, _) = setup(false);
    let (c, r) = left_canvas(&wm, left);
    down(&mut wm, c, r);
    assert_eq!(wm.active_pane_id(), left);
    assert_pristine(&wm, left);
    fire(&mut wm, MouseEventKind::Drag(MouseButton::Left), c + 8, r + 1);
    assert_pristine(&wm, left);
    fire(&mut wm, MouseEventKind::Up(MouseButton::Left), c + 8, r + 1);
    assert_pristine(&wm, left);
    assert!(wm.pane(left).unwrap().prompt().selection_anchor().is_none());
}

#[test]
fn inactive_prompt_click_does_not_move_cursor() {
    let (mut wm, left, _) = setup(false);
    wm.pane_mut(left).unwrap().prompt_mut().insert_str("hello world");
    let before = (wm.pane(left).unwrap().prompt().cursor_row(), wm.pane(left).unwrap().prompt().cursor_col());
    let inner = bilterm::wm::inset_window_frame(rect_of(&wm, left));
    let p = wm.pane(left).unwrap();
    assert!(p.is_prompt_visible());
    let pr = p.prompt_rect(inner);
    assert!(pr.height > 2);
    down(&mut wm, pr.x + 5, pr.y + 1);
    let p = wm.pane(left).unwrap();
    assert_eq!(before, (p.prompt().cursor_row(), p.prompt().cursor_col()));
    assert!(p.mouse_drag().is_none());
    assert_eq!(wm.active_pane_id(), left);
}

#[test]
fn second_click_on_now_active_pane_is_forwarded_normally() {
    let (mut wm, left, _) = setup(true);
    let (c, r) = left_canvas(&wm, left);
    down(&mut wm, c, r);
    assert!(wm.pane(left).unwrap().last_pty_write().is_empty());
    fire(&mut wm, MouseEventKind::Up(MouseButton::Left), c, r);
    down(&mut wm, c, r);
    let w = String::from_utf8_lossy(wm.pane(left).unwrap().last_pty_write()).into_owned();
    assert!(w.starts_with("\x1b[<") && w.ends_with('M'), "expected SGR press, got {w:?}");
}

#[test]
fn inactive_right_click_only_focuses() {
    let (mut wm, left, _) = setup(true);
    let (c, r) = left_canvas(&wm, left);
    fire(&mut wm, MouseEventKind::Down(MouseButton::Right), c, r);
    assert_eq!(wm.active_pane_id(), left);
    assert_pristine(&wm, left);
    fire(&mut wm, MouseEventKind::Up(MouseButton::Right), c, r);
    assert_pristine(&wm, left);
}

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

#[test]
fn inactive_close_badge_acts_on_first_click() {
    let (mut wm, left, right) = setup(false);
    let o = rect_of(&wm, left);
    let bc = badge_col(&wm, o, 2);
    down(&mut wm, bc, o.y);
    assert!(wm.pane(left).is_none());
    assert!(wm.pane(right).is_some());
}

#[test]
fn inactive_zoom_badge_acts_on_first_click() {
    let (mut wm, left, _) = setup(false);
    let o = rect_of(&wm, left);
    let bc = badge_col(&wm, o, 1);
    down(&mut wm, bc, o.y);
    assert_eq!(wm.zoomed_pane(), Some(left));
}

#[test]
fn inactive_minimize_badge_acts_on_first_click() {
    let (mut wm, left, _) = setup(false);
    let o = rect_of(&wm, left);
    let bc = badge_col(&wm, o, 0);
    down(&mut wm, bc, o.y);
    assert!(wm.z_stack().is_minimized(left));
}

#[test]
fn inactive_border_drag_still_resizes() {
    let (mut wm, left, _) = setup(false);
    let lo = rect_of(&wm, left);
    let (x, y) = (lo.x + lo.width - 1, lo.y + lo.height / 2);
    down(&mut wm, x, y);
    assert!(matches!(wm.wm_mouse_drag(), Some(WmMouseDrag::ResizeWindow { pane_id, .. }) if pane_id == left));
    fire(&mut wm, MouseEventKind::Drag(MouseButton::Left), x + 10, y);
    assert!(rect_of(&wm, left).width > lo.width);
    fire(&mut wm, MouseEventKind::Up(MouseButton::Left), x + 10, y);
    assert_eq!(wm.wm_mouse_drag(), None);
}

#[test]
fn click_on_top_pane_while_quick_terminal_has_focus_only_focuses() {
    let (mut wm, _, right) = setup(false);
    wm.pane_mut(right).unwrap().process_pty_output(SGR_SETUP);
    wm.toggle_quick_terminal();
    assert!(wm.is_quick_terminal_visible());
    let qt = wm.quick_terminal().overlay_rect(AREA);
    let o = rect_of(&wm, right);
    // A canvas cell of the right pane outside the quick-terminal overlay.
    let row = (qt.y + qt.height).max(o.y + 3).min(o.y + o.height - 3);
    let (c, r) = (o.x + 6, row);
    assert!(!(c >= qt.x && c < qt.x + qt.width && r >= qt.y && r < qt.y + qt.height));
    down(&mut wm, c, r);
    fire(&mut wm, MouseEventKind::Up(MouseButton::Left), c, r);
    assert_pristine(&wm, right);
}

#[test]
fn inactive_middle_click_only_focuses_and_sends_no_bytes() {
    let (mut wm, left, _) = setup(true);
    let (c, r) = left_canvas(&wm, left);
    fire(&mut wm, MouseEventKind::Down(MouseButton::Middle), c, r);
    fire(&mut wm, MouseEventKind::Up(MouseButton::Middle), c, r);
    assert_eq!(wm.active_pane_id(), left);
    assert_pristine(&wm, left);
}
