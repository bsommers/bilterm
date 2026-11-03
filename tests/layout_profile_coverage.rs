//! Coverage for layout profiles (save/restore/arrange/switch/delete, corrupt files) and the
//! `SessionTree` data structure (round trips, edge cases).

use bilterm::wm::layout_profile::{BUILTIN_LAYOUT_PROFILES, LayoutProfileMeta, sanitize_profile_name};
use bilterm::wm::session_tree::{SessionNodeKind, SessionTree};
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::{WindowManager, WorkspaceLayoutMode};
use ratatui::layout::Rect;

const WS: Rect = Rect { x: 0, y: 0, width: 80, height: 24 };

fn wm_in(tmp: &tempfile::TempDir) -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().session_dir = Some(tmp.path().to_string_lossy().into_owned());
    wm
}

fn status(wm: &WindowManager) -> String {
    wm.active_pane().status_message().unwrap_or("").to_string()
}

fn with_panes(wm: &mut WindowManager, n: usize) {
    while wm.pane_count() < n {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
}

// ---------------------------------------------------------------- sanitize / meta

#[test]
fn sanitize_profile_name_strips_unsafe_characters_and_defaults_when_empty() {
    assert_eq!(sanitize_profile_name("  dev-1_x  "), "dev-1_x");
    assert_eq!(sanitize_profile_name("../../etc/passwd"), "etcpasswd");
    assert_eq!(sanitize_profile_name("a b/c.d"), "abcd");
    assert_eq!(sanitize_profile_name(""), "default");
    assert_eq!(sanitize_profile_name("   "), "default");
    assert_eq!(sanitize_profile_name("///"), "default");
    assert_eq!(sanitize_profile_name("héllo"), "hllo");
}

#[test]
fn layout_profile_meta_default_and_persisted_round_trip() {
    let m = LayoutProfileMeta::default();
    assert_eq!((m.default_profile.as_str(), m.active_profile.as_str(), m.autosave), ("default", "default", true));

    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    wm.config_mut().default_layout_profile = "alpha".into();
    wm.config_mut().active_layout_profile = "beta".into();
    wm.config_mut().layout_autosave = false;
    wm.persist_layout_profile_meta();
    let raw = std::fs::read_to_string(tmp.path().join("layout_profile_meta.toml")).unwrap();
    let parsed: LayoutProfileMeta = toml::from_str(&raw).unwrap();
    assert_eq!(parsed, LayoutProfileMeta { default_profile: "alpha".into(), active_profile: "beta".into(), autosave: false });

    let mut wm2 = wm_in(&tmp);
    let loaded = wm2.load_layout_profile_meta().expect("meta loads");
    assert_eq!(loaded, parsed);
    assert_eq!(wm2.default_layout_profile(), "alpha");
    assert_eq!(wm2.active_layout_profile(), "beta");
    assert!(!wm2.is_layout_autosave_enabled());
}

#[test]
fn load_meta_sanitizes_names_and_returns_none_for_missing_or_corrupt_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    assert!(wm.load_layout_profile_meta().is_none(), "missing file");
    assert_eq!(wm.active_layout_profile(), "default", "state untouched");

    std::fs::write(tmp.path().join("layout_profile_meta.toml"), "this is = = not toml [[").unwrap();
    assert!(wm.load_layout_profile_meta().is_none(), "corrupt file");
    std::fs::write(tmp.path().join("layout_profile_meta.toml"), "default_profile = \"x\"\n").unwrap();
    assert!(wm.load_layout_profile_meta().is_none(), "missing required fields");

    std::fs::write(
        tmp.path().join("layout_profile_meta.toml"),
        "default_profile = \"../evil\"\nactive_profile = \"\"\nautosave = true\n",
    )
    .unwrap();
    let meta = wm.load_layout_profile_meta().expect("loads");
    assert_eq!(meta.default_profile, "../evil", "raw value is returned as stored");
    assert_eq!(wm.default_layout_profile(), "evil");
    assert_eq!(wm.active_layout_profile(), "default");
}

#[test]
fn persist_meta_is_noop_for_headless_without_session_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().session_dir = None;
    wm.persist_layout_profile_meta();
    assert!(std::fs::read_dir(tmp.path()).unwrap().next().is_none());
}

// ---------------------------------------------------------------- autosave

#[test]
fn maybe_autosave_returns_false_when_disabled_and_true_when_enabled() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    assert!(!wm.set_layout_autosave(Some(false)));
    assert!(!wm.maybe_autosave_layout());
    assert!(!wm.autosave_active_layout_profile());
    assert!(!tmp.path().join("default.toml").exists());
    assert!(wm.set_layout_autosave(Some(true)), "enabling autosaves immediately");
    assert!(tmp.path().join("default.toml").is_file());
    assert!(wm.autosave_active_layout_profile());
    // None toggles
    assert!(!wm.set_layout_autosave(None));
    assert!(wm.set_layout_autosave(None));
    let meta = std::fs::read_to_string(tmp.path().join("layout_profile_meta.toml")).unwrap();
    assert!(meta.contains("autosave = true"));
}

#[test]
fn autosave_without_session_dir_in_headless_stays_in_memory() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().session_dir = None;
    assert!(wm.maybe_autosave_layout());
    assert!(wm.has_saved_layout_profile("default"), "kept in memory");
}

