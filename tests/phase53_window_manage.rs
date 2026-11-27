//! Phase 53 Wave 2 test suite: Move, Rename, Multi-Select, Bulk Actions, Undo & Persistence (`SES-01..04`).

use bilterm::action::Action;
use bilterm::pane::ExitProtectionPolicy;
use bilterm::wm::window_manage::{
    BulkWindowOp, SessionManageOp, WindowManageOp, WindowSelectTarget,
};
use bilterm::wm::{SavedWorkspaceSession, SessionNodeKind, WindowManager};

#[test]
fn test_move_window_preserves_pty() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();

    // Write canvas output, running command, and prompt text
    wm.active_pane_mut()
        .process_pty_output(b"LIVE_PROCESS_DATA_42\r\n");
    wm.active_pane_mut().prompt_mut().running_command = Some("python worker.py".to_string());
    wm.active_pane_mut().prompt_mut().set_text("pending input");

    // Create target background session "dev"
    wm.daemon_state_mut()
        .session_tree
        .upsert_session("dev", SessionNodeKind::Rooted, None);

    // Split source session into 2 panes so source remains non-empty
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();
    assert_eq!(wm.pane_count(), 2);

    // Move p1 to session "dev" (switch = false)
    let res = wm.apply(Action::WindowManage(WindowManageOp::Move {
        window_id: Some(p1),
        target_session: "dev".to_string(),
        switch: false,
    }));
    assert!(res.is_ok(), "move failed: {res:?}");
    assert_eq!(wm.pane_count(), 1);
    assert_eq!(wm.active_pane_id(), p2);

    // Switch to session "dev"
    let switch_res = wm.switch_to_session_preserving_panes("dev");
    assert!(switch_res.is_ok(), "switch failed: {switch_res:?}");
    assert_eq!(wm.config().history_session, "dev");
    assert_eq!(wm.active_pane_id(), p1);

    // Assert PTY scrollback, running command, and prompt text were 100% preserved
    let canvas_text = wm.active_pane().vt().snapshot().contents();
    assert!(
        canvas_text.contains("LIVE_PROCESS_DATA_42"),
        "scrollback lost across session move: {canvas_text}"
    );
    assert_eq!(
        wm.active_pane().prompt().running_command.as_deref(),
        Some("python worker.py")
    );
    assert_eq!(wm.active_pane().prompt().text(), "pending input");
}

#[test]
fn test_move_window_to_nested_session() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();

    // Register nested session "infra/cluster" in SessionTree
    wm.daemon_state_mut()
        .session_tree
        .upsert_session("infra/cluster", SessionNodeKind::Subtree, Some("infra"));

    let res = wm.apply(Action::WindowManage(WindowManageOp::Move {
        window_id: Some(p1),
        target_session: "infra/cluster".to_string(),
        switch: true,
    }));
    assert!(res.is_ok(), "move to nested session failed: {res:?}");
    assert_eq!(wm.config().history_session, "infra/cluster");
    assert_eq!(wm.active_pane_id(), p1);
}

#[test]
fn test_move_window_to_current_session_fails() {
    let mut wm = WindowManager::new_headless(24, 80);
    let cur = wm.config().history_session.clone();

    let res = wm.window_manage_apply(&WindowManageOp::Move {
        window_id: None,
        target_session: cur.clone(),
        switch: false,
    });
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.contains("already in session"),
        "unexpected error message: {err}"
    );
}

#[test]
fn test_move_window_to_nonexistent_session_fails() {
    let mut wm = WindowManager::new_headless(24, 80);

    let res = wm.window_manage_apply(&WindowManageOp::Move {
        window_id: None,
        target_session: "ghost-session".to_string(),
        switch: false,
    });
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.contains("Target session 'ghost-session' not found"),
        "unexpected error message: {err}"
    );
}

