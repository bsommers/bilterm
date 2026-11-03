//! Phase 49-04: `:get`, `:set`, `:config list|save`, `:state` locally and over RPC/MCP, with the
//! `[api] allow_config_write` gate.

use std::sync::Mutex;

use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::wm::WindowManager;

static ENV_LOCK: Mutex<()> = Mutex::new(());
const DISABLED: &str =
    "config writes over the API are disabled; set [api] allow_config_write = true in config.toml";

fn with_temp_xdg<T>(f: impl FnOnce(&std::path::Path) -> T) -> T {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("bilterm-api-cfg-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY-equivalent: serialized by ENV_LOCK; every test in this file goes through here.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("XDG_STATE_HOME", &dir);
    }
    f(&dir)
}

fn rpc(plane: &mut AutomationControlPlane, cmd: &str) -> JsonValue {
    let line = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"wm.command","params":{{"command":"{cmd}"}}}}"#
    );
    JsonValue::parse(&plane.handle_jsonrpc_line(&line)).expect("json")
}

fn rpc_err(v: &JsonValue) -> String {
    v.get("error").and_then(|e| e.get("message")).and_then(JsonValue::as_str).unwrap_or("").to_string()
}

fn mcp(plane: &mut AutomationControlPlane, cmd: &str) -> (bool, String) {
    let line = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"bilterm_colon_command","arguments":{{"command":"{cmd}"}}}}}}"#
    );
    let v = JsonValue::parse(&plane.handle_mcp_line(&line).unwrap()).unwrap();
    let r = v.get("result").expect("result");
    let is_err = r.get("isError").and_then(JsonValue::as_bool).unwrap_or(false);
    let text = match r.get("content") {
        Some(JsonValue::Array(a)) => a[0].get("text").and_then(JsonValue::as_str).unwrap().to_string(),
        _ => String::new(),
    };
    (is_err, text)
}

fn run_local(wm: &mut WindowManager, cmd: &str) {
    let action = parse_colon_command(cmd).unwrap().unwrap();
    execute_wm_colon_command(wm, action);
}

#[test]
fn local_get_set_list_state() {
    let mut wm = WindowManager::new_headless(24, 80);
    run_local(&mut wm, ":get appearance.profile");
    let d = JsonValue::parse(&wm.take_command_data().unwrap()).unwrap();
    assert_eq!(d.get("value").and_then(JsonValue::as_str), Some("(unset)"));
    run_local(&mut wm, ":set theme turbo-cyan");
    assert_eq!(bilterm::config::dotted::get_key(wm.config(), "theme").unwrap(), "\"turbo-cyan\"");
    run_local(&mut wm, ":set api.allow_config_write true"); // local prompt is not gated
    assert!(wm.config().api.allow_config_write);
    run_local(&mut wm, ":config list");
    let d = JsonValue::parse(&wm.take_command_data().unwrap()).unwrap();
    assert!(d.get("theme").is_some());
    run_local(&mut wm, ":state");
    let d = JsonValue::parse(&wm.take_command_data().unwrap()).unwrap();
    assert_eq!(d.get("pane_count").and_then(JsonValue::as_usize), Some(1));
    assert!(d.get("drawer_visible").is_some() && d.get("appearance").is_some());
    assert!(wm.take_command_data().is_none());
}

#[test]
fn api_writes_are_rejected_by_default_and_config_unchanged() {
    with_temp_xdg(|dir| {
        let mut plane = AutomationControlPlane::new_headless(24, 80);
        let before = plane.wm.config().clone();
        let r = rpc(&mut plane, ":set theme turbo-cyan");
        assert!(rpc_err(&r).contains(DISABLED), "{}", r.to_json_string());
        let r = rpc(&mut plane, ":config save");
        assert!(rpc_err(&r).contains(DISABLED));
        let r = rpc(&mut plane, ":appearance save");
        assert!(rpc_err(&r).contains(DISABLED));
        let (is_err, text) = mcp(&mut plane, ":set theme turbo-cyan");
        assert!(is_err && text.contains(DISABLED), "{text}");
        let (is_err, text) = mcp(&mut plane, ":config save");
        assert!(is_err && text.contains(DISABLED));
        assert_eq!(*plane.wm.config(), before);
        assert!(!dir.join("bilterm").join("config.toml").exists());
    });
}

#[test]
fn reads_always_work_over_api_and_carry_data() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let r = rpc(&mut plane, ":get theme");
    let data = r.get("result").and_then(|x| x.get("data")).expect("data field");
    assert!(data.get("value").and_then(JsonValue::as_str).is_some());
    let r = rpc(&mut plane, ":state");
    assert!(r.get("result").and_then(|x| x.get("data")).and_then(|d| d.get("layout_mode")).is_some());
    let r = rpc(&mut plane, ":config list");
    assert!(r.get("result").and_then(|x| x.get("data")).and_then(|d| d.get("theme")).is_some());
    let (is_err, text) = mcp(&mut plane, ":state");
    assert!(!is_err);
    assert!(JsonValue::parse(&text).unwrap().get("data").is_some());
    // Commands without data keep the legacy shape.
    let r = rpc(&mut plane, ":zoom");
    assert!(r.get("result").unwrap().get("data").is_none());
}

#[test]
fn api_writes_work_when_enabled_and_save_to_temp_config() {
    with_temp_xdg(|dir| {
        let mut plane = AutomationControlPlane::new_headless(24, 80);
        plane.wm.config_mut().api.allow_config_write = true;
        let r = rpc(&mut plane, ":set appearance.profile midnight");
        assert!(r.get("error").is_none(), "{}", r.to_json_string());
        let r = rpc(&mut plane, ":config save");
        let written = r.get("result").and_then(|x| x.get("data")).and_then(|d| d.get("written")).expect("written");
        assert!(written.to_json_string().contains("appearance.profile"));
        let path = dir.join("bilterm").join("config.toml");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("midnight") && text.contains("allow_config_write"), "{text}");
        let (is_err, _) = mcp(&mut plane, ":set font_size_pt 20");
        assert!(!is_err);
        assert_eq!(plane.wm.config().font_size_pt, 20);
    });
}

#[test]
fn api_can_never_change_the_api_section() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    plane.wm.config_mut().api.allow_config_write = true;
    let r = rpc(&mut plane, ":set api.allow_config_write false");
    assert!(rpc_err(&r).contains("[api]"), "{}", r.to_json_string());
    assert!(plane.wm.config().api.allow_config_write);
    let (is_err, text) = mcp(&mut plane, ":set api.allow_config_write true");
    assert!(is_err && text.contains("[api]"));
}

#[test]
fn send_keys_cannot_bypass_the_gate() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let line = r#"{"jsonrpc":"2.0","id":1,"method":"pane.send_keys","params":{"keys":[":","s","e","t"," ","f","o","n","t","_","s","i","z","e","_","p","t"," ","2","0","Enter"]}}"#;
    let _ = plane.handle_jsonrpc_line(line);
    assert_ne!(plane.wm.config().font_size_pt, 20);
}