#[test]
fn autosave_sanitizes_a_dirty_active_profile_name() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    wm.config_mut().active_layout_profile = "we ird/na.me".into();
    assert!(wm.maybe_autosave_layout());
    assert_eq!(wm.active_layout_profile(), "weirdname");
    assert!(tmp.path().join("weirdname.toml").is_file());
}

// ---------------------------------------------------------------- save / restore

#[test]
fn save_layout_profile_uses_explicit_name_then_active_name_and_writes_toml() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    with_panes(&mut wm, 3);
    assert_eq!(wm.save_layout_profile(Some(" my prof! ")).unwrap(), "myprof");
    assert_eq!(wm.active_layout_profile(), "myprof");
    let raw = std::fs::read_to_string(tmp.path().join("myprof.toml")).unwrap();
    assert!(raw.contains("layout_mode"));
    assert_eq!(raw.matches("[[panes]]").count(), 3, "three panes persisted:\n{raw}");
    // blank name falls back to the active profile
    assert_eq!(wm.save_layout_profile(Some("   ")).unwrap(), "myprof");
    assert_eq!(wm.save_layout_profile(None).unwrap(), "myprof");
    assert!(wm.has_saved_layout_profile("myprof"));
    assert!(!wm.has_saved_layout_profile("nothere"));
    assert!(wm.has_saved_layout_profile("my prof"), "name is sanitized before lookup");
}

#[test]
fn switch_to_saved_profile_restores_pane_count_and_mode_from_disk_after_memory_loss() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    with_panes(&mut wm, 3);
    wm.auto_arrange_windows("cascade");
    wm.save_layout_profile(Some("deck")).unwrap();

    let mut fresh = wm_in(&tmp);
    assert_eq!(fresh.pane_count(), 1);
    assert_eq!(fresh.switch_layout_profile("deck").unwrap(), 3);
    assert_eq!(fresh.layout_mode(), WorkspaceLayoutMode::Stacked);
    assert_eq!(fresh.active_layout_profile(), "deck");
}

#[test]
fn switch_to_unknown_profile_snapshots_current_layout_under_new_name() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    with_panes(&mut wm, 2);
    assert_eq!(wm.switch_layout_profile("brand-new").unwrap(), 2);
    assert_eq!(wm.active_layout_profile(), "brand-new");
    assert!(tmp.path().join("brand-new.toml").is_file());
    assert_eq!(wm.pane_count(), 2);
}

#[test]
fn switch_autosaves_outgoing_profile_only_when_enabled() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    wm.set_layout_autosave(Some(false));
    with_panes(&mut wm, 2);
    wm.switch_layout_profile("other").unwrap();
    assert!(!tmp.path().join("default.toml").exists(), "outgoing not saved with autosave off");

    let tmp2 = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp2);
    with_panes(&mut wm, 2);
    wm.switch_layout_profile("other").unwrap();
    assert!(tmp2.path().join("default.toml").is_file(), "outgoing saved with autosave on");
}

#[test]
fn builtin_templates_produce_expected_pane_counts_modes_and_geometry() {
    let cases: &[(&str, usize, WorkspaceLayoutMode)] = &[
        ("single", 1, WorkspaceLayoutMode::Tiled),
        ("dual-v", 2, WorkspaceLayoutMode::Tiled),
        ("dual-h", 2, WorkspaceLayoutMode::Tiled),
        ("main-side", 3, WorkspaceLayoutMode::Tiled),
        ("quad", 4, WorkspaceLayoutMode::Tiled),
        ("cascade", 3, WorkspaceLayoutMode::Stacked),
    ];
    for (name, panes, mode) in cases {
        let tmp = tempfile::tempdir().unwrap();
        let mut wm = wm_in(&tmp);
        assert_eq!(wm.switch_layout_profile(name).unwrap(), *panes, "{name}");
        assert_eq!(wm.pane_count(), *panes, "{name}");
        assert_eq!(wm.layout_mode(), *mode, "{name}");
        assert_eq!(wm.active_layout_profile(), *name);
        assert!(tmp.path().join(format!("{name}.toml")).is_file(), "{name} saved by autosave");
    }
    // geometry checks
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    wm.switch_layout_profile("dual-v").unwrap();
    let r = wm.split_tree().compute_rects(WS);
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].1.y, r[1].1.y, "side by side");
    assert!(r[0].1.x < r[1].1.x);
    wm.switch_layout_profile("dual-h").unwrap();
    let r = wm.split_tree().compute_rects(WS);
    assert_eq!(r[0].1.x, r[1].1.x, "stacked");
    assert!(r[0].1.y < r[1].1.y);
    wm.switch_layout_profile("quad").unwrap();
    assert_eq!(wm.split_tree().compute_rects(WS).len(), 4);
    wm.switch_layout_profile("single").unwrap();
    assert_eq!(wm.pane_count(), 1, "single trims extra panes");
    assert_eq!(wm.split_tree().compute_rects(WS)[0].1, WS);
}

#[test]
fn builtin_template_without_autosave_is_not_written_to_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    wm.set_layout_autosave(Some(false));
    wm.switch_layout_profile("quad").unwrap();
    assert_eq!(wm.pane_count(), 4);
    assert!(!tmp.path().join("quad.toml").exists());
}

