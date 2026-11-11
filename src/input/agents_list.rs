//! Mouse handling for the AGENTS list window (Phase 51-07, UIT-03, `51-CONTRACT.md` section 5).
//!
//! The logic is split in two: [`plan_agents_list_mouse`] is a pure function from an event and a
//! `&WindowManager` to a [`ListMousePlan`] (which `Action`s to run, what to do with the drag
//! bookkeeping, whether the event is swallowed), so tests assert exactly which Action every gesture
//! produces. `handle_agents_list_mouse` is the thin wrapper that stores the drag, applies every
//! Action through `WindowManager::apply` and reports whether the event was consumed. The only
//! pointer state kept is the drag (`AgentsListState::drag`); every visible effect is an
//! `Action::AgentsList`.

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use super::mouse_wm::{apply_rect_edge_resize, ctrl_wheel_scale_delta};
use crate::action::Action;
use crate::ui::typography::PanelTarget;
use crate::wm::WindowManager;
use crate::wm::agents_list::ListDrag;
use crate::wm::agents_list_cmd::AgentsListOp;
use crate::wm::agents_list_geometry::{ListBadge, ListChip, ListHit};

/// Rows scrolled per wheel notch.
const WHEEL_ROWS: i16 = 3;

/// What one mouse event does to the list window.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ListMousePlan {
    /// Actions to run through `WindowManager::apply`, in order.
    pub actions: Vec<Action>,
    /// Drag bookkeeping: `None` = unchanged, `Some(None)` = clear, `Some(Some(d))` = start `d`.
    pub drag: Option<Option<ListDrag>>,
    /// The event is consumed (no pane, drawer or traffic window beneath sees it).
    pub swallow: bool,
    /// The event goes on to whatever lies beneath (a press outside a focused window).
    pub pass_through: bool,
}

fn act(op: AgentsListOp) -> Action {
    Action::AgentsList(op)
}

/// Plans `mouse` (real workspace coordinates) against the list window; `None` when the window is
/// closed or has no rect. Pure: nothing is mutated.
pub fn plan_agents_list_mouse(wm: &WindowManager, mouse: MouseEvent, workspace_area: Rect) -> Option<ListMousePlan> {
    // The window may be drawn at its own GUI scale: work in its virtual (scaled) cell space.
    let (mouse, ws) = wm.panel_space(mouse, workspace_area, PanelTarget::AgentsList);
    if !wm.agents_list().active || wm.agents_list_rect(ws).is_none() {
        return None;
    }
    if let Some(plan) = plan_drag(wm, mouse, ws) {
        return Some(plan);
    }
    let focused = wm.agents_list().focused;
    let hit = wm.agents_list_hit(ws, mouse.column, mouse.row);
    if hit == ListHit::Outside {
        // Not modal: a press elsewhere hands the keyboard back and then goes on to the panes.
        let blur = focused && matches!(mouse.kind, MouseEventKind::Down(_));
        return Some(ListMousePlan {
            actions: if blur { vec![act(AgentsListOp::Blur)] } else { Vec::new() },
            pass_through: blur,
            ..ListMousePlan::default()
        });
    }
    let mut plan = ListMousePlan { swallow: true, ..ListMousePlan::default() };
    if let Some(delta) = ctrl_wheel_scale_delta(&mouse) {
        plan.actions.push(act(AgentsListOp::ScaleStep(delta)));
        return Some(plan);
    }
    match mouse.kind {
        MouseEventKind::ScrollUp => plan.actions.push(act(AgentsListOp::Scroll(-WHEEL_ROWS))),
        MouseEventKind::ScrollDown => plan.actions.push(act(AgentsListOp::Scroll(WHEEL_ROWS))),
        MouseEventKind::Down(button) => plan_press(wm, button, hit, mouse, ws, &mut plan),
        _ => {}
    }
    Some(plan)
}

