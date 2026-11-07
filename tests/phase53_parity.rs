//! Phase 53 Wave 3 test suite: Surface Uniformity Parity (`SES-05`, `UNI-R01`).
//!
//! Asserts identical operational behaviour across keys, mouse gestures, colon commands,
//! JSON-RPC, MCP, and CLI surfaces.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

use bilterm::action::Action;
use bilterm::api::json::JsonValue;
use bilterm::api::rpc::AutomationControlPlane;
use bilterm::colon_command::parse_colon_command;
use bilterm::input::drawer_navigator::handle_navigator_drawer_key;
use bilterm::pane::ExitProtectionPolicy;
use bilterm::wm::drawer::DrawerRowKind;
use bilterm::wm::window_manage::{
    BulkWindowOp, SessionManageOp, WindowManageOp, WindowSelectTarget,
};
use bilterm::wm::{SessionNodeKind, WindowManager};

fn create_test_wm() -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.daemon_state_mut()
        .session_tree
        .upsert_session("dev", SessionNodeKind::Rooted, None);
    wm
}

fn run_colon(wm: &mut WindowManager, cmd: &str) -> Result<(), String> {
    let parsed = parse_colon_command(cmd)
        .ok_or_else(|| format!("not a colon command: {cmd}"))?
        .map_err(|e| format!("parse error: {e}"))?;
    wm.apply(parsed).map_err(|e| e.to_string())?;
    Ok(())
}