#[test]
fn corrupt_profile_file_makes_switch_fail_and_leaves_workspace_intact() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    with_panes(&mut wm, 2);
    std::fs::write(tmp.path().join("broken.toml"), "panes = [[[ garbage").unwrap();
    assert!(wm.switch_layout_profile("broken").is_err());
    assert_eq!(wm.pane_count(), 2, "panes preserved after failed restore");
    wm.execute_layout_profile_command("switch broken");
    assert!(status(&wm).starts_with("Profile switch error:"), "{}", status(&wm));
}

#[test]
fn restore_default_on_startup_none_without_saved_profile_and_with_builtin_default() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    assert!(wm.restore_default_layout_profile_on_startup().is_none(), "nothing saved yet");

    std::fs::write(
        tmp.path().join("layout_profile_meta.toml"),
        "default_profile = \"quad\"\nactive_profile = \"quad\"\nautosave = false\n",
    )
    .unwrap();
    let (name, n) = wm.restore_default_layout_profile_on_startup().expect("builtin default");
    assert_eq!((name.as_str(), n), ("quad", 4));
    assert!(!wm.is_layout_autosave_enabled());
}

#[test]
fn restore_default_on_startup_with_corrupt_default_profile_returns_none() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    std::fs::write(tmp.path().join("default.toml"), "{{{{").unwrap();
    assert!(wm.restore_default_layout_profile_on_startup().is_none());
    assert_eq!(wm.pane_count(), 1);
}

// ---------------------------------------------------------------- default / list / delete / cycle

#[test]
fn set_default_saves_unknown_custom_profile_but_not_builtin_templates() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    with_panes(&mut wm, 2);
    assert_eq!(wm.set_default_layout_profile(Some("work")).unwrap(), "work");
    assert_eq!(wm.default_layout_profile(), "work");
    assert!(tmp.path().join("work.toml").is_file());
    assert_eq!(wm.set_default_layout_profile(Some("quad")).unwrap(), "quad");
    assert!(!tmp.path().join("quad.toml").exists(), "builtin template stays virtual");
    // None uses active profile
    assert_eq!(wm.set_default_layout_profile(None).unwrap(), wm.active_layout_profile());
    let meta = std::fs::read_to_string(tmp.path().join("layout_profile_meta.toml")).unwrap();
    assert!(meta.contains(&format!("default_profile = \"{}\"", wm.default_layout_profile())));
}

#[test]
fn list_profiles_contains_builtins_saved_and_active_in_stable_order() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    wm.save_layout_profile(Some("zeta")).unwrap();
    let list = wm.list_layout_profiles();
    let builtin: Vec<String> = BUILTIN_LAYOUT_PROFILES.iter().map(|(n, _)| n.to_string()).collect();
    assert_eq!(&list[..builtin.len()], builtin.as_slice());
    assert!(list.contains(&"zeta".to_string()));
    assert!(!list.contains(&"layout_profile_meta".to_string()));
    assert_eq!(list.iter().filter(|n| n.as_str() == "zeta").count(), 1, "no duplicates");

    wm.config_mut().default_layout_profile = "ghost-default".into();
    wm.config_mut().active_layout_profile = "ghost-active".into();
    let list = wm.list_layout_profiles();
    assert_eq!(list[0], "ghost-default", "unknown default inserted first");
    assert_eq!(list.last().unwrap(), "ghost-active", "unknown active appended last");
}

#[test]
fn delete_layout_profile_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    assert_eq!(wm.delete_layout_profile("default").unwrap_err(), "Cannot delete the 'default' layout profile");
    assert_eq!(wm.delete_layout_profile("nope").unwrap_err(), "Layout profile 'nope' not found");
    wm.save_layout_profile(Some("tmpx")).unwrap();
    wm.set_default_layout_profile(Some("tmpx")).unwrap();
    assert!(tmp.path().join("tmpx.toml").is_file());
    assert_eq!(wm.delete_layout_profile("tmpx").unwrap(), "tmpx");
    assert!(!tmp.path().join("tmpx.toml").exists());
    assert_eq!(wm.default_layout_profile(), "default", "default falls back");
    assert_eq!(wm.active_layout_profile(), "default", "active falls back");
    assert!(!wm.has_saved_layout_profile("tmpx"));
    // disk-only deletion
    std::fs::write(tmp.path().join("disk-only.toml"), "x = 1").unwrap();
    assert_eq!(wm.delete_layout_profile("disk-only").unwrap(), "disk-only");
    assert!(!tmp.path().join("disk-only.toml").exists());
}

#[test]
fn cycle_profiles_wraps_in_both_directions() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    let list = wm.list_layout_profiles();
    let (prev, _) = wm.cycle_layout_profile(false).unwrap();
    assert_eq!(prev, *list.last().unwrap(), "prev from first wraps to last");
    let (next, _) = wm.cycle_layout_profile(true).unwrap();
    assert_eq!(next, list[0], "next from last wraps to first");
}

// ---------------------------------------------------------------- arrange

fn assert_disjoint(rects: &[(usize, Rect)]) {
    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            let (a, b) = (rects[i].1, rects[j].1);
            assert!(
                a.x + a.width <= b.x || b.x + b.width <= a.x || a.y + a.height <= b.y || b.y + b.height <= a.y,
                "{a:?} overlaps {b:?}"
            );
        }
    }
}