#[test]
fn test_source_session_focus_and_zorder_fallback() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();
    let p3 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Horizontal).unwrap();
    assert_eq!(wm.active_pane_id(), p3);

    wm.daemon_state_mut()
        .session_tree
        .upsert_session("bg", SessionNodeKind::Rooted, None);

    // Move focused window p3 to session "bg"
    let res = wm.apply(Action::WindowManage(WindowManageOp::Move {
        window_id: Some(p3),
        target_session: "bg".to_string(),
        switch: false,
    }));
    assert!(res.is_ok());

    // Focus in source session falls back to topmost remaining window in z-order (p2)
    assert_eq!(wm.active_pane_id(), p2);
    assert_eq!(wm.pane_count(), 2);
    assert!(wm.panes().contains_key(&p1));
    assert!(wm.panes().contains_key(&p2));
    assert!(!wm.panes().contains_key(&p3));
}

#[test]
fn test_move_last_window_spawns_default_shell() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    assert_eq!(wm.pane_count(), 1);

    wm.daemon_state_mut()
        .session_tree
        .upsert_session("other", SessionNodeKind::Rooted, None);

    let res = wm.apply(Action::WindowManage(WindowManageOp::Move {
        window_id: Some(p1),
        target_session: "other".to_string(),
        switch: false,
    }));
    assert!(res.is_ok());

    // Source session is kept valid: fresh default window spawned
    assert_eq!(wm.pane_count(), 1);
    let new_p = wm.active_pane_id();
    assert_ne!(new_p, p1);
}

#[test]
fn test_window_rename_and_disambiguation() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();

    // Rename active window to "editor"
    let res = wm.apply(Action::WindowManage(WindowManageOp::Rename {
        window_id: Some(p1),
        new_name: "editor".to_string(),
    }));
    assert!(res.is_ok());
    assert_eq!(wm.pane(p1).unwrap().title(), "editor");

    // Split second pane
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();

    // Renaming second pane to "editor" collides and disambiguates to "editor-2"
    let res2 = wm.apply(Action::WindowManage(WindowManageOp::Rename {
        window_id: Some(p2),
        new_name: "editor".to_string(),
    }));
    assert!(res2.is_ok());
    assert_eq!(wm.pane(p2).unwrap().title(), "editor-2");
}

#[test]
fn test_window_rename_validation_rejects_traversal() {
    let mut wm = WindowManager::new_headless(24, 80);

    let res = wm.window_manage_apply(&WindowManageOp::Rename {
        window_id: None,
        new_name: "../escape".to_string(),
    });
    assert!(res.is_err());

    let res_slash = wm.window_manage_apply(&WindowManageOp::Rename {
        window_id: None,
        new_name: "a/b".to_string(),
    });
    assert!(res_slash.is_err());
}

#[test]
fn test_session_rename_updates_tree_and_active_config() {
    let mut wm = WindowManager::new_headless(24, 80);
    let dir = tempfile::tempdir().unwrap();
    wm.config_mut().session_dir = Some(dir.path().to_string_lossy().into_owned());
    wm.config_mut().history_session = "alpha".to_string();

    let res = wm.apply(Action::SessionManage(SessionManageOp::Rename {
        old_name: Some("alpha".to_string()),
        new_name: "beta".to_string(),
    }));
    assert!(res.is_ok());
    assert_eq!(wm.config().history_session, "beta");
    assert!(wm.daemon_state().session_tree.node("beta").is_some());
}

#[test]
fn test_multi_select_toggle_and_clear() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();
    let p3 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Horizontal).unwrap();

    // Toggle p1 and p2
    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::Toggle(p1),
    )))
    .unwrap();
    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::Toggle(p2),
    )))
    .unwrap();
    assert_eq!(wm.selected_windows().len(), 2);
    assert!(wm.is_window_selected(p1));
    assert!(wm.is_window_selected(p2));
    assert!(!wm.is_window_selected(p3));

    // Clear
    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::None,
    )))
    .unwrap();
    assert!(wm.selected_windows().is_empty());
}

#[test]
fn test_multi_select_all_and_invert() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();
    let p3 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Horizontal).unwrap();

    // Select all
    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::All,
    )))
    .unwrap();
    assert_eq!(wm.selected_windows().len(), 3);

    // Invert -> 0
    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::Invert,
    )))
    .unwrap();
    assert!(wm.selected_windows().is_empty());

    // Toggle p2 -> {p2}, then invert -> {p1, p3}
    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::Toggle(p2),
    )))
    .unwrap();
    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::Invert,
    )))
    .unwrap();
    assert_eq!(wm.selected_windows().len(), 2);
    assert!(wm.is_window_selected(p1));
    assert!(!wm.is_window_selected(p2));
    assert!(wm.is_window_selected(p3));
}

