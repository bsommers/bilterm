//! Key handling for the AGENT TRAFFIC window (`FLT-01`, Phase 50-08).
//!
//! This module only resolves which key scopes are active, looks the key up in the declarative
//! [`AGENTS_WINDOW_BINDINGS`](super::keymap::AGENTS_WINDOW_BINDINGS) table and runs the resulting
//! [`Action`](crate::action::Action) through `WindowManager::apply`. It never changes window, pane or
//! selection state itself (`ARCH-R01`); the mouse handler below only keeps pointer-gesture
//! bookkeeping ([`AgentsDrag`]) and lowers every drag step to an `Action::Agents(..)`.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::AppAction;
use super::keymap::{AgentsKeyScope, agents_window_action, is_leader_key_with_tmux};
use super::mouse_wm::{apply_rect_edge_resize, ctrl_wheel_scale_delta, detect_title_bar_badge_click};
use crate::action::Action;
use crate::wm::agents_cmd::{AgentsOp, KeywordEdit, SnoopFilterOp};
use crate::ui::agents_ui::{SnoopButton, agents_pin_badge_x, snoop_layout};
use crate::ui::typography::PanelTarget;
use crate::wm::agents_geometry::{AgentsHit, AgentsPaneRects, graph_cell_permille};
use crate::wm::agents_window::{AgentsDrag, WindowPane};
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
    let state = wm.agents_window();
    let entry = state.snoop.keyword_entry.is_some();
    let focus = state.pane_focus;
    // Most specific scope first. Keyword entry and compose see their own rows first; printable
    // characters then edit the keyword (or are swallowed in compose) before any `Window` row.
    let own: &[AgentsKeyScope] = match (entry, focus) {
        (true, _) => &[AgentsKeyScope::KeywordEntry],
        (false, WindowPane::Compose) => &[AgentsKeyScope::Compose],
        (false, WindowPane::Snoop) => &[AgentsKeyScope::Snoop],
        (false, WindowPane::Graph) if state.grab_mode => &[AgentsKeyScope::Grab, AgentsKeyScope::Graph],
        (false, WindowPane::Graph) => &[AgentsKeyScope::Graph],
    };
    let mut action = agents_window_action(own, key);
    if action.is_none() {
        let printable = match key.code {
            KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => Some(c),
            _ => None,
        };
        match (printable, entry, focus) {
            (Some(c), true, _) => action = Some(Action::Agents(AgentsOp::KeywordEdit(KeywordEdit::Push(c)))),
            (Some(_), false, WindowPane::Compose) => return Some(AppAction::Handled),
            _ => action = agents_window_action(&[AgentsKeyScope::Window], key),
        }
    }
    if let Some(action) = action {
        let _ = wm.apply(action);
    }
    Some(AppAction::Handled)
}

/// Routes a bracketed-paste payload (CR-01).
///
/// While the AGENT TRAFFIC window owns the keyboard the paste must never reach the active pane's
/// PTY or hidden prompt. Only keyword entry accepts the text (control characters dropped); every
/// other window pane (compose, graph, conversation) swallows it. Returns `None` when the window
/// does not capture input so the caller pastes into the pane as usual.
pub(super) fn handle_agents_window_paste(wm: &mut WindowManager, text: &str) -> Option<AppAction> {
    if !wm.agents_window_captures_input() {
        return None;
    }
    if wm.agents_window().snoop.keyword_entry.is_some() {
        for c in text.chars().filter(|c| !c.is_control()) {
            let _ = wm.apply(Action::Agents(AgentsOp::KeywordEdit(KeywordEdit::Push(c))));
        }
    }
    Some(AppAction::Handled)
}

