//! Phase 63-03: key-conflict engine and `UNI-R01` parity for the keyboard surface (`KEY-02`).
//!
//! Every capability must be reachable and agree across keys, colon command, palette, menu, JSON-RPC
//! and MCP. Window mode has no key router on this branch (63-02 adds it), so the parity checks
//! assert on `Action` lowering and `wm.key_mode()` state, not on keys pressed inside Window mode.

use bilterm::action::{Action, KeysOp, WinSub, WindowModeOp};
use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::parse_colon_command;
use bilterm::config::BiltermConfig;
use bilterm::config::keys::{KeyProfileName, KeysConfig};
use bilterm::input::handle_wm_key_event;
use bilterm::input::keyconflict::{self, Finding};
use bilterm::input::keyconflict_data::{Class, Severity};
use bilterm::input::keyenv::{HostOs, KeyEnv, Surface};
use bilterm::input::keyprofile::KeyProfile;
use bilterm::ui::modals::PALETTE_CATALOG;
use bilterm::wm::{KeyMode, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const CS: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::SHIFT);

fn ch(c: char, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), mods)
}

fn gui() -> KeyEnv {
    KeyEnv::new(HostOs::Linux, Surface::Gui)
}

fn wm_linux() -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.set_key_env(gui());
    wm.set_key_profile(KeyProfileName::Linux);
    wm
}

fn plane() -> AutomationControlPlane {
    let mut p = AutomationControlPlane::new_headless(24, 80);
    p.wm.set_key_env(gui());
    p.wm.set_key_profile(KeyProfileName::Linux);
    p
}

fn writable_plane() -> AutomationControlPlane {
    let mut p = plane();
    p.wm.config_mut().api.allow_config_write = true;
    p
}

fn params(pairs: &[(&str, JsonValue)]) -> JsonValue {
    JsonValue::obj(pairs.iter().map(|(k, v)| (*k, v.clone())))
}

fn s(v: &str) -> JsonValue {
    JsonValue::String(v.to_string())
}

/// Calls `bilterm_keys` and returns the parsed tool payload, or the error text.
fn mcp_keys(p: &mut AutomationControlPlane, args: &[(&str, JsonValue)]) -> Result<JsonValue, String> {
    let req = JsonValue::obj([
        ("jsonrpc", s("2.0")),
        ("id", JsonValue::Number(1)),
        ("method", s("tools/call")),
        ("params", params(&[("name", s("bilterm_keys")), ("arguments", params(args))])),
    ]);
    let resp = JsonValue::parse(&p.handle_mcp_line(&req.to_json_string()).expect("mcp response")).expect("json");
    if let Some(err) = resp.get("error") {
        return Err(err.to_json_string());
    }
    let result = resp.get("result").expect("result");
    let text = result
        .get("content")
        .and_then(JsonValue::as_array)
        .and_then(|a| a.first())
        .and_then(|c| c.get("text"))
        .and_then(JsonValue::as_str)
        .expect("content text")
        .to_string();
    if result.get("isError").and_then(JsonValue::as_bool) == Some(true) {
        return Err(text);
    }
    Ok(JsonValue::parse(&text).expect("tool payload is json"))
}

