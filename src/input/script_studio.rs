//! Mouse input handling for the SCRIPT STUDIO window (Plan 51.2-03, WIN-03).

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use super::mouse_wm::apply_rect_edge_resize;
use crate::action::Action;
use crate::ui::script_studio_ui::{
    ScriptStudioButton, ScriptStudioHit, script_studio_hit,
};
use crate::wm::WindowManager;
use crate::wm::agents_list::ListDrag;
use crate::wm::script_studio::ScriptStudioOp;

fn act(op: ScriptStudioOp) -> Action {
    Action::ScriptStudio(op)
}

/// Handles mouse events over the Script Studio window; returns `None` if the event is outside
/// or not handled so lower panels can process it.
pub fn handle_script_studio_mouse(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<AppAction> {
    if !wm.script_studio().active || wm.script_studio_rect(workspace_area).is_none() {
        return None;
    }

    // Process active drag gesture if any
    if let Some(drag) = wm.script_studio().drag.clone() {
        match mouse.kind {
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(cur) = wm.script_studio_rect(workspace_area) {
                    let rect = match drag {
                        ListDrag::Move { grab_dx, grab_dy } => Rect::new(
                            mouse.column.saturating_sub(grab_dx),
                            mouse.row.saturating_sub(grab_dy),
                            cur.width,
                            cur.height,
                        ),
                        ListDrag::Resize { edge, initial } => {
                            apply_rect_edge_resize(initial, edge, mouse.column, mouse.row, workspace_area)
                        }
                    };
                    let _ = wm.apply(act(ScriptStudioOp::SetRect {
                        x: rect.x,
                        y: rect.y,
                        w: rect.width,
                        h: rect.height,
                    }));
                }
                return Some(AppAction::Handled);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                wm.script_studio_mut().drag = None;
                return Some(AppAction::Handled);
            }
            _ => {}
        }
    }

    let hit = script_studio_hit(wm, workspace_area, mouse.column, mouse.row);
    if hit == ScriptStudioHit::Outside {
        if wm.script_studio().focused && matches!(mouse.kind, MouseEventKind::Down(_)) {
            let _ = wm.apply(act(ScriptStudioOp::Blur));
        }
        return None;
    }

    // Inside window
    let outer = wm.script_studio_rect(workspace_area)?;

    if !wm.script_studio().focused && matches!(mouse.kind, MouseEventKind::Down(_)) {
        let _ = wm.apply(act(ScriptStudioOp::Open));
    }

    match (mouse.kind, hit) {
        (MouseEventKind::Down(MouseButton::Left), ScriptStudioHit::Title) => {
            let grab_dx = mouse.column.saturating_sub(outer.x);
            let grab_dy = mouse.row.saturating_sub(outer.y);
            wm.script_studio_mut().drag = Some(ListDrag::Move { grab_dx, grab_dy });
        }
        (MouseEventKind::Down(MouseButton::Left), ScriptStudioHit::Border(edge)) => {
            wm.script_studio_mut().drag = Some(ListDrag::Resize { edge, initial: outer });
        }
        (MouseEventKind::Down(MouseButton::Left), ScriptStudioHit::CloseBadge) => {
            let _ = wm.apply(act(ScriptStudioOp::Close));
        }
        (MouseEventKind::Down(MouseButton::Left), ScriptStudioHit::DockBadge) => {
            let _ = wm.apply(act(ScriptStudioOp::Dock(None)));
        }
        (MouseEventKind::Down(MouseButton::Left), ScriptStudioHit::PinBadge) => {
            let _ = wm.apply(act(ScriptStudioOp::Pin(None)));
        }
        (MouseEventKind::Down(MouseButton::Left), ScriptStudioHit::Button(btn)) => match btn {
            ScriptStudioButton::Run => {
                let _ = wm.apply(act(ScriptStudioOp::RunAll));
            }
            ScriptStudioButton::Step => {
                let _ = wm.apply(act(ScriptStudioOp::StepNext));
            }
            ScriptStudioButton::Reset => {
                let _ = wm.apply(act(ScriptStudioOp::ResetStep));
            }
            ScriptStudioButton::Record => {
                let _ = wm.apply(act(ScriptStudioOp::Record(None)));
            }
            ScriptStudioButton::Edit => {
                let _ = wm.apply(act(ScriptStudioOp::EditMode(None)));
            }
            ScriptStudioButton::Save => {
                let _ = wm.apply(act(ScriptStudioOp::Save(None)));
            }
        },
        (MouseEventKind::Down(MouseButton::Left), ScriptStudioHit::Line(idx)) => {
            let _ = wm.apply(act(ScriptStudioOp::SelectLine(idx)));
        }
        (MouseEventKind::ScrollUp, _) => {
            let cur = wm.script_studio().scroll;
            wm.script_studio_mut().scroll = cur.saturating_sub(1);
        }
        (MouseEventKind::ScrollDown, _) => {
            let cur = wm.script_studio().scroll;
            if cur + 1 < wm.script_studio().lines.len() {
                wm.script_studio_mut().scroll = cur + 1;
            }
        }
        _ => {}
    }

    Some(AppAction::Handled)
}