#[test]
fn arrange_with_one_pane_fills_workspace_for_every_pattern() {
    for pat in ["columns", "rows", "main-side", "main-top", "grid", "tile", "", "stack", "stack-grid", "bogus"] {
        let tmp = tempfile::tempdir().unwrap();
        let mut wm = wm_in(&tmp);
        assert_eq!(wm.auto_arrange_windows(pat), 1, "pattern {pat:?}");
        assert_eq!(wm.pane_count(), 1);
        let r = wm.compute_workspace_rects(WS);
        assert_eq!(r.len(), 1, "{pat}");
    }
}

#[test]
fn arrange_aliases_and_case_insensitivity() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    with_panes(&mut wm, 3);
    for alias in ["COLUMNS", "cols", "vertical", "v", "side-by-side"] {
        wm.auto_arrange_windows(alias);
        let r = wm.split_tree().compute_rects(WS);
        assert!(r.iter().all(|(_, rc)| rc.y == 0 && rc.height == WS.height), "{alias}");
    }
    for alias in ["rows", "horizontal", "h", "top-bottom"] {
        wm.auto_arrange_windows(alias);
        let r = wm.split_tree().compute_rects(WS);
        assert!(r.iter().all(|(_, rc)| rc.x == 0 && rc.width == WS.width), "{alias}");
    }
    for alias in ["stack", "stacked", "cascade", "deck"] {
        wm.auto_arrange_windows(alias);
        assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked, "{alias}");
        assert!(!wm.z_stack().has_any_custom_geometry(), "{alias}");
    }
    for alias in ["stack-grid", "float-grid"] {
        wm.auto_arrange_windows(alias);
        assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked, "{alias}");
        assert!(wm.z_stack().has_any_custom_geometry(), "{alias}");
        assert_disjoint(&wm.compute_workspace_rects(WS));
    }
}

#[test]
fn arrange_main_top_puts_main_pane_on_top_and_rest_side_by_side_below() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    with_panes(&mut wm, 3);
    wm.auto_arrange_windows("main-top");
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Tiled);
    let r = wm.split_tree().compute_rects(WS);
    assert_eq!(r[0].1.y, 0);
    assert_eq!(r[0].1.width, WS.width);
    assert!(r[1].1.y > 0 && r[2].1.y > 0 && r[1].1.y == r[2].1.y);
    assert!(r[1].1.x < r[2].1.x);
    let alias_tree = {
        wm.auto_arrange_windows("main-horizontal");
        wm.split_tree().compute_rects(WS)
    };
    assert_eq!(alias_tree, r);
}

#[test]
fn arrange_clears_zoom_and_autosaves_to_active_profile() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    with_panes(&mut wm, 2);
    wm.toggle_zoom();
    assert!(wm.zoomed_pane().is_some());
    wm.auto_arrange_windows("rows");
    assert_eq!(wm.zoomed_pane(), None);
    let raw = std::fs::read_to_string(tmp.path().join("default.toml")).unwrap();
    assert!(raw.contains("layout_mode = \"tiled\"") || raw.to_lowercase().contains("tiled"), "{raw}");

    wm.set_layout_autosave(Some(false));
    let _ = std::fs::remove_file(tmp.path().join("default.toml"));
    wm.auto_arrange_windows("columns");
    assert!(!tmp.path().join("default.toml").exists(), "no autosave when disabled");
}

#[test]
fn arrange_with_many_panes_has_no_overlap_in_grid_and_stack_grid() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = WindowManager::new_headless(40, 120);
    wm.config_mut().session_dir = Some(tmp.path().to_string_lossy().into_owned());
    with_panes(&mut wm, 7);
    let ws = Rect::new(0, 0, 120, 40);
    wm.auto_arrange_windows("grid");
    let r = wm.split_tree().compute_rects(ws);
    assert_eq!(r.len(), 7);
    assert_disjoint(&r);
    wm.auto_arrange_windows("stack-grid");
    assert_eq!(wm.compute_workspace_rects(ws).len(), 7);
    assert_disjoint(&wm.compute_workspace_rects(ws));
}

// ---------------------------------------------------------------- window positions

#[test]
fn save_window_positions_with_explicit_name_keeps_active_profile() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    with_panes(&mut wm, 3);
    wm.auto_arrange_windows("stack-grid");
    assert_eq!(wm.save_window_positions(Some("snap1")).unwrap(), "snap1");
    assert_eq!(wm.active_layout_profile(), "default", "explicit name does not hijack active profile");
    assert!(tmp.path().join("snap1.toml").is_file());
    // blank name saves under active
    assert_eq!(wm.save_window_positions(None).unwrap(), "default");
    assert_eq!(wm.save_window_positions(Some("  ")).unwrap(), "default");
}

