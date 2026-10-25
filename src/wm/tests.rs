use super::*;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

#[test]
fn apply_config_restores_session_history_into_existing_and_split_panes() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("sessions/proj/history.log");
    std::fs::create_dir_all(log.parent().unwrap()).unwrap();
    std::fs::write(
        &log,
        "[2026-01-01T00:00:00Z | session=proj | run=r1 | pane=1] ls\n\
         [2026-01-01T00:00:01Z | session=proj | run=r1 | pane=1] ls\n\
         [2026-01-01T00:00:02Z | session=proj | run=r2 | pane=2] echo a\\necho b\n",
    )
    .unwrap();

    let mut wm = WindowManager::new_headless(25, 80);
    assert!(wm.active_pane().prompt().history.is_empty());
    let cfg = BiltermConfig {
        history_dir: Some(tmp.path().to_string_lossy().into_owned()),
        history_session: "proj".to_string(),
        ..BiltermConfig::default()
    };
    wm.apply_config(cfg);
    let expected = vec!["ls".to_string(), "echo a\necho b".to_string()];
    assert_eq!(wm.active_pane().prompt().history, expected);

    wm.split_focused(SplitDirection::Vertical).unwrap();
    assert_eq!(wm.active_pane().prompt().history, expected);
    assert_eq!(wm.active_pane().history_store().session_file_path, log);
}

#[test]
fn window_manager_splits_stacks_shuffles_raises_and_translates_sgr_mouse() {
    let mut wm = WindowManager::new_headless(25, 80);
    let screen = Rect::new(0, 0, 80, 25);

    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    assert_eq!(p2, 2);
    assert_eq!(wm.active_pane_id(), 2);

    let p3 = wm.split_focused(SplitDirection::Horizontal).unwrap();
    assert_eq!(p3, 3);
    assert_eq!(wm.active_pane_id(), 3);

    // Shuffle deck top-to-bottom: 3 -> bottom, 2 becomes active
    assert_eq!(wm.shuffle_deck_next(), Some(2));
    assert_eq!(wm.active_pane_id(), 2);

    // Selectively raise pane #1
    assert!(wm.focus_and_raise(1));
    assert_eq!(wm.active_pane_id(), 1);

    // Hit-test pane #2 on the right half of the screen (col=60, row=5)
    let (hit_focus, _outer, inner) = wm.hit_test_pane(60, 5, screen).expect("hit right pane");
    assert_eq!(hit_focus, Focus::Pane(2));
    let sgr = encode_sgr_mouse_local(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 60,
            row: 5,
            modifiers: KeyModifiers::NONE,
        },
        inner,
    )
    .expect("sgr bytes");
    // Pane #2 outer starts at x=40, y=0; inner starts at x=41, y=1 -> local_col = 60 - 41 + 1 = 20, local_row = 5 - 1 + 1 = 5
    assert_eq!(String::from_utf8(sgr).unwrap(), "\x1b[<0;20;5M");
}

#[test]
fn reap_exited_zoomed_pane_clears_zoom_and_updates_tree_and_z_stack() {
    let mut wm = WindowManager::new_headless(25, 80);
    let screen = Rect::new(0, 0, 80, 25);

    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    assert_eq!(p2, 2);
    wm.toggle_zoom();
    assert_eq!(wm.zoomed_pane(), Some(2));

    wm.pane_mut(2)
        .unwrap()
        .ingest_pty_events(vec![crate::pty::PtyEvent::Exited(Some(0))]);
    wm.poll_all_ptys();

    assert_eq!(wm.pane_count(), 1);
    assert_eq!(wm.zoomed_pane(), None);
    assert_eq!(wm.active_pane_id(), 1);
    assert_eq!(wm.compute_workspace_rects(screen), vec![(1, screen)]);
}

#[test]
fn disconnect_fleet_node_removes_associated_ssh_pane_via_remove_pane() {
    let mut wm = WindowManager::new_headless(25, 80);

    let ssh_pid = wm
        .spawn_ssh_pane("ubuntu@10.0.0.42", None, vec![])
        .expect("spawn headless ssh pane");
    assert_eq!(ssh_pid, 2);
    wm.toggle_zoom();
    assert_eq!(wm.zoomed_pane(), Some(2));

    assert!(wm.disconnect_fleet_node("ubuntu@10.0.0.42-#2"));
    assert_eq!(wm.pane_count(), 1);
    assert_eq!(wm.zoomed_pane(), None);
    assert_eq!(wm.active_pane_id(), 1);
    assert!(wm.fleet().nodes.is_empty());
}

#[test]
fn modal_enum_enforces_mutual_exclusion_and_focus_avoids_zero_sentinel() {
    let mut wm = WindowManager::new_headless(25, 80);
    assert_eq!(wm.modal(), &Modal::None);
    assert_eq!(wm.focus(), Focus::Pane(1));
    assert_eq!(wm.active_pane_id(), 1);

    wm.toggle_help();
    assert!(wm.is_help_visible());
    assert!(!wm.is_inspector_visible());
    assert!(!wm.is_palette_visible());

    wm.toggle_inspector();
    assert!(!wm.is_help_visible());
    assert!(wm.is_inspector_visible());
    assert!(!wm.is_palette_visible());

    wm.toggle_palette();
    assert!(!wm.is_help_visible());
    assert!(!wm.is_inspector_visible());
    assert!(wm.is_palette_visible());

    wm.set_menubar_open_category(Some(2));
    assert!(!wm.is_palette_visible());
    assert_eq!(wm.menubar_open_category(), Some(2));

    wm.close_modal();
    assert_eq!(wm.modal(), &Modal::None);

    wm.toggle_quick_terminal();
    assert!(wm.is_quick_terminal_visible());
    assert_eq!(wm.focus(), Focus::Quick);
    assert_eq!(wm.active_pane_id(), 1);
}

