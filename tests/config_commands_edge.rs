//! Edge cases for `:get`, `:set`, `:config list|save`, `:appearance save` and `:state`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::config::{dotted, BiltermConfig};
use bilterm::wm::WindowManager;

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Runs `f` with XDG_CONFIG_HOME/XDG_STATE_HOME pointing at a fresh temp dir (serialised).
fn with_xdg<T>(tag: &str, f: impl FnOnce(&Path) -> T) -> T {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("bilterm-cfgedge-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: serialised by ENV_LOCK; every env-reading test goes through here.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("XDG_STATE_HOME", &dir);
    }
    f(&dir)
}

fn cfg_path(dir: &Path) -> PathBuf {
    dir.join("bilterm").join("config.toml")
}

fn run(wm: &mut WindowManager, cmd: &str) {
    let action = parse_colon_command(cmd).expect("colon command").expect("parses");
    execute_wm_colon_command(wm, action);
}

fn status(wm: &WindowManager) -> String {
    wm.active_pane().status_message().unwrap_or("").to_string()
}

fn data(wm: &mut WindowManager) -> Option<JsonValue> {
    wm.take_command_data().and_then(|d| JsonValue::parse(&d))
}

fn s_of(v: &JsonValue, k: &str) -> String {
    v.get(k).and_then(JsonValue::as_str).unwrap_or_else(|| panic!("no {k} in {}", v.to_json_string())).to_string()
}

// ---------------------------------------------------------------- parsing

#[test]
fn parse_usage_errors() {
    let err = |c: &str| parse_colon_command(c).expect("owned").unwrap_err();
    assert_eq!(err(":get"), "Usage: :get <key>  (dotted path, e.g. appearance.profile)");
    assert_eq!(err(":get a b"), "Usage: :get <key>  (dotted path, e.g. appearance.profile)");
    assert_eq!(err(":set"), "Usage: :set <key> <value>");
    assert_eq!(err(":set theme"), "Usage: :set <key> <value>");
    assert_eq!(err(":set theme    "), "Usage: :set <key> <value>");
    assert_eq!(err(":config"), "Usage: :config list | save");
    assert_eq!(err(":config bogus"), "Usage: :config list | save");
    assert_eq!(err(":config list extra"), "Usage: :config list | save");
    assert_eq!(err(":config save now"), "Usage: :config list | save");
}

// ---------------------------------------------------------------- :get

#[test]
fn get_unknown_nested_and_malformed_keys() {
    let mut wm = WindowManager::new_headless(24, 80);
    for (cmd, needle) in [
        (":get no_such_key", "unknown config key `no_such_key`"),
        (":get appearance.no_such", "unknown config key `appearance.no_such`"),
        (":get .theme", "invalid config key `.theme`"),
        (":get theme.", "invalid config key `theme.`"),
        (":get a..b", "invalid config key `a..b`"),
    ] {
        run(&mut wm, cmd);
        assert_eq!(status(&wm), needle, "{cmd}");
        assert!(wm.take_command_data().is_none(), "{cmd} must not publish data");
    }
}

#[test]
fn get_returns_toml_text_for_each_value_kind() {
    let mut wm = WindowManager::new_headless(24, 80);
    run(&mut wm, ":get crosshair_cursor");
    let d = data(&mut wm).unwrap();
    assert_eq!(s_of(&d, "key"), "crosshair_cursor");
    assert_eq!(s_of(&d, "value"), "false");
    run(&mut wm, ":get font_size_pt");
    assert_eq!(s_of(&data(&mut wm).unwrap(), "value"), wm.config().font_size_pt.to_string());
    run(&mut wm, ":get theme");
    assert!(s_of(&data(&mut wm).unwrap(), "value").starts_with('"'));
    // A table key renders as a TOML block containing its leaf keys.
    run(&mut wm, ":get api");
    assert_eq!(s_of(&data(&mut wm).unwrap(), "value"), "allow_config_write = false");
    assert_eq!(status(&wm), "api = allow_config_write = false");
}

