//! Mouse handling and drag-and-drop planning for `SESSION MAP` (Phase 54, VIEW-03).

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use super::mouse_wm::ctrl_wheel_scale_delta;
use crate::action::Action;
use crate::ui::typography::PanelTarget;
use crate::wm::WindowManager;
use crate::wm::WindowResizeEdge;
use crate::wm::session_map::{SessionMapDrag, SessionMapOp};

/// Result of hit-testing mouse coordinates against the session map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionMapHit {
    Outside,
    Border(WindowResizeEdge),
    TitleBar,
    CloseBadge,
    DockBadge,
    PinBadge,
    WindowBox { pane_id: usize, session_name: String },
    SessionContainer(String),
    Interior,
}

/// What one mouse event does to the session map window.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SessionMapMousePlan {
    pub actions: Vec<Action>,
    pub drag: Option<Option<SessionMapDrag>>,
    pub swallow: bool,
    pub pass_through: bool,
}

fn act(op: SessionMapOp) -> Action {
    Action::SessionMap(op)
}

fn rect_contains(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.right() && row >= r.y && row < r.bottom()
}

/// Hit-tests `(col, row)` against the session map window and its packed interior.
pub fn hit_test_session_map(wm: &WindowManager, area: Rect, col: u16, row: u16) -> SessionMapHit {
    let Some(outer) = wm.session_map_rect(area) else {
        return SessionMapHit::Outside;
    };
    if !rect_contains(outer, col, row) {
        return SessionMapHit::Outside;
    }

    let right = outer.right();
    let bottom = outer.bottom();

    // Top border line
    if row == outer.y {
        if col >= right.saturating_sub(4) && col < right.saturating_sub(1) {
            return SessionMapHit::CloseBadge;
        }
        if col >= right.saturating_sub(8) && col < right.saturating_sub(5) {
            return SessionMapHit::DockBadge;
        }
        if col >= right.saturating_sub(12) && col < right.saturating_sub(9) {
            return SessionMapHit::PinBadge;
        }
        return SessionMapHit::TitleBar;
    }

    // Outer borders
    if row == bottom.saturating_sub(1) {
        if col == outer.x {
            return SessionMapHit::Border(WindowResizeEdge::BottomLeft);
        }
        if col == right.saturating_sub(1) {
            return SessionMapHit::Border(WindowResizeEdge::BottomRight);
        }
        return SessionMapHit::Border(WindowResizeEdge::Bottom);
    }
    if col == outer.x {
        return SessionMapHit::Border(WindowResizeEdge::Left);
    }
    if col == right.saturating_sub(1) {
        return SessionMapHit::Border(WindowResizeEdge::Right);
    }

    // Interior area
    let interior = Rect::new(
        outer.x + 1,
        outer.y + 2,
        outer.width.saturating_sub(2),
        outer.height.saturating_sub(3),
    );
    if interior.width >= 4 && interior.height >= 3 && rect_contains(interior, col, row) {
        let layout = wm.compute_session_pack_layout(interior);
        for sess in &layout {
            for win in &sess.windows {
                if rect_contains(win.rect, col, row) {
                    return SessionMapHit::WindowBox {
                        pane_id: win.pane_id,
                        session_name: sess.session_name.clone(),
                    };
                }
            }
            if rect_contains(sess.outer, col, row) {
                return SessionMapHit::SessionContainer(sess.session_name.clone());
            }
        }
    }

    SessionMapHit::Interior
}

