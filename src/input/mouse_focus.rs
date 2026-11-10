//! Mouse focus isolation (Phase 65 `GUARD-03`): hit resolution for in-flight drags and the rule
//! that a button press on an inactive pane only focuses and raises it.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::wm::{Focus, WindowManager};

/// Resolves the focus target plus its outer and inner rects for a mouse event. While the active
/// pane owns a drag, Drag/Up events stay bound to it even when the pointer leaves its rect.
pub(super) fn resolve_hit(
    wm: &WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<(Focus, Rect, Rect)> {
    let drag_owned = wm.active_pane().mouse_drag().is_some()
        && matches!(
            mouse.kind,
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left)
        );
    if !drag_owned {
        return wm.hit_test_pane(mouse.column, mouse.row, workspace_area);
    }
    match wm.focus() {
        Focus::Quick => {
            let qt_outer = wm.quick_terminal().overlay_rect(workspace_area);
            Some((Focus::Quick, qt_outer, crate::wm::inset_window_frame(qt_outer)))
        }
        Focus::Pane(id) => {
            let rects = wm.compute_workspace_rects(workspace_area);
            if let Some(&(_, r_outer)) = rects.iter().find(|&&(pid, _)| pid == id) {
                let r_inner = if wm.is_framed() {
                    crate::wm::inset_window_frame(r_outer)
                } else {
                    r_outer
                };
                Some((Focus::Pane(id), r_outer, r_inner))
            } else {
                wm.hit_test_pane(mouse.column, mouse.row, workspace_area)
            }
        }
    }
}

/// The quick terminal always counts as active; a pane is active only when it holds input focus.
/// `active_pane_id()` is the z-stack top, which stays set while the visible quick terminal owns
/// focus, so a click on that pane must still count as a focus change.
pub(super) fn is_active_hit(wm: &WindowManager, hit: Focus) -> bool {
    match hit {
        Focus::Quick => true,
        Focus::Pane(id) => id == wm.active_pane_id() && wm.focus() == Focus::Pane(id),
    }
}

/// Any button press on a pane focuses and raises it.
pub(super) fn focus_on_down(wm: &mut WindowManager, kind: MouseEventKind, hit: Focus) {
    if matches!(kind, MouseEventKind::Down(_))
        && let Focus::Pane(id) = hit
    {
        wm.focus_and_raise(id);
    }
}

/// `true` when the event is a button press on a pane that was inactive before this click, so it
/// must be swallowed after window-management controls (title buttons, borders) had their turn.
/// Middle is included: a focusing middle click must not paste or reach the child.
pub(super) fn consumes_inactive_click(was_active: bool, kind: MouseEventKind) -> bool {
    !was_active
        && matches!(
            kind,
            MouseEventKind::Down(MouseButton::Left | MouseButton::Right | MouseButton::Middle)
        )
}

/// Tracks the press/release pair of a focus-only click. A new press clears stale state; the Drag
/// and Up that follow a consumed press are swallowed (returns `true`) so a release never reaches
/// the child or selection code without its press.
pub(super) fn swallow_focus_click_tail(wm: &mut WindowManager, kind: MouseEventKind) -> bool {
    match kind {
        MouseEventKind::Down(_) => {
            wm.active_pane_mut().set_focus_click_pending(false);
            false
        }
        MouseEventKind::Drag(_) if wm.active_pane().focus_click_pending() => true,
        MouseEventKind::Up(_) if wm.active_pane().focus_click_pending() => {
            wm.active_pane_mut().set_focus_click_pending(false);
            true
        }
        _ => false,
    }
}

/// Records that the press just consumed by [`consumes_inactive_click`] awaits its release.
pub(super) fn mark_focus_click(wm: &mut WindowManager) {
    wm.active_pane_mut().set_focus_click_pending(true);
}