#[test]
fn z_stack_and_tiled_windows_support_mouse_and_keyboard_move_and_resize() {
    use crate::colon_command::parse_colon_command;
    use crate::input::{handle_wm_key_event, handle_wm_mouse_event};
    use crossterm::event::{KeyCode, KeyEvent};

    let area = Rect::new(0, 0, 80, 24);
    let mut wm = WindowManager::new_headless(24, 80);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(area).unwrap();

    // 1. Tiled mode: keyboard resize widens active pane #2
    let rects_before = wm.compute_workspace_rects(area);
    let w2_before = rects_before.iter().find(|(id, _)| *id == 2).unwrap().1.width;
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(
            KeyCode::Right,
            KeyModifiers::CONTROL.union(KeyModifiers::ALT),
        ),
    )
    .unwrap();
    let rects_after = wm.compute_workspace_rects(area);
    let w2_after = rects_after.iter().find(|(id, _)| *id == 2).unwrap().1.width;
    assert!(w2_after > w2_before, "expected pane #2 to widen in Tiled mode");

    // 2. Tiled mode: mouse drag on pane #2's left border resizes the split ratio live
    let p2_rect = rects_after.iter().find(|(id, _)| *id == 2).unwrap().1;
    for (kind, col) in [
        (MouseEventKind::Down(MouseButton::Left), p2_rect.x),
        (MouseEventKind::Drag(MouseButton::Left), 24),
        (MouseEventKind::Up(MouseButton::Left), 24),
    ] {
        handle_wm_mouse_event(
            &mut wm,
            MouseEvent {
                kind,
                column: col,
                row: p2_rect.y + 5,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
    }
    let rects_dragged = wm.compute_workspace_rects(area);
    let p1_dragged = rects_dragged.iter().find(|(id, _)| *id == 1).unwrap().1;
    assert!(p1_dragged.width <= 26, "expected left split to shrink near col 24");

    // 3. Switch to Z-Stack mode: drag top title bar to move window, drag bottom-right corner to resize
    wm.set_layout_mode(WorkspaceLayoutMode::Stacked);
    wm.resize_all(area).unwrap();
    let z_before = wm.z_stack().rect_for(2, area).unwrap();

    for (kind, col, row) in [
        (MouseEventKind::Down(MouseButton::Left), z_before.x + 2, z_before.y),
        (MouseEventKind::Drag(MouseButton::Left), 12, 4),
        (MouseEventKind::Up(MouseButton::Left), 12, 4),
    ] {
        handle_wm_mouse_event(
            &mut wm,
            MouseEvent {
                kind,
                column: col,
                row,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
    }
    let z_moved = wm.z_stack().rect_for(2, area).unwrap();
    assert_eq!((z_moved.x, z_moved.y), (10, 4));

    let br_col = z_moved.x + z_moved.width - 1;
    let br_row = z_moved.y + z_moved.height - 1;
    for (kind, col, row) in [
        (MouseEventKind::Down(MouseButton::Left), br_col, br_row),
        (MouseEventKind::Drag(MouseButton::Left), z_moved.x + 39, z_moved.y + 11),
        (MouseEventKind::Up(MouseButton::Left), z_moved.x + 39, z_moved.y + 11),
    ] {
        handle_wm_mouse_event(
            &mut wm,
            MouseEvent {
                kind,
                column: col,
                row,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
    }
    let z_resized = wm.z_stack().rect_for(2, area).unwrap();
    assert_eq!((z_resized.width, z_resized.height), (40, 12));

    // Keyboard :move, :resize, and :cascade in Stacked mode
    let move_act = parse_colon_command(":move right 5").unwrap().unwrap();
    wm.apply(move_act).unwrap();
    assert_eq!(wm.z_stack().rect_for(2, area).unwrap().x, 15);

    let resize_act = parse_colon_command(":resize down 3").unwrap().unwrap();
    wm.apply(resize_act).unwrap();
    assert_eq!(wm.z_stack().rect_for(2, area).unwrap().height, 15);

    wm.apply(Action::WindowResetGeometry).unwrap();
    assert!(!wm.z_stack().has_any_custom_geometry());
    assert_eq!(wm.z_stack().rect_for(2, area).unwrap(), z_before);

    // Help modal (F1) resize, move, maximize, and pane minimize
    wm.toggle_help();
    let m0 = wm.active_modal_rect(area).unwrap();
    wm.resize_focused_pane_or_window(-12, -4);
    assert_eq!(wm.active_modal_rect(area).unwrap().width, m0.width - 12);
    wm.toggle_maximize_modal();
    assert_eq!(wm.active_modal_rect(area).unwrap(), area);
    wm.close_modal();

    wm.toggle_minimize_pane(2);
    assert!(wm.z_stack().is_minimized(2));
    assert_eq!(wm.z_stack().rect_for(2, area).unwrap().height, 3);
    wm.toggle_minimize_pane(2);
    assert!(!wm.z_stack().is_minimized(2));
}