#[test]
fn window_mode_enter_parity() {
    let enter = Action::WindowMode(WindowModeOp::Enter);
    let expect = KeyMode::Window(WinSub::Focus);

    // Leader `w`.
    let mut wm = wm_linux();
    handle_wm_key_event(&mut wm, ch('x', KeyModifiers::CONTROL)).unwrap();
    handle_wm_key_event(&mut wm, ch('w', KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.key_mode(), expect, "leader w");

    // Primary+O in a GUI env.
    let mut wm = wm_linux();
    handle_wm_key_event(&mut wm, ch('O', CS)).unwrap();
    assert_eq!(wm.key_mode(), expect, "Primary+O");

    // Colon command.
    let mut wm = wm_linux();
    assert_eq!(parse_colon_command(":wmode on").unwrap().unwrap(), enter);
    wm.apply(enter.clone()).unwrap();
    assert_eq!(wm.key_mode(), expect, ":wmode on");

    // Palette entries lower to the same action (toggle from Normal enters).
    let on = PALETTE_CATALOG.iter().find(|e| e.0 == ":wmode on").expect("palette has :wmode on");
    assert_eq!(parse_colon_command(on.0).unwrap().unwrap(), enter);
    assert!(PALETTE_CATALOG.iter().any(|e| e.0 == ":wmode"), "palette has :wmode");
    let mut wm = wm_linux();
    wm.apply(parse_colon_command(":wmode").unwrap().unwrap()).unwrap();
    assert_eq!(wm.key_mode(), expect, "palette :wmode toggles on");

    // RPC and MCP.
    let mut p = writable_plane();
    p.dispatch_method("keys.window_mode", &params(&[("op", s("on"))])).unwrap();
    assert_eq!(p.wm.key_mode(), expect, "keys.window_mode");
    let mut p = writable_plane();
    let out = mcp_keys(&mut p, &[("action", s("window_mode")), ("op", s("on"))]).unwrap();
    assert_eq!(p.wm.key_mode(), expect, "bilterm_keys window_mode");
    assert_eq!(out.get("key_mode").and_then(JsonValue::as_str), Some("window:focus"));
}

#[test]
fn menu_open_parity() {
    let want = Some((0, 0));

    let mut wm = wm_linux();
    handle_wm_key_event(&mut wm, ch('x', KeyModifiers::CONTROL)).unwrap();
    handle_wm_key_event(&mut wm, ch('m', KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.menubar_state(), want, "leader m");

    let mut wm = wm_linux();
    handle_wm_key_event(&mut wm, ch(':', CS)).unwrap();
    assert_eq!(wm.menubar_state(), want, "Primary+;");

    let mut wm = wm_linux();
    wm.apply(parse_colon_command(":menu open").unwrap().unwrap()).unwrap();
    assert_eq!(wm.menubar_state(), want, ":menu open");

    let mut p = writable_plane();
    p.dispatch_method("menu.open", &params(&[])).unwrap();
    assert_eq!(p.wm.menubar_state(), want, "menu.open");

    let mut p = writable_plane();
    mcp_keys(&mut p, &[("action", s("menu_open"))]).unwrap();
    assert_eq!(p.wm.menubar_state(), want, "bilterm_keys menu_open");
}

/// Commands written in the menu bar sources (modals.rs today, menu_model.rs after 63-02) must
/// parse, and the keyboard-surface ones must lower to the frozen actions.
#[test]
fn menu_items_for_the_keyboard_surface_lower_to_known_actions() {
    let root = env!("CARGO_MANIFEST_DIR");
    let mut seen = 0;
    for rel in ["src/ui/modals.rs", "src/ui/menu_model.rs"] {
        let Ok(text) = std::fs::read_to_string(format!("{root}/{rel}")) else { continue };
        for cmd in text.split('"').filter(|t| t.starts_with(":wmode") || t.starts_with(":menu open") || t.starts_with(":keys")) {
            let parsed = parse_colon_command(cmd).unwrap_or_else(|| panic!("{cmd} is not a colon command")).unwrap_or_else(|e| panic!("{cmd}: {e}"));
            assert!(matches!(parsed, Action::WindowMode(_) | Action::MenuOpen { .. } | Action::Keys(_)), "{cmd} -> {parsed:?}");
            seen += 1;
        }
    }
    eprintln!("keyboard-surface menu items checked: {seen}");
}

fn conflict_config() -> BiltermConfig {
    let mut c = BiltermConfig::default();
    c.keys.profile = KeyProfileName::Linux;
    c.keys.bind.insert("Ctrl+A".into(), ":split v".into());
    c.keys.bind.insert("Alt+F2".into(), ":help".into());
    c.keys.bind.insert("Ctrl+Q".into(), ":help".into());
    c.keys.bind.insert("Primary+L".into(), ":layout cycle".into());
    c.keybindings.insert("Super+K".into(), ":help".into());
    c
}

fn keys_of(v: &JsonValue) -> Vec<(String, String, String)> {
    let mut rows: Vec<_> = v
        .get("findings")
        .and_then(JsonValue::as_array)
        .expect("findings array")
        .iter()
        .map(|f| {
            let g = |k: &str| f.get(k).and_then(JsonValue::as_str).unwrap_or("").to_string();
            (g("chord"), g("severity"), g("consumer"))
        })
        .collect();
    rows.sort();
    rows
}

fn sorted(f: &[Finding]) -> Vec<(String, String, String)> {
    let mut rows: Vec<_> = f.iter().map(|f| (f.chord.clone(), f.severity.as_str().to_string(), f.consumer.clone())).collect();
    rows.sort();
    rows
}

#[test]
fn keys_check_parity() {
    let make = || {
        let mut p = writable_plane();
        *p.wm.config_mut() = {
            let mut c = conflict_config();
            c.api.allow_config_write = true;
            c
        };
        p.wm.set_key_env(gui());
        p
    };

    // Engine ground truth.
    let p0 = make();
    let km = p0.wm.keymap();
    let expected = sorted(&keyconflict::check_config(&km, &km.env(), p0.wm.config()));
    assert!(expected.iter().any(|r| r.1 == "block"), "the fixture must produce a Block finding");
    assert!(expected.iter().any(|r| r.1 == "warn"), "and a Warn finding");

    // Colon command.
    let mut p = make();
    let status_wm = &mut p.wm;
    status_wm.apply(parse_colon_command(":keys check").unwrap().unwrap()).unwrap();
    let colon = JsonValue::parse(&status_wm.take_command_data().unwrap()).unwrap();
    assert_eq!(keys_of(&colon), expected, ":keys check");

    // RPC.
    let mut p = make();
    let rpc = p.dispatch_method("keys.check", &params(&[])).unwrap();
    assert_eq!(keys_of(&rpc), expected, "keys.check");

    // MCP.
    let mut p = make();
    let mcp = mcp_keys(&mut p, &[("action", s("check"))]).unwrap();
    assert_eq!(keys_of(&mcp), expected, "bilterm_keys check");

    // Palette entry exists and lowers to the same action.
    let row = PALETTE_CATALOG.iter().find(|e| e.0 == ":keys check").expect("palette has :keys check");
    assert_eq!(parse_colon_command(row.0).unwrap().unwrap(), Action::Keys(KeysOp::Check));
}

#[test]
fn keys_explain_and_list_parity() {
    let mut p = writable_plane();
    let rpc = p.dispatch_method("keys.explain", &params(&[("chord", s("Primary+L"))])).unwrap();
    let mut wm = wm_linux();
    wm.apply(parse_colon_command(":keys explain Primary+L").unwrap().unwrap()).unwrap();
    let colon = JsonValue::parse(&wm.take_command_data().unwrap()).unwrap();
    assert_eq!(rpc, colon);
    assert_eq!(rpc.get("action").and_then(JsonValue::as_str), Some("LayoutCycle"));
    assert_eq!(rpc.get("origin").and_then(JsonValue::as_str), Some("profile"));
    let mcp = mcp_keys(&mut p, &[("action", s("explain")), ("chord", s("Primary+L"))]).unwrap();
    assert_eq!(mcp, rpc);
    assert!(p.dispatch_method("keys.explain", &params(&[("chord", s("Hyper+Q"))])).is_err());
    assert!(p.dispatch_method("keys.explain", &params(&[])).unwrap_err().starts_with("missing "));
    let list = p.dispatch_method("keys.list", &params(&[])).unwrap();
    assert_eq!(list.get("profile").and_then(JsonValue::as_str), Some("linux"));
    assert_eq!(mcp_keys(&mut p, &[("action", s("list"))]).unwrap(), list);
}

fn bind_cfg(chord: &str) -> BiltermConfig {
    let mut c = BiltermConfig::default();
    c.keys.bind.insert(chord.to_string(), ":help".to_string());
    c
}

#[test]
fn user_binding_block_rejected_per_os() {
    let table: &[(Option<HostOs>, &str, bool)] = &[
        (Some(HostOs::MacOs), "Cmd+Space", true),
        (Some(HostOs::Windows), "Alt+F4", true),
        (Some(HostOs::Linux), "Super+L", true),
        (Some(HostOs::MacOs), "Alt+F4", false),
        (Some(HostOs::Linux), "Ctrl+A", false),
        (None, "Ctrl+Q", true),
        (None, "ctrl+q", true),
    ];
    for (os, chord, expect_err) in table {
        for o in os.map_or(vec![HostOs::MacOs, HostOs::Windows, HostOs::Linux], |o| vec![o]) {
            for surface in [Surface::Tui, Surface::Gui] {
                let env = KeyEnv::new(o, surface);
                let r = keyconflict::validate_user_bindings(&bind_cfg(chord), &env);
                assert_eq!(r.is_err(), *expect_err, "{o:?} {surface:?} {chord}: {r:?}");
            }
        }
    }
    let err = keyconflict::validate_user_bindings(&bind_cfg("Alt+F4"), &KeyEnv::new(HostOs::Windows, Surface::Tui)).unwrap_err().to_string();
    assert!(err.contains("key binding 'Alt+F4' is reserved by Windows (close window)"), "{err}");
    // `[keybindings]` is covered too, and Primary resolves before matching.
    let mut legacy = BiltermConfig::default();
    legacy.keybindings.insert("Alt+F4".into(), ":help".into());
    assert!(keyconflict::validate_user_bindings(&legacy, &KeyEnv::new(HostOs::Windows, Surface::Tui)).is_err());
    assert!(keyconflict::validate_user_bindings(&bind_cfg("Primary+Space"), &KeyEnv::new(HostOs::MacOs, Surface::Gui)).is_err());
    assert!(keyconflict::validate_user_bindings(&bind_cfg("Primary+Space"), &KeyEnv::new(HostOs::Linux, Surface::Gui)).is_ok());
}

#[test]
fn load_policy_falls_back_to_default_keys_on_block() {
    let env = KeyEnv::new(HostOs::Windows, Surface::Gui);
    let mut cfg = bind_cfg("Alt+F4");
    cfg.keys.profile = KeyProfileName::Portable;
    let msg = keyconflict::enforce_load_policy(&mut cfg, &env).expect("rejected");
    assert!(msg.starts_with("config error: key binding 'Alt+F4' is reserved by Windows"), "{msg}");
    assert!(msg.ends_with("(using default keys)"), "{msg}");
    assert_eq!(cfg.keys, KeysConfig::default());
    assert!(keyconflict::enforce_load_policy(&mut cfg, &env).is_none());
}

#[test]
fn load_time_warns_but_loads() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[keys.bind]\n\"Ctrl+A\" = \":split v\"\n").unwrap();
    let cfg = BiltermConfig::load(&path).expect("loads").expect("present");
    let env = KeyEnv::new(HostOs::Linux, Surface::Tui);
    keyconflict::validate_user_bindings(&cfg, &env).expect("Warn-class chords load");
    let km = KeyProfile::resolve(&cfg.keys, &env);
    let f = keyconflict::check_config(&km, &env, &cfg);
    let user: Vec<_> = f.iter().filter(|x| x.source == "keys.bind" && x.class == Class::Consumer).collect();
    assert_eq!(user.len(), 1, "{user:?}");
    assert_eq!(user[0].severity, Severity::Warn);
    assert!(user[0].consumer.contains("readline"));
    let status = keyconflict::load_time_status(&cfg, &env).expect("status");
    assert!(status.contains(":keys check") && status.starts_with("1 key conflict"), "{status}");
    // A clean config says nothing.
    assert_eq!(keyconflict::load_time_status(&BiltermConfig::default(), &env), None);
}

