//! Phase 50-13: shared GUI wheel routing and X11 modifier mapping (FLT-01 SC4, D-07).
//! `gui_runtime_` tests exercise the pure routing used by the real X11 runner on a GUI-surface WM.

use bilterm::input::gui_route::{GuiWheelRoute, modifiers_from_x11_state, route_gui_wheel};
use bilterm::input::handle_wm_mouse_event;
use bilterm::ui::shell::ShellLayout;
use bilterm::ui::typography::PanelTarget;
use bilterm::wm::{SplitDirection, WindowManager};
use crossterm::event::{KeyModifiers, MouseEventKind};
use ratatui::layout::Rect;

const AREA: Rect = Rect { x: 0, y: 0, width: 120, height: 36 };

fn gui_wm() -> WindowManager {
    let mut wm = WindowManager::new_headless(AREA.height, AREA.width);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(AREA).unwrap();
    wm.set_gui_surface(true);
    wm
}

fn workspace(wm: &WindowManager) -> Rect {
    ShellLayout::compute(AREA, wm.config(), wm.pane_count()).workspace
}

/// A cell inside `r` that is not on its border.
fn centre(r: Rect) -> (u16, u16) {
    (r.x + r.width / 2, r.y + r.height / 2)
}

#[test]
fn gui_runtime_ctrl_wheel_over_drawer_forwards_and_scales() {
    let mut wm = gui_wm();
    wm.toggle_drawer();
    let rect = wm.drawer_rect(workspace(&wm)).expect("drawer visible");
    let (c, r) = centre(rect);
    let route = route_gui_wheel(&wm, c, r, true, KeyModifiers::CONTROL, AREA);
    let GuiWheelRoute::Forward(ev) = route else { panic!("expected Forward, got {route:?}") };
    assert!(ev.modifiers.contains(KeyModifiers::CONTROL));
    assert_eq!(ev.kind, MouseEventKind::ScrollUp);
    let before = wm.scale_pct(PanelTarget::Drawer);
    handle_wm_mouse_event(&mut wm, ev, AREA).unwrap();
    assert_eq!(wm.scale_pct(PanelTarget::Drawer), before + 10);
}

#[test]
fn gui_runtime_ctrl_wheel_over_agents_window_scales_agents() {
    let mut wm = gui_wm();
    wm.open_agents_window(None);
    let rect = wm.agents_window_rect(workspace(&wm)).expect("agents window");
    let (c, r) = centre(rect);
    let route = route_gui_wheel(&wm, c, r, false, KeyModifiers::CONTROL, AREA);
    let GuiWheelRoute::Forward(ev) = route else { panic!("expected Forward, got {route:?}") };
    let before = wm.scale_pct(PanelTarget::Agents);
    handle_wm_mouse_event(&mut wm, ev, AREA).unwrap();
    assert_eq!(wm.scale_pct(PanelTarget::Agents), before - 10);
    assert_eq!(wm.scale_pct(PanelTarget::Drawer), 100);
}

#[test]
fn gui_runtime_ctrl_wheel_over_pane_is_font_zoom() {
    let wm = gui_wm();
    // No panel is open: Ctrl+wheel anywhere is the whole-terminal font zoom.
    let (c, r) = centre(workspace(&wm));
    assert_eq!(
        route_gui_wheel(&wm, c, r, true, KeyModifiers::CONTROL, AREA),
        GuiWheelRoute::FontZoom { up: true }
    );
    assert_eq!(
        route_gui_wheel(&wm, c, r, false, KeyModifiers::CONTROL | KeyModifiers::SHIFT, AREA),
        GuiWheelRoute::FontZoom { up: false }
    );
    // With the drawer open, a cell outside it is still a font zoom.
    let mut wm = gui_wm();
    wm.toggle_drawer();
    let rect = wm.drawer_rect(workspace(&wm)).unwrap();
    let ws = workspace(&wm);
    let outside_col = if rect.x > ws.x { ws.x } else { rect.x + rect.width };
    assert!(outside_col < ws.x + ws.width);
    assert_eq!(
        route_gui_wheel(&wm, outside_col, ws.y + 1, true, KeyModifiers::CONTROL, AREA),
        GuiWheelRoute::FontZoom { up: true }
    );
}

