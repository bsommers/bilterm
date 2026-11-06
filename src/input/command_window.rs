//! Mouse handling for the Command Palette window (WIN-02, Phase 51.2-02).

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use super::mouse_wm::apply_rect_edge_resize;
use crate::action::Action;
use crate::wm::WindowManager;
use crate::wm::agents_list::ListDrag;
use crate::wm::command_window::{CommandWindowBadge, CommandWindowHit, CommandWindowOp};

const WHEEL_ROWS: i32 = 3;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CommandWindowMousePlan {
    pub actions: Vec<Action>,
    pub drag: Option<Option<ListDrag>>,
    pub swallow: bool,
    pub pass_through: bool,
}

fn act(op: CommandWindowOp) -> Action {
    Action::CommandWindow(op)
}

pub fn plan_command_window_mouse(
    wm: &WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<CommandWindowMousePlan> {
    if !wm.command_window().active || wm.command_window_rect(workspace_area).is_none() {
        return None;
    }
    if let Some(plan) = plan_drag(wm, mouse, workspace_area) {
        return Some(plan);
    }
    let focused = wm.command_window().focused;
    let hit = wm.command_window_hit(workspace_area, mouse.column, mouse.row);
    if hit == CommandWindowHit::Outside {
        let blur = focused && matches!(mouse.kind, MouseEventKind::Down(_));
        return Some(CommandWindowMousePlan {
            actions: if blur { vec![act(CommandWindowOp::Blur)] } else { Vec::new() },
            pass_through: blur,
            ..CommandWindowMousePlan::default()
        });
    }
    let mut plan = CommandWindowMousePlan {
        swallow: true,
        ..CommandWindowMousePlan::default()
    };
    match mouse.kind {
        MouseEventKind::ScrollUp => plan.actions.push(act(CommandWindowOp::Scroll(-WHEEL_ROWS))),
        MouseEventKind::ScrollDown => plan.actions.push(act(CommandWindowOp::Scroll(WHEEL_ROWS))),
        MouseEventKind::Down(button) => plan_press(wm, button, hit, mouse, workspace_area, &mut plan),
        _ => {}
    }
    Some(plan)
}

fn plan_drag(wm: &WindowManager, mouse: MouseEvent, ws: Rect) -> Option<CommandWindowMousePlan> {
    let drag = wm.command_window().drag.clone()?;
    let swallow = |actions, drag| Some(CommandWindowMousePlan { actions, drag, swallow: true, pass_through: false });
    match mouse.kind {
        MouseEventKind::Drag(MouseButton::Left) => {
            let cur = wm.command_window_rect(ws)?;
            let rect = match drag {
                ListDrag::Move { grab_dx, grab_dy } => Rect::new(
                    mouse.column.saturating_sub(grab_dx),
                    mouse.row.saturating_sub(grab_dy),
                    cur.width,
                    cur.height,
                ),
                ListDrag::Resize { edge, initial } => apply_rect_edge_resize(initial, edge, mouse.column, mouse.row, ws),
            };
            let op = CommandWindowOp::SetRect { x: rect.x, y: rect.y, w: rect.width, h: rect.height };
            swallow(vec![act(op)], None)
        }
        MouseEventKind::Up(MouseButton::Left) => swallow(Vec::new(), Some(None)),
        _ => None,
    }
}

fn plan_press(
    wm: &WindowManager,
    button: MouseButton,
    hit: CommandWindowHit,
    mouse: MouseEvent,
    ws: Rect,
    plan: &mut CommandWindowMousePlan,
) {
    let leaves = matches!(hit, CommandWindowHit::Badge(CommandWindowBadge::Close));
    if !wm.command_window().focused && !leaves {
        plan.actions.push(act(CommandWindowOp::Open));
    }
    let Some(outer) = wm.command_window_rect(ws) else { return };
    match (button, hit) {
        (MouseButton::Left, CommandWindowHit::Row(idx)) => {
            if idx == wm.command_window().selected {
                plan.actions.push(act(CommandWindowOp::ExecuteSelected));
            } else {
                plan.actions.push(act(CommandWindowOp::SelectRow(idx)));
            }
        }
        (MouseButton::Left, CommandWindowHit::Badge(badge)) => plan.actions.push(act(match badge {
            CommandWindowBadge::Pin => CommandWindowOp::Pin(None),
            CommandWindowBadge::Dock => CommandWindowOp::Dock(None),
            CommandWindowBadge::Close => CommandWindowOp::Close,
        })),
        (MouseButton::Left, CommandWindowHit::Title) => {
            let grab_dx = mouse.column.saturating_sub(outer.x);
            let grab_dy = mouse.row.saturating_sub(outer.y);
            plan.drag = Some(Some(ListDrag::Move { grab_dx, grab_dy }));
        }
        (MouseButton::Left, CommandWindowHit::Border(edge)) => {
            plan.drag = Some(Some(ListDrag::Resize { edge, initial: outer }));
        }
        _ => {}
    }
}

pub(super) fn handle_command_window_mouse(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<AppAction> {
    let plan = plan_command_window_mouse(wm, mouse, workspace_area)?;
    if let Some(drag) = plan.drag {
        wm.command_window_mut().drag = drag;
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
