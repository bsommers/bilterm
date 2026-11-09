//! Mouse handling for the TRAFFIC MONITOR window (`TMON-04`, Phase 64-03): a pure planner plus a
//! thin applier that only calls `WindowManager::apply`.

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use super::mouse_wm::apply_rect_edge_resize;
use crate::action::Action;
use crate::ui::typography::PanelTarget;
use crate::wm::WindowManager;
use crate::wm::agents_list::ListDrag;
use crate::wm::traffic_monitor::TrafficMonitorOp as Op;
use crate::wm::traffic_monitor_geometry::{TrafficBadge, TrafficChip, TrafficHit};

const WHEEL_ROWS: i32 = 3;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TrafficMonitorMousePlan {
    pub actions: Vec<Action>,
    pub drag: Option<Option<ListDrag>>,
    pub swallow: bool,
    pub pass_through: bool,
}

fn act(op: Op) -> Action {
    Action::TrafficMonitor(op)
}

pub fn plan_traffic_monitor_mouse(wm: &WindowManager, mouse: MouseEvent, ws: Rect) -> Option<TrafficMonitorMousePlan> {
    let (mouse, ws) = wm.panel_space(mouse, ws, PanelTarget::Traffic);
    if !wm.traffic_monitor().active() || wm.traffic_monitor_rect(ws).is_none() {
        return None;
    }
    if let Some(plan) = plan_drag(wm, mouse, ws) {
        return Some(plan);
    }
    let hit = wm.traffic_monitor_hit(ws, mouse.column, mouse.row);
    if hit == TrafficHit::Outside {
        let blur = wm.traffic_monitor().focused && matches!(mouse.kind, MouseEventKind::Down(_));
        return Some(TrafficMonitorMousePlan {
            actions: if blur { vec![act(Op::Blur)] } else { Vec::new() },
            pass_through: blur,
            ..Default::default()
        });
    }
    let mut plan = TrafficMonitorMousePlan { swallow: true, ..Default::default() };
    let ctrl = mouse.modifiers.contains(KeyModifiers::CONTROL);
    let scale = i32::from(wm.scale_setting(PanelTarget::Traffic));
    match mouse.kind {
        MouseEventKind::ScrollUp if ctrl => plan.actions.push(act(Op::Scale((scale + 10).clamp(75, 200) as u16))),
        MouseEventKind::ScrollDown if ctrl => plan.actions.push(act(Op::Scale((scale - 10).clamp(75, 200) as u16))),
        MouseEventKind::ScrollUp => plan.actions.push(act(Op::Scroll(-WHEEL_ROWS))),
        MouseEventKind::ScrollDown => plan.actions.push(act(Op::Scroll(WHEEL_ROWS))),
        MouseEventKind::Down(button) => plan_press(wm, button, hit, mouse, ws, &mut plan),
        _ => {}
    }
    Some(plan)
}

fn plan_drag(wm: &WindowManager, mouse: MouseEvent, ws: Rect) -> Option<TrafficMonitorMousePlan> {
    let drag = wm.traffic_monitor().drag.clone()?;
    let out = |actions, drag| Some(TrafficMonitorMousePlan { actions, drag, swallow: true, pass_through: false });
    match mouse.kind {
        MouseEventKind::Drag(MouseButton::Left) => {
            let cur = wm.traffic_monitor_rect(ws)?;
            let rect = match drag {
                ListDrag::Move { grab_dx, grab_dy } => {
                    Rect::new(mouse.column.saturating_sub(grab_dx), mouse.row.saturating_sub(grab_dy), cur.width, cur.height)
                }
                ListDrag::Resize { edge, initial } => apply_rect_edge_resize(initial, edge, mouse.column, mouse.row, ws),
            };
            out(vec![act(Op::SetRect { x: rect.x, y: rect.y, w: rect.width, h: rect.height })], None)
        }
        MouseEventKind::Up(MouseButton::Left) => out(Vec::new(), Some(None)),
        _ => None,
    }
}

fn plan_press(
    wm: &WindowManager,
    button: MouseButton,
    hit: TrafficHit,
    mouse: MouseEvent,
    ws: Rect,
    plan: &mut TrafficMonitorMousePlan,
) {
    if !wm.traffic_monitor().focused && !matches!(hit, TrafficHit::Badge(TrafficBadge::Close)) {
        plan.actions.push(act(Op::Open));
    }
    let Some(outer) = wm.traffic_monitor_rect(ws) else { return };
    match (button, hit) {
        (MouseButton::Left, TrafficHit::Row(idx)) => {
            // A click on the already selected row drills (the double-click gesture).
            plan.actions.push(act(if idx == wm.traffic_monitor().selected { Op::Drill } else { Op::SelectRow(idx) }));
        }
        (MouseButton::Right, TrafficHit::Row(idx)) => plan.actions.push(act(Op::SelectRow(idx))),
        (MouseButton::Left, TrafficHit::GroupTab(g)) => plan.actions.push(act(Op::GroupBy(g))),
        (MouseButton::Left, TrafficHit::Header(k)) => plan.actions.push(act(Op::Sort(k, None))),
        (MouseButton::Left, TrafficHit::LocalityChip(l)) => plan.actions.push(act(Op::Locality(l))),
        (MouseButton::Left, TrafficHit::Filter) => plan.actions.push(act(Op::FilterFocus(true))),
        (MouseButton::Left, TrafficHit::Chip(c)) => plan.actions.push(act(match c {
            TrafficChip::Pause => Op::Pause(None),
            TrafficChip::Reset => Op::Reset,
            TrafficChip::Drill => Op::Drill,
        })),
        (MouseButton::Left, TrafficHit::Badge(b)) => plan.actions.push(act(match b {
            TrafficBadge::Pin => Op::Pin(None),
            TrafficBadge::Dock => Op::Dock(None),
            TrafficBadge::Close => Op::Close,
        })),
        (MouseButton::Left, TrafficHit::Title) => {
            plan.drag = Some(Some(ListDrag::Move {
                grab_dx: mouse.column.saturating_sub(outer.x),
                grab_dy: mouse.row.saturating_sub(outer.y),
            }));
        }
        (MouseButton::Left, TrafficHit::Edge(edge)) => {
            plan.drag = Some(Some(ListDrag::Resize { edge, initial: outer }));
        }
        _ => {}
    }
}

pub(super) fn handle_traffic_monitor_mouse(wm: &mut WindowManager, mouse: MouseEvent, ws: Rect) -> Option<AppAction> {
    let plan = plan_traffic_monitor_mouse(wm, mouse, ws)?;
    if let Some(drag) = plan.drag {
        wm.traffic_monitor_mut().drag = drag;
    }
    let mut last = None;
    for action in plan.actions {
        if let Ok(a) = wm.apply(action) {
            last = Some(a);
        }
    }
    plan.swallow.then(|| last.unwrap_or(AppAction::Handled))
}
