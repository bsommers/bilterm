use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use bilterm::input::{handle_key_event, handle_wm_key_event, handle_wm_mouse_event};
use bilterm::pane::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::pane::{CompositePane, PromptAnchor};
use bilterm::ui::{CompositePaneWidget, WorkspaceWidget, buffer_to_snapshot_string};
use bilterm::vt::{CanvasHintKind, Osc9Progress};
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::{WindowManager, WorkspaceLayoutMode};

#[test]
fn v030_osc9_progress_gauge_and_decset_2031_color_scheme_notifications() {
    let mut wm = WindowManager::new_headless(25, 100);
    wm.active_pane_mut()
        .process_pty_output(b"\x1b[?2031h\x1b[?996n\x1b]9;4;1;50\x07");

    assert_eq!(
        wm.active_pane().vt.osc9_progress(),
        Some(Osc9Progress::Normal(50))
    );
    let replies = wm.active_pane_mut().vt.take_query_responses();
    assert_eq!(replies, vec![b"\x1b[?997;1n".to_vec()]);

    let area = Rect::new(0, 0, 100, 25);
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(&wm).render(area, &mut buf);
    let snap = buffer_to_snapshot_string(&buf);
    assert!(snap.contains("[████░░░░]  50%"));

    // Indeterminate and clear states
    wm.active_pane_mut().process_pty_output(b"\x1b]9;4;3\x07");
    assert_eq!(
        wm.active_pane().vt.osc9_progress(),
        Some(Osc9Progress::Indeterminate)
    );
    wm.active_pane_mut().process_pty_output(b"\x1b]9;4;0\x07");
    assert_eq!(wm.active_pane().vt.osc9_progress(), None);

    // Switching theme via :theme emits unsolicited CSI ? 997 ; 1 n to panes with ?2031 enabled
    let cmd = parse_colon_command(":theme 3279").unwrap().unwrap();
    execute_wm_colon_command(&mut wm, cmd);
    let theme_replies = wm.active_pane_mut().vt.take_query_responses();
    assert_eq!(theme_replies, vec![b"\x1b[?997;1n".to_vec()]);
}

#[test]
fn v030_quickselect_3270_field_hints_detects_urls_paths_ips_and_shas() {
    let mut pane = CompositePane::new_headless(1, 20, 90);
    pane.process_pty_output(
        b"\r\n\r\n\r\n\r\nDeploy https://bilterm.dev from ./src/main.rs at commit d974c7f on 192.168.1.42:8080\r\n",
    );

    let hints = pane.vt.detect_visible_hints();
    assert_eq!(hints.len(), 4);
    assert_eq!(hints[0].kind, CanvasHintKind::Url);
    assert_eq!(hints[0].text, "https://bilterm.dev");
    assert_eq!(hints[0].label, "a");
    assert_eq!(hints[1].kind, CanvasHintKind::Path);
    assert_eq!(hints[1].text, "./src/main.rs");
    assert_eq!(hints[1].label, "b");
    assert_eq!(hints[2].kind, CanvasHintKind::GitSha);
    assert_eq!(hints[2].text, "d974c7f");
    assert_eq!(hints[2].label, "c");
    assert_eq!(hints[3].kind, CanvasHintKind::IpAddr);
    assert_eq!(hints[3].text, "192.168.1.42:8080");
    assert_eq!(hints[3].label, "d");

    // Press Alt+F to open 3270 Field Hints mode
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT)).unwrap();
    assert!(pane.hints_mode.active);

    // Render and verify [a] [b] [c] [d] badges appear on the canvas
    let area = Rect::new(0, 0, 90, 20);
    let mut buf = Buffer::empty(area);
    CompositePaneWidget::new(&pane, true).render(area, &mut buf);
    let snap = buffer_to_snapshot_string(&buf);
    assert!(snap.contains("[a]"));
    assert!(snap.contains("[b]"));
    assert!(snap.contains("[c]"));
    assert!(snap.contains("[d]"));

    // Type 'c' to select the git SHA hint ("d974c7f") -> inserts into prompt and yanks via OSC 52
    handle_key_event(
        &mut pane,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
    )
    .unwrap();
    assert!(!pane.hints_mode.active);
    assert_eq!(pane.prompt.text(), "d974c7f");
    assert_eq!(pane.prompt.yank_buffer, "d974c7f");
    assert!(pane.last_osc52_sequence.is_some());
}

