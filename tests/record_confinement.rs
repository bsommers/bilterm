//! `:record pane <path>` honours automation confinement and writes recordings safely.

use bilterm::colon_command::parse_colon_command;
use bilterm::wm::WindowManager;

fn cmd(line: &str) -> bilterm::action::Action {
    parse_colon_command(line).unwrap().unwrap()
}

#[test]
fn confined_record_pane_rejects_absolute_and_traversal_paths() {
    let confine = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let abs = outside.path().join("abs.cast");
    let mut wm = WindowManager::new_headless(80, 24);

    for line in [
        format!(":record pane {}", abs.display()),
        ":record pane ../x.cast".to_string(),
    ] {
        let err = wm
            .apply_automation(cmd(&line), Some(confine.path()))
            .unwrap_err();
        assert!(!err.is_empty(), "{line}");
        assert!(
            !wm.active_pane().vt().is_recording(),
            "{line} must not start a recording"
        );
    }
    let _ = wm.apply_automation(cmd(":record stop"), Some(confine.path()));
    assert!(!abs.exists());
    assert!(!confine.path().parent().unwrap().join("x.cast").exists());
}

#[test]
fn confined_record_pane_plain_name_lands_in_confine_dir() {
    let confine = tempfile::tempdir().unwrap();
    let mut wm = WindowManager::new_headless(80, 24);
    wm.apply_automation(cmd(":record pane demo.cast"), Some(confine.path()))
        .unwrap();
    assert!(wm.active_pane().vt().is_recording());
    wm.apply_automation(cmd(":record stop"), Some(confine.path()))
        .unwrap();
    assert!(confine.path().join("demo.cast").is_file());
}

#[test]
fn interactive_record_pane_accepts_user_path_and_writes_0600() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub").join("mine.cast");
    let mut wm = WindowManager::new_headless(80, 24);
    wm.apply(cmd(&format!(":record pane {}", path.display())))
        .unwrap();
    assert!(wm.active_pane().vt().is_recording());
    wm.apply(cmd(":record stop")).unwrap();
    assert!(path.is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[test]
#[cfg(unix)]
fn record_pane_does_not_follow_final_path_symlink() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("victim.txt");
    std::fs::write(&target, b"original").unwrap();
    let link = dir.path().join("link.cast");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let mut wm = WindowManager::new_headless(80, 24);
    wm.apply(cmd(&format!(":record pane {}", link.display())))
        .unwrap();
    wm.apply(cmd(":record stop")).unwrap();

    assert_eq!(std::fs::read(&target).unwrap(), b"original");
    let md = std::fs::symlink_metadata(&link).unwrap();
    assert!(
        md.file_type().is_file(),
        "symlink must be replaced, not written through"
    );
}
