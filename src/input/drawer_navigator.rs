//! Key handling for the focused navigator drawer (`FLT-01`, Phase 50-06).
//!
//! This module only resolves which key scopes are active, looks the key up in the declarative
//! [`NAVIGATOR_DRAWER_BINDINGS`](super::keymap::NAVIGATOR_DRAWER_BINDINGS) table and runs the resulting
//! [`Action`] through `WindowManager::apply`. It never changes drawer, pane or
//! prompt state itself (`ARCH-R01`).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use super::mouse_wm::detect_title_bar_badge_click;
use super::keymap::{DrawerKeyScope, navigator_drawer_action};
use crate::action::Action;
use crate::ui::drawer::navigator::{
    NavigatorButtonKind, navigator_fold_marker_cells, navigator_footer_buttons, navigator_header_plus,
    navigator_comms_cells, navigator_menu_entry_at, navigator_plus_cells, navigator_row_at,
};
use crate::wm::agents_cmd::AgentsOp;
use crate::wm::drawer::DrawerNavOp;
use crate::wm::{DrawerEdge, DrawerRowKind, DrawerVisibilityMode, WindowManager};

/// Looks `key` up in the navigator key table; `None` lets the classic drawer handler run.
pub fn handle_navigator_drawer_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    // An open context menu is dismissed by any key; Esc is consumed by the dismissal.
    if wm.drawer().navigator_menu_open() {
        let _ = wm.apply(Action::DrawerNav(DrawerNavOp::MenuClose));
        if key.code == KeyCode::Esc {
            return Some(AppAction::Handled);
        }
    }
    // If multi-selection is active, Esc clears selection instead of falling through to unfocus.
    if key.code == KeyCode::Esc && !wm.selected_windows().is_empty() {
        let _ = wm.apply(Action::WindowManage(
            crate::wm::window_manage::WindowManageOp::Select(
                crate::wm::window_manage::WindowSelectTarget::None,
            ),
        ));
        return Some(AppAction::Handled);
    }
    // Space on a window row toggles selection (SES-03).
    if key.code == KeyCode::Char(' ')
        && key.modifiers.is_empty()
        && let Some(id) = wm.selected_drawer_pane_id()
    {
        let _ = wm.apply(Action::WindowManage(
            crate::wm::window_manage::WindowManageOp::Select(
                crate::wm::window_manage::WindowSelectTarget::Toggle(id),
            ),
        ));
        return Some(AppAction::Handled);
    }
    // Shift+Down / Shift+Up extends range selection on window rows (SES-03).
    if key.modifiers.contains(KeyModifiers::SHIFT)
        && matches!(key.code, KeyCode::Down | KeyCode::Up)
        && let Some(from) = wm.selected_drawer_pane_id()
    {
        if key.code == KeyCode::Down {
            wm.drawer_select_next();
        } else {
            wm.drawer_select_prev();
        }
        if let Some(to) = wm.selected_drawer_pane_id() {
            let _ = wm.apply(Action::WindowManage(
                crate::wm::window_manage::WindowManageOp::Select(
                    crate::wm::window_manage::WindowSelectTarget::Range { from, to },
                ),
            ));
        }
        return Some(AppAction::Handled);
    }
    // Ctrl+A selects all windows in current session (SES-03).
    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('a') | KeyCode::Char('A')) {
        let _ = wm.apply(Action::WindowManage(
            crate::wm::window_manage::WindowManageOp::Select(
                crate::wm::window_manage::WindowSelectTarget::All,
            ),
        ));
        return Some(AppAction::Handled);
    }
    // m / M opens move prompt (SES-01).
    if key.modifiers.is_empty() && matches!(key.code, KeyCode::Char('m') | KeyCode::Char('M')) {
        wm.drawer_mut().focused = false;
        let prompt = wm.active_pane_mut().prompt_mut();
        prompt.manual_raw_override = false;
        prompt.force_command_prompt = true;
        prompt.set_text(":window move ");
        return Some(AppAction::Handled);
    }
    // u / U undoes last window/session mutation (SES-04).
    if key.modifiers.is_empty() && matches!(key.code, KeyCode::Char('u') | KeyCode::Char('U')) {
        let _ = wm.apply(Action::WindowManage(crate::wm::window_manage::WindowManageOp::Undo));
        return Some(AppAction::Handled);
    }
    let comms = wm.navigator_comms_focused();
    let scopes = [DrawerKeyScope::NavigatorComms, DrawerKeyScope::Navigator];
    let action = navigator_drawer_action(if comms { &scopes } else { &scopes[1..] }, key)?;
    // Claim guard: Right belongs to the comms glyph only on a row that shows one; elsewhere the classic
    // block keeps its Right behaviour. Reads state only.
    if action == Action::DrawerNav(DrawerNavOp::CommsFocus(true)) && !selected_row_has_comms(wm) {
        return None;
    }
    let _ = wm.apply(action);
    Some(AppAction::Handled)
}