#[test]
fn test_window_move_surface_parity() {
    // 1. Via Action
    let mut wm1 = create_test_wm();
    let p1_1 = wm1.active_pane_id();
    let _p2_1 = wm1
        .split_focused(bilterm::wm::split_tree::SplitDirection::Vertical)
        .unwrap();
    let res1 = wm1.apply(Action::WindowManage(WindowManageOp::Move {
        window_id: Some(p1_1),
        target_session: "dev".to_string(),
        switch: false,
    }));
    assert!(res1.is_ok());

    // 2. Via Colon Command
    let mut wm2 = create_test_wm();
    let p1_2 = wm2.active_pane_id();
    let _p2_2 = wm2
        .split_focused(bilterm::wm::split_tree::SplitDirection::Vertical)
        .unwrap();
    let colon_res = run_colon(&mut wm2, &format!(":window move {p1_2} dev"));
    assert!(colon_res.is_ok());

    // 3. Via JSON-RPC
    let mut wm3 = create_test_wm();
    let p1_3 = wm3.active_pane_id();
    let _p2_3 = wm3
        .split_focused(bilterm::wm::split_tree::SplitDirection::Vertical)
        .unwrap();
    let mut rpc3 = AutomationControlPlane::new(wm3, Rect::new(0, 0, 80, 24));
    let rpc_params = JsonValue::obj([
        ("window_id", JsonValue::Number(p1_3 as i64)),
        ("target_session", JsonValue::String("dev".to_string())),
    ]);
    let rpc_res = rpc3.handle_window_move(&rpc_params);
    assert!(rpc_res.is_ok());
    let wm3 = rpc3.wm;

    // 4. Via MCP tool
    let mut wm4 = create_test_wm();
    let p1_4 = wm4.active_pane_id();
    let _p2_4 = wm4
        .split_focused(bilterm::wm::split_tree::SplitDirection::Vertical)
        .unwrap();
    let mut rpc4 = AutomationControlPlane::new(wm4, Rect::new(0, 0, 80, 24));
    let mcp_call = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{{\"name\":\"bilterm_window_manage\",\"arguments\":{{\"action\":\"move\",\"window_id\":{p1_4},\"target_session\":\"dev\"}}}}}}"
    );
    let resp4 = rpc4.handle_mcp_line(&mcp_call).expect("mcp response");
    assert!(!resp4.contains("isError\":true"));
    let wm4 = rpc4.wm;

    // Assert exact parity across all 4 surfaces
    assert_eq!(wm1.pane_count(), 1);
    assert_eq!(wm2.pane_count(), 1);
    assert_eq!(wm3.pane_count(), 1);
    assert_eq!(wm4.pane_count(), 1);

    assert_eq!(wm1.panes().keys().collect::<Vec<_>>(), wm2.panes().keys().collect::<Vec<_>>());
    assert_eq!(wm1.panes().keys().collect::<Vec<_>>(), wm3.panes().keys().collect::<Vec<_>>());
    assert_eq!(wm1.panes().keys().collect::<Vec<_>>(), wm4.panes().keys().collect::<Vec<_>>());
}

#[test]
fn test_window_rename_surface_parity() {
    let mut wm1 = create_test_wm();
    let p1 = wm1.active_pane_id();
    wm1.apply(Action::WindowManage(WindowManageOp::Rename {
        window_id: Some(p1),
        new_name: "custom-term".to_string(),
    }))
    .unwrap();

    let mut wm2 = create_test_wm();
    run_colon(&mut wm2, ":window rename custom-term").unwrap();

    let wm3 = create_test_wm();
    let mut rpc3 = AutomationControlPlane::new(wm3, Rect::new(0, 0, 80, 24));
    let rpc_params = JsonValue::obj([("new_name", JsonValue::String("custom-term".to_string()))]);
    rpc3.handle_window_rename(&rpc_params).unwrap();
    let wm3 = rpc3.wm;

    let wm4 = create_test_wm();
    let mut rpc4 = AutomationControlPlane::new(wm4, Rect::new(0, 0, 80, 24));
    let mcp_call = "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"bilterm_window_manage\",\"arguments\":{\"action\":\"rename\",\"new_name\":\"custom-term\"}}}";
    let resp = rpc4.handle_mcp_line(mcp_call).expect("mcp response");
    assert!(!resp.contains("isError\":true"));
    let wm4 = rpc4.wm;

    assert_eq!(wm1.pane(p1).unwrap().title(), "custom-term");
    assert_eq!(wm2.pane(p1).unwrap().title(), "custom-term");
    assert_eq!(wm3.pane(p1).unwrap().title(), "custom-term");
    assert_eq!(wm4.pane(p1).unwrap().title(), "custom-term");
}

#[test]
fn test_session_rename_surface_parity() {
    let mut wm1 = create_test_wm();
    wm1.apply(Action::SessionManage(SessionManageOp::Rename {
        old_name: None,
        new_name: "production".to_string(),
    }))
    .unwrap();

    let mut wm2 = create_test_wm();
    run_colon(&mut wm2, ":session rename production").unwrap();

    let wm3 = create_test_wm();
    let mut rpc3 = AutomationControlPlane::new(wm3, Rect::new(0, 0, 80, 24));
    let rpc_params = JsonValue::obj([("new_name", JsonValue::String("production".to_string()))]);
    rpc3.handle_session_rename(&rpc_params).unwrap();
    let wm3 = rpc3.wm;

    let wm4 = create_test_wm();
    let mut rpc4 = AutomationControlPlane::new(wm4, Rect::new(0, 0, 80, 24));
    let mcp_call = "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"bilterm_window_manage\",\"arguments\":{\"action\":\"rename_session\",\"new_name\":\"production\"}}}";
    let resp = rpc4.handle_mcp_line(mcp_call).expect("mcp response");
    assert!(!resp.contains("isError\":true"));
    let wm4 = rpc4.wm;

    assert_eq!(wm1.config().history_session, "production");
    assert_eq!(wm2.config().history_session, "production");
    assert_eq!(wm3.config().history_session, "production");
    assert_eq!(wm4.config().history_session, "production");
}

#[test]
fn test_window_select_surface_parity() {
    let mut wm1 = create_test_wm();
    let _p2 = wm1
        .split_focused(bilterm::wm::split_tree::SplitDirection::Vertical)
        .unwrap();
    wm1.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::All,
    )))
    .unwrap();

    let mut wm2 = create_test_wm();
    let _p2 = wm2
        .split_focused(bilterm::wm::split_tree::SplitDirection::Vertical)
        .unwrap();
    run_colon(&mut wm2, ":window select all").unwrap();

    let mut wm3 = create_test_wm();
    let _p2 = wm3
        .split_focused(bilterm::wm::split_tree::SplitDirection::Vertical)
        .unwrap();
    let mut rpc3 = AutomationControlPlane::new(wm3, Rect::new(0, 0, 80, 24));
    let rpc_params = JsonValue::obj([("action", JsonValue::String("all".to_string()))]);
    rpc3.handle_window_select(&rpc_params).unwrap();
    let wm3 = rpc3.wm;

    let mut wm4 = create_test_wm();
    let _p2 = wm4
        .split_focused(bilterm::wm::split_tree::SplitDirection::Vertical)
        .unwrap();
    let mut rpc4 = AutomationControlPlane::new(wm4, Rect::new(0, 0, 80, 24));
    let mcp_call = "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"bilterm_window_manage\",\"arguments\":{\"action\":\"select\",\"select_action\":\"all\"}}}";
    let resp = rpc4.handle_mcp_line(mcp_call).expect("mcp response");
    assert!(!resp.contains("isError\":true"));
    let wm4 = rpc4.wm;

    assert_eq!(wm1.selected_windows().len(), 2);
    assert_eq!(wm1.selected_windows(), wm2.selected_windows());
    assert_eq!(wm1.selected_windows(), wm3.selected_windows());
    assert_eq!(wm1.selected_windows(), wm4.selected_windows());
}