#[test]
fn shipped_defaults_are_clean_on_every_os_and_surface() {
    for os in [HostOs::MacOs, HostOs::Windows, HostOs::Linux] {
        for surface in [Surface::Tui, Surface::Gui] {
            for kitty in [false, true] {
                let env = KeyEnv::new(os, surface).with_kitty(kitty);
                for profile in [KeyProfileName::Auto, KeyProfileName::Portable] {
                    let cfg = BiltermConfig { keys: KeysConfig { profile, ..KeysConfig::default() }, ..BiltermConfig::default() };
                    let km = KeyProfile::resolve(&cfg.keys, &env);
                    let f = keyconflict::check_config(&km, &env, &cfg);
                    let bad: Vec<_> = f.iter().filter(|x| x.severity <= Severity::Warn).collect();
                    assert!(bad.is_empty(), "{os:?} {surface:?} kitty={kitty} {profile:?}: {bad:?}");
                    assert!(keyconflict::load_status(&f).is_none());
                }
            }
        }
    }
}

#[test]
fn keys_profile_write_gate() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = AutomationControlPlane::new_headless(24, 80).with_snapshot_dir(dir.path().to_path_buf());
    assert!(!p.wm.config().api.allow_config_write);

    // Read-only methods work for a read-only client.
    for m in ["keys.list", "keys.check"] {
        p.dispatch_method(m, &params(&[])).unwrap_or_else(|e| panic!("{m}: {e}"));
    }
    p.dispatch_method("keys.explain", &params(&[("chord", s("Ctrl+B"))])).unwrap();
    p.dispatch_method("keys.profile", &params(&[])).unwrap();
    mcp_keys(&mut p, &[("action", s("check"))]).unwrap();

    // Mutations are refused and change nothing.
    let before = p.wm.keymap().profile();
    for (m, args) in [
        ("keys.profile", params(&[("name", s("mac"))])),
        ("keys.window_mode", params(&[("op", s("on"))])),
        ("menu.open", params(&[])),
    ] {
        let e = p.dispatch_method(m, &args).unwrap_err();
        assert!(e.contains("allow_config_write"), "{m}: {e}");
    }
    assert!(mcp_keys(&mut p, &[("action", s("profile")), ("name", s("mac"))]).is_err());
    let e = p.dispatch_method("wm.command", &params(&[("command", s(":keys profile mac"))])).unwrap_err();
    assert!(e.contains("allow_config_write"), "{e}");
    assert_eq!(p.wm.keymap().profile(), before);
    assert_eq!(p.wm.key_mode(), KeyMode::Normal);
    assert_eq!(p.wm.menubar_state(), None);

    // With the gate open they work, and unknown names list the valid ones.
    p.wm.config_mut().api.allow_config_write = true;
    let out = p.dispatch_method("keys.profile", &params(&[("name", s("mac"))])).unwrap();
    assert_eq!(out.get("profile").and_then(JsonValue::as_str), Some("mac"));
    assert_eq!(p.wm.keymap().profile(), KeyProfileName::Mac);
    let e = p.dispatch_method("keys.profile", &params(&[("name", s("bogus"))])).unwrap_err();
    assert!(e.contains("valid:") && e.contains("portable"), "{e}");
    assert!(p.dispatch_method("keys.window_mode", &params(&[("op", s("sideways"))])).is_err());
    assert!(p.dispatch_method("keys.window_mode", &params(&[])).unwrap_err().starts_with("missing "));
}