/// `true` when the selected drawer row shows a comms glyph.
fn selected_row_has_comms(wm: &WindowManager) -> bool {
    wm.drawer_rows()
        .get(wm.drawer().selected_index)
        .and_then(|r| r.navigator.as_ref())
        .is_some_and(|m| m.comms.is_some())
}

/// Action a footer button applies, the same one its key twin lowers to. Reads state only to choose the
/// pin direction and the selected pane id for zoom.
pub(super) fn navigator_button_action(wm: &WindowManager, kind: NavigatorButtonKind) -> Option<Action> {
    Some(match kind {
        NavigatorButtonKind::Pin => Action::DrawerVisibility(Some(
            if wm.drawer_config().visibility == DrawerVisibilityMode::Fixed {
                DrawerVisibilityMode::OnDemand
            } else {
                DrawerVisibilityMode::Fixed
            },
        )),
        NavigatorButtonKind::Attention => Action::DrawerNav(DrawerNavOp::NextAttention),
        NavigatorButtonKind::NewSession => Action::DrawerNav(DrawerNavOp::NewSession),
        NavigatorButtonKind::NewPane => Action::DrawerNav(DrawerNavOp::NewPane),
        NavigatorButtonKind::Rename => Action::DrawerNav(DrawerNavOp::RenamePrompt),
        NavigatorButtonKind::Close => Action::DrawerNav(DrawerNavOp::Close),
        NavigatorButtonKind::Zoom => {
            let pane = wm.drawer_rows().get(wm.drawer().selected_index).and_then(|r| match r.kind {
                DrawerRowKind::Pane(id) => Some(id),
                _ => None,
            });
            Action::Agents(AgentsOp::Toggle(pane))
        }
    })
}

