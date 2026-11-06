//! Keys and clicks for the tmux link banner (Phase 51.1, plan 10, UNI-R01).
//!
//! The banner is the one place the user looks when a link is down, so it carries the hotkeys:
//! `Enter`/`r` reconnect, `x` stop, `d` detach, and a left click on the banner row reconnects
//! (uniformity exception E4). Everything lowers to an [`Action`] and runs through
//! `WindowManager::apply`; the GUI and the TUI share these handlers.
//!
//! Passwords are never swallowed: only an *interactive* banner (link down, nothing being typed
//! into an ssh prompt) owns any key. The `Connecting` banner is non-interactive, so `r`, `x`, `d`
//! and `Enter` reach the pane's raw input path untouched.

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use crate::action::Action;
use crate::error::Result;
use crate::wm::link_cmd::RemoteLinkOp;
use crate::wm::{Focus, Modal, WindowManager};

/// Banner hotkeys for the ACTIVE pane. `None` unless the modal is closed, no leader chord is
/// pending, the key is unmodified and the pane shows an interactive link banner.
pub(super) fn handle_link_banner_key(wm: &mut WindowManager, key: KeyEvent) -> Result<Option<AppAction>> {
    if !matches!(wm.modal(), Modal::None)
        || wm.is_leader_pending()
        || !key.modifiers.is_empty()
        || !wm.active_pane().link_banner().is_some_and(|b| b.interactive)
    {
        return Ok(None);
    }
    match key.code {
        KeyCode::Enter | KeyCode::Char('r') | KeyCode::Char('R') => {
            wm.apply(Action::RemoteLink(RemoteLinkOp::Reconnect))?;
            Ok(Some(AppAction::Handled))
        }
        KeyCode::Char('x') | KeyCode::Char('X') => {
            wm.apply(Action::RemoteLink(RemoteLinkOp::Disconnect))?;
            Ok(Some(AppAction::Handled))
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            wm.detach_client();
            Ok(Some(AppAction::Quit))
        }
        _ => Ok(None),
    }
}

/// `pub` (not `pub(super)`): `tests/phase51_1_link_seam.rs` calls it directly.
///
/// A left click on the banner row of a pane: reconnect an interactive link banner, or respawn the
/// held shell of an exited remote pane. Everything else (other buttons, drags, moves, other rows,
/// non-interactive banners) returns `None` and the normal pane routing runs.
///
/// Geometry matches the renderer: the pane's inner rect is the outer rect inset by the window
/// frame when framed (`WorkspaceWidget::render` in `ui/mod.rs`, mirrored by
/// `WindowManager::hit_test_pane`); the banner is the last row of `CompositePane::canvas_rect(inner)`
/// (`render_link_banner` / `render_held_shell_exited_banner`).
pub fn handle_link_banner_click(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<AppAction> {
    if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
        return None;
    }
    let (Focus::Pane(id), _outer, inner) = wm.hit_test_pane(mouse.column, mouse.row, workspace_area)? else {
        return None;
    };
    let pane = wm.pane(id)?;
    let canvas = pane.canvas_rect(inner);
    let on_banner_row = canvas.height > 0
        && mouse.row == canvas.y + canvas.height - 1
        && mouse.column >= canvas.x
        && mouse.column < canvas.x + canvas.width;
    if !on_banner_row {
        return None;
    }
    // The link banner is drawn over the held banner, so it decides what a click means.
    let action = match pane.link_banner().filter(|b| !b.text.is_empty()) {
        Some(b) if b.interactive => Action::RemoteLink(RemoteLinkOp::Reconnect),
        Some(_) => return None,
        None if pane.held_exited_shell().is_some() && pane.remote_target().is_some() => {
            Action::RespawnPaneInPlace
        }
        None => return None,
    };
    wm.focus_and_raise(id);
    let _ = wm.apply(action);
    Some(AppAction::Handled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn ev(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    #[test]
    fn typed_bytes_reach_the_link_input_tap_not_the_banner() {
        let mut wm = WindowManager::new_headless(24, 80);
        {
            let p = wm.active_pane_mut();
            p.set_link_banner(Some(crate::pane::LinkBanner { text: "Connecting".into(), interactive: false }));
            p.set_remote_input_tap(true);
            p.set_link_raw_input(true);
        }
        for k in [KeyCode::Char('r'), KeyCode::Char('x'), KeyCode::Char('d'), KeyCode::Enter] {
            assert_eq!(handle_link_banner_key(&mut wm, ev(k)).unwrap(), None);
            let _ = crate::input::handle_wm_key_event(&mut wm, ev(k)).unwrap();
        }
        assert_eq!(wm.active_pane_mut().take_remote_input(), b"rxd\r".to_vec());
        assert!(wm.active_pane().prompt().history.is_empty());
    }

    #[test]
    fn modified_and_unrelated_keys_fall_through() {
        let mut wm = WindowManager::new_headless(24, 80);
        wm.active_pane_mut()
            .set_link_banner(Some(crate::pane::LinkBanner { text: "down".into(), interactive: true }));
        assert_eq!(handle_link_banner_key(&mut wm, ev(KeyCode::Char('q'))).unwrap(), None);
        let ctrl_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        assert_eq!(handle_link_banner_key(&mut wm, ctrl_r).unwrap(), None);
    }
}
