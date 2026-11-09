//! Platform-neutral routing helpers shared by the native GUI runners (X11, Win32, macOS).
//!
//! The runners translate OS events into crossterm events; the pure functions here decide what a
//! wheel notch means (Navigator/`:mcp`/agents panel scale versus whole-terminal font zoom) and
//! convert X11 modifier state, so the behaviour is unit-testable on every OS.

use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::ui::typography::PanelTarget;
use crate::wm::WindowManager;

/// X11 `ShiftMask`.
pub const X11_SHIFT_MASK: u32 = 1 << 0;
/// X11 `ControlMask`.
pub const X11_CONTROL_MASK: u32 = 1 << 2;
/// X11 `Mod1Mask` (Alt).
pub const X11_MOD1_MASK: u32 = 1 << 3;

/// What a GUI wheel notch should do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiWheelRoute {
    /// Deliver this mouse event to the normal mouse handler (panel scale, scrollback, ...).
    Forward(MouseEvent),
    /// Ctrl+wheel outside every panel: zoom the whole terminal font.
    FontZoom { up: bool },
}

/// Maps an X11 event `state` bitmask to crossterm modifiers.
pub fn modifiers_from_x11_state(state: u32) -> KeyModifiers {
    let mut mods = KeyModifiers::empty();
    if state & X11_SHIFT_MASK != 0 {
        mods |= KeyModifiers::SHIFT;
    }
    if state & X11_CONTROL_MASK != 0 {
        mods |= KeyModifiers::CONTROL;
    }
    if state & X11_MOD1_MASK != 0 {
        mods |= KeyModifiers::ALT;
    }
    mods
}

fn contains(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x.saturating_add(r.width) && row >= r.y && row < r.y.saturating_add(r.height)
}

/// True when cell (`col`,`row`) lies inside the drawer, the `:mcp` console or the agents window,
/// each hit-tested in its own (possibly scaled) panel space.
fn over_scalable_panel(wm: &WindowManager, mouse: MouseEvent, workspace: Rect) -> bool {
    let inside = |target: PanelTarget, rect_of: &dyn Fn(Rect) -> Option<Rect>| {
        let (pm, pws) = wm.panel_space(mouse, workspace, target);
        rect_of(pws).is_some_and(|r| contains(r, pm.column, pm.row))
    };
    inside(PanelTarget::AgentsList, &|a| wm.agents_list_rect(a))
        || inside(PanelTarget::SessionMap, &|a| wm.session_map_rect(a))
        || inside(PanelTarget::Clipboard, &|a| wm.clipboard_window_rect(a))
        || inside(PanelTarget::Traffic, &|a| wm.traffic_monitor_rect(a))
        || inside(PanelTarget::Agents, &|a| wm.agents_window_rect(a))
        || inside(PanelTarget::Mcp, &|a| wm.mcp_console().panel_rect(a))
        || inside(PanelTarget::Drawer, &|a| wm.drawer_rect(a))
}

/// Routes one wheel notch at cell (`col`,`row`) of the full-screen `viewport`.
///
/// With Ctrl held over a scalable panel the event is forwarded (with CONTROL set) so the panel
/// handler scales that panel; with Ctrl elsewhere it becomes a font zoom; without Ctrl it is
/// always forwarded with the real modifiers.
pub fn route_gui_wheel(
    wm: &WindowManager,
    col: u16,
    row: u16,
    up: bool,
    mods: KeyModifiers,
    viewport: Rect,
) -> GuiWheelRoute {
    let kind = if up { MouseEventKind::ScrollUp } else { MouseEventKind::ScrollDown };
    let mouse = MouseEvent { kind, column: col, row, modifiers: mods };
    if !mods.contains(KeyModifiers::CONTROL) {
        return GuiWheelRoute::Forward(mouse);
    }
    let shell = crate::ui::shell::ShellLayout::compute(viewport, wm.config(), wm.pane_count());
    if over_scalable_panel(wm, mouse, shell.workspace) {
        GuiWheelRoute::Forward(mouse)
    } else {
        GuiWheelRoute::FontZoom { up }
    }
}
