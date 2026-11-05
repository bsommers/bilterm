//! Pinning and scaling for the MCP console and the slide-out drawer.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use bilterm::colon_command::parse_colon_command;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::typography::PanelTarget::{Drawer, Mcp};
use bilterm::wm::{DrawerVisibilityMode, WindowManager};

fn run(wm: &mut WindowManager, cmd: &str) {
    let c = parse_colon_command(cmd).unwrap().unwrap();
    wm.apply(c).unwrap();
}

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

#[test]
fn mcp_pin_keeps_the_window_open_and_releases_the_keyboard_on_esc() {
    let mut wm = WindowManager::new_headless(30, 100);
    run(&mut wm, ":mcp pin");
    assert!(wm.mcp_console().active && wm.mcp_console().pinned && wm.mcp_console().captures_input());

    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
    assert!(wm.mcp_console().active, "Esc on a pinned window must not close it");
    assert!(!wm.mcp_console().captures_input());

    run(&mut wm, ":mcp unpin");
    assert!(!wm.mcp_console().pinned && wm.mcp_console().active);
    assert!(wm.mcp_console().captures_input(), "an unpinned window is modal again");
}

#[test]
fn star_toggles_the_mcp_pin_and_close_clears_it() {
    let mut wm = WindowManager::new_headless(30, 100);
    run(&mut wm, ":mcp");
    assert!(!wm.mcp_console().pinned);
    handle_wm_key_event(&mut wm, key('*')).unwrap();
    assert!(wm.mcp_console().pinned);
    handle_wm_key_event(&mut wm, key('*')).unwrap();
    assert!(!wm.mcp_console().pinned);
    handle_wm_key_event(&mut wm, key('*')).unwrap();
    run(&mut wm, ":mcp close");
    assert!(!wm.mcp_console().active && !wm.mcp_console().pinned);
}

#[test]
fn drawer_pin_and_unpin_map_to_fixed_and_on_demand() {
    let mut wm = WindowManager::new_headless(30, 100);
    run(&mut wm, ":drawer pin");
    assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::Fixed);
    assert!(wm.is_drawer_effectively_visible());
    run(&mut wm, ":drawer unpin");
    assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::OnDemand);
}

#[test]
fn star_in_the_focused_drawer_toggles_its_pin() {
    let mut wm = WindowManager::new_headless(30, 100);
    run(&mut wm, ":drawer focus");
    assert!(wm.drawer().focused);
    handle_wm_key_event(&mut wm, key('*')).unwrap();
    assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::Fixed);
    handle_wm_key_event(&mut wm, key('*')).unwrap();
    assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::OnDemand);
}

#[test]
fn bracket_keys_and_commands_scale_each_panel_independently() {
    let mut wm = WindowManager::new_headless(30, 100);
    run(&mut wm, ":mcp");
    handle_wm_key_event(&mut wm, key(']')).unwrap();
    assert_eq!((wm.scale_setting(Mcp), wm.scale_setting(Drawer)), (110, 100));
    handle_wm_key_event(&mut wm, key('[')).unwrap();
    handle_wm_key_event(&mut wm, key('[')).unwrap();
    assert_eq!(wm.scale_setting(Mcp), 90);
    run(&mut wm, ":mcp scale 150");
    run(&mut wm, ":drawer scale 80");
    assert_eq!((wm.scale_setting(Mcp), wm.scale_setting(Drawer)), (150, 80));
    run(&mut wm, ":panel scale 120");
    assert_eq!((wm.scale_setting(Mcp), wm.scale_setting(Drawer)), (120, 120));
    for _ in 0..30 {
        wm.step_scale(Mcp, 10);
        wm.step_scale(Drawer, -10);
    }
    assert_eq!((wm.scale_setting(Mcp), wm.scale_setting(Drawer)), (200, 75));
}

#[test]
fn drawer_keys_scale_only_the_drawer() {
    let mut wm = WindowManager::new_headless(30, 100);
    run(&mut wm, ":drawer focus");
    handle_wm_key_event(&mut wm, key(']')).unwrap();
    assert_eq!((wm.scale_setting(Drawer), wm.scale_setting(Mcp)), (110, 100));
}

#[test]
fn ctrl_wheel_over_the_mcp_window_scales_it_and_plain_wheel_does_not() {
    let mut wm = WindowManager::new_headless(30, 100);
    run(&mut wm, ":mcp");
    let ws = ratatui::layout::Rect::new(0, 0, 100, 30);
    let r = wm.mcp_console().panel_rect(ws).expect("mcp rect");
    let at = |mods| MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: r.x + 2,
        row: r.y + 2,
        modifiers: mods,
    };
    let _ = handle_wm_mouse_event(&mut wm, at(KeyModifiers::NONE), ws);
    assert_eq!(wm.scale_setting(Mcp), 100);
    let _ = handle_wm_mouse_event(&mut wm, at(KeyModifiers::CONTROL), ws);
    assert_eq!((wm.scale_setting(Mcp), wm.scale_setting(Drawer)), (110, 100));
    let _ = MouseButton::Left;
}

#[test]
fn clicking_outside_a_pinned_mcp_window_drops_its_keyboard_focus() {
    let mut wm = WindowManager::new_headless(30, 100);
    run(&mut wm, ":mcp pin");
    assert!(wm.mcp_console().captures_input());
    let ws = ratatui::layout::Rect::new(0, 0, 100, 30);
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 29,
        modifiers: KeyModifiers::NONE,
    };
    let _ = handle_wm_mouse_event(&mut wm, click, ws);
    assert!(wm.mcp_console().active && !wm.mcp_console().captures_input());
}

#[test]
fn modern_mcp_title_exposes_a_pin_badge_that_follows_state() {
    use bilterm::mcp_console::McpConsoleState;
    use bilterm::ui::mcp_ui::mcp_modern_title;
    let cfg = bilterm::config::BiltermConfig::default();
    let mut state = McpConsoleState::default();
    let (off, s0, e0) = mcp_modern_title(&cfg, &state);
    assert!(off.contains("Pin (*)") && !off.contains("Pinned") && e0 > s0);
    state.set_pinned(true);
    let (on, s1, _) = mcp_modern_title(&cfg, &state);
    assert!(on.contains("Pinned (*)") && s1 == s0, "badge keeps its start column");
}