#[test]
fn restore_window_positions_from_named_disk_profile_and_missing_file_error() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    with_panes(&mut wm, 3);
    wm.auto_arrange_windows("stack-grid");
    let before = wm.compute_workspace_rects(WS);
    wm.save_window_positions(Some("snap")).unwrap();

    // Fresh manager with the same pane ids, no memory of the snapshot: loads from disk
    let mut other = wm_in(&tmp);
    with_panes(&mut other, 3);
    other.auto_arrange_windows("columns");
    assert_eq!(other.restore_window_positions(Some("snap")).unwrap(), 3);
    assert_eq!(other.layout_mode(), WorkspaceLayoutMode::Stacked);
    assert_eq!(other.compute_workspace_rects(WS), before);

    assert!(other.restore_window_positions(Some("never-saved")).is_err());
    std::fs::write(tmp.path().join("bad.toml"), "not = [valid").unwrap();
    assert!(other.restore_window_positions(Some("bad")).is_err());
    assert_eq!(other.pane_count(), 3);
}

#[test]
fn restore_window_positions_with_pane_count_mismatch_rebuilds_balanced_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    with_panes(&mut wm, 2);
    wm.save_window_positions(Some("two")).unwrap();
    with_panes(&mut wm, 4);
    assert_eq!(wm.restore_window_positions(Some("two")).unwrap(), 4);
    let leaves = wm.split_tree().leaves();
    assert_eq!(leaves.len(), 4, "tree rebuilt over all live panes");
    let mut sorted = leaves.clone();
    sorted.sort();
    assert_eq!(sorted, vec![1, 2, 3, 4]);
}

#[test]
fn restore_window_positions_default_uses_last_saved_snapshot() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    with_panes(&mut wm, 3);
    wm.auto_arrange_windows("stack-grid");
    wm.save_layout_profile(None).unwrap();
    wm.auto_arrange_windows("columns");
    wm.set_layout_autosave(Some(false));
    assert_eq!(wm.restore_window_positions(None).unwrap(), 3);
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked);
}

#[test]
fn restore_window_positions_without_any_saved_profile_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    assert!(wm.restore_window_positions(None).is_err());
}

// ---------------------------------------------------------------- command dispatcher

#[test]
fn profile_command_list_reports_active_default_tags_and_autosave_state() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    wm.execute_layout_profile_command("list");
    let s = status(&wm);
    assert!(s.starts_with("Layout Profiles (autosave=ON): "), "{s}");
    assert!(s.contains("default[ACTIVE,DEFAULT]"), "{s}");
    assert!(s.contains(" | quad"), "{s}");
    wm.set_layout_autosave(Some(false));
    for alias in ["ls", "profiles", "status", ""] {
        wm.execute_layout_profile_command(alias);
        assert!(status(&wm).contains("autosave=OFF"), "{alias}");
    }
}

#[test]
fn profile_command_autosave_arguments() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    for (arg, expect) in [("off", false), ("on", true), ("0", false), ("1", true), ("disable", false), ("enabled", true), ("false", false), ("true", true), ("disabled", false), ("enable", true)] {
        wm.execute_layout_profile_command(&format!("autosave {arg}"));
        assert_eq!(wm.is_layout_autosave_enabled(), expect, "{arg}");
        assert!(status(&wm).contains(if expect { "autosave: ON" } else { "autosave: OFF" }), "{}", status(&wm));
    }
    wm.execute_layout_profile_command("auto");
    assert!(!wm.is_layout_autosave_enabled(), "no arg toggles");
    wm.execute_layout_profile_command("auto toggle");
    assert!(wm.is_layout_autosave_enabled());
}

#[test]
fn profile_command_save_pos_restore_pos_arrange_and_direct_patterns() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    with_panes(&mut wm, 3);
    wm.execute_layout_profile_command("save mine");
    assert!(status(&wm).starts_with("Saved layout profile 'mine' (3 pane(s), mode=Tiled)"), "{}", status(&wm));
    wm.execute_layout_profile_command("save");
    assert!(status(&wm).contains("'mine'"));
    wm.execute_layout_profile_command("snapshot posA");
    assert!(status(&wm).starts_with("Saved mid-flight window positions to 'posA'"), "{}", status(&wm));
    wm.execute_layout_profile_command("arrange cascade");
    assert_eq!(status(&wm), "Auto-arranged 3 window(s) as 'cascade' (mode=Stacked)");
    wm.execute_layout_profile_command("arrange");
    assert_eq!(status(&wm), "Auto-arranged 3 window(s) as 'grid' (mode=Tiled)");
    for pat in ["grid", "equal", "columns", "cols", "vertical", "rows", "horizontal", "main-side", "main-vertical", "ide", "main-top", "main-horizontal", "stack-grid", "float-grid"] {
        wm.execute_layout_profile_command(pat);
        assert!(status(&wm).starts_with(&format!("Auto-arranged 3 window(s) as '{pat}'")), "{pat}: {}", status(&wm));
    }
    wm.execute_layout_profile_command("apply-pos posA");
    assert!(status(&wm).starts_with("Restored mid-flight window positions (3 live pane(s) preserved"), "{}", status(&wm));
    wm.execute_layout_profile_command("restore-pos missing-one");
    assert!(status(&wm).starts_with("Restore window positions error:"), "{}", status(&wm));
}

