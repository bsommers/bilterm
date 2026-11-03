use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::wm::{WindowManager, WorkspaceLayoutMode};

fn run_cmd(wm: &mut WindowManager, cmd_str: &str) {
    let cmd = parse_colon_command(cmd_str)
        .unwrap_or_else(|| panic!("expected valid colon command: {cmd_str}"))
        .unwrap_or_else(|e| panic!("failed to parse {cmd_str}: {e}"));
    execute_wm_colon_command(wm, cmd);
}

#[test]
fn layout_autosave_is_enabled_by_default_and_can_be_toggled_off_and_on() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().session_dir = Some(tmp.path().to_string_lossy().into_owned());

    // 1. Autosave is enabled by default
    assert!(wm.is_layout_autosave_enabled());
    assert_eq!(wm.active_layout_profile(), "default");
    assert_eq!(wm.default_layout_profile(), "default");

    // 2. Splitting panes automatically saves to default.toml when autosave is ON
    run_cmd(&mut wm, ":split v");
    assert_eq!(wm.pane_count(), 2);
    assert!(tmp.path().join("default.toml").is_file());

    // 3. Turn off layout autosave via :layout autosave off
    run_cmd(&mut wm, ":layout autosave off");
    assert!(!wm.is_layout_autosave_enabled());

    // Further splits while autosave is OFF do not overwrite the 2-pane saved default profile
    run_cmd(&mut wm, ":split h");
    run_cmd(&mut wm, ":split v");
    assert_eq!(wm.pane_count(), 4);

    // A fresh WindowManager restoring the default profile sees the 2-pane layout saved before autosave was turned off
    let mut wm_restored = WindowManager::new_headless(24, 80);
    wm_restored.config_mut().session_dir = Some(tmp.path().to_string_lossy().into_owned());
    let (prof, count) = wm_restored
        .restore_default_layout_profile_on_startup()
        .expect("restore default profile");
    assert_eq!(prof, "default");
    assert_eq!(count, 2);
    // Persisted meta also remembers autosave was turned off
    assert!(!wm_restored.is_layout_autosave_enabled());

    // 4. Toggle autosave back ON via :autosave toggle -> immediately autosaves the 4-pane layout
    run_cmd(&mut wm, ":autosave toggle");
    assert!(wm.is_layout_autosave_enabled());

    let mut wm_restored_after_on = WindowManager::new_headless(24, 80);
    wm_restored_after_on.config_mut().session_dir =
        Some(tmp.path().to_string_lossy().into_owned());
    let (_, count_after) = wm_restored_after_on
        .restore_default_layout_profile_on_startup()
        .expect("restore updated default profile");
    assert_eq!(count_after, 4);
}

#[test]
fn multiple_layout_profiles_dynamic_switching_and_active_default_selection() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().session_dir = Some(tmp.path().to_string_lossy().into_owned());

    // 1. Create a custom 2-pane 'dev' profile
    run_cmd(&mut wm, ":split v");
    assert_eq!(wm.pane_count(), 2);
    run_cmd(&mut wm, ":profile save dev");
    assert_eq!(wm.active_layout_profile(), "dev");
    assert!(tmp.path().join("dev.toml").is_file());

    // 2. Dynamically switch to built-in 'quad' (4-pane 2x2 grid) profile
    run_cmd(&mut wm, ":profile switch quad");
    assert_eq!(wm.active_layout_profile(), "quad");
    assert_eq!(wm.pane_count(), 4);
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Tiled);

    // 3. Dynamically switch to built-in 'cascade' (3-pane Stacked Z-Order) profile
    run_cmd(&mut wm, ":layout switch cascade");
    assert_eq!(wm.active_layout_profile(), "cascade");
    assert_eq!(wm.pane_count(), 3);
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked);

    // 4. Dynamically switch back to 'dev' -> restores 2-pane Tiled layout
    run_cmd(&mut wm, ":profile switch dev");
    assert_eq!(wm.active_layout_profile(), "dev");
    assert_eq!(wm.pane_count(), 2);
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Tiled);

    // 5. Select 'quad' as the active default startup profile
    run_cmd(&mut wm, ":profile default quad");
    assert_eq!(wm.default_layout_profile(), "quad");
    assert_eq!(wm.active_layout_profile(), "quad");
    assert_eq!(wm.pane_count(), 4);

    // 6. Verify a newly launched WindowManager automatically boots into the user's selected default ('quad')
    let mut wm_startup = WindowManager::new_headless(24, 80);
    wm_startup.config_mut().session_dir = Some(tmp.path().to_string_lossy().into_owned());
    let (startup_prof, startup_panes) = wm_startup
        .restore_default_layout_profile_on_startup()
        .expect("startup restores user-selected default profile");
    assert_eq!(startup_prof, "quad");
    assert_eq!(startup_panes, 4);
    assert_eq!(wm_startup.pane_count(), 4);

    // 7. Cycle dynamically across profiles via :profile next and :profile prev
    run_cmd(&mut wm_startup, ":profile next");
    assert_eq!(wm_startup.active_layout_profile(), "cascade");
    assert_eq!(wm_startup.pane_count(), 3);

    run_cmd(&mut wm_startup, ":profile prev");
    assert_eq!(wm_startup.active_layout_profile(), "quad");
    assert_eq!(wm_startup.pane_count(), 4);
}

