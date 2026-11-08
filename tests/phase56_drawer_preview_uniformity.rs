use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use bilterm::action::{Action, DrawerPreviewOp};
use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::input::{
    DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS, handle_wm_key_event, handle_wm_mouse_event,
};
use bilterm::ui::modals::help_catalog::PALETTE_CATALOG;
use bilterm::wm::{SplitDirection, WindowManager};

fn make_wm() -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().history_session = format!("p56_uni_{}", std::process::id());
    wm.config_mut().drawer.style = bilterm::wm::drawer::DrawerStyle::Navigator;
    wm
}

#[test]
fn keyboard_drawer_v_scroll_enter_esc_and_global_chords_work_uniformly() {
    let mut wm = make_wm();
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.focus_and_raise(1);
    let area = Rect::new(0, 0, 120, 36);
    wm.resize_all(area).unwrap();

    // 1. Global Alt+P (uppercase P) opens drawer + preview
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('P'), KeyModifiers::ALT)).unwrap();
    assert!(wm.drawer().preview.active);
    assert!(wm.drawer().focused);

    // 2. Esc closes preview first while keeping drawer focused
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
    assert!(!wm.drawer().preview.active);
    assert!(wm.drawer().focused);

    // 3. 'v' inside focused Navigator drawer toggles preview back on
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE)).unwrap();
    assert!(wm.drawer().preview.active);

    // 4. Scrolling with Down / 'j' updates selection and keeps preview synced
    for _ in 0..6 {
        if wm.selected_drawer_pane_id() == Some(p2) {
            break;
        }
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)).unwrap();
    }
    assert_eq!(wm.selected_drawer_pane_id(), Some(p2));

    // 5. Enter commits focus to previewed pane #2 and closes preview
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert!(!wm.drawer().preview.active);
    assert_eq!(wm.active_pane_id(), p2);

    // 6. Verify leader chord table and global chord table contain DrawerPreview(Toggle)
    assert!(
        DEFAULT_LEADER_BINDINGS
            .iter()
            .any(|(c, a)| *c == KeyCode::Char('P')
                && *a == Action::DrawerPreview(DrawerPreviewOp::Toggle))
    );
    assert!(
        DEFAULT_GLOBAL_BINDINGS
            .iter()
            .any(|(_, _, a)| *a == Action::DrawerPreview(DrawerPreviewOp::Toggle))
    );
    assert!(
        PALETTE_CATALOG
            .iter()
            .any(|e| e.0.starts_with(":drawer preview"))
    );
}

#[test]
fn mouse_click_inside_preview_commits_focus_and_close_badge_dismisses() {
    let mut wm = make_wm();
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.focus_and_raise(1);
    let area = Rect::new(0, 0, 120, 36);
    wm.resize_all(area).unwrap();

    // Preview pane #2 while pane #1 remains active
    wm.apply(Action::DrawerPreview(DrawerPreviewOp::Pane(p2)))
        .unwrap();
    assert_eq!(wm.active_pane_id(), 1);
    let pr = wm.drawer_preview_rect(area).expect("preview rect");

    // Click `[×]` badge at top-right of preview -> closes preview without changing active pane
    let close_click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: pr.x + pr.width - 2,
        row: pr.y,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, close_click, area).unwrap();
    assert!(!wm.drawer().preview.active);
    assert_eq!(wm.active_pane_id(), 1);

    // Re-open preview on pane #2 and click inside preview body -> commits focus to pane #2
    wm.apply(Action::DrawerPreview(DrawerPreviewOp::Pane(p2)))
        .unwrap();
    let pr = wm.drawer_preview_rect(area).expect("preview rect");
    let body_click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: pr.x + pr.width / 2,
        row: pr.y + pr.height / 2,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, body_click, area).unwrap();
    assert!(!wm.drawer().preview.active);
    assert_eq!(wm.active_pane_id(), p2);
}

#[test]
fn rpc_drawer_preview_and_mcp_bilterm_drawer_preview_parity() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let p2 = plane.wm.split_focused(SplitDirection::Vertical).unwrap();
    plane.wm.focus_and_raise(1);

    // 1. JSON-RPC `drawer.preview`
    let params = JsonValue::parse(&format!(r#"{{"op":"pane","pane_id":{p2}}}"#)).unwrap();
    let res = plane.dispatch_method("drawer.preview", &params).unwrap();
    assert_eq!(res.get("active").and_then(JsonValue::as_bool), Some(true));
    let target = res.get("target").expect("target object");
    assert_eq!(
        target.get("kind").and_then(JsonValue::as_str),
        Some("active_pane")
    );
    assert_eq!(
        target.get("pane_id").and_then(JsonValue::as_i64),
        Some(p2 as i64)
    );
    assert!(plane.wm.drawer().preview.active);

    // 2. MCP `tools/call` `bilterm_drawer_preview` commit
    let mcp_line = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"bilterm_drawer_preview","arguments":{"op":"commit"}}}"#;
    let mcp_resp = plane.handle_mcp_line(mcp_line).expect("mcp response");
    let mcp_val = JsonValue::parse(&mcp_resp).expect("valid mcp json");
    assert_eq!(
        mcp_val
            .get("result")
            .and_then(|r| r.get("isError"))
            .and_then(JsonValue::as_bool),
        Some(false)
    );
    assert!(!plane.wm.drawer().preview.active);
    assert_eq!(plane.wm.active_pane_id(), p2);
}