#[test]
fn profile_command_default_switch_next_prev_delete_and_fallthrough() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(&tmp);
    wm.execute_layout_profile_command("switch");
    assert!(status(&wm).starts_with("Usage: :profile switch <name>"));
    wm.execute_layout_profile_command("delete");
    assert_eq!(status(&wm), "Usage: :profile delete <name>");
    wm.execute_layout_profile_command("delete default");
    assert_eq!(status(&wm), "Cannot delete the 'default' layout profile");
    wm.execute_layout_profile_command("rm ghost");
    assert_eq!(status(&wm), "Layout profile 'ghost' not found");

    wm.execute_layout_profile_command("set-default dual-v");
    assert!(status(&wm).starts_with("Default layout profile set to 'dual-v' (active='dual-v', autosave=ON)"), "{}", status(&wm));
    assert_eq!(wm.pane_count(), 2);

    wm.execute_layout_profile_command("use quad");
    assert!(status(&wm).starts_with("Switched to layout profile 'quad' (4 pane(s)"), "{}", status(&wm));
    wm.execute_layout_profile_command("next");
    assert!(status(&wm).starts_with("Switched to next layout profile 'cascade' (3 pane(s), mode=Stacked)"), "{}", status(&wm));
    wm.execute_layout_profile_command("previous");
    assert!(status(&wm).starts_with("Switched to previous layout profile 'quad'"), "{}", status(&wm));
    wm.execute_layout_profile_command("prev");
    assert!(status(&wm).contains("previous layout profile 'main-side'"), "{}", status(&wm));

    // unknown head = implicit switch (creates a new profile from current layout)
    wm.execute_layout_profile_command("freshname");
    assert!(status(&wm).starts_with("Switched to layout profile 'freshname'"), "{}", status(&wm));
    assert!(tmp.path().join("freshname.toml").is_file());
    wm.execute_layout_profile_command("remove freshname");
    assert!(status(&wm).starts_with("Deleted layout profile 'freshname'"), "{}", status(&wm));
    assert!(!tmp.path().join("freshname.toml").exists());
}

// ---------------------------------------------------------------- session tree

#[test]
fn new_tree_has_single_rooted_main() {
    let t = SessionTree::default();
    assert_eq!(t.nodes.len(), 1);
    let main = t.node("main").unwrap();
    assert_eq!(main.kind, SessionNodeKind::Rooted);
    assert!(main.parent.is_none() && main.children.is_empty());
    assert_eq!((main.pane_count, main.active_commands), (1, 0));
    assert!(!main.collapsed && !main.frozen);
    assert!(t.node("ghost").is_none());
}

#[test]
fn upsert_session_ignores_blank_trims_creates_missing_parent_and_relinks() {
    let mut t = SessionTree::new();
    t.upsert_session("   ", SessionNodeKind::Independent, None);
    assert_eq!(t.nodes.len(), 1);
    t.upsert_session("  a  ", SessionNodeKind::Independent, None);
    assert_eq!(t.node("a").unwrap().kind, SessionNodeKind::Independent);
    t.upsert_session("b", SessionNodeKind::Independent, Some("newroot"));
    assert_eq!(t.node("newroot").unwrap().kind, SessionNodeKind::Rooted, "missing parent auto-created");
    assert_eq!(t.node("newroot").unwrap().children, vec!["b".to_string()]);
    assert_eq!(t.node("b").unwrap().kind, SessionNodeKind::Subtree, "parent forces Subtree");
    // moving b under a removes it from the old parent's children
    t.upsert_session("b", SessionNodeKind::Independent, Some("a"));
    assert!(t.node("newroot").unwrap().children.is_empty());
    assert_eq!(t.node("a").unwrap().children, vec!["b".to_string()]);
    // re-upserting under the same parent does not duplicate
    t.upsert_session("b", SessionNodeKind::Independent, Some("a"));
    assert_eq!(t.node("a").unwrap().children, vec!["b".to_string()]);
    // upsert as root keeps the requested kind
    t.upsert_session("b", SessionNodeKind::Rooted, None);
    assert_eq!(t.node("b").unwrap().kind, SessionNodeKind::Rooted);
    assert_eq!(t.node("b").unwrap().parent, None);
}

#[test]
fn branch_rejects_empty_self_and_cycles() {
    let mut t = SessionTree::new();
    assert_eq!(t.branch("", "x").unwrap_err(), "session names must not be empty");
    assert_eq!(t.branch("x", "  ").unwrap_err(), "session names must not be empty");
    assert_eq!(t.branch("x", "x").unwrap_err(), "cannot branch session 'x' under itself");
    t.branch("main", "a").unwrap();
    t.branch("a", "b").unwrap();
    let err = t.branch("b", "main").unwrap_err();
    assert_eq!(err, "cycle detected: 'b' is already a descendant of 'main'");
    let err = t.branch("b", "a").unwrap_err();
    assert!(err.starts_with("cycle detected"), "{err}");
    // unknown parent is created
    t.branch("fresh", "kid").unwrap();
    assert_eq!(t.node("fresh").unwrap().kind, SessionNodeKind::Rooted);
    assert_eq!(t.node("kid").unwrap().parent.as_deref(), Some("fresh"));
}