#[test]
fn test_multi_select_range() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();
    let p3 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Horizontal).unwrap();

    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::Range { from: p1, to: p2 },
    )))
    .unwrap();
    assert!(wm.is_window_selected(p1));
    assert!(wm.is_window_selected(p2));
    assert!(!wm.is_window_selected(p3));
}

#[test]
fn test_bulk_move_selected_windows() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();
    let p3 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Horizontal).unwrap();

    wm.daemon_state_mut()
        .session_tree
        .upsert_session("archive", SessionNodeKind::Rooted, None);

    // Select p2 and p3
    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::Toggle(p2),
    )))
    .unwrap();
    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::Toggle(p3),
    )))
    .unwrap();

    // Bulk move to "archive"
    let res = wm.apply(Action::WindowManage(WindowManageOp::Bulk(
        BulkWindowOp::Move {
            target_session: "archive".to_string(),
        },
    )));
    assert!(res.is_ok());
    assert_eq!(wm.pane_count(), 1);
    assert!(wm.panes().contains_key(&p1));
    assert!(!wm.panes().contains_key(&p2));
    assert!(!wm.panes().contains_key(&p3));
    assert!(wm.selected_windows().is_empty());
}

#[test]
fn test_bulk_close_honors_exit_guard() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().exit_protection = ExitProtectionPolicy::Guard;

    let p1 = wm.active_pane_id();
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();

    // Mark p2 with active running command
    wm.pane_mut(p2).unwrap().prompt_mut().running_command = Some("cargo watch".to_string());

    // Select both
    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::All,
    )))
    .unwrap();

    // Bulk close without force is refused because p2 has active process
    let res = wm.window_manage_apply(&WindowManageOp::Bulk(BulkWindowOp::Close));
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.contains("protected by exit guard"),
        "unexpected error message: {err}"
    );

    // Both windows are still alive (all-or-nothing atomicity!)
    assert_eq!(wm.pane_count(), 2);
    assert!(wm.panes().contains_key(&p1));
    assert!(wm.panes().contains_key(&p2));
}

#[test]
fn test_bulk_close_force_bypasses_exit_guard() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().exit_protection = ExitProtectionPolicy::Guard;

    let _p1 = wm.active_pane_id();
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();
    wm.pane_mut(p2).unwrap().prompt_mut().running_command = Some("cargo watch".to_string());

    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::All,
    )))
    .unwrap();

    let res = wm.apply(Action::WindowManage(WindowManageOp::Bulk(
        BulkWindowOp::ForceClose,
    )));
    assert!(res.is_ok());
    // All original windows closed, fresh window spawned to keep workspace valid
    assert_eq!(wm.pane_count(), 1);
}

#[test]
fn test_bulk_rename_prefix() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();

    wm.pane_mut(p1).unwrap().set_title("term1".to_string());
    wm.pane_mut(p2).unwrap().set_title("term2".to_string());

    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::All,
    )))
    .unwrap();

    let res = wm.apply(Action::WindowManage(WindowManageOp::Bulk(
        BulkWindowOp::RenamePrefix {
            prefix: "corp-".to_string(),
        },
    )));
    assert!(res.is_ok());
    assert_eq!(wm.pane(p1).unwrap().title(), "corp-term1");
    assert_eq!(wm.pane(p2).unwrap().title(), "corp-term2");
}

#[test]
fn test_bulk_tile_and_pin() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();

    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::All,
    )))
    .unwrap();

    // Bulk tile
    let res = wm.apply(Action::WindowManage(WindowManageOp::Bulk(
        BulkWindowOp::Tile,
    )));
    assert!(res.is_ok());

    // Bulk pin
    let res_pin = wm.apply(Action::WindowManage(WindowManageOp::Bulk(
        BulkWindowOp::Pin(Some(true)),
    )));
    assert!(res_pin.is_ok());
    assert!(wm.pane(p1).unwrap().is_read_only());
    assert!(wm.pane(p2).unwrap().is_read_only());

    // Bulk unpin
    let res_unpin = wm.apply(Action::WindowManage(WindowManageOp::Bulk(
        BulkWindowOp::Pin(Some(false)),
    )));
    assert!(res_unpin.is_ok());
    assert!(!wm.pane(p1).unwrap().is_read_only());
    assert!(!wm.pane(p2).unwrap().is_read_only());
}