#[test]
fn rpc_and_mcp_surfaces_are_listed() {
    for m in ["keys.list", "keys.check", "keys.explain", "keys.profile", "keys.window_mode", "menu.open"] {
        assert!(bilterm::api::RPC_METHODS.contains(&m), "{m} in RPC_METHODS");
    }
    let mut p = plane();
    let resp = JsonValue::parse(&p.handle_mcp_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap()).unwrap();
    let tools = resp.get("result").and_then(|r| r.get("tools")).and_then(JsonValue::as_array).unwrap();
    assert!(tools.iter().any(|t| t.get("name").and_then(JsonValue::as_str) == Some("bilterm_keys")));
    let e = mcp_keys(&mut p, &[("action", s("nope"))]).unwrap_err();
    assert!(e.contains("invalid 'action'"), "{e}");
}

#[test]
fn chord_caps() {
    let mut big = String::from("[keys.bind]\n");
    for i in 0..513 {
        big.push_str(&format!("\"Ctrl+Alt+F{}x{i}\" = \":help\"\n", i % 20));
    }
    assert!(BiltermConfig::from_toml_str(&big).is_err(), "513 [keys.bind] entries rejected");
    let long = format!("[keys.bind]\n\"{}\" = \":help\"\n", "Ctrl+".repeat(13));
    assert_eq!(long.lines().nth(1).unwrap().trim_matches(|c| c == '"' || c == ' ').split('"').next().unwrap().len(), 65);
    assert!(BiltermConfig::from_toml_str(&long).is_err(), "65-char chord rejected");
    // 512 entries and a 64-char chord are still accepted by the deserializer.
    let mut ok = String::from("[keys.bind]\n");
    for i in 0..512 {
        ok.push_str(&format!("\"x{i}\" = \":help\"\n"));
    }
    assert!(BiltermConfig::from_toml_str(&ok).is_ok());
    // And the post-parse validation rejects an oversized table built in code.
    let mut cfg = KeysConfig::default();
    for i in 0..513 {
        cfg.bind.insert(format!("Ctrl+Alt+{i}"), ":help".into());
    }
    assert!(cfg.validate().is_err());
}

