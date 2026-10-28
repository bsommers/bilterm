//! Phase 36 (`MCP-03`): Interactive Mouse & Keyboard Navigation + Live Telemetry in `:mcp` Console Window.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use bilterm::colon_command::parse_colon_command;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::mcp_console::McpRowAction;
use bilterm::ui::theme::ThemePreset;
use bilterm::ui::{WorkspaceWidget, buffer_to_snapshot_string};
use bilterm::wm::WindowManager;
use bilterm::wm::split_tree::SplitDirection;

#[test]
fn mcp_tree_keyboard_navigation_focuses_panes_and_branches_or_freezes_sessions() {
    let viewport = Rect::new(0, 0, 100, 30);
    let mut wm = WindowManager::new_headless(30, 100);
    let pane2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(viewport).unwrap();
    assert_eq!(wm.active_pane_id(), pane2);

    // Open `:mcp tree`
    let cmd = parse_colon_command(":mcp tree").unwrap().unwrap();
    wm.apply(cmd).unwrap();
    assert!(wm.mcp_console().active);

    // First interactive row is the `main` session node, followed by the active workspace `default` node
    assert_eq!(
        wm.mcp_console().selected_action(),
        Some(McpRowAction::SessionNode("main".to_string()))
    );
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)).unwrap();
    assert_eq!(
        wm.mcp_console().selected_action(),
        Some(McpRowAction::SessionNode("default".to_string()))
    );

    // Press Down to select `Pane #1`, then Enter to focus `Pane #1`
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)).unwrap();
    assert_eq!(
        wm.mcp_console().selected_action(),
        Some(McpRowAction::FocusPane(1))
    );
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane_id(), 1);

    // Press Down to select `Pane #2`, then Enter to focus `Pane #2`
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)).unwrap();
    assert_eq!(
        wm.mcp_console().selected_action(),
        Some(McpRowAction::FocusPane(pane2))
    );
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane_id(), pane2);

    // Press Up three times to return to the `main` session node
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)).unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)).unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)).unwrap();
    assert_eq!(
        wm.mcp_console().selected_action(),
        Some(McpRowAction::SessionNode("main".to_string()))
    );

    // Press 'n' to spawn a new child branch under `main`
    let before_nodes = wm.session_tree().nodes.len();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.session_tree().nodes.len(), before_nodes + 1);

    // Press 'f' to freeze `main` (and its subtree), then 'f' again to thaw
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE)).unwrap();
    assert!(wm.session_tree().node("main").unwrap().frozen);
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE)).unwrap();
    assert!(!wm.session_tree().node("main").unwrap().frozen);

    // Press 'r' to refresh the live tree view and render to verify the selection cursor (`► `)
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)).unwrap();
    let mut buf = Buffer::empty(viewport);
    WorkspaceWidget::new(&wm).render(viewport, &mut buf);
    let snap = buffer_to_snapshot_string(&buf);
    assert!(snap.contains("► "));
    assert!(snap.contains("Enter/Click:Activate"));
}

#[test]
fn theme_list_keyboard_and_mouse_click_live_switches_active_theme() {
    let viewport = Rect::new(0, 0, 100, 30);
    let mut wm = WindowManager::new_headless(30, 100);
    wm.resize_all(viewport).unwrap();
    assert_eq!(wm.config().theme, ThemePreset::TurboBlue);

    // Open `:theme list`
    let cmd = parse_colon_command(":theme list").unwrap().unwrap();
    wm.apply(cmd).unwrap();
    assert!(wm.mcp_console().active);
    assert_eq!(wm.mcp_console().selected_row, 0);

    // Press Down once to select the 2nd theme (`TurboCyan`) and press Enter
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)).unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.config().theme, ThemePreset::TurboCyan);

    // Left-click content row 2 (index 2 -> `Ibm3278Green`) inside the MCP panel rect
    let mcp_rect = wm.mcp_console().panel_rect(viewport).unwrap();
    let click_event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: mcp_rect.x + 4,
        row: mcp_rect.y + 1 + 2,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, click_event, viewport).unwrap();
    assert_eq!(wm.config().theme, ThemePreset::Ibm3278Green);
}