#[test]
fn test_single_step_topology_undo() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    wm.pane_mut(p1).unwrap().set_title("initial".to_string());

    // 1. Rename and undo
    wm.apply(Action::WindowManage(WindowManageOp::Rename {
        window_id: Some(p1),
        new_name: "mutated".to_string(),
    }))
    .unwrap();
    assert_eq!(wm.pane(p1).unwrap().title(), "mutated");

    wm.apply(Action::WindowManage(WindowManageOp::Undo)).unwrap();
    assert_eq!(wm.pane(p1).unwrap().title(), "initial");

    // 2. Move and undo
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();
    wm.daemon_state_mut()
        .session_tree
        .upsert_session("storage", SessionNodeKind::Rooted, None);

    wm.apply(Action::WindowManage(WindowManageOp::Move {
        window_id: Some(p2),
        target_session: "storage".to_string(),
        switch: false,
    }))
    .unwrap();
    assert_eq!(wm.pane_count(), 1);

    wm.apply(Action::WindowManage(WindowManageOp::Undo)).unwrap();
    assert_eq!(wm.pane_count(), 2);
    assert!(wm.panes().contains_key(&p2));
}

#[test]
fn test_session_toml_and_persistence_round_trip() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();

    wm.pane_mut(p1).unwrap().set_title("main-editor".to_string());
    wm.pane_mut(p2).unwrap().set_title("log-stream".to_string());

    // Select p2
    wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::Toggle(p2),
    )))
    .unwrap();

    // Snapshot and serialize to TOML
    let snap = wm.snapshot_session("persist_test");
    let toml_str = toml::to_string_pretty(&snap).expect("serialize toml");
    assert!(toml_str.contains("custom_name = \"main-editor\""));
    assert!(toml_str.contains("custom_name = \"log-stream\""));
    assert!(toml_str.contains("selected_windows"));

    // Deserialize into fresh SavedWorkspaceSession
    let loaded: SavedWorkspaceSession = toml::from_str(&toml_str).expect("deserialize toml");
    assert_eq!(loaded.name, "persist_test");
    assert_eq!(loaded.selected_windows.len(), 1);
    assert!(loaded.selected_windows.contains(&p2));

    // Restore into fresh WindowManager
    let mut wm2 = WindowManager::new_headless(24, 80);
    wm2.apply_saved_session_snapshot(&loaded);
    assert_eq!(wm2.pane(p1).unwrap().title(), "main-editor");
    assert_eq!(wm2.pane(p2).unwrap().title(), "log-stream");
    assert!(wm2.is_window_selected(p2));
}

#[test]
fn custom_pane_rename_survives_osc_title_and_ignores_unfocused_drawer_selection() {
    use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};

    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();
    wm.focus_and_raise(p2);

    // Point unfocused drawer selection at p1; active pane is p2.
    let rows = wm.drawer_rows();
    let p1_row = rows
        .iter()
        .position(|r| r.kind == bilterm::wm::DrawerRowKind::Pane(p1))
        .expect("p1 row");
    wm.drawer_mut().selected_index = p1_row;
    assert!(!wm.drawer().focused);

    let cmd = parse_colon_command(":pane renname newname")
        .expect("typo alias parses")
        .expect("ok");
    execute_wm_colon_command(&mut wm, cmd);
    assert_eq!(wm.pane(p2).unwrap().title(), "newname");
    assert!(wm.pane(p2).unwrap().is_title_locked());
    assert_ne!(wm.pane(p1).unwrap().title(), "newname");

    // Subsequent shell OSC 2 title sequence must NOT overwrite user-assigned name.
    wm.pane_mut(p2).unwrap().process_pty_output(b"\x1b]2;zsh:~/src\x07");
    assert_eq!(wm.pane(p2).unwrap().title(), "newname");
}