#[test]
fn set_revalidates_blocked_key_bindings() {
    // `:set` goes through `dotted::set_key`, which re-runs the load-time Block check. The host OS
    // decides which chord is reserved, so use `Ctrl+Q` (reserved everywhere).
    let mut cfg = BiltermConfig::default();
    let r = bilterm::config::dotted::set_key(&mut cfg, "keys.bind", "{ \"Ctrl+Q\" = \":help\" }");
    let e = r.expect_err("Ctrl+Q cannot be rebound through :set");
    assert!(e.contains("key binding 'Ctrl+Q' is reserved by Bilterm Exit Guard"), "{e}");
    assert!(cfg.keys.bind.is_empty());
    bilterm::config::dotted::set_key(&mut cfg, "keys.bind", "{ \"Ctrl+Alt+Z\" = \":help\" }").unwrap();
    assert_eq!(cfg.keys.bind.len(), 1);
}

#[test]
fn copy_mode_entry_contract_for_phase_62() {
    use bilterm::input::keyhint_window::window_mode_sequence;
    for (os, profile) in [
        (HostOs::MacOs, KeyProfileName::Mac),
        (HostOs::Windows, KeyProfileName::Windows),
        (HostOs::Linux, KeyProfileName::Linux),
        (HostOs::Linux, KeyProfileName::Portable),
        (HostOs::Linux, KeyProfileName::Legacy),
    ] {
        for surface in [Surface::Tui, Surface::Gui] {
            let env = KeyEnv::new(os, surface).with_kitty(true);
            let km = KeyProfile::resolve(&KeysConfig { profile, ..KeysConfig::default() }, &env);
            let leader = km.leader_keys_for_action(&Action::CopyModeToggle);
            assert!(leader.contains(&KeyCode::Char('[')), "{profile:?} leader [");
            assert!(leader.contains(&KeyCode::F(8)), "{profile:?} leader F8");
            let chords = km.keys_for_action(&Action::CopyModeToggle);
            assert!(chords.iter().any(|c| c.code == KeyCode::F(8) && !c.primary && c.mods.is_empty()), "{profile:?} F8 chord");
            assert!(
                !chords.iter().any(|c| c.primary && matches!(c.code, KeyCode::Char('c' | 'x' | 'v'))),
                "{profile:?}: Primary+C/X/V stay with the host clipboard"
            );
            // Window mode `[` / `c` (63-02 fills `window_mode_sequence`; vacuous until then).
            if let Some(seq) = window_mode_sequence(&Action::CopyModeToggle) {
                assert!(seq == "[" || seq == "c", "{seq}");
            }
        }
    }
}
