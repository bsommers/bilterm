//! `J`/`K` on a session node at the edge of the MCP list must say so (7d41e69).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use bilterm::colon_command::parse_colon_command;
use bilterm::input::handle_wm_key_event;
use bilterm::mcp_console::McpRowAction;
use bilterm::wm::WindowManager;

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT)
}

fn tree_wm() -> WindowManager {
    let mut wm = WindowManager::new_headless(30, 100);
    let cmd = parse_colon_command(":mcp tree").unwrap().unwrap();
    wm.apply(cmd).unwrap();
    assert_eq!(
        wm.mcp_console().selected_action(),
        Some(McpRowAction::SessionNode("main".to_string()))
    );
    wm
}

#[test]
fn k_on_the_first_root_session_reports_already_at_the_top() {
    let mut wm = tree_wm();
    handle_wm_key_event(&mut wm, key('K')).unwrap();
    let msg = wm.active_pane().status_message().unwrap_or_default();
    assert!(msg.contains("already at the top"), "{msg:?}");
    assert!(msg.contains("main"), "{msg:?}");
}

#[test]
fn j_repeated_until_the_last_root_session_reports_already_at_the_bottom() {
    let mut wm = tree_wm();
    let mut last = String::new();
    for _ in 0..16 {
        handle_wm_key_event(&mut wm, key('J')).unwrap();
        last = wm.active_pane().status_message().unwrap_or_default().to_string();
        if last.contains("already at the bottom") {
            return;
        }
        assert!(last.contains("Moved session"), "{last:?}");
    }
    panic!("never reached the bottom edge; last status {last:?}");
}