// ---------------------------------------------------------------- :set

#[test]
fn set_value_kinds_and_quoting() {
    let mut wm = WindowManager::new_headless(24, 80);
    // bool
    run(&mut wm, ":set crosshair_cursor true");
    assert!(wm.config().crosshair_cursor);
    assert_eq!(s_of(&data(&mut wm).unwrap(), "value"), "true");
    assert!(status(&wm).ends_with("(session only; :config save to persist)"));
    // number
    run(&mut wm, ":set font_size_pt 21");
    assert_eq!(wm.config().font_size_pt, 21);
    // bare string vs quoted string vs string that looks like a number
    run(&mut wm, ":set font_family Fira Code");
    assert_eq!(wm.config().font_family, "Fira Code");
    run(&mut wm, r#":set font_family "JetBrains Mono""#);
    assert_eq!(wm.config().font_family, "JetBrains Mono");
    run(&mut wm, ":set leader_key 123");
    assert_eq!(wm.config().leader_key, "123");
    assert_eq!(dotted::get_key(wm.config(), "leader_key").unwrap(), "\"123\"");
    // enum
    run(&mut wm, ":set appearance.profile midnight");
    assert_eq!(dotted::get_key(wm.config(), "appearance.profile").unwrap(), "\"midnight\"");
    // array
    run(&mut wm, r#":set session_order ["b","a"]"#);
    assert_eq!(wm.config().session_order, vec!["b".to_string(), "a".to_string()]);
    assert_eq!(dotted::get_key(wm.config(), "session_order").unwrap(), r#"["b", "a"]"#);
    // surrounding whitespace is trimmed
    run(&mut wm, ":set   font_size_pt    22   ");
    assert_eq!(wm.config().font_size_pt, 22);
}

#[test]
fn set_failures_leave_config_untouched() {
    let mut wm = WindowManager::new_headless(24, 80);
    let before = wm.config().clone();
    for (cmd, prefix) in [
        (":set no_such_key 1", "Cannot set no_such_key: unknown config key `no_such_key`"),
        (":set appearance.no_such 1", "Cannot set appearance.no_such: unknown config key"),
        (":set font_size_pt banana", "Cannot set font_size_pt: "),
        (":set font_size_pt -5", "Cannot set font_size_pt: "),
        (":set font_size_pt 99999999", "Cannot set font_size_pt: "),
        (":set crosshair_cursor maybe", "Cannot set crosshair_cursor: "),
        (":set crosshair_cursor 3", "Cannot set crosshair_cursor: "),
        (":set theme no-such-theme", "Cannot set theme: "),
        (":set appearance.profile nonsense", "Cannot set appearance.profile: "),
        (":set session_order [1,2]", "Cannot set session_order: "),
        (":set theme.sub x", "Cannot set theme.sub: "),
        (":set .x 1", "Cannot set .x: invalid config key `.x`"),
        (":set a..b 1", "Cannot set a..b: invalid config key `a..b`"),
    ] {
        run(&mut wm, cmd);
        let st = status(&wm);
        assert!(st.starts_with(prefix), "{cmd}: {st}");
        assert!(wm.take_command_data().is_none(), "{cmd}");
        assert_eq!(*wm.config(), before, "{cmd} mutated config");
    }
}

#[test]
fn set_does_not_clobber_runtime_only_fields_and_empty_value_is_usage() {
    let mut wm = WindowManager::new_headless(24, 80);
    let parse = parse_colon_command(":set theme \"\"").unwrap().unwrap();
    // `""` is a non-empty token for the parser; the config layer rejects the empty theme.
    execute_wm_colon_command(&mut wm, parse);
    assert!(status(&wm).starts_with("Cannot set theme: "), "{}", status(&wm));
    assert!(parse_colon_command(":set theme ").unwrap().is_err());
    // set_key directly: empty raw value is a string "" and a non-string key rejects it.
    let mut c = BiltermConfig::default();
    assert!(dotted::set_key(&mut c, "font_size_pt", "").is_err());
    assert!(dotted::set_key(&mut c, "", "1").unwrap_err().contains("invalid config key"));
}

// ---------------------------------------------------------------- :config list / :state

#[test]
fn config_list_is_flat_sorted_and_reflects_sets() {
    let mut wm = WindowManager::new_headless(24, 80);
    run(&mut wm, ":set appearance.profile midnight");
    run(&mut wm, ":config list");
    let d = data(&mut wm).unwrap();
    let JsonValue::Object(map) = &d else { panic!() };
    assert!(map.len() > 20, "{} keys", map.len());
    assert_eq!(map.get("appearance.profile").and_then(JsonValue::as_str), Some("\"midnight\""));
    assert_eq!(map.get("api.allow_config_write").and_then(JsonValue::as_str), Some("false"));
    assert!(map.keys().all(|k| !k.is_empty() && !k.starts_with('.') && !k.ends_with('.')));
    assert!(status(&wm).starts_with(&format!("Config: {} keys", map.len())));
}

#[test]
fn state_json_fields_and_types() {
    let mut wm = WindowManager::new_headless(24, 80);
    run(&mut wm, ":state");
    let d = data(&mut wm).unwrap();
    let JsonValue::Object(map) = &d else { panic!() };
    let keys: Vec<&str> = map.keys().map(String::as_str).collect();
    assert_eq!(keys, ["active_pane_id", "appearance", "display_mode", "drawer_visible", "layout_mode", "pane_count"]);
    assert_eq!(d.get("pane_count").and_then(JsonValue::as_usize), Some(1));
    assert_eq!(d.get("active_pane_id").and_then(JsonValue::as_usize), Some(wm.active_pane_id()));
    assert!(matches!(s_of(&d, "layout_mode").as_str(), "tiled" | "stacked"));
    assert!(d.get("drawer_visible").and_then(JsonValue::as_bool).is_some());
    assert!(!s_of(&d, "appearance").is_empty() && !s_of(&d, "display_mode").is_empty());
    assert_eq!(status(&wm), format!("State: {}", d.to_json_string()));
    // :state reflects an appearance change.
    run(&mut wm, ":set appearance.profile midnight");
    run(&mut wm, ":state");
    let d2 = data(&mut wm).unwrap();
    assert_ne!(s_of(&d, "appearance"), s_of(&d2, "appearance"));
    // Data is consumed exactly once.
    assert!(wm.take_command_data().is_none());
}

// ---------------------------------------------------------------- save

#[test]
fn config_save_into_unwritable_dir_reports_error_and_leaves_no_file() {
    with_xdg("unwritable", |dir| {
        // XDG_CONFIG_HOME is a regular file, so `<it>/bilterm` cannot be created.
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, "not a dir").unwrap();
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &blocker) };
        for cmd in [":config save", ":appearance save"] {
            let mut wm = WindowManager::new_headless(24, 80);
            wm.config_mut().font_size_pt = 23;
            run(&mut wm, ":set appearance.profile midnight");
            let _ = wm.take_command_data();
            run(&mut wm, cmd);
            let st = status(&wm);
            assert!(st.starts_with("Config save failed: cannot read ") && st.contains("Not a directory"), "{cmd}: {st}");
            assert!(wm.take_command_data().is_none());
        }
        assert_eq!(std::fs::read_to_string(&blocker).unwrap(), "not a dir");
        assert_eq!(std::fs::read_dir(dir).unwrap().count(), 1, "no stray files");
    });
}