/// A drag in progress owns Left-drag and Left-up wherever the pointer is.
fn plan_drag(wm: &WindowManager, mouse: MouseEvent, ws: Rect) -> Option<ListMousePlan> {
    let drag = wm.agents_list().drag.clone()?;
    let swallow = |actions, drag| Some(ListMousePlan { actions, drag, swallow: true, pass_through: false });
    match mouse.kind {
        MouseEventKind::Drag(MouseButton::Left) => {
            let cur = wm.agents_list_rect(ws)?;
            let rect = match drag {
                ListDrag::Move { grab_dx, grab_dy } => Rect::new(
                    mouse.column.saturating_sub(grab_dx),
                    mouse.row.saturating_sub(grab_dy),
                    cur.width,
                    cur.height,
                ),
                ListDrag::Resize { edge, initial } => apply_rect_edge_resize(initial, edge, mouse.column, mouse.row, ws),
            };
            let op = AgentsListOp::SetRect { x: rect.x, y: rect.y, w: rect.width, h: rect.height };
            swallow(vec![act(op)], None)
        }
        MouseEventKind::Up(MouseButton::Left) => swallow(Vec::new(), Some(None)),
        _ => None,
    }
}

/// A press inside the window. An unfocused window first retakes the keyboard (`Open`), except for
/// the badges that leave the window anyway.
fn plan_press(wm: &WindowManager, button: MouseButton, hit: ListHit, mouse: MouseEvent, ws: Rect, plan: &mut ListMousePlan) {
    // A row click and the leaving badges hand the keyboard on, so they do not take it first.
    // Ctrl+click only selects (the pointer twin of the arrow keys) and so takes the keyboard.
    let select_only = mouse.modifiers.contains(KeyModifiers::CONTROL);
    let leaves = matches!(hit, ListHit::Badge(ListBadge::Close | ListBadge::Minimize))
        || (!select_only && matches!((button, &hit), (MouseButton::Left, ListHit::Row(_))));
    if !wm.agents_list().focused && !leaves {
        plan.actions.push(act(AgentsListOp::Open));
    }
    let Some(outer) = wm.agents_list_rect(ws) else { return };
    match (button, hit) {
        // One click selects the row and jumps: the pane is raised and gets the keyboard, the list
        // stays open, blurred (FOCUS-01).
        (MouseButton::Left, ListHit::Row(id)) => {
            plan.actions.push(act(AgentsListOp::SelectPane(id)));
            if !select_only {
                plan.actions.push(act(AgentsListOp::Jump(Some(id))));
            }
        }
        (MouseButton::Right, ListHit::Row(id)) => plan.actions.push(act(AgentsListOp::Zoom(Some(id)))),
        (MouseButton::Left, ListHit::Badge(badge)) => plan.actions.push(act(match badge {
            ListBadge::Pin => AgentsListOp::Pin(None),
            ListBadge::Minimize => AgentsListOp::Dismiss,
            ListBadge::Dock => AgentsListOp::Dock(None),
            ListBadge::Close => AgentsListOp::Close,
        })),
        (MouseButton::Left, ListHit::Chip(chip)) => plan.actions.push(act(match chip {
            ListChip::Filter => AgentsListOp::Filter(None),
            ListChip::Dock => AgentsListOp::Dock(None),
            ListChip::Zoom => AgentsListOp::Zoom(None),
        })),
        (MouseButton::Left, ListHit::Title) => {
            let grab_dx = mouse.column.saturating_sub(outer.x);
            let grab_dy = mouse.row.saturating_sub(outer.y);
            plan.drag = Some(Some(ListDrag::Move { grab_dx, grab_dy }));
        }
        (MouseButton::Left, ListHit::Border(edge)) => {
            plan.drag = Some(Some(ListDrag::Resize { edge, initial: outer }));
        }
        _ => {}
    }
}

/// Handles a mouse event over the AGENTS list window; `None` when the event is not for it (closed,
/// outside, or a press outside that merely blurs it) so the traffic window and panes see it next.
pub(super) fn handle_agents_list_mouse(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<AppAction> {
    let plan = plan_agents_list_mouse(wm, mouse, workspace_area)?;
    if let Some(drag) = plan.drag {
        wm.agents_list_mut().drag = drag;
    }
    for action in plan.actions {
        let _ = wm.apply(action);
    }
    plan.swallow.then_some(AppAction::Handled)
}
