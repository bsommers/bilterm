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
