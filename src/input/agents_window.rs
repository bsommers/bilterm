//! Key handling for the AGENT TRAFFIC window (`FLT-01`, Phase 50-08).
//!
//! This module only resolves which key scopes are active, looks the key up in the declarative
//! [`AGENTS_WINDOW_BINDINGS`](super::keymap::AGENTS_WINDOW_BINDINGS) table and runs the resulting
//! [`Action`](crate::action::Action) through `WindowManager::apply`. It never changes window, pane or
//! selection state itself (`ARCH-R01`); the mouse handler below only keeps pointer-gesture
//! bookkeeping ([`AgentsDrag`]) and lowers every drag step to an `Action::Agents(..)`.

use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use super::keymap::{AgentsKeyScope, agents_window_action, is_leader_key_with_tmux};
use super::mouse_wm::{apply_rect_edge_resize, detect_title_bar_badge_click};
use crate::action::Action;
use crate::wm::agents_cmd::AgentsOp;
use crate::wm::agents_geometry::{AgentsHit, graph_cell_permille};
use crate::wm::agents_window::AgentsDrag;
use crate::wm::{WindowManager, rect_contains};

/// Handles `key` while the agents window captures input.
///
/// Returns `None` for the leader key so the leader table stays reachable; every other key is
/// consumed (looked up, applied when it has a row, otherwise ignored) so keys never leak into the
/// pane below.
pub(super) fn handle_agents_window_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    let inside_tmux = crate::tmux::TmuxEnv::detect().is_some();
    if is_leader_key_with_tmux(key, &wm.config().leader_key, inside_tmux) {
        return None;
    }
    let scopes: &[AgentsKeyScope] = if wm.agents_window().grab_mode {
        &[AgentsKeyScope::Grab, AgentsKeyScope::Graph, AgentsKeyScope::Window]
    } else {
        &[AgentsKeyScope::Graph, AgentsKeyScope::Window]
    };
    if let Some(action) = agents_window_action(scopes, key) {
        let _ = wm.apply(action);
    }
    Some(AppAction::Handled)
}

/// Handles a mouse event over the AGENT TRAFFIC window (`FLT-01`, Phase 50-09).
///
/// Every visible effect lowers to an `Action::Agents(..)` run through `WindowManager::apply`; the
/// only state kept here is which pointer drag is in progress (`set_agents_drag`, bookkeeping). The
/// window swallows events inside it so the drawer and panes beneath never see them. Returns `None`
/// when the window is closed or the event is not for it.
pub(super) fn handle_agents_window_mouse(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<AppAction> {
    if !wm.agents_window().active || wm.agents_window_rect(workspace_area).is_none() {
        return None;
    }
    if let Some(drag) = wm.agents_window().drag.clone() {
        match mouse.kind {
            MouseEventKind::Drag(MouseButton::Left) => {
                drag_motion(wm, drag, mouse, workspace_area);
                return Some(AppAction::Handled);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if let AgentsDrag::Node { moved: false, was_selected: true, .. } = drag {
                    run(wm, AgentsOp::Activate);
                }
                wm.set_agents_drag(None);
                return Some(AppAction::Handled);
            }
            _ => {}
        }
    }
    let hit = wm.agents_hit(workspace_area, mouse.column, mouse.row);
    if hit == AgentsHit::Outside {
        let pinned = wm.agents_window().pinned;
        if pinned {
            // A click elsewhere gives the keyboard back to the panes; the click itself goes on.
            if matches!(mouse.kind, MouseEventKind::Down(_)) {
                run(wm, AgentsOp::Dismiss);
            }
            return None;
        }
        return rect_contains(workspace_area, mouse.column, mouse.row).then_some(AppAction::Handled);
    }
    if let MouseEventKind::Down(button) = mouse.kind {
        press(wm, button, hit, mouse, workspace_area);
    }
    Some(AppAction::Handled)
}

fn run(wm: &mut WindowManager, op: AgentsOp) {
    let _ = wm.apply(Action::Agents(op));
}

fn press(wm: &mut WindowManager, button: MouseButton, hit: AgentsHit, mouse: MouseEvent, ws: Rect) {
    let Some(rects) = wm.agents_pane_rects(ws) else {
        return;
    };
    match (button, hit) {
        (MouseButton::Left, AgentsHit::Title) => match detect_title_bar_badge_click(wm, rects.outer, mouse.column) {
            Some(1) => run(wm, AgentsOp::Maximize),
            Some(_) => run(wm, AgentsOp::Close),
            None => wm.set_agents_drag(Some(AgentsDrag::Move {
                grab_dx: mouse.column.saturating_sub(rects.outer.x),
                grab_dy: 0,
            })),
        },
        (MouseButton::Left, AgentsHit::Border(edge)) => {
            wm.set_agents_drag(Some(AgentsDrag::Resize { edge, initial: rects.outer }));
        }
        (MouseButton::Left, AgentsHit::Splitter) => wm.set_agents_drag(Some(AgentsDrag::Splitter)),
        (MouseButton::Left, AgentsHit::Node(key)) => {
            let sel = &wm.agents_window().selection;
            let was_selected = sel.node.as_ref() == Some(&key) && sel.edge.is_none();
            if !was_selected {
                run(wm, AgentsOp::SelectNode(key.clone()));
            }
            wm.set_agents_drag(Some(AgentsDrag::Node { key, was_selected, moved: false }));
        }
        (MouseButton::Left, AgentsHit::Edge(a, b)) => {
            if wm.agents_window().selection.edge.as_ref() == Some(&(a.clone(), b.clone())) {
                run(wm, AgentsOp::Activate);
            } else {
                run(wm, AgentsOp::SelectEdge(a, b));
            }
        }
        (MouseButton::Right, AgentsHit::Node(key)) => run(wm, AgentsOp::UnpinNode(Some(key))),
        _ => {}
    }
}

fn drag_motion(wm: &mut WindowManager, drag: AgentsDrag, mouse: MouseEvent, ws: Rect) {
    let (col, row) = (mouse.column, mouse.row);
    match drag {
        AgentsDrag::Move { grab_dx, grab_dy } => {
            if let Some(cur) = wm.agents_window_rect(ws) {
                run(
                    wm,
                    AgentsOp::SetRect {
                        x: col.saturating_sub(grab_dx),
                        y: row.saturating_sub(grab_dy),
                        w: cur.width,
                        h: cur.height,
                    },
                );
            }
        }
        AgentsDrag::Resize { edge, initial } => {
            let r = apply_rect_edge_resize(initial, edge, col, row, ws);
            run(wm, AgentsOp::SetRect { x: r.x, y: r.y, w: r.width, h: r.height });
        }
        AgentsDrag::Splitter => {
            if let Some(rects) = wm.agents_pane_rects(ws) {
                let w = u32::from(rects.inner.width.max(1));
                let d = u32::from(col.saturating_sub(rects.inner.x));
                run(wm, AgentsOp::Split(((d * 100 + w / 2) / w).min(100) as u16));
            }
        }
        AgentsDrag::Node { key, was_selected, .. } => {
            if let Some(rects) = wm.agents_pane_rects(ws) {
                let (x_pm, y_pm) = graph_cell_permille(rects.graph, col, row);
                run(wm, AgentsOp::PinNode { key: key.clone(), x_pm, y_pm });
                wm.set_agents_drag(Some(AgentsDrag::Node { key, was_selected, moved: true }));
            }
        }
    }
}