#[test]
fn graft_errors_and_success_relink() {
    let mut t = SessionTree::new();
    t.branch("main", "a").unwrap();
    t.branch("a", "a1").unwrap();
    t.upsert_session("other", SessionNodeKind::Independent, None);
    assert_eq!(t.graft("", "main").unwrap_err(), "session names must not be empty");
    assert_eq!(t.graft("a", "a").unwrap_err(), "cannot graft session 'a' onto itself");
    assert_eq!(t.graft("zzz", "main").unwrap_err(), "source session 'zzz' does not exist");
    assert_eq!(t.graft("a", "zzz").unwrap_err(), "destination parent session 'zzz' does not exist");
    assert_eq!(
        t.graft("a", "a1").unwrap_err(),
        "cycle detected: cannot graft 'a' under its own descendant 'a1'"
    );
    t.graft("a", "other").unwrap();
    assert!(t.node("main").unwrap().children.is_empty(), "removed from old parent");
    assert_eq!(t.node("other").unwrap().children, vec!["a".to_string()]);
    assert_eq!(t.node("a").unwrap().parent.as_deref(), Some("other"));
    assert_eq!(t.node("a").unwrap().kind, SessionNodeKind::Subtree);
    assert_eq!(t.subtree_session_names("other"), vec!["other", "a", "a1"]);
    // re-grafting under the current parent is reported, not silently accepted
    assert_eq!(
        t.graft("a", "other").unwrap_err(),
        "session 'a' is already a child of 'other'"
    );
    assert_eq!(t.node("other").unwrap().children.len(), 1);
}

#[test]
fn detach_subtree_promotes_to_independent_keeping_descendants() {
    let mut t = SessionTree::new();
    t.branch("main", "a").unwrap();
    t.branch("a", "a1").unwrap();
    assert_eq!(t.detach_subtree("ghost").unwrap_err(), "session 'ghost' does not exist");
    t.detach_subtree(" a ").unwrap();
    assert!(t.node("main").unwrap().children.is_empty());
    assert_eq!(t.node("a").unwrap().kind, SessionNodeKind::Independent);
    assert_eq!(t.node("a").unwrap().parent, None);
    assert_eq!(t.subtree_session_names("a"), vec!["a", "a1"]);
    // detaching an existing root is fine and keeps it Independent
    t.detach_subtree("main").unwrap();
    assert_eq!(t.node("main").unwrap().kind, SessionNodeKind::Independent);
}

#[test]
fn move_node_among_siblings_and_roots_with_boundaries() {
    let mut t = SessionTree::new();
    t.branch("main", "a").unwrap();
    t.branch("main", "b").unwrap();
    t.branch("main", "c").unwrap();
    assert!(!t.move_node("ghost", true));
    assert!(!t.move_node("a", true), "first child can't go up");
    assert!(!t.move_node("c", false), "last child can't go down");
    assert!(t.move_node("a", false));
    assert_eq!(t.node("main").unwrap().children, vec!["b", "a", "c"]);
    assert!(t.move_node(" c ", true));
    assert_eq!(t.node("main").unwrap().children, vec!["b", "c", "a"]);

    // root ordering
    t.upsert_session("r2", SessionNodeKind::Independent, None);
    t.upsert_session("r3", SessionNodeKind::Independent, None);
    let roots = |t: &SessionTree| -> Vec<String> {
        t.nodes.iter().filter(|n| n.parent.is_none()).map(|n| n.name.clone()).collect()
    };
    assert_eq!(roots(&t), vec!["main", "r2", "r3"]);
    assert!(!t.move_node("main", true));
    assert!(!t.move_node("r3", false));
    assert!(t.move_node("r2", true));
    assert_eq!(roots(&t), vec!["r2", "main", "r3"]);
    assert!(t.move_node("r2", false));
    assert_eq!(roots(&t), vec!["main", "r2", "r3"]);
}

#[test]
fn remove_session_reparents_children_to_grandparent_or_root() {
    let mut t = SessionTree::new();
    t.branch("main", "a").unwrap();
    t.branch("a", "a1").unwrap();
    t.branch("a", "a2").unwrap();
    assert!(!t.remove_session("ghost"));
    assert!(t.remove_session("a"));
    assert!(t.node("a").is_none());
    assert_eq!(t.node("main").unwrap().children, vec!["a1", "a2"]);
    assert_eq!(t.node("a1").unwrap().parent.as_deref(), Some("main"));
    // removing a root orphan-promotes children to Independent
    assert!(t.remove_session("main"));
    let a1 = t.node("a1").unwrap();
    assert_eq!(a1.parent, None);
    assert_eq!(a1.kind, SessionNodeKind::Independent);
    assert!(t.remove_session("a1") && t.remove_session("a2"));
    assert!(t.nodes.is_empty(), "tree may become empty");
    assert!(t.subtree_session_names("main").is_empty());
    assert_eq!(t.subtree_rollup("main"), (0, 0));
}

#[test]
fn subtree_names_are_preorder_and_survive_cycles_in_corrupt_data() {
    let mut t = SessionTree::new();
    t.branch("main", "a").unwrap();
    t.branch("main", "b").unwrap();
    t.branch("a", "a1").unwrap();
    assert_eq!(t.subtree_session_names("main"), vec!["main", "a", "a1", "b"]);
    assert!(t.subtree_session_names("nope").is_empty());
    // hand-corrupt: a's children include main
    t.node_mut("a").unwrap().children.push("main".into());
    assert_eq!(t.subtree_session_names("main"), vec!["main", "a", "a1", "b"], "visited-set breaks cycle");
}

