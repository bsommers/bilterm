//! Mouse handling for the `CLIPBOARD` viewer window (`CLIP-03`, `CLIP-04`, Phase 57).

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use super::mouse_wm::apply_rect_edge_resize;
use crate::action::Action;
use crate::wm::WindowManager;
use crate::wm::agents_list::ListDrag;
use crate::wm::clipboard_window::{
    ClipboardActionChip, ClipboardWindowBadge, ClipboardWindowHit, ClipboardWindowOp,
};

const WHEEL_ROWS: i32 = 3;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ClipboardWindowMousePlan {
    pub actions: Vec<Action>,
    pub drag: Option<Option<ListDrag>>,
    pub swallow: bool,
    pub pass_through: bool,
}

fn act(op: ClipboardWindowOp) -> Action {
    Action::ClipboardWindow(op)
}

pub fn plan_clipboard_window_mouse(
    wm: &WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<ClipboardWindowMousePlan> {
    let (mouse, workspace_area) =
        wm.panel_space(mouse, workspace_area, crate::ui::typography::PanelTarget::Clipboard);
    if !wm.clipboard_window().active || wm.clipboard_window_rect(workspace_area).is_none() {
        return None;
    }
    if let Some(plan) = plan_drag(wm, mouse, workspace_area) {
        return Some(plan);
    }
    let focused = wm.clipboard_window().focused;
    let hit = wm.clipboard_window_hit(workspace_area, mouse.column, mouse.row);
    if hit == ClipboardWindowHit::Outside {
        let blur = focused && matches!(mouse.kind, MouseEventKind::Down(_));
        return Some(ClipboardWindowMousePlan {
            actions: if blur {
                vec![act(ClipboardWindowOp::Blur)]
            } else {
                Vec::new()
            },
            pass_through: blur,
            ..ClipboardWindowMousePlan::default()
        });
    }
    let mut plan = ClipboardWindowMousePlan {
        swallow: true,
        ..ClipboardWindowMousePlan::default()
    };
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            if mouse.modifiers.contains(KeyModifiers::CONTROL) {
                plan.actions.push(act(ClipboardWindowOp::Scale(10)));
            } else {
                plan.actions.push(act(ClipboardWindowOp::Scroll(-WHEEL_ROWS)));
            }
        }
        MouseEventKind::ScrollDown => {
            if mouse.modifiers.contains(KeyModifiers::CONTROL) {
                plan.actions.push(act(ClipboardWindowOp::Scale(-10)));
            } else {
                plan.actions.push(act(ClipboardWindowOp::Scroll(WHEEL_ROWS)));
            }
        }
        MouseEventKind::Down(button) => {
            plan_press(wm, button, hit, mouse, workspace_area, &mut plan);
        }
        _ => {}
    }
    Some(plan)
}

fn plan_drag(wm: &WindowManager, mouse: MouseEvent, ws: Rect) -> Option<ClipboardWindowMousePlan> {
    let drag = wm.clipboard_window().drag.clone()?;
    let swallow = |actions, drag| {
        Some(ClipboardWindowMousePlan {
            actions,
            drag,
            swallow: true,
            pass_through: false,
        })
    };
    match mouse.kind {
        MouseEventKind::Drag(MouseButton::Left) => {
            let cur = wm.clipboard_window_rect(ws)?;
            let rect = match drag {
                ListDrag::Move { grab_dx, grab_dy } => Rect::new(
                    mouse.column.saturating_sub(grab_dx),
                    mouse.row.saturating_sub(grab_dy),
                    cur.width,
                    cur.height,
                ),
                ListDrag::Resize { edge, initial } => {
                    apply_rect_edge_resize(initial, edge, mouse.column, mouse.row, ws)
                }
            };
            let op = ClipboardWindowOp::SetRect {
                x: rect.x,
                y: rect.y,
                w: rect.width,
                h: rect.height,
            };
            swallow(vec![act(op)], None)
        }
        MouseEventKind::Up(MouseButton::Left) => swallow(Vec::new(), Some(None)),
        _ => None,
    }
}