/// Pure planning of `mouse` against the session map window.
pub fn plan_session_map_mouse(
    wm: &WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<SessionMapMousePlan> {
    let (mouse, ws) = wm.panel_space(mouse, workspace_area, PanelTarget::SessionMap);
    if !wm.session_map().active || wm.session_map_rect(ws).is_none() {
        return None;
    }

    // 1. Ongoing drag has priority
    if let Some(ref drag) = wm.session_map().drag {
        let mut plan = SessionMapMousePlan {
            swallow: true,
            ..SessionMapMousePlan::default()
        };
        match mouse.kind {
            MouseEventKind::Drag(MouseButton::Left) => {
                match drag {
                    SessionMapDrag::Move { grab_dx, grab_dy } => {
                        let nx = mouse.column.saturating_sub(*grab_dx);
                        let ny = mouse.row.saturating_sub(*grab_dy);
                        let outer = wm.session_map().window_rect(ws);
                        plan.actions.push(act(SessionMapOp::SetRect {
                            x: nx,
                            y: ny,
                            w: outer.width,
                            h: outer.height,
                        }));
                    }
                    SessionMapDrag::Resize { edge, initial } => {
                        let (x, y, mut w, mut h) =
                            (initial.x, initial.y, initial.width, initial.height);
                        match edge {
                            WindowResizeEdge::Right => {
                                w = mouse.column.saturating_sub(x).max(20);
                            }
                            WindowResizeEdge::Bottom => {
                                h = mouse.row.saturating_sub(y).max(8);
                            }
                            WindowResizeEdge::BottomRight => {
                                w = mouse.column.saturating_sub(x).max(20);
                                h = mouse.row.saturating_sub(y).max(8);
                            }
                            _ => {}
                        }
                        plan.actions.push(act(SessionMapOp::SetRect { x, y, w, h }));
                    }
                    SessionMapDrag::WindowDrag {
                        pane_id,
                        source_session,
                        grab_x,
                        grab_y,
                        ..
                    } => {
                        let hit = hit_test_session_map(wm, ws, mouse.column, mouse.row);
                        let target = match hit {
                            SessionMapHit::SessionContainer(ref s) | SessionMapHit::WindowBox { session_name: ref s, .. } => {
                                if s != source_session {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        };
                        plan.drag = Some(Some(SessionMapDrag::WindowDrag {
                            pane_id: *pane_id,
                            source_session: source_session.clone(),
                            grab_x: *grab_x,
                            grab_y: *grab_y,
                            current_x: mouse.column,
                            current_y: mouse.row,
                            target_session: target.clone(),
                        }));
                        if let Some(t) = target {
                            plan.actions.push(act(SessionMapOp::TargetSession(t)));
                        }
                    }
                }
                return Some(plan);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if let SessionMapDrag::WindowDrag { target_session, .. } = drag {
                    if target_session.is_some() {
                        plan.actions.push(act(SessionMapOp::Drop));
                    } else {
                        plan.actions.push(act(SessionMapOp::CancelDrag));
                    }
                }
                plan.drag = Some(None);
                return Some(plan);
            }
            _ => return Some(plan),
        }
    }

    // 2. Normal hit testing
    let hit = hit_test_session_map(wm, ws, mouse.column, mouse.row);
    if hit == SessionMapHit::Outside {
        let blur = wm.session_map().focused && matches!(mouse.kind, MouseEventKind::Down(_));
        return Some(SessionMapMousePlan {
            actions: if blur { vec![act(SessionMapOp::Blur)] } else { Vec::new() },
            pass_through: blur,
            ..SessionMapMousePlan::default()
        });
    }

    let mut plan = SessionMapMousePlan {
        swallow: true,
        ..SessionMapMousePlan::default()
    };

    if let Some(delta) = ctrl_wheel_scale_delta(&mouse) {
        plan.actions.push(act(SessionMapOp::ScaleStep(delta)));
        return Some(plan);
    }

    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
        match hit {
            SessionMapHit::CloseBadge => plan.actions.push(act(SessionMapOp::Close)),
            SessionMapHit::DockBadge => plan.actions.push(act(SessionMapOp::Dock(None))),
            SessionMapHit::PinBadge => plan.actions.push(act(SessionMapOp::Pin(None))),
            SessionMapHit::TitleBar => {
                let outer = wm.session_map().window_rect(ws);
                plan.drag = Some(Some(SessionMapDrag::Move {
                    grab_dx: mouse.column.saturating_sub(outer.x),
                    grab_dy: mouse.row.saturating_sub(outer.y),
                }));
            }
            SessionMapHit::Border(edge) => {
                let outer = wm.session_map().window_rect(ws);
                plan.drag = Some(Some(SessionMapDrag::Resize {
                    edge,
                    initial: outer,
                }));
            }
            SessionMapHit::WindowBox { pane_id, session_name } => {
                plan.actions.push(act(SessionMapOp::Pickup(Some(pane_id))));
                plan.drag = Some(Some(SessionMapDrag::WindowDrag {
                    pane_id,
                    source_session: session_name,
                    grab_x: mouse.column,
                    grab_y: mouse.row,
                    current_x: mouse.column,
                    current_y: mouse.row,
                    target_session: None,
                }));
            }
            SessionMapHit::SessionContainer(name) => {
                plan.actions.push(act(SessionMapOp::TargetSession(name)));
            }
            _ => {}
        }
    }

    Some(plan)
}

/// Applies `plan_session_map_mouse` and updates window manager drag state.
pub fn handle_session_map_mouse(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<AppAction> {
    let plan = plan_session_map_mouse(wm, mouse, workspace_area)?;
    if let Some(drag) = plan.drag {
        wm.session_map_mut().drag = drag;
    }
    for action in plan.actions {
        let _ = wm.apply(action);
    }
    if plan.pass_through {
        None
    } else if plan.swallow {
        Some(AppAction::Handled)
    } else {
        None
    }
}
