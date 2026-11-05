//! Key and mouse handling for the Find window (Phase 50.1-01 skeleton; mouse body in 50.1-08).
//!
//! Both handlers only look gestures up and lower them to `Action::Find`, which runs through
//! `WindowManager::apply` (`ARCH-R01`); nothing here mutates `Modal::Find` directly.

use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;

use super::AppAction;
use super::keymap::{default_global_action, find_modal_key_action};
use crate::action::Action;
use crate::error::Result;
use crate::ui::find_ui::FindLayout;
use crate::wm::WindowManager;
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

/// Handles `mouse` inside the Find window's outer rect. Skeleton: always `None` (the generic
/// title-bar and border handling applies); plan 50.1-08 fills it through [`lower_find_mouse`].
pub fn handle_find_modal_mouse(
    _wm: &mut WindowManager,
    _mouse: MouseEvent,
    _outer: Rect,
    _workspace_area: Rect,
) -> Option<AppAction> {
    None
}

/// Pure mapping of one mouse event to the `Action::Find` ops it stands for. Skeleton: always `None`
/// (filled by plan 50.1-08 against the same [`FindLayout`] the renderer uses).
pub fn lower_find_mouse(
    _mouse: MouseEvent,
    _layout: &FindLayout,
    _row_count: usize,
    _selected: usize,
    _kind: FindKind,
) -> Option<Vec<Action>> {
    None
}
