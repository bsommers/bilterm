//! Key and mouse handling for the Find window (Phase 50.1-01 key path, 50.1-08 mouse path).
//!
//! Both handlers only look gestures up and lower them to `Action::Find`, which runs through
//! `WindowManager::apply` (`ARCH-R01`); nothing here mutates `Modal::Find` directly.

use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use super::keymap::{default_global_action, find_modal_key_action};
use crate::action::Action;
use crate::error::Result;
use crate::ui::find_ui::{FindLayout, find_layout};
use crate::wm::{WindowManager, inset_window_frame, rect_contains};
use crate::wm::find_cmd::{FindKind, FindOp};

/// Handles `key` while `Modal::Find` is open: the entry chord closes, table rows and printable
/// characters lower to `Action::Find`, everything else is swallowed (never leaks to the pane).
pub fn handle_find_modal_key(wm: &mut WindowManager, key: KeyEvent) -> Result<Option<AppAction>> {
    let action = if matches!(default_global_action(key, false), Some(Action::Find(FindOp::Open { .. }))) {
        Some(Action::Find(FindOp::Close))
    } else {
        find_modal_key_action(key)
    };
    if let Some(action) = action {
        wm.apply(action)?;
    }
    Ok(Some(AppAction::Handled))
}

/// Rows a mouse wheel notch moves the selection (`:finder move +-3` is the colon twin).
const WHEEL_ROWS: i16 = 3;

/// First list row drawn for `selected` in a viewport of `list_rows` rows. Duplicates the one-line
/// formula the renderer uses in `src/ui/find_ui.rs` (`selected.saturating_sub(list_rows - 1)`);
/// that file is owned by plan 50.1-07, so the formula is repeated here and tested instead.
pub fn list_viewport_start(selected: usize, list_rows: usize) -> usize {
    selected.saturating_sub(list_rows.saturating_sub(1))
}

/// Handles `mouse` inside the Find window's outer rect. Gestures that mean something to Find are
/// lowered through [`lower_find_mouse`] and applied via `wm.apply`; everything else returns `None`
/// so the generic title-bar drag, border resize and badge handling keeps working.
pub fn handle_find_modal_mouse(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    outer: Rect,
    _workspace_area: Rect,
) -> Option<AppAction> {
    let (kind, _, selected) = wm.find_modal_state()?;
    let layout = find_layout(inset_window_frame(outer), wm.config());
    let row_count = wm.find_results().rows.len();
    let actions = lower_find_mouse(mouse, &layout, row_count, selected, kind)?;
    for action in actions {
        if wm.apply(action).is_err() {
            break;
        }
    }
    Some(AppAction::Handled)
}

/// Index of the list row under `(col, row)` (viewport-aware), when it holds a result.
fn list_row_at(layout: &FindLayout, col: u16, row: u16, row_count: usize, selected: usize) -> Option<usize> {
    if !rect_contains(layout.list, col, row) {
        return None;
    }
    let start = list_viewport_start(selected, usize::from(layout.list.height));
    let index = start + usize::from(row - layout.list.y);
    (index < row_count).then_some(index)
}

/// Pure mapping of one mouse event to the `Action::Find` ops it stands for, against the same
/// [`FindLayout`] the renderer draws with. `None` means "not a Find gesture" (title row, border,
/// outside, empty list area); `Some(vec![])` means "consumed, nothing to do" (hover over the
/// already selected row, so a hover storm applies no `Select`).
pub fn lower_find_mouse(
    mouse: MouseEvent,
    layout: &FindLayout,
    row_count: usize,
    selected: usize,
    _kind: FindKind,
) -> Option<Vec<Action>> {
    let (col, row) = (mouse.column, mouse.row);
    let find = |op| Action::Find(op);
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if rect_contains(layout.clear_chip, col, row) {
                return Some(vec![find(FindOp::ClearQuery)]);
            }
            let kinds = [FindKind::Agent, FindKind::Session, FindKind::History];
            if let Some(i) = layout.tab_chips.iter().position(|r| rect_contains(*r, col, row)) {
                return Some(vec![find(FindOp::SetKind(kinds[i]))]);
            }
            let footer = [
                FindOp::Activate,
                FindOp::CycleKind(true),
                FindOp::Prefill,
                FindOp::Close,
            ];
            if let Some(i) = layout.footer_chips.iter().position(|r| rect_contains(*r, col, row)) {
                return Some(vec![find(footer[i].clone())]);
            }
            let index = list_row_at(layout, col, row, row_count, selected)?;
            Some(vec![find(FindOp::Select(index)), find(FindOp::Activate)])
        }
        MouseEventKind::Moved => {
            let index = list_row_at(layout, col, row, row_count, selected)?;
            Some(if index == selected { Vec::new() } else { vec![find(FindOp::Select(index))] })
        }
        MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
            let inner = layout.tabs.union(layout.footer);
            if !rect_contains(inner, col, row) {
                return None;
            }
            let n = if mouse.kind == MouseEventKind::ScrollDown { WHEEL_ROWS } else { -WHEEL_ROWS };
            Some(vec![find(FindOp::Move(n))])
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::list_viewport_start;

    #[test]
    fn viewport_follows_the_selection_to_the_bottom_row() {
        assert_eq!(list_viewport_start(0, 12), 0);
        assert_eq!(list_viewport_start(11, 12), 0);
        assert_eq!(list_viewport_start(12, 12), 1);
        assert_eq!(list_viewport_start(35, 12), 24);
        assert_eq!(list_viewport_start(3, 0), 3);
    }
}