/// Handles a mouse event over the AGENT TRAFFIC window (`FLT-01`, Phase 50-09).
///
/// Every visible effect lowers to an `Action::Agents(..)` run through `WindowManager::apply`; the
/// only state kept here is which pointer drag is in progress (`set_agents_drag`, bookkeeping). The
/// window swallows events inside it so the drawer and panes beneath never see them; events outside
/// it (the window is not modal) are left for the panes. Returns `None` when the window is closed or
/// the event is not for it.
pub(super) fn handle_agents_window_mouse(
    wm: &mut WindowManager,
    mouse: MouseEvent,
    workspace_area: Rect,
) -> Option<AppAction> {
    // The window may be drawn at its own GUI scale: work in its virtual (scaled) cell space.
    let (mouse, workspace_area) = wm.panel_space(mouse, workspace_area, PanelTarget::Agents);
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
        // The window is not modal: a press elsewhere gives the keyboard back to the panes and the
        // press itself goes on to whatever lies under the pointer; every other event passes through.
        if matches!(mouse.kind, MouseEventKind::Down(_)) && wm.agents_window().focused {
            run(wm, AgentsOp::Blur);
        }
        return None;
    }
    if let Some(delta) = ctrl_wheel_scale_delta(&mouse) {
        run(wm, AgentsOp::ScaleStep(delta));
        return Some(AppAction::Handled);
    }
    if hit == AgentsHit::Snoop {
        match mouse.kind {
            MouseEventKind::ScrollUp => run(wm, AgentsOp::SnoopScroll(-3)),
            MouseEventKind::ScrollDown => run(wm, AgentsOp::SnoopScroll(3)),
            _ => {}
        }
    }
    if let MouseEventKind::Down(button) = mouse.kind {
        // A click inside a window whose keyboard was released takes the keyboard back.
        if !wm.agents_window().focused {
            run(wm, AgentsOp::Zoom(None));
        }
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
    if button == MouseButton::Left
        && matches!(hit, AgentsHit::Node(_) | AgentsHit::Edge(..) | AgentsHit::Graph)
        && wm.agents_window().pane_focus != WindowPane::Graph
    {
        run(wm, AgentsOp::FocusPane(Some(WindowPane::Graph)));
    }
    match (button, hit) {
        (MouseButton::Left, AgentsHit::Title) if agents_pin_badge_x(rects.outer) == Some(mouse.column) => {
            run(wm, AgentsOp::Pin(None))
        }
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
        (MouseButton::Left, AgentsHit::Snoop) => snoop_press(wm, &rects, mouse),
        (MouseButton::Right, AgentsHit::Node(key)) => run(wm, AgentsOp::UnpinNode(Some(key))),
        _ => {}
    }
}

/// Left press inside the conversation pane: focus it, then apply what sits under the pointer
/// according to [`snoop_layout`] (the layout the renderer draws).
fn snoop_press(wm: &mut WindowManager, rects: &AgentsPaneRects, mouse: MouseEvent) {
    let lay = snoop_layout(wm, rects);
    let at = |r: Rect| rect_contains(r, mouse.column, mouse.row);
    if at(lay.compose) {
        if wm.agents_window().pane_focus != WindowPane::Compose {
            run(wm, AgentsOp::FocusPane(Some(WindowPane::Compose)));
        }
        if at(lay.send) {
            run(wm, AgentsOp::ComposeSubmit);
        }
        return;
    }
    if wm.agents_window().pane_focus != WindowPane::Snoop {
        run(wm, AgentsOp::FocusPane(Some(WindowPane::Snoop)));
    }
    if let Some((_, button)) = lay.header_buttons.iter().find(|(r, _)| at(*r)) {
        run(
            wm,
            match button {
                SnoopButton::Agent => AgentsOp::Filter(SnoopFilterOp::AgentFromSelection),
                SnoopButton::Pair => AgentsOp::Filter(SnoopFilterOp::PairFromSelection),
                SnoopButton::Keyword => AgentsOp::KeywordEdit(KeywordEdit::Begin),
                SnoopButton::Clear => AgentsOp::Filter(SnoopFilterOp::Clear),
            },
        );
    } else if let Some((r, kind)) = lay.chips.iter().find(|(r, _)| at(*r)) {
        // Only the chip's trailing `×` removes the filter.
        if mouse.column + 1 == r.x + r.width {
            run(wm, AgentsOp::Filter(SnoopFilterOp::Remove(*kind)));
        }
    } else if at(lay.follow) {
        run(wm, AgentsOp::Follow(None));
    } else if at(lay.copy) {
        run(wm, AgentsOp::Copy);
    } else if at(lay.lines_area)
        && let Some((_, seq)) = lay.lines.iter().find(|(y, _)| *y == mouse.row)
    {
        run(wm, AgentsOp::SnoopSelect(*seq));
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