/// Mouse handling for the navigator-style drawer; the caller guarantees the event is inside `dr`.
///
/// Every visible effect lowers to an [`Action`] applied with `WindowManager::apply` (`ARCH-R01`), so the
/// keyboard, the colon prompt and the mouse share one code path. Events no gesture claims are swallowed so
/// the classic absolute-column zones are never reached in navigator style.
pub fn handle_navigator_drawer_mouse(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    dr: Rect,
    workspace_area: Rect,
) -> Option<AppAction> {
    if matches!(mouse.kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown) {
        let _ = wm.apply(Action::DrawerNav(DrawerNavOp::MenuClose));
        // Navigator rows truncate instead of scrolling sideways, so Shift scrolls like the plain wheel.
        let step = if matches!(mouse.kind, MouseEventKind::ScrollUp) { -3 } else { 3 };
        let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Scroll(step)));
        return Some(AppAction::Handled);
    }
    if let MouseEventKind::Down(MouseButton::Right) = mouse.kind {
        // Right-click on a row selects it and opens the context menu there; elsewhere inside the drawer it
        // only dismisses an open menu (the paste behaviour stays with right-clicks outside the drawer).
        match navigator_row_at(wm, workspace_area, mouse.column, mouse.row) {
            Some(i) => {
                let _ = wm.apply(Action::DrawerFocus);
                let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Row(i)));
                let at = Some((mouse.column, mouse.row));
                let _ = wm.apply(Action::DrawerNav(DrawerNavOp::MenuOpen(at)));
            }
            None => {
                let _ = wm.apply(Action::DrawerNav(DrawerNavOp::MenuClose));
            }
        }
        return Some(AppAction::Handled);
    }
    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
        let (col, row) = (mouse.column, mouse.row);
        if wm.drawer().navigator_menu_open() {
            let action = match navigator_menu_entry_at(wm, workspace_area, col, row) {
                Some(i) => DrawerNavOp::MenuPick(i),
                None => DrawerNavOp::MenuClose,
            };
            let _ = wm.apply(Action::DrawerNav(action));
            return Some(AppAction::Handled);
        }
        if row == dr.y
            && let Some(badge) = detect_title_bar_badge_click(wm, dr, col)
        {
            match badge {
                0 => {
                    let _ = wm.apply(Action::DrawerSize(0));
                }
                1 => {
                    let _ = wm.apply(Action::DrawerSize(u16::MAX));
                }
                _ => {
                    let _ = wm.apply(Action::DrawerVisibility(Some(DrawerVisibilityMode::OnDemand)));
                    let _ = wm.apply(Action::DrawerToggle);
                }
            }
            return Some(AppAction::Handled);
        }
        let on_resize_border = match wm.drawer_config().edge {
            DrawerEdge::Left => col == dr.x + dr.width.saturating_sub(1),
            DrawerEdge::Right => col == dr.x,
            DrawerEdge::Top => row == dr.y + dr.height.saturating_sub(1),
            DrawerEdge::Bottom => row == dr.y,
        };
        if on_resize_border {
            // Pointer-gesture bookkeeping, not an operation: the resize itself lowers to `DrawerSize`.
            wm.drawer_mut().resizing_drag = true;
            return Some(AppAction::Handled);
        }
        if row == dr.y
            && navigator_header_plus(wm, workspace_area).is_some_and(|r| col >= r.x && col < r.x + r.width)
        {
            let _ = wm.apply(Action::DrawerNav(DrawerNavOp::NewSession));
            return Some(AppAction::Handled);
        }
        let button = navigator_footer_buttons(wm, workspace_area)
            .into_iter()
            .find(|b| b.y == row && col >= b.x && col < b.x + b.width);
        if let Some(b) = button {
            if let Some(action) = navigator_button_action(wm, b.kind) {
                let _ = wm.apply(action);
            }
            return Some(AppAction::Handled);
        }
        let plus = navigator_plus_cells(wm, workspace_area)
            .into_iter()
            .find(|c| c.y == row && col >= c.x && col < c.x + 3);
        if let Some(cell) = plus {
            let _ = wm.apply(Action::DrawerFocus);
            let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Row(cell.row_index)));
            let _ = wm.apply(Action::DrawerNav(DrawerNavOp::AddWindow(None)));
            return Some(AppAction::Handled);
        }
        let comms = navigator_comms_cells(wm, workspace_area)
            .into_iter()
            .find(|c| c.y == row && col >= c.x && col < c.x + c.width);
        if let Some(cell) = comms {
            let _ = wm.apply(Action::DrawerFocus);
            let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Row(cell.row_index)));
            let _ = wm.apply(Action::DrawerNav(DrawerNavOp::OpenComms));
            return Some(AppAction::Handled);
        }
        let marker = navigator_fold_marker_cells(wm, workspace_area).into_iter().find(|c| c.x == col && c.y == row);
        if let Some(cell) = marker {
            let _ = wm.apply(Action::DrawerFocus);
            let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Row(cell.row_index)));
            let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Fold));
        } else if let Some(i) = navigator_row_at(wm, workspace_area, col, row) {
            let _ = wm.apply(Action::DrawerFocus);
            let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Row(i)));
            if mouse.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                if let Some(pid) = wm.selected_drawer_pane_id() {
                    let _ = wm.apply(Action::WindowManage(
                        crate::wm::window_manage::WindowManageOp::Select(
                            crate::wm::window_manage::WindowSelectTarget::Toggle(pid),
                        ),
                    ));
                }
            } else {
                let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Activate));
            }
        }
    }
    Some(AppAction::Handled)
}

/// Edge-drag motion in navigator style: lowers to `Action::DrawerSize`. The clamp keeps a drag away from
/// `0` and `u16::MAX`, the minimize and maximize values; `resize_drawer` applies the real per-edge range.
pub(super) fn navigator_drag_resize(wm: &mut WindowManager, new_size: u16) -> Option<AppAction> {
    let _ = wm.apply(Action::DrawerSize(new_size.clamp(1, u16::MAX - 1)));
    Some(AppAction::Handled)
}

/// Click-away for the context menu: a press outside the drawer closes it, then the event proceeds normally
/// (so a right-click elsewhere still pastes). A no-op unless the navigator menu is open.
pub(super) fn navigator_menu_click_away(wm: &mut WindowManager, mouse: &MouseEvent, dr: Rect) {
    if wm.drawer_config().style == crate::wm::drawer::DrawerStyle::Navigator
        && wm.drawer().navigator_menu_open()
        && matches!(mouse.kind, MouseEventKind::Down(_))
        && !crate::wm::rect_contains(dr, mouse.column, mouse.row)
    {
        let _ = wm.apply(Action::DrawerNav(DrawerNavOp::MenuClose));
    }
}