#[test]
fn mid_flight_auto_arrange_and_position_save_restore_preserve_live_pane_output() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().session_dir = Some(tmp.path().to_string_lossy().into_owned());
    let ws_area = ratatui::layout::Rect::new(0, 0, 80, 24);

    // Spawn 4 panes and inject unique live PTY output into each pane
    run_cmd(&mut wm, ":split v");
    run_cmd(&mut wm, ":split h");
    run_cmd(&mut wm, ":split v");
    assert_eq!(wm.pane_count(), 4);

    let pane_ids = wm.ordered_pane_ids();
    assert_eq!(pane_ids, vec![1, 2, 3, 4]);
    for &id in &pane_ids {
        let marker = format!("LIVE_PANE_{id}\r\n");
        wm.pane_mut(id)
            .expect("pane exists")
            .process_pty_output(marker.as_bytes());
    }

    let assert_all_panes_preserved = |wm: &WindowManager| {
        assert_eq!(wm.pane_count(), 4);
        for &id in &pane_ids {
            let contents = wm
                .pane(id)
                .expect("pane still open")
                .vt()
                .contents()
                .replace('\n', "");
            assert!(
                contents.contains(&format!("LIVE_PANE_{id}")),
                "pane #{id} live PTY output must be preserved in-place, got: {contents:?}"
            );
        }
    };

    // 1. :arrange columns -> 4 side-by-side vertical columns in Tiled mode
    run_cmd(&mut wm, ":arrange columns");
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Tiled);
    assert_all_panes_preserved(&wm);
    let col_rects = wm.split_tree().compute_rects(ws_area);
    assert_eq!(col_rects.len(), 4);
    for (_, r) in &col_rects {
        assert_eq!(r.y, ws_area.y);
        assert_eq!(r.height, ws_area.height);
    }
    assert!(col_rects[0].1.x < col_rects[1].1.x);
    assert!(col_rects[1].1.x < col_rects[2].1.x);
    assert!(col_rects[2].1.x < col_rects[3].1.x);

    // 2. :arrange rows -> 4 stacked horizontal rows in Tiled mode
    run_cmd(&mut wm, ":arrange rows");
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Tiled);
    assert_all_panes_preserved(&wm);
    let row_rects = wm.split_tree().compute_rects(ws_area);
    assert_eq!(row_rects.len(), 4);
    for (_, r) in &row_rects {
        assert_eq!(r.x, ws_area.x);
        assert_eq!(r.width, ws_area.width);
    }
    assert!(row_rects[0].1.y < row_rects[1].1.y);
    assert!(row_rects[1].1.y < row_rects[2].1.y);
    assert!(row_rects[2].1.y < row_rects[3].1.y);

    // 3. :arrange main-side -> main pane on left (50%), remaining 3 stacked horizontally on right
    run_cmd(&mut wm, ":arrange main-side");
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Tiled);
    assert_all_panes_preserved(&wm);
    let main_side_rects = wm.split_tree().compute_rects(ws_area);
    assert_eq!(main_side_rects.len(), 4);
    assert_eq!(main_side_rects[0].1.x, ws_area.x);
    assert_eq!(main_side_rects[0].1.width, ws_area.width / 2);
    assert_eq!(main_side_rects[0].1.height, ws_area.height);
    for (_, r) in &main_side_rects[1..] {
        assert_eq!(r.x, ws_area.x + ws_area.width / 2);
    }
    assert!(main_side_rects[1].1.y < main_side_rects[2].1.y);
    assert!(main_side_rects[2].1.y < main_side_rects[3].1.y);

    // 4. :arrange grid -> balanced 2x2 grid in Tiled mode
    run_cmd(&mut wm, ":arrange grid");
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Tiled);
    assert_all_panes_preserved(&wm);
    let grid_rects = wm.split_tree().compute_rects(ws_area);
    assert_eq!(grid_rects.len(), 4);
    assert_eq!(grid_rects[0].1.x, ws_area.x);
    assert_eq!(grid_rects[0].1.y, ws_area.y);
    assert_eq!(grid_rects[1].1.x, ws_area.x);
    assert!(grid_rects[1].1.y > ws_area.y);
    assert!(grid_rects[2].1.x > ws_area.x);
    assert_eq!(grid_rects[2].1.y, ws_area.y);
    assert!(grid_rects[3].1.x > ws_area.x);
    assert!(grid_rects[3].1.y > ws_area.y);

    // 5. :arrange cascade -> Stacked mode with clean Borland cascade (no custom geometries)
    run_cmd(&mut wm, ":arrange cascade");
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked);
    assert!(!wm.z_stack().has_any_custom_geometry());
    assert_all_panes_preserved(&wm);

    // 6. :arrange stack-grid -> Stacked mode with non-overlapping 2D grid rectangles
    run_cmd(&mut wm, ":arrange stack-grid");
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked);
    assert!(wm.z_stack().has_any_custom_geometry());
    assert_all_panes_preserved(&wm);
    let stack_grid_rects = wm.compute_workspace_rects(ws_area);
    assert_eq!(stack_grid_rects.len(), 4);
    for i in 0..stack_grid_rects.len() {
        for j in (i + 1)..stack_grid_rects.len() {
            let a = stack_grid_rects[i].1;
            let b = stack_grid_rects[j].1;
            let horiz_disjoint = a.x + a.width <= b.x || b.x + b.width <= a.x;
            let vert_disjoint = a.y + a.height <= b.y || b.y + b.height <= a.y;
            assert!(
                horiz_disjoint || vert_disjoint,
                "stack-grid windows must not overlap: {a:?} vs {b:?}"
            );
        }
    }

    // 7. Save mid-flight window positions via :layout save-pos, rearrange to columns, then restore-pos
    run_cmd(&mut wm, ":layout save-pos");
    run_cmd(&mut wm, ":arrange columns");
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Tiled);
    assert!(!wm.z_stack().has_any_custom_geometry());

    run_cmd(&mut wm, ":layout restore-pos");
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked);
    assert!(wm.z_stack().has_any_custom_geometry());
    assert_eq!(wm.compute_workspace_rects(ws_area), stack_grid_rects);
    assert_all_panes_preserved(&wm);
}