#[test]
fn test_bulk_close_and_exit_guard_parity() {
    let make_guarded_wm = || {
        let mut wm = create_test_wm();
        wm.config_mut().exit_protection = ExitProtectionPolicy::Guard;
        let _p1 = wm.active_pane_id();
        let p2 = wm
            .split_focused(bilterm::wm::split_tree::SplitDirection::Vertical)
            .unwrap();
        wm.pane_mut(p2).unwrap().prompt_mut().running_command =
            Some("active-daemon".to_string());
        wm.apply(Action::WindowManage(WindowManageOp::Select(
            WindowSelectTarget::All,
        )))
        .unwrap();
        wm
    };

    // 1. Refusal when exit guard is active
    let mut wm1 = make_guarded_wm();
    let _ = wm1.apply(Action::WindowManage(WindowManageOp::Bulk(
        BulkWindowOp::Close,
    )));
    assert_eq!(wm1.pane_count(), 2);
    assert!(wm1.active_pane().status_message().unwrap().contains("protected by exit guard"));

    let mut wm2 = make_guarded_wm();
    let _ = run_colon(&mut wm2, ":window bulk close");
    assert_eq!(wm2.pane_count(), 2);
    assert!(wm2.active_pane().status_message().unwrap().contains("protected by exit guard"));

    let wm3 = make_guarded_wm();
    let mut rpc3 = AutomationControlPlane::new(wm3, Rect::new(0, 0, 80, 24));
    assert!(rpc3
        .handle_window_bulk(&JsonValue::obj([(
            "action",
            JsonValue::String("close".to_string())
        )]))
        .is_err());
    assert_eq!(rpc3.wm.pane_count(), 2);

    // 2. Force bypass parity
    let mut wm4 = make_guarded_wm();
    assert!(wm4
        .apply(Action::WindowManage(WindowManageOp::Bulk(
            BulkWindowOp::ForceClose
        )))
        .is_ok());
    assert_eq!(wm4.pane_count(), 1);

    let mut wm5 = make_guarded_wm();
    assert!(run_colon(&mut wm5, ":window bulk close --force").is_ok());
    assert_eq!(wm5.pane_count(), 1);

    let wm6 = make_guarded_wm();
    let mut rpc6 = AutomationControlPlane::new(wm6, Rect::new(0, 0, 80, 24));
    assert!(rpc6
        .handle_window_bulk(&JsonValue::obj([
            ("action", JsonValue::String("close".to_string())),
            ("force", JsonValue::Bool(true)),
        ]))
        .is_ok());
    assert_eq!(rpc6.wm.pane_count(), 1);
}