#[cfg(unix)]
#[test]
fn config_save_into_readonly_dir_leaves_no_partial_file() {
    use std::os::unix::fs::PermissionsExt;
    with_xdg("readonly", |dir| {
        let ro = dir.join("ro");
        std::fs::create_dir_all(ro.join("bilterm")).unwrap();
        std::fs::set_permissions(ro.join("bilterm"), std::fs::Permissions::from_mode(0o500)).unwrap();
        // Root ignores permission bits; skip the assertion there.
        let writable_anyway = std::fs::write(ro.join("bilterm").join("probe"), "x").is_ok();
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &ro) };
        let mut wm = WindowManager::new_headless(24, 80);
        wm.config_mut().font_size_pt = 23;
        run(&mut wm, ":config save");
        if !writable_anyway {
            assert!(status(&wm).starts_with("Config save failed: cannot write "), "{}", status(&wm));
            let leftovers: Vec<_> = std::fs::read_dir(ro.join("bilterm")).unwrap().collect();
            assert!(leftovers.is_empty(), "partial file left behind: {leftovers:?}");
        }
        std::fs::set_permissions(ro.join("bilterm"), std::fs::Permissions::from_mode(0o700)).unwrap();
    });
}

#[test]
fn save_preserves_comments_and_unrelated_keys_and_is_idempotent() {
    with_xdg("preserve", |dir| {
        let path = cfg_path(dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = "# my hand-written config\nfont_size_pt = 14  # keep this comment\n\n[appearance]\n# profile comment\nprofile = \"midnight\"\n";
        std::fs::write(&path, original).unwrap();
        let mut wm = WindowManager::new_headless(24, 80);
        *wm.config_mut() = BiltermConfig::load(&path).unwrap().unwrap();
        run(&mut wm, ":config save");
        assert!(status(&wm).starts_with("Config: nothing to save"), "{}", status(&wm));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original, "no-op save must not rewrite");
        run(&mut wm, ":set font_size_pt 19");
        run(&mut wm, ":config save");
        let d = data(&mut wm).unwrap();
        assert_eq!(d.get("written").and_then(JsonValue::as_array).unwrap().len(), 1);
        assert_eq!(s_of(&d, "path"), path.display().to_string());
        assert!(status(&wm).contains("font_size_pt"));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# profile comment"), "{text}");
        assert!(text.contains("font_size_pt = 19") && text.contains("# keep this comment"), "{text}");
        assert!(text.contains("profile = \"midnight\""), "{text}");
        assert!(!text.contains("font_size_pt = 14"));
        // Saving again writes nothing.
        run(&mut wm, ":config save");
        assert!(status(&wm).starts_with("Config: nothing to save"));
        // Reloading yields the running config's saved fields.
        assert_eq!(BiltermConfig::load(&path).unwrap().unwrap().font_size_pt, 19);
    });
}

