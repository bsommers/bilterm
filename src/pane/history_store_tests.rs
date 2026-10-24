use super::*;

fn settings(dir: &Path, session: &str) -> HistorySettings {
    HistorySettings {
        enabled: true,
        max_files: 25,
        max_entries: 100,
        load_limit: 1000,
        dir: Some(dir.to_string_lossy().into_owned()),
        session: session.to_string(),
    }
}

fn store(dir: &Path, session: &str, run: &str, pane: usize) -> HistoryStore {
    let mut s = HistoryStore::new_headless(pane);
    s.run_id = run.to_string();
    s.apply_settings(&settings(dir, session));
    s
}

#[test]
fn entries_survive_a_new_store_instance_for_the_same_session() {
    let tmp = tempfile::tempdir().unwrap();
    let mut first = store(tmp.path(), "work", "run-a", 1);
    let written = first.record_command("tail -f /dev/null").unwrap().unwrap();
    assert_eq!(written, tmp.path().join("sessions/work/history.log"));
    let text = fs::read_to_string(&written).unwrap();
    assert!(text.contains("session=work | run=run-a | pane=1] tail -f /dev/null"));

    let second = store(tmp.path(), "work", "run-b", 1);
    let entries = second.load_entries().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].command, "tail -f /dev/null");
    assert_eq!(entries[0].run_id, "run-a");
    assert_eq!(entries[0].session, "work");
    assert_eq!(
        second.load_recent_commands(),
        vec!["tail -f /dev/null".to_string()]
    );
}

#[cfg(unix)]
#[test]
fn session_dir_and_file_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let mut s = store(tmp.path(), "default", "r", 1);
    let path = s.record_command("ls").unwrap().unwrap();
    let fmode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    let dmode = fs::metadata(path.parent().unwrap())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(fmode, 0o600);
    assert_eq!(dmode, 0o700);
}

#[test]
fn different_session_names_are_isolated_and_sanitized() {
    let tmp = tempfile::tempdir().unwrap();
    let mut a = store(tmp.path(), "alpha", "r1", 1);
    let mut b = store(tmp.path(), "../be ta/", "r1", 2);
    a.record_command("echo alpha").unwrap();
    b.record_command("echo beta").unwrap();
    assert_eq!(b.session_name, "_be_ta_");
    assert!(b.session_file_path.starts_with(tmp.path().join("sessions")));
    assert_eq!(a.load_recent_commands(), vec!["echo alpha".to_string()]);
    assert_eq!(b.load_recent_commands(), vec!["echo beta".to_string()]);
    assert_eq!(sanitize_session_name("   "), "default");
    assert_eq!(sanitize_session_name(".."), "default");
}

#[test]
fn segments_group_entries_by_run() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r1 = store(tmp.path(), "s", "run-1", 1);
    r1.record_command("one").unwrap();
    r1.record_command("two").unwrap();
    let mut r2 = store(tmp.path(), "s", "run-2", 3);
    r2.record_command("three").unwrap();
    let segs = r2.segments();
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[0].run_id, "run-1");
    assert_eq!(segs[0].count, 2);
    assert!(segs[0].started_at.ends_with('Z'));
    assert_eq!(segs[1].run_id, "run-2");
    assert_eq!(segs[1].count, 1);
}

#[test]
fn compaction_keeps_newest_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let mut s = store(tmp.path(), "s", "r", 1);
    s.max_entries = 3;
    for i in 0..7 {
        s.record_command(&format!("cmd {i}")).unwrap();
    }
    let cmds: Vec<String> = s
        .load_entries()
        .unwrap()
        .into_iter()
        .map(|e| e.command)
        .collect();
    assert_eq!(cmds, vec!["cmd 4", "cmd 5", "cmd 6"]);
    let leftovers: Vec<_> = fs::read_dir(s.session_file_path.parent().unwrap())
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(leftovers.len(), 1, "temp file must be renamed away");
}

#[test]
fn recent_commands_unescape_newlines_collapse_duplicates_and_respect_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let mut s = store(tmp.path(), "s", "r", 1);
    for c in [
        "ls",
        "ls",
        "for i in 1 2\ndo echo $i\ndone",
        "printf 'a\\nb'",
        "ls",
        "ls",
    ] {
        s.record_command(c).unwrap();
    }
    let raw = fs::read_to_string(&s.session_file_path).unwrap();
    assert_eq!(raw.lines().count(), 6);
    assert_eq!(
        s.load_recent_commands(),
        vec![
            "ls",
            "for i in 1 2\ndo echo $i\ndone",
            "printf 'a\\nb'",
            "ls"
        ]
    );
    s.load_limit = 2;
    assert_eq!(s.load_recent_commands(), vec!["printf 'a\\nb'", "ls"]);
}