#[test]
fn gui_runtime_plain_wheel_forwards_with_modifiers() {
    let mut wm = gui_wm();
    wm.toggle_drawer();
    let (c, r) = centre(wm.drawer_rect(workspace(&wm)).unwrap());
    for mods in [KeyModifiers::empty(), KeyModifiers::SHIFT, KeyModifiers::ALT, KeyModifiers::SHIFT | KeyModifiers::ALT] {
        let route = route_gui_wheel(&wm, c, r, false, mods, AREA);
        let GuiWheelRoute::Forward(ev) = route else { panic!("expected Forward, got {route:?}") };
        assert_eq!(ev.modifiers, mods);
        assert_eq!(ev.kind, MouseEventKind::ScrollDown);
        assert_eq!((ev.column, ev.row), (c, r));
    }
}

#[test]
fn gui_runtime_x11_state_to_modifiers() {
    assert_eq!(modifiers_from_x11_state(0), KeyModifiers::empty());
    assert_eq!(modifiers_from_x11_state(1), KeyModifiers::SHIFT);
    assert_eq!(modifiers_from_x11_state(4), KeyModifiers::CONTROL);
    assert_eq!(modifiers_from_x11_state(8), KeyModifiers::ALT);
    assert_eq!(modifiers_from_x11_state(1 | 4), KeyModifiers::SHIFT | KeyModifiers::CONTROL);
    assert_eq!(modifiers_from_x11_state(4 | 8), KeyModifiers::CONTROL | KeyModifiers::ALT);
    assert_eq!(
        modifiers_from_x11_state(1 | 4 | 8),
        KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT
    );
    // Unrelated bits (Lock, Mod2 NumLock, button masks) are ignored.
    assert_eq!(modifiers_from_x11_state(2 | 16 | 0x100), KeyModifiers::empty());
}

#[test]
fn gui_runtime_scaled_panel_is_hit_in_panel_space() {
    let mut wm = gui_wm();
    wm.toggle_drawer();
    wm.set_scale(PanelTarget::Drawer, 150);
    let ws = workspace(&wm);
    let (pws_rect, _) = {
        let (_, pws) = wm.panel_space(
            crossterm::event::MouseEvent { kind: MouseEventKind::Moved, column: 0, row: 0, modifiers: KeyModifiers::NONE },
            ws,
            PanelTarget::Drawer,
        );
        (wm.drawer_rect(pws).unwrap(), pws)
    };
    // The real cell of the panel's centre (scaled up by 1.5 from its virtual position).
    let real_c = ws.x + ((pws_rect.x + pws_rect.width / 2 - ws.x) as u32 * 150 / 100) as u16;
    let real_r = ws.y + ((pws_rect.y + pws_rect.height / 2 - ws.y) as u32 * 150 / 100) as u16;
    assert!(matches!(
        route_gui_wheel(&wm, real_c, real_r, true, KeyModifiers::CONTROL, AREA),
        GuiWheelRoute::Forward(_)
    ));
}

#[test]
fn gui_runtime_route_is_platform_neutral() {
    // Pure function of (wm, cell, modifiers, viewport): identical inputs give identical routes,
    // with no dependence on cfg(windows)/cfg(macos) or any global state.
    let mut wm = gui_wm();
    wm.toggle_drawer();
    let (c, r) = centre(wm.drawer_rect(workspace(&wm)).unwrap());
    for mods in [KeyModifiers::empty(), KeyModifiers::CONTROL, KeyModifiers::CONTROL | KeyModifiers::ALT] {
        for up in [true, false] {
            let a = route_gui_wheel(&wm, c, r, up, mods, AREA);
            let b = route_gui_wheel(&wm, c, r, up, mods, AREA);
            assert_eq!(a, b);
        }
    }
    assert_eq!(wm.scale_pct(PanelTarget::Drawer), 100, "routing must not mutate the manager");
}