#[cfg(unix)]
#[test]
fn saved_config_file_is_private() {
    use std::os::unix::fs::PermissionsExt;
    with_xdg("perm", |dir| {
        let mut wm = WindowManager::new_headless(24, 80);
        run(&mut wm, ":set font_size_pt 17");
        run(&mut wm, ":config save");
        let mode = std::fs::metadata(cfg_path(dir)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    });
}

#[test]
fn appearance_save_writes_only_appearance_keys() {
    with_xdg("appearance", |dir| {
        let path = cfg_path(dir);
        let mut wm = WindowManager::new_headless(24, 80);
        run(&mut wm, ":set font_size_pt 17");
        run(&mut wm, ":set appearance.profile midnight");
        run(&mut wm, ":appearance save");
        let d = data(&mut wm).unwrap();
        let written: Vec<String> = d
            .get("written")
            .and_then(JsonValue::as_array)
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert!(!written.is_empty() && written.iter().all(|k| k.starts_with("appearance.")), "{written:?}");
        let on_disk = BiltermConfig::load(&path).unwrap().unwrap();
        assert_eq!(on_disk.font_size_pt, BiltermConfig::default().font_size_pt, "non-appearance key leaked");
        assert!(std::fs::read_to_string(&path).unwrap().contains("midnight"));
        // A full save then picks up the remaining key.
        run(&mut wm, ":config save");
        let d = data(&mut wm).unwrap();
        let all = d.get("written").and_then(JsonValue::as_array).unwrap();
        assert!(all.iter().any(|v| v.as_str() == Some("font_size_pt")));
        assert!(!all.iter().any(|v| v.as_str().is_some_and(|k| k.starts_with("appearance."))));
    });
}

#[test]
fn save_removes_key_reverted_to_default_and_rejects_corrupt_file() {
    with_xdg("revert", |dir| {
        let path = cfg_path(dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "font_size_pt = 30\n").unwrap();
        let cfg = BiltermConfig::default();
        let written = dotted::save_changes(&cfg, &path, None).unwrap();
        assert_eq!(written, vec!["font_size_pt".to_string()]);
        // Corrupt TOML is refused, and the file is left byte-for-byte as it was.
        std::fs::write(&path, "font_size_pt = [[[\n").unwrap();
        let err = dotted::save_changes(&cfg, &path, None).unwrap_err();
        assert!(!err.is_empty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "font_size_pt = [[[\n");
        // Through the colon command the same failure is surfaced, not swallowed.
        let mut wm = WindowManager::new_headless(24, 80);
        run(&mut wm, ":config save");
        assert!(status(&wm).starts_with("Config save failed: "), "{}", status(&wm));
    });
}

// ---------------------------------------------------------------- API gating + errors over RPC

#[test]
fn rpc_error_surface_for_config_commands() {
    with_xdg("rpc", |dir| {
        let mut p = AutomationControlPlane::new_headless(24, 80);
        let call = |p: &mut AutomationControlPlane, cmd: &str| -> JsonValue {
            let line = format!(
                r#"{{"jsonrpc":"2.0","id":1,"method":"wm.command","params":{{"command":{}}}}}"#,
                JsonValue::String(cmd.to_string()).to_json_string()
            );
            JsonValue::parse(&p.handle_jsonrpc_line(&line)).unwrap()
        };
        let msg = |v: &JsonValue| {
            v.get("error").and_then(|e| e.get("message")).and_then(JsonValue::as_str).unwrap_or("").to_string()
        };
        // Reads: errors are reported for unknown keys.
        assert!(msg(&call(&mut p, ":get nope")).contains("unknown config key `nope`"));
        assert!(msg(&call(&mut p, ":get")).contains("Usage: :get <key>"));
        assert!(msg(&call(&mut p, ":set theme")).contains("Usage: :set <key> <value>"));
        // Enable writes, then: bad value is an error and nothing is persisted.
        p.wm.config_mut().api.allow_config_write = true;
        assert!(msg(&call(&mut p, ":set font_size_pt banana")).contains("Cannot set font_size_pt:"));
        assert!(msg(&call(&mut p, ":set api.allow_config_write false")).contains("[api] section cannot be changed"));
        assert!(msg(&call(&mut p, ":set api true")).contains("[api] section cannot be changed"));
        assert!(!cfg_path(dir).exists());
        // Unwritable target: error propagates to the RPC caller, no file created.
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, "x").unwrap();
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &blocker) };
        let r = call(&mut p, ":set font_size_pt 20");
        assert!(r.get("error").is_none());
        let m = msg(&call(&mut p, ":config save"));
        assert!(m.contains("Config save failed: cannot "), "{m}");
        assert!(msg(&call(&mut p, ":appearance save")).contains("Config save failed: "));
        assert_eq!(std::fs::read_to_string(&blocker).unwrap(), "x");
    });
}

#[test]
fn save_preserves_leading_comment_above_changed_key() {
    with_xdg("leadcomment", |dir| {
        let path = cfg_path(dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "# my hand-written config\nfont_size_pt = 14\n").unwrap();
        let mut wm = WindowManager::new_headless(24, 80);
        *wm.config_mut() = BiltermConfig::load(&path).unwrap().unwrap();
        run(&mut wm, ":set font_size_pt 19");
        run(&mut wm, ":config save");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# my hand-written config\n"), "{text}");
    });
}

#[test]
fn get_child_of_scalar_key_is_unknown() {
    let mut wm = WindowManager::new_headless(24, 80);
    run(&mut wm, ":get theme.sub");
    assert_eq!(status(&wm), "unknown config key `theme.sub`");
}