#[test]
fn fold_freeze_and_rollup() {
    let mut t = SessionTree::new();
    t.branch("main", "a").unwrap();
    t.branch("a", "a1").unwrap();
    t.node_mut("main").unwrap().pane_count = 2;
    t.node_mut("a").unwrap().pane_count = 3;
    t.node_mut("a").unwrap().active_commands = 1;
    t.node_mut("a1").unwrap().active_commands = 4;
    assert_eq!(t.subtree_rollup("main"), (2 + 3 + 1, 5));
    assert_eq!(t.subtree_rollup("a1"), (1, 4));
    assert_eq!(t.subtree_rollup("ghost"), (0, 0));

    assert!(t.toggle_fold("a"));
    assert!(!t.toggle_fold("a"));
    assert!(!t.toggle_fold("ghost"));

    let hit = t.set_subtree_frozen("a", true);
    assert_eq!(hit, vec!["a", "a1"]);
    assert!(t.node("a1").unwrap().frozen && !t.node("main").unwrap().frozen);
    assert_eq!(t.set_subtree_frozen("a", false), vec!["a", "a1"]);
    assert!(!t.node("a").unwrap().frozen);
    assert!(t.set_subtree_frozen("ghost", true).is_empty());
}

#[test]
fn herdr_tree_rendering_exact_lines() {
    let mut t = SessionTree::new();
    assert_eq!(
        t.render_herdr_tree_lines("main"),
        vec!["◈ ROOT [MCP — MASTER CONTROL PROGRAM]", "╰─ • main* [MCP] (1p/0r)"]
    );
    t.branch("main", "a").unwrap();
    t.branch("main", "b").unwrap();
    t.upsert_session("solo", SessionNodeKind::Independent, None);
    t.node_mut("b").unwrap().frozen = true;
    t.node_mut("a").unwrap().active_commands = 2;
    let lines = t.render_herdr_tree_lines("a");
    assert_eq!(
        lines,
        vec![
            "◈ ROOT [MCP — MASTER CONTROL PROGRAM]".to_string(),
            "├─ ▾ main [MCP] (subtree: 3p/2r)".to_string(),
            "│  ├─ • a* [SUB] (1p/2r)".to_string(),
            "│  ╰─ • b [SUB] [❄ FROZEN] (1p/0r)".to_string(),
            "╰─ • solo [IND] (1p/0r)".to_string(),
        ]
    );
    // collapsed parent hides children and shows the folded glyph
    t.toggle_fold("main");
    let lines = t.render_herdr_tree_lines("nobody");
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[1], "├─ ▸ main [MCP] (subtree: 3p/2r)");
}

#[test]
fn herdr_render_skips_dangling_children_and_handles_empty_tree() {
    let mut t = SessionTree::new();
    t.node_mut("main").unwrap().children.push("dangling".into());
    let lines = t.render_herdr_tree_lines("x");
    assert_eq!(lines.len(), 2, "dangling child produces no line");
    assert!(lines[1].contains("▾ main"));
    t.nodes.clear();
    assert_eq!(t.render_herdr_tree_lines("x"), vec!["◈ ROOT [MCP — MASTER CONTROL PROGRAM]"]);
}

#[test]
fn session_tree_serializes_and_round_trips_through_toml() {
    let mut t = SessionTree::new();
    t.branch("main", "a").unwrap();
    t.branch("a", "a1").unwrap();
    t.upsert_session("solo", SessionNodeKind::Independent, None);
    t.toggle_fold("a");
    t.set_subtree_frozen("a1", true);
    t.node_mut("solo").unwrap().pane_count = 7;
    let s = toml::to_string(&t).unwrap();
    assert!(s.contains("kind = \"Subtree\"") && s.contains("kind = \"Independent\""), "{s}");
    let back: SessionTree = toml::from_str(&s).unwrap();
    assert_eq!(back, t);
    assert_eq!(back.render_herdr_tree_lines("main"), t.render_herdr_tree_lines("main"));

    // empty tree round trips too
    let empty = SessionTree { nodes: vec![] };
    let back: SessionTree = toml::from_str(&toml::to_string(&empty).unwrap()).unwrap();
    assert_eq!(back, empty);

    // unknown kind is rejected
    let bad = s.replace("Subtree", "Weird");
    assert!(toml::from_str::<SessionTree>(&bad).is_err());
    assert_eq!(SessionNodeKind::default(), SessionNodeKind::Independent);
}

#[test]
fn deep_nesting_round_trip_and_rollup() {
    let mut t = SessionTree::new();
    let mut parent = "main".to_string();
    for i in 0..25 {
        let child = format!("n{i}");
        t.branch(&parent, &child).unwrap();
        parent = child;
    }
    assert_eq!(t.subtree_session_names("main").len(), 26);
    assert_eq!(t.subtree_rollup("main"), (26, 0));
    let back: SessionTree = toml::from_str(&toml::to_string(&t).unwrap()).unwrap();
    assert_eq!(back, t);
    let lines = t.render_herdr_tree_lines("n24");
    assert_eq!(lines.len(), 27);
    assert!(lines.last().unwrap().contains("n24* [SUB] (1p/0r)"));
    // grafting the root under its deepest descendant is a cycle
    assert!(t.graft("main", "n24").is_err());
}