#[test]
fn v030_inspector_modal_top_menubar_read_only_lock_and_broadcast_input() {
    let mut wm = WindowManager::new_headless(26, 96);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    assert_eq!(wm.panes.len(), 2);

    // Enable Top Menu Bar (:menubar on) and click "View" category on row 0
    let cmd = parse_colon_command(":menubar on").unwrap().unwrap();
    execute_wm_colon_command(&mut wm, cmd);
    assert!(wm.config.show_top_menubar);

    let area = Rect::new(0, 0, 96, 26);
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 30,
            row: 0,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    assert!(wm.menubar_open_category.is_some());

    // Press Esc to close menu dropdown, then press F9 to open Live 3270 Terminal Inspector
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
    assert!(wm.menubar_open_category.is_none());

    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE)).unwrap();
    assert!(wm.inspector_visible);

    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(&wm).render(area, &mut buf);
    let snap = buffer_to_snapshot_string(&buf);
    assert!(snap.contains("≡ System"));
    assert!(snap.contains("[F9] 3270 TERMINAL INSPECTOR"));
    assert!(snap.contains("DECSET MODES"));

    // Close inspector with F9
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE)).unwrap();
    assert!(!wm.inspector_visible);

    // Lock Pane #1 as Read-Only (🔒 PROT), leave Pane #2 unlocked, and enable :broadcast
    wm.focus_and_raise(1);
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('l'), KeyModifiers::ALT)).unwrap();
    assert!(wm.panes.get(&1).unwrap().read_only);

    // Attempting to run a shell command in locked Pane #1 is blocked
    wm.active_pane_mut().prompt.set_text("rm -rf /");
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert!(wm.panes.get(&1).unwrap().vt.command_blocks().is_empty());

    // Focus Pane #2, enable :broadcast on, and submit a command -> fans out to unlocked Pane #2 while skipping locked Pane #1
    wm.focus_and_raise(2);
    let bcast_cmd = parse_colon_command(":broadcast on").unwrap().unwrap();
    execute_wm_colon_command(&mut wm, bcast_cmd);
    assert!(wm.broadcast_input);

    wm.active_pane_mut().prompt.set_text("uptime");
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.panes.get(&1).unwrap().vt.command_blocks().len(), 0);
    assert_eq!(wm.panes.get(&2).unwrap().vt.command_blocks().len(), 1);
    assert_eq!(
        wm.panes.get(&2).unwrap().vt.command_blocks()[0].command,
        "uptime"
    );

    // Unlock Pane #1 and broadcast again -> both Pane #1 and Pane #2 receive the command
    wm.panes.get_mut(&1).unwrap().set_read_only(false);
    wm.active_pane_mut().prompt.set_text("echo cluster-ready");
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.panes.get(&1).unwrap().vt.command_blocks().len(), 1);
    assert_eq!(wm.panes.get(&2).unwrap().vt.command_blocks().len(), 2);
}

#[test]
fn v030_workspace_session_save_list_and_restore_roundtrip() {
    let tmp_dir = std::env::temp_dir().join(format!(
        "bilterm-session-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp_dir);

    let mut wm = WindowManager::new_headless(25, 80);
    wm.config.session_dir = Some(tmp_dir.to_string_lossy().into_owned());
    wm.split_focused(SplitDirection::Horizontal).unwrap();
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.layout_mode = WorkspaceLayoutMode::Stacked;
    wm.panes
        .get_mut(&1)
        .unwrap()
        .set_prompt_anchor(PromptAnchor::Bottom);
    wm.panes.get_mut(&2).unwrap().set_read_only(true);
    wm.panes
        .get_mut(&3)
        .unwrap()
        .prompt
        .history
        .push("cargo test".to_string());

    let save_cmd = parse_colon_command(":session save prod_ops").unwrap().unwrap();
    execute_wm_colon_command(&mut wm, save_cmd);
    assert_eq!(wm.list_sessions(), vec!["prod_ops".to_string()]);

    // Reset workspace to a single pane and restore 'prod_ops'
    let mut restored_wm = WindowManager::new_headless(25, 80);
    restored_wm.config.session_dir = Some(tmp_dir.to_string_lossy().into_owned());
    assert_eq!(restored_wm.panes.len(), 1);

    let restore_cmd = parse_colon_command(":session restore prod_ops")
        .unwrap()
        .unwrap();
    execute_wm_colon_command(&mut restored_wm, restore_cmd);

    assert_eq!(restored_wm.panes.len(), 3);
    assert_eq!(restored_wm.layout_mode, WorkspaceLayoutMode::Stacked);
    assert_eq!(
        restored_wm.panes.get(&1).unwrap().prompt.anchor,
        PromptAnchor::Bottom
    );
    assert!(restored_wm.panes.get(&2).unwrap().read_only);
    assert_eq!(
        restored_wm.panes.get(&3).unwrap().prompt.history,
        vec!["cargo test".to_string()]
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
}