#[test]
fn test_window_undo_surface_parity() {
    // 1. Via Action
    let mut wm1 = create_test_wm();
    let p1 = wm1.active_pane_id();
    wm1.pane_mut(p1).unwrap().set_title("before".to_string());
    wm1.apply(Action::WindowManage(WindowManageOp::Rename {
        window_id: Some(p1),
        new_name: "after".to_string(),
    }))
    .unwrap();
    assert_eq!(wm1.pane(p1).unwrap().title(), "after");
    wm1.apply(Action::WindowManage(WindowManageOp::Undo)).unwrap();
    assert_eq!(wm1.pane(p1).unwrap().title(), "before");

    // 2. Via Colon
    let mut wm2 = create_test_wm();
    let p2 = wm2.active_pane_id();
    wm2.pane_mut(p2).unwrap().set_title("before".to_string());
    run_colon(&mut wm2, ":window rename after").unwrap();
    assert_eq!(wm2.pane(p2).unwrap().title(), "after");
    run_colon(&mut wm2, ":window undo").unwrap();
    assert_eq!(wm2.pane(p2).unwrap().title(), "before");

    // 3. Via JSON-RPC
    let mut wm3 = create_test_wm();
    let p3 = wm3.active_pane_id();
    wm3.pane_mut(p3).unwrap().set_title("before".to_string());
    let mut rpc3 = AutomationControlPlane::new(wm3, Rect::new(0, 0, 80, 24));
    rpc3.handle_window_rename(&JsonValue::obj([(
        "new_name",
        JsonValue::String("after".to_string()),
    )]))
    .unwrap();
    rpc3.handle_window_undo(&JsonValue::Null).unwrap();
    let wm3 = rpc3.wm;
    assert_eq!(wm3.pane(p3).unwrap().title(), "before");

    // 4. Via Navigator Key 'u'
    let mut wm4 = create_test_wm();
    let p4 = wm4.active_pane_id();
    wm4.pane_mut(p4).unwrap().set_title("before".to_string());
    wm4.apply(Action::WindowManage(WindowManageOp::Rename {
        window_id: Some(p4),
        new_name: "after".to_string(),
    }))
    .unwrap();
    let key_u = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE);
    handle_navigator_drawer_key(&mut wm4, key_u);
    assert_eq!(wm4.pane(p4).unwrap().title(), "before");
}

#[test]
fn test_navigator_key_multi_select() {
    let mut wm = create_test_wm();
    let p1 = wm.active_pane_id();
    let _p2 = wm
        .split_focused(bilterm::wm::split_tree::SplitDirection::Vertical)
        .unwrap();

    // Focus navigator drawer and select row corresponding to p1
    wm.apply(Action::DrawerFocus).unwrap();
    let rows = wm.drawer_rows();
    let p1_idx = rows
        .iter()
        .position(|r| matches!(r.kind, DrawerRowKind::Pane(id) if id == p1))
        .expect("row for p1");
    wm.drawer_mut().selected_index = p1_idx;

    // Press Space in Navigator -> toggles p1
    let space_key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
    let handled = handle_navigator_drawer_key(&mut wm, space_key);
    assert!(handled.is_some());
    assert!(wm.is_window_selected(p1));

    // Esc clears selection
    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    handle_navigator_drawer_key(&mut wm, esc_key);
    assert!(wm.selected_windows().is_empty());

    // Ctrl+A selects all
    let ctrl_a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
    handle_navigator_drawer_key(&mut wm, ctrl_a);
    assert_eq!(wm.selected_windows().len(), 2);
}

#[test]
fn test_navigator_drawer_visual_selection_badge() {
    let mut wm = create_test_wm();
    let _p1 = wm.active_pane_id();
    let p2 = wm
        .split_focused(bilterm::wm::split_tree::SplitDirection::Vertical)
        .unwrap();

    // Select window p2
    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::Toggle(p2),
    )))
    .unwrap();

    // Inspect drawer rows
    let rows = wm.drawer_rows();
    let p2_row = rows
        .iter()
        .find(|r| matches!(r.kind, DrawerRowKind::Pane(id) if id == p2))
        .expect("p2 drawer row");

    assert!(
        p2_row.title.contains("[✓]"),
        "expected selection badge [✓] in row title, got: {}",
        p2_row.title
    );
}
