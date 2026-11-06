//! Surface Parity, Drag-and-Drop & Keyboard Workflow Tests (Phase 54, VIEW-03, VIEW-04, UNI-R01).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use bilterm::action::Action;
use bilterm::api::json::JsonValue;
use bilterm::api::rpc::AutomationControlPlane;
use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::input::session_map::{hit_test_session_map, plan_session_map_mouse, SessionMapHit};
use bilterm::input::session_map_keys::handle_session_map_key;
use bilterm::wm::session_map::SessionMapOp;
use bilterm::wm::WindowManager;

fn key_event(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn mouse_event(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

#[test]
fn test_mouse_drag_and_drop_flow() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.apply(Action::SessionMap(SessionMapOp::Open)).unwrap();

    let ws = Rect::new(0, 0, 80, 24);
    let outer = wm.session_map().window_rect(ws);

    // Initial pane is in "main"
    let pane_id = wm.active_pane_id();
    assert_eq!(
        wm.session_map().window_session_overrides.get(&pane_id),
        None
    );

    // 1. Mouse down on interior to initiate pickup/drag
    // Hit test interior to find a valid window box coordinate
    let interior = Rect::new(outer.x + 1, outer.y + 2, outer.width - 2, outer.height - 3);
    let layout = wm.compute_session_pack_layout(interior);
    assert!(!layout.is_empty());
    assert!(!layout[0].windows.is_empty());

    let win_rect = layout[0].windows[0].rect;
    let click_col = win_rect.x;
    let click_row = win_rect.y;

    let hit = hit_test_session_map(&wm, ws, click_col, click_row);
    assert!(matches!(hit, SessionMapHit::WindowBox { .. }));

    // Press down
    let plan = plan_session_map_mouse(
        &wm,
        mouse_event(MouseEventKind::Down(MouseButton::Left), click_col, click_row),
        ws,
    );
    assert!(plan.is_some());
    let p = plan.unwrap();
    assert_eq!(p.actions, vec![Action::SessionMap(SessionMapOp::Pickup(Some(pane_id)))]);

    // Apply pickup and set target
    wm.apply(Action::SessionMap(SessionMapOp::Pickup(Some(pane_id)))).unwrap();
    assert_eq!(wm.session_map().holding_pane_id, Some(pane_id));

    wm.apply(Action::SessionMap(SessionMapOp::TargetSession("workers".to_string()))).unwrap();
    assert_eq!(wm.session_map().target_session.as_deref(), Some("workers"));

    // Drop
    wm.apply(Action::SessionMap(SessionMapOp::Drop)).unwrap();
    assert_eq!(wm.session_map().holding_pane_id, None);
    assert_eq!(
        wm.session_map().window_session_overrides.get(&pane_id),
        Some(&"workers".to_string())
    );
}

#[test]
fn test_mouse_drag_cancel() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.apply(Action::SessionMap(SessionMapOp::Open)).unwrap();

    let pane_id = wm.active_pane_id();
    wm.apply(Action::SessionMap(SessionMapOp::Pickup(Some(pane_id)))).unwrap();
    assert_eq!(wm.session_map().holding_pane_id, Some(pane_id));

    // Cancel gesture
    wm.apply(Action::SessionMap(SessionMapOp::CancelDrag)).unwrap();
    assert_eq!(wm.session_map().holding_pane_id, None);
    assert_eq!(
        wm.session_map().window_session_overrides.get(&pane_id),
        None
    );
}

#[test]
fn test_keyboard_pickup_target_drop_workflow() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.apply(Action::SessionMap(SessionMapOp::Open)).unwrap();
    assert!(wm.session_map().captures_input());

    let pane_id = wm.active_pane_id();

    // 1. Space: pickup selected window
    handle_session_map_key(&mut wm, key_event(KeyCode::Char(' '), KeyModifiers::NONE));
    assert_eq!(wm.session_map().holding_pane_id, Some(pane_id));

    // 2. Select target session via colon or action
    wm.apply(Action::SessionMap(SessionMapOp::TargetSession("background".to_string()))).unwrap();

    // 3. Enter: drop window into target session
    handle_session_map_key(&mut wm, key_event(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(wm.session_map().holding_pane_id, None);
    assert_eq!(
        wm.session_map().window_session_overrides.get(&pane_id),
        Some(&"background".to_string())
    );

    // 4. Pickup again and Esc: cancel
    handle_session_map_key(&mut wm, key_event(KeyCode::Char('m'), KeyModifiers::NONE));
    assert_eq!(wm.session_map().holding_pane_id, Some(pane_id));
    handle_session_map_key(&mut wm, key_event(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(wm.session_map().holding_pane_id, None);
}

#[test]
fn test_surface_parity_colon_and_rpc() {
    let pane_id = 1;

    // A. Colon command surface
    let mut wm1 = WindowManager::new_headless(24, 80);
    execute_wm_colon_command(&mut wm1, parse_colon_command(":sessions map open").unwrap().unwrap());
    execute_wm_colon_command(&mut wm1, parse_colon_command(":sessions map pickup 1").unwrap().unwrap());
    execute_wm_colon_command(&mut wm1, parse_colon_command(":sessions map target cluster").unwrap().unwrap());
    execute_wm_colon_command(&mut wm1, parse_colon_command(":sessions map drop").unwrap().unwrap());

    // B. RPC surface
    let mut ctrl = AutomationControlPlane::new_headless(24, 80);
    ctrl.dispatch_method(
        "session.map",
        &JsonValue::obj([
            ("op", JsonValue::String("open".into())),
        ]),
    ).unwrap();
    ctrl.dispatch_method(
        "session.map",
        &JsonValue::obj([
            ("op", JsonValue::String("pickup".into())),
            ("pane_id", JsonValue::Number(1)),
        ]),
    ).unwrap();
    ctrl.dispatch_method(
        "session.map",
        &JsonValue::obj([
            ("op", JsonValue::String("target".into())),
            ("session", JsonValue::String("cluster".into())),
        ]),
    ).unwrap();
    ctrl.dispatch_method(
        "session.map",
        &JsonValue::obj([
            ("op", JsonValue::String("drop".into())),
        ]),
    ).unwrap();

    // Verify parity
    assert_eq!(
        wm1.session_map().window_session_overrides.get(&pane_id),
        Some(&"cluster".to_string())
    );
    assert_eq!(
        ctrl.wm.session_map().window_session_overrides.get(&pane_id),
        Some(&"cluster".to_string())
    );
    assert_eq!(
        wm1.session_map().window_session_overrides,
        ctrl.wm.session_map().window_session_overrides
    );
}