#[test]
fn prunes_least_recently_modified_session_dirs_but_never_active() {
    let tmp = tempfile::tempdir().unwrap();
    for name in ["old1", "old2"] {
        let mut s = store(tmp.path(), name, "r", 1);
        s.record_command("x").unwrap();
    }
    let mut active = store(tmp.path(), "active", "r", 1);
    active.max_files = 2;
    active.record_command("y").unwrap();
    let mut names: Vec<String> = fs::read_dir(tmp.path().join("sessions"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"active".to_string()));
    let legacy: Vec<_> = fs::read_dir(tmp.path())
        .unwrap()
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("bilterm-history-")
        })
        .collect();
    assert!(legacy.is_empty());
}

#[test]
fn headless_store_never_resolves_user_dir_and_exports_temp_file() {
    let mut s = HistoryStore::new_headless(4);
    assert!(s.base_dir.starts_with(std::env::temp_dir()));
    let changed = s.apply_settings(&HistorySettings {
        dir: None,
        ..settings(Path::new("/unused"), "other")
    });
    assert!(changed);
    assert!(s.base_dir.starts_with(std::env::temp_dir()));

    let exported = s
        .export_to_temp_file(&["ls -la".to_string(), "cargo test".to_string()], None)
        .unwrap();
    let private_root = crate::secure_fs::private_runtime_dir().unwrap();
    assert!(exported.starts_with(private_root.join("exports")));
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            fs::symlink_metadata(&exported).unwrap().mode() & 0o777,
            0o600
        );
    }
    let exp_text = fs::read_to_string(&exported).unwrap();
    assert!(exp_text.contains("# Session: other"));
    assert!(exp_text.contains("cargo test"));
    let _ = fs::remove_file(&exported);
}

#[test]
fn default_exports_get_unique_names() {
    let mut s = HistoryStore::new_headless(1);
    let exp1 = s.export_to_temp_file(&["cmd1".to_string()], None).unwrap();
    let exp2 = s.export_to_temp_file(&["cmd2".to_string()], None).unwrap();
    assert_ne!(exp1, exp2);
    assert!(exp1.exists());
    assert!(exp2.exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(fs::symlink_metadata(&exp1).unwrap().mode() & 0o777, 0o600);
        assert_eq!(fs::symlink_metadata(&exp2).unwrap().mode() & 0o777, 0o600);
    }
    let text1 = fs::read_to_string(&exp1).unwrap();
    assert!(text1.contains("cmd1"));
    let _ = fs::remove_file(&exp1);
    let _ = fs::remove_file(&exp2);
}

#[test]
fn default_export_does_not_overwrite_existing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("out.sh");
    write_export_exclusive(&target, "first").unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "first");

    let err = write_export_exclusive(&target, "second").unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read_to_string(&target).unwrap(), "first");
}

#[test]
fn custom_export_path_still_works() {
    let tmp = tempfile::tempdir().unwrap();
    let custom = tmp.path().join("custom.sh");
    let mut s = HistoryStore::new_headless(1);
    let out = s
        .export_to_temp_file(&["custom_cmd".to_string()], Some(&custom))
        .unwrap();
    assert_eq!(out, custom);
    assert_eq!(s.last_exported_temp_path.as_deref(), Some(custom.as_path()));
    assert!(fs::read_to_string(&custom).unwrap().contains("custom_cmd"));
}

#[test]
fn empty_base_dir_disables_persistence() {
    let sessions_exist_before = Path::new("sessions").exists();
    let mut s = HistoryStore::new(
        1,
        DEFAULT_HISTORY_SESSION,
        "run",
        PathBuf::new(),
        true,
        25,
    );
    let res = s.record_command("ls").unwrap();
    assert_eq!(res, None);
    assert_eq!(s.load_entries().unwrap(), Vec::new());
    assert_eq!(s.compact().unwrap(), 0);
    assert_eq!(s.prune_excess_files().unwrap(), 0);
    assert_eq!(Path::new("sessions").exists(), sessions_exist_before);
}