#[test]
fn classic_drawer_right_click_rename_selects_row_and_commits() {
    use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();
    wm.focus_and_raise(p1);
    wm.toggle_drawer();
    wm.drawer_mut().selected_index = 0;

    let area = Rect::new(0, 0, 80, 24);
    let shell = bilterm::ui::shell::ShellLayout::compute(area, wm.config(), wm.pane_count());
    let dr = wm.drawer_rect(shell.workspace).expect("drawer rect");
    let rows = wm.drawer_rows();
    let p2_row = rows
        .iter()
        .position(|r| r.kind == bilterm::wm::DrawerRowKind::Pane(p2))
        .expect("p2 row");
    let (hdr_h, _, row_step) = bilterm::ui::drawer::drawer_vertical_layout_metrics(
        wm.config(),
        dr.height.saturating_sub(2),
        rows.len(),
    );
    let click_y = dr.y + 1 + hdr_h + (p2_row * row_step) as u16;
    let click_x = dr.x + 2;

    // Right-click on p2's row in Classic drawer
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: click_x,
            row: click_y,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    assert_eq!(wm.drawer().selected_index, p2_row);

    let menu = bilterm::ui::drawer::navigator::standalone_context_menu_layout(&wm, area)
        .expect("context menu open");
    let (_, rename_y) = menu
        .entries
        .iter()
        .find(|&&(e, _)| e == bilterm::wm::drawer::NavigatorMenuEntry::Rename)
        .copied()
        .expect("Rename entry");

    // Left-click "Rename"
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: menu.rect.x + 1,
            row: rename_y,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    assert!(wm.rename_dialog_open(), "rename dialog should open");

    // Clear input (Ctrl+U) and type "renamed-p2" + Enter
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL)).unwrap();
    for ch in "renamed-p2".chars() {
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert!(!wm.rename_dialog_open());
    assert_eq!(wm.pane(p2).unwrap().title(), "renamed-p2");
}

#[test]
fn pane_list_and_window_list_work_across_colon_rpc_mcp_and_cli() {
    use bilterm::api::rpc::AutomationControlPlane;
    use bilterm::api::JsonValue;
    use bilterm::cli::{CliEnv, parse as cli_parse};
    use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
    use ratatui::layout::Rect;

    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    let p2 = wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();
    wm.rename_window(Some(p1), "alpha").unwrap();
    wm.rename_window(Some(p2), "beta").unwrap();

    for cmd_str in [":window list", ":window ls", ":pane list", ":pane ls"] {
        let cmd = parse_colon_command(cmd_str).expect("parses").expect("ok");
        execute_wm_colon_command(&mut wm, cmd);
        let status = wm.active_pane().status_message().unwrap_or("");
        assert!(status.contains("Windows (2):"), "{cmd_str} -> {status}");
        assert!(status.contains("alpha"), "{cmd_str} -> {status}");
        assert!(status.contains("beta"), "{cmd_str} -> {status}");
    }

    // RPC window.list + pane.list
    let mut plane = AutomationControlPlane::new(wm, Rect::new(0, 0, 80, 24));
    let win_list = plane.dispatch_method("window.list", &JsonValue::Null).unwrap();
    assert_eq!(win_list.get("count").and_then(JsonValue::as_i64), Some(2));
    let pane_list = plane.dispatch_method("pane.list", &JsonValue::Null).unwrap();
    assert_eq!(pane_list.get("panes").and_then(|v| v.as_array().map(|a| a.len())), Some(2));

    // MCP bilterm_window_manage(list) & bilterm_agent_control(window_list / pane_list)
    let mcp_win = plane
        .handle_mcp_line(r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"bilterm_window_manage","arguments":{"action":"list"}}}"#)
        .unwrap();
    assert!(mcp_win.contains("alpha") && mcp_win.contains("beta"));
    let mcp_ctrl = plane
        .handle_mcp_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"bilterm_agent_control","arguments":{"action":"window_list"}}}"#)
        .unwrap();
    assert!(mcp_ctrl.contains("alpha") && mcp_ctrl.contains("beta"));

    // CLI bilterm window list & bilterm pane list
    let cli_win = cli_parse(&["window".into(), "list".into()], &CliEnv::default()).unwrap();
    assert_eq!(cli_win.method, "window.list");
    let cli_pane = cli_parse(&["pane".into(), "list".into()], &CliEnv::default()).unwrap();
    assert_eq!(cli_pane.method, "pane.list");
}