fn plan_press(
    wm: &WindowManager,
    button: MouseButton,
    hit: ClipboardWindowHit,
    mouse: MouseEvent,
    ws: Rect,
    plan: &mut ClipboardWindowMousePlan,
) {
    let leaves = matches!(hit, ClipboardWindowHit::Badge(ClipboardWindowBadge::Close));
    if !wm.clipboard_window().focused && !leaves {
        plan.actions.push(act(ClipboardWindowOp::Open { scope: None }));
    }
    let Some(outer) = wm.clipboard_window_rect(ws) else {
        return;
    };
    match (button, hit) {
        (MouseButton::Left, ClipboardWindowHit::EntryRow(idx)) => {
            if idx == wm.clipboard_window().selected {
                plan.actions.push(act(ClipboardWindowOp::PasteSelected));
            } else {
                plan.actions.push(act(ClipboardWindowOp::SelectRow(idx)));
            }
        }
        (MouseButton::Right, ClipboardWindowHit::EntryRow(idx)) => {
            if idx != wm.clipboard_window().selected {
                plan.actions.push(act(ClipboardWindowOp::SelectRow(idx)));
            }
            plan.actions.push(act(ClipboardWindowOp::YankSelected));
        }
        (MouseButton::Middle, ClipboardWindowHit::EntryRow(idx)) => {
            if idx != wm.clipboard_window().selected {
                plan.actions.push(act(ClipboardWindowOp::SelectRow(idx)));
            }
            plan.actions.push(act(ClipboardWindowOp::TogglePinSelected));
        }
        (MouseButton::Left, ClipboardWindowHit::ScopeTab(scope)) => {
            plan.actions.push(act(ClipboardWindowOp::SetScope(scope)));
        }
        (MouseButton::Left, ClipboardWindowHit::SessionCycle) => {
            plan.actions.push(act(ClipboardWindowOp::CycleSessionFilter));
        }
        (MouseButton::Left, ClipboardWindowHit::WindowCycle) => {
            plan.actions.push(act(ClipboardWindowOp::CycleWindowFilter));
        }
        (MouseButton::Left, ClipboardWindowHit::SearchRow) => {
            plan.actions.push(act(ClipboardWindowOp::SearchFocus(true)));
        }
        (MouseButton::Left, ClipboardWindowHit::ActionChip(chip)) => {
            plan.actions.push(act(match chip {
                ClipboardActionChip::Paste => ClipboardWindowOp::PasteSelected,
                ClipboardActionChip::Yank => ClipboardWindowOp::YankSelected,
                ClipboardActionChip::PinEntry => ClipboardWindowOp::TogglePinSelected,
                ClipboardActionChip::DeleteEntry => ClipboardWindowOp::DeleteSelected,
                ClipboardActionChip::ClearUnpinned => ClipboardWindowOp::ClearUnpinned,
                ClipboardActionChip::FocusSearch => ClipboardWindowOp::SearchFocus(true),
            }));
        }
        (MouseButton::Left, ClipboardWindowHit::Badge(badge)) => {
            plan.actions.push(act(match badge {
                ClipboardWindowBadge::Pin => ClipboardWindowOp::Pin(None),
                ClipboardWindowBadge::Dock => ClipboardWindowOp::Dock(None),
                ClipboardWindowBadge::Close => ClipboardWindowOp::Close,
            }));
        }
        (MouseButton::Left, ClipboardWindowHit::Title) => {
            let grab_dx = mouse.column.saturating_sub(outer.x);
            let grab_dy = mouse.row.saturating_sub(outer.y);
            plan.drag = Some(Some(ListDrag::Move { grab_dx, grab_dy }));
        }
        (MouseButton::Left, ClipboardWindowHit::Border(edge)) => {
            plan.drag = Some(Some(ListDrag::Resize {
                edge,
                initial: outer,
            }));
        }
        _ => {}
    }
}

pub(super) fn handle_clipboard_window_mouse(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<AppAction> {
    let plan = plan_clipboard_window_mouse(wm, mouse, workspace_area)?;
    if let Some(drag) = plan.drag {
        wm.clipboard_window_mut().drag = drag;
    }
    let mut last = None;
    for action in plan.actions {
        if let Ok(a) = wm.apply(action) {
            last = Some(a);
        }
    }
    if plan.swallow {
        Some(last.unwrap_or(AppAction::Handled))
    } else {
        None
    }
}
