//! Keys and mouse of the Mission Control HUD and the replay view (Phase 70, HUD-01/02).
//!
//! Every gesture is *pure data first*: [`hud_key_action`] / [`replay_key_action`] /
//! [`hud_mouse_action`] map an event to an [`Action`], which the handlers then run through
//! `WindowManager::apply` (`UNI-R01`). Nothing here can reach a PTY writer.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use crate::action::Action;
use crate::wm::WindowManager;
use crate::wm::agents_hud::hud_inner;
use crate::wm::agents_hud_cmd::{HudOp, ReplayOp};
use crate::wm::mission_control_hud::{HudButton, HudButtonKind, HudHit};

fn press(kind: HudButtonKind) -> Action {
    Action::AgentsHud(HudOp::Press {
        pane: None,
        kind,
        generation: None,
    })
}

/// In-HUD key table (`r` replay is in [`agents_hud_key_action`]): `Esc`/`q` close, arrows/Tab/`hjkl` move, PgUp/PgDn page, `a` Approve,
/// `p` Prompt, `d` Diff, `z`/Enter Zoom. Chords with Ctrl/Alt are not the HUD's (`None`).
pub fn hud_key_action(key: KeyEvent) -> Option<Action> {
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return None;
    }
    let op = |o| Some(Action::AgentsHud(o));
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => op(HudOp::Close),
        KeyCode::Left | KeyCode::Up | KeyCode::BackTab | KeyCode::Char('h' | 'k') => {
            op(HudOp::Step(-1))
        }
        KeyCode::Right | KeyCode::Down | KeyCode::Tab | KeyCode::Char('l' | 'j') => {
            op(HudOp::Step(1))
        }
        KeyCode::PageUp => op(HudOp::Page(-1)),
        KeyCode::PageDown => op(HudOp::Page(1)),
        KeyCode::Char('a') => Some(press(HudButtonKind::Approve)),
        KeyCode::Char('p') => Some(press(HudButtonKind::Prompt)),
        KeyCode::Char('d') => Some(press(HudButtonKind::Diff)),
        KeyCode::Char('z') | KeyCode::Enter => Some(press(HudButtonKind::Zoom)),
        _ => None,
    }
}

/// Replay key table: Left/Right previous/next turn, Up/Down scroll, PgUp/PgDn page, Home/End
/// first/last turn, `Esc`/`q` leave.
pub fn replay_key_action(key: KeyEvent, viewport: usize) -> Option<Action> {
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return None;
    }
    let page = i16::try_from(viewport.max(1)).unwrap_or(i16::MAX);
    let op = match key.code {
        KeyCode::Esc | KeyCode::Char('q') => ReplayOp::Close,
        KeyCode::Left => ReplayOp::Prev,
        KeyCode::Right => ReplayOp::Next,
        KeyCode::Up => ReplayOp::Scroll(-1),
        KeyCode::Down => ReplayOp::Scroll(1),
        KeyCode::PageUp => ReplayOp::Scroll(-page),
        KeyCode::PageDown => ReplayOp::Scroll(page),
        KeyCode::Home => ReplayOp::First,
        KeyCode::End => ReplayOp::Last,
        _ => return None,
    };
    Some(Action::AgentsReplay(op))
}

/// [`replay_key_action`] / [`hud_key_action`] for the window that currently owns the keyboard,
/// plus `r` (open the replay of the selected tile, which needs the selection).
pub fn agents_hud_key_action(wm: &WindowManager, key: KeyEvent) -> Option<Action> {
    if wm.agents_hud().replay_pane().is_some() {
        return replay_key_action(key, wm.agents_hud().viewport());
    }
    if key.code == KeyCode::Char('r')
        && !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return wm
            .agents_hud()
            .hud()
            .selected()
            .map(|p| Action::AgentsReplay(ReplayOp::Open(p)));
    }
    hud_key_action(key)
}

/// Routes a key to the replay view (first) or the HUD. Plain keys are always swallowed while a
/// view is open, so nothing typed reaches the pane underneath.
pub(super) fn handle_agents_hud_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return None;
    }
    if let Some(a) = agents_hud_key_action(wm, key) {
        let _ = wm.apply(a);
    }
    Some(AppAction::Handled)
}

/// Maps a mouse event over the HUD / replay window to an action.
///
/// `None`: the event is not the window's. `Some(None)`: swallowed, nothing to do.
pub fn hud_mouse_action(
    wm: &WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<Option<Action>> {
    let outer = wm.agents_hud_rect(workspace_area)?;
    let inside = mouse.column >= outer.x
        && mouse.row >= outer.y
        && mouse.column < outer.x + outer.width
        && mouse.row < outer.y + outer.height;
    let replay = wm.agents_hud().replay_pane().is_some();
    let close = || {
        Some(Some(if replay {
            Action::AgentsReplay(ReplayOp::Close)
        } else {
            Action::AgentsHud(HudOp::Close)
        }))
    };
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) if !inside => close(),
        _ if !inside => None,
        MouseEventKind::ScrollUp if replay => {
            Some(Some(Action::AgentsReplay(ReplayOp::Scroll(-3))))
        }
        MouseEventKind::ScrollDown if replay => {
            Some(Some(Action::AgentsReplay(ReplayOp::Scroll(3))))
        }
        MouseEventKind::ScrollUp => Some(Some(Action::AgentsHud(HudOp::Page(-1)))),
        MouseEventKind::ScrollDown => Some(Some(Action::AgentsHud(HudOp::Page(1)))),
        MouseEventKind::Down(MouseButton::Right) if !replay => {
            let hit = wm
                .agents_hud()
                .hud()
                .hit_test(hud_inner(outer), mouse.column, mouse.row);
            Some(match hit {
                Some(
                    HudHit::Button(HudButton { pane, .. })
                    | HudHit::Disabled { pane, .. }
                    | HudHit::Card(pane),
                ) => Some(Action::AgentsReplay(ReplayOp::Open(pane))),
                None => None,
            })
        }
        MouseEventKind::Down(MouseButton::Left) if !replay => {
            let hit = wm
                .agents_hud()
                .hud()
                .hit_test(hud_inner(outer), mouse.column, mouse.row);
            Some(match hit {
                Some(HudHit::Button(b)) => Some(Action::AgentsHud(HudOp::Press {
                    pane: Some(b.pane),
                    kind: b.kind,
                    generation: b.generation,
                })),
                Some(HudHit::Disabled { pane, .. } | HudHit::Card(pane)) => {
                    Some(Action::AgentsHud(HudOp::SelectPane(pane)))
                }
                None => None,
            })
        }
        _ => Some(None),
    }
}

/// Mouse handler: runs the mapped action; a Zoom / Prompt press re-fits the panes afterwards.
pub(super) fn handle_agents_hud_mouse(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<AppAction> {
    let mapped = hud_mouse_action(wm, mouse, workspace_area)?;
    if let Some(action) = mapped {
        let refit = matches!(
            &action,
            Action::AgentsHud(HudOp::Press {
                kind: HudButtonKind::Zoom | HudButtonKind::Prompt,
                ..
            })
        );
        let _ = wm.apply(action);
        if refit {
            let _ = wm.resize_all(workspace_area);
        }
    }
    Some(AppAction::Handled)
}
