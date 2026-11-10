//! Phase 66 (LIVE-01..03) and Phase 67 (HERDR-01..03): always-on foreground RPC socket, child env
//! injection, multi-instance discovery, selector resolution, smart prompt injection, and the
//! `bilterm agent|pane|workspace` CLI.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bilterm::api::instance_registry::{
    DEFAULT_SOCK, InstanceGuard, default_link_path, instance_socket_path, instances_dir,
    list_live_instances, resolve_client_socket,
};
use bilterm::api::selector::{Selector, parse_selector, resolve_in};
use bilterm::api::{AutomationControlPlane, JsonValue, send_rpc_request};
use bilterm::cli::{CliEnv, RpcCall, parse, resolve_endpoint};
use bilterm::pane::CompositePane;
use bilterm::pty::{PtyConfig, live_child_env};
use bilterm::wm::WindowManager;
use bilterm::wm::agent_state::AgentState;

/// Points every XDG state/config/data dir at a throwaway root (once) so session and history
/// writes never touch the developer's real profile.
fn isolate() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let root = std::env::temp_dir().join(format!("bt66-home-{}", std::process::id()));
        let _ = std::fs::create_dir_all(root.join("home"));
        // SAFETY: runs once, before any plane or pane of this test binary is created.
        unsafe {
            std::env::set_var("XDG_STATE_HOME", root.join("state"));
            std::env::set_var("XDG_CONFIG_HOME", root.join("config"));
            std::env::set_var("XDG_DATA_HOME", root.join("data"));
            std::env::set_var("HOME", root.join("home"));
        }
    });
}

/// `bilterm::pty::set_live_socket` is process-global: tests that set it (directly or through
/// `LiveRpc::start`) hold this lock so a parallel test cannot swap the path under a spawning child.
fn live_socket_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn runtime_dir(tag: &str) -> PathBuf {
    // Unix socket paths are limited to ~104 bytes (macOS); its per-user temp dir alone is ~50, so
    // sockets live under a short root instead of `std::env::temp_dir()`.
    let root = if cfg!(unix) { PathBuf::from("/tmp") } else { std::env::temp_dir() };
    let dir = root.join(format!("bt66-{tag}-{}-{}", std::process::id(), bilterm::secure_fs::unique_suffix()));
    bilterm::secure_fs::ensure_private_dir(&dir).unwrap();
    dir
}

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(ToString::to_string).collect()
}

fn obj(entries: &[(&str, JsonValue)]) -> JsonValue {
    JsonValue::obj(entries.iter().map(|(k, v)| (k.to_string(), v.clone())).collect::<Vec<_>>())
}

fn js(s: &str) -> JsonValue {
    JsonValue::String(s.to_string())
}

fn plane_with_panes(n: usize) -> AutomationControlPlane {
    isolate();
    let mut cp = AutomationControlPlane::new_headless(24, 80);
    for _ in 1..n {
        cp.dispatch_method("pane.split", &obj(&[("direction", js("v"))])).unwrap();
    }
    assert_eq!(cp.wm.pane_count(), n);
    cp
}

// ------------------------------------------------------------------ LIVE-02 env injection

#[test]
fn live_child_env_has_all_four_variables() {
    let env = live_child_env(Some(Path::new("/run/user/1000/bilterm/instances/42.sock")), 42, "w1", 7);
    let get = |k: &str| env.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    assert_eq!(get("BILTERM_SOCKET"), Some("/run/user/1000/bilterm/instances/42.sock"));
    assert_eq!(get("BILTERM_INSTANCE_PID"), Some("42"));
    assert_eq!(get("BILTERM_WORKSPACE_ID"), Some("w1"));
    assert_eq!(get("BILTERM_PANE_ID"), Some("7"));
    let no_sock = live_child_env(None, 42, "w1", 7);
    assert!(no_sock.iter().all(|(k, _)| k != "BILTERM_SOCKET"));
}

#[test]
fn spawned_child_pty_receives_bilterm_env() {
    isolate();
    let _live = live_socket_lock();
    bilterm::pty::set_live_socket(Some(PathBuf::from("/tmp/bt66-env-test.sock")));
    let cfg = PtyConfig {
        program: "/bin/sh".to_string(),
        args: args(&["-c", "printf 'ENV[%s|%s|%s|%s]' \"$BILTERM_SOCKET\" \"$BILTERM_INSTANCE_PID\" \"$BILTERM_WORKSPACE_ID\" \"$BILTERM_PANE_ID\"; sleep 2"]),
        ..PtyConfig::default()
    };
    let mut pane = CompositePane::spawn(9, cfg).expect("spawn");
    let want = format!("ENV[/tmp/bt66-env-test.sock|{}|w1|9]", std::process::id());
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut screen = String::new();
    while Instant::now() < deadline {
        pane.poll_pty_events();
        let (rows, _) = pane.vt().size();
        screen = (0..rows).map(|r| pane.vt().row_text(r)).collect::<Vec<_>>().join("\n");
        if screen.contains(&want) {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    bilterm::pty::set_live_socket(None);
    assert!(screen.contains(&want), "child env missing; screen was: {screen:?}");
}

// ------------------------------------------------------------------ LIVE-03 registry

#[test]
fn registry_registers_enumerates_and_prunes_stale() {
    let rt = runtime_dir("reg");
    let pid = std::process::id();
    let guard = InstanceGuard::register(&rt, pid).unwrap();
    assert_eq!(guard.socket_path(), instance_socket_path(&rt, pid));
    // The directory is private (0700) and the socket file is what discovery keys on.
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(instances_dir(&rt)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
    std::fs::write(guard.socket_path(), b"").unwrap();
    // Stale (dead pid), forged and malformed names.
    let stale = instances_dir(&rt).join("2000000000.sock");
    std::fs::write(&stale, b"").unwrap();
    for bogus in ["abc.sock", "0.sock", "1.sock", "-5.sock", "99999999999.sock", "12.txt"] {
        std::fs::write(instances_dir(&rt).join(bogus), b"").unwrap();
    }
    let live = list_live_instances(&rt);
    assert_eq!(live.len(), 1, "{live:?}");
    assert_eq!(live[0].pid, pid);
    assert!(!stale.exists(), "stale instance socket must be pruned");
    drop(guard);
    assert!(list_live_instances(&rt).is_empty());
    let _ = std::fs::remove_dir_all(&rt);
}

#[test]
fn registry_default_link_follows_newest_and_never_clobbers_daemon_socket() {
    let rt = runtime_dir("link");
    let me = std::process::id();
    let parent = std::os::unix::process::parent_id();
    let a = InstanceGuard::register(&rt, parent).unwrap();
    std::fs::write(a.socket_path(), b"").unwrap();
    assert!(a.publish_default_link().unwrap());
    assert_eq!(std::fs::read_link(default_link_path(&rt)).unwrap(), a.socket_path());
    let b = InstanceGuard::register(&rt, me).unwrap();
    std::fs::write(b.socket_path(), b"").unwrap();
    assert!(b.publish_default_link().unwrap());
    assert_eq!(std::fs::read_link(default_link_path(&rt)).unwrap(), b.socket_path());
    // Dropping the newest repoints the link at the surviving instance.
    drop(b);
    assert_eq!(std::fs::read_link(default_link_path(&rt)).unwrap(), a.socket_path());
    drop(a);
    assert!(std::fs::symlink_metadata(default_link_path(&rt)).is_err());

    // A real socket at bilterm.sock (headless daemon) is never replaced.
    let _daemon = std::os::unix::net::UnixListener::bind(rt.join(DEFAULT_SOCK)).unwrap();
    let c = InstanceGuard::register(&rt, me).unwrap();
    assert!(!c.publish_default_link().unwrap());
    assert!(!std::fs::symlink_metadata(default_link_path(&rt)).unwrap().file_type().is_symlink());
    drop(c);
    let _ = std::fs::remove_dir_all(&rt);
}

#[test]
fn client_socket_resolution_order() {
    let _live = live_socket_lock();
    let rt = runtime_dir("res");
    assert_eq!(resolve_client_socket(&rt, Some("/x/y.sock")), Some(PathBuf::from("/x/y.sock")));
    assert_eq!(resolve_client_socket(&rt, Some("unix:/x/y.sock")), Some(PathBuf::from("/x/y.sock")));
    assert_eq!(resolve_client_socket(&rt, None), None);
    let g = InstanceGuard::register(&rt, std::process::id()).unwrap();
    std::fs::write(g.socket_path(), b"").unwrap();
    assert_eq!(resolve_client_socket(&rt, None), Some(g.socket_path().to_path_buf()));
    drop(g);
    let _ = std::fs::remove_dir_all(&rt);
}

// ------------------------------------------------------------------ LIVE-01 live socket

#[tokio::test]
// The guard only serialises tests that set the process-global live socket; nothing else contends for it.
#[allow(clippy::await_holding_lock)]
async fn foreground_socket_executes_against_the_live_window_manager() {
    isolate();
    let _live = live_socket_lock();
    let rt = runtime_dir("live");
    // SAFETY: this is the only test in this binary that reads XDG_RUNTIME_DIR.
    unsafe { std::env::set_var("XDG_RUNTIME_DIR", &rt) };
    let mut wm = WindowManager::new_headless(24, 80);
    let (mut rx, mut handler) = bilterm::api::live::LiveRpc::start(wm.traffic_meter()).expect("live rpc starts");
    let sock = handler.socket_path().to_path_buf();
    assert_eq!(sock, instance_socket_path(&rt.join("bilterm"), std::process::id()));
    let endpoint = format!("unix:{}", sock.display());
    assert_eq!(std::fs::read_link(default_link_path(&rt.join("bilterm"))).unwrap(), sock);

    let ep = endpoint.clone();
    let client = tokio::spawn(async move {
        send_rpc_request(&ep, "pane.split", obj(&[("direction", js("v"))])).await.unwrap()
    });
    let (line, reply) = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
    handler.handle(&mut wm, &line, reply);
    let resp = client.await.unwrap();
    assert!(resp.contains("\"split\""), "{resp}");
    assert_eq!(wm.pane_count(), 2, "the live window manager itself must have been mutated");

    let ep = endpoint.clone();
    let client = tokio::spawn(async move { send_rpc_request(&ep, "pane.list", obj(&[])).await.unwrap() });
    let (line, reply) = rx.recv().await.unwrap();
    handler.handle(&mut wm, &line, reply);
    let parsed = JsonValue::parse(&client.await.unwrap()).unwrap();
    let panes = parsed.get("result").and_then(|r| r.get("panes")).and_then(JsonValue::as_array).unwrap().len();
    assert_eq!(panes, 2);

    // Socket is private.
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(&sock).unwrap().permissions().mode() & 0o777, 0o600);
    drop(handler);
    assert!(!sock.exists(), "socket removed on shutdown");
    let _ = std::fs::remove_dir_all(&rt);
}

// ------------------------------------------------------------------ HERDR-02 selectors

#[test]
fn selector_grammar() {
    assert_eq!(parse_selector("w1:p3").unwrap(), Selector::Pane { workspace: 1, pane: 3 });
    assert_eq!(parse_selector("W2:P10").unwrap(), Selector::Pane { workspace: 2, pane: 10 });
    assert_eq!(parse_selector("p4").unwrap(), Selector::Pane { workspace: 1, pane: 4 });
    assert_eq!(parse_selector("5").unwrap(), Selector::Pane { workspace: 1, pane: 5 });
    assert_eq!(parse_selector("@idle").unwrap(), Selector::State(AgentState::Idle));
    assert_eq!(parse_selector("@working").unwrap(), Selector::State(AgentState::Working));
    assert_eq!(parse_selector("@blocked").unwrap(), Selector::State(AgentState::Blocked));
    assert_eq!(parse_selector("@focused").unwrap(), Selector::Focused);
    assert_eq!(parse_selector("@Claude").unwrap(), Selector::Named("claude".to_string()));
    assert_eq!(parse_selector("@agy").unwrap(), Selector::Named("agy".to_string()));
    for bad in ["", "@", "w1:3", "x1:p2", "w:p1", "p", "w1:p", "@a b", "@;rm", &"a".repeat(65)] {
        assert!(parse_selector(bad).is_err(), "{bad:?} should be rejected");
    }
}

#[test]
fn selector_resolution_in_a_live_plane() {
    let mut cp = plane_with_panes(3);
    cp.dispatch_method("window.rename", &obj(&[("window_id", JsonValue::Number(2)), ("new_name", js("claude-main"))])).unwrap();
    cp.dispatch_method("window.rename", &obj(&[("window_id", JsonValue::Number(3)), ("new_name", js("agy-helper"))])).unwrap();
    cp.wm.update_agent_states(Instant::now());
    cp.wm.report_agent_state(2, Some(AgentState::Blocked));
    cp.wm.report_agent_state(3, Some(AgentState::Working));

    let sel = |cp: &mut AutomationControlPlane, s: &str, from: Option<usize>| {
        let mut e = vec![("selector", js(s))];
        if let Some(f) = from {
            e.push(("from_pane", JsonValue::Number(f as i64)));
        }
        let mut p = obj(&e);
        if let JsonValue::Object(m) = &mut p {
            m.insert("until".to_string(), js("attention"));
        }
        cp.dispatch_method("agent.wait", &p).map(|r| r.get("pane_id").and_then(JsonValue::as_usize).unwrap())
    };
    assert_eq!(sel(&mut cp, "@claude", None), Ok(2));
    assert_eq!(sel(&mut cp, "@agy", None), Ok(3));
    assert_eq!(sel(&mut cp, "@blocked", None), Ok(2));
    assert_eq!(sel(&mut cp, "@working", None), Ok(3));
    assert_eq!(sel(&mut cp, "w1:p3", None), Ok(3));
    assert_eq!(sel(&mut cp, "p1", None), Ok(1));
    // The caller never selects itself through a semantic selector.
    assert!(sel(&mut cp, "@claude", Some(2)).is_err());
    assert!(sel(&mut cp, "w2:p1", None).unwrap_err().contains("not live"));
    assert!(sel(&mut cp, "p99", None).unwrap_err().contains("not found"));
    assert!(sel(&mut cp, "@nosuchagent", None).is_err());
}

#[test]
fn pure_resolution_prefers_lowest_pane_and_honours_exclusion() {
    use bilterm::wm::agent_state::AgentStatus;
    let st = |id: usize, state: AgentState, profile: Option<&str>| AgentStatus {
        pane_id: id,
        name: String::new(),
        state,
        since_ms: 0,
        profile: profile.map(str::to_string),
        reported: false,
    };
    let table = vec![st(1, AgentState::Idle, None), st(2, AgentState::Idle, Some("claude")), st(3, AgentState::Idle, Some("claude"))];
    let titles = |_id: usize| String::new();
    assert_eq!(resolve_in(&Selector::State(AgentState::Idle), &table, &titles, 1, None), Ok(1));
    assert_eq!(resolve_in(&Selector::State(AgentState::Idle), &table, &titles, 1, Some(1)), Ok(2));
    assert_eq!(resolve_in(&Selector::Named("claude".into()), &table, &titles, 1, Some(2)), Ok(3));
    assert_eq!(resolve_in(&Selector::Focused, &table, &titles, 3, None), Ok(3));
}

#[test]
fn agent_wait_selector_form_reports_met_or_pending_without_blocking() {
    let mut cp = plane_with_panes(2);
    cp.wm.update_agent_states(Instant::now());
    cp.wm.report_agent_state(2, Some(AgentState::Working));
    let p = obj(&[("selector", js("p2")), ("until", js("idle"))]);
    let r = cp.dispatch_method("agent.wait", &p).unwrap();
    assert_eq!(r.get("status").and_then(JsonValue::as_str), Some("pending"));
    assert_eq!(r.get("state").and_then(JsonValue::as_str), Some("working"));
    cp.wm.report_agent_state(2, Some(AgentState::Idle));
    let r = cp.dispatch_method("agent.wait", &p).unwrap();
    assert_eq!(r.get("status").and_then(JsonValue::as_str), Some("met"));
    assert!(cp.dispatch_method("agent.wait", &obj(&[("selector", js("p2")), ("until", js("bogus"))])).is_err());
}

// ------------------------------------------------------------------ HERDR-03 prompt injection

fn last_write(cp: &AutomationControlPlane, id: usize) -> Vec<u8> {
    cp.wm.pane(id).unwrap().last_pty_write().to_vec()
}

#[test]
fn prompt_injection_shell_floating_prompt_mode() {
    let mut cp = plane_with_panes(2);
    let r = cp.dispatch_method("agent.prompt", &obj(&[("selector", js("p2")), ("text", js("echo hi"))])).unwrap();
    assert_eq!(r.get("mode").and_then(JsonValue::as_str), Some("shell_prompt"));
    assert_eq!(r.get("ref").and_then(JsonValue::as_str), Some("w1:p2"));
    assert_eq!(last_write(&cp, 2), b"echo hi\n");
    // It went through the prompt path: a command block was recorded.
    assert_eq!(cp.wm.pane(2).unwrap().vt().command_blocks().last().unwrap().command, "echo hi");
}

#[test]
fn prompt_injection_raw_tui_bracketed_paste_when_decset_2004() {
    let mut cp = plane_with_panes(2);
    {
        let pane = cp.wm.pane_mut(2).unwrap();
        pane.process_pty_output(b"\x1b[?2004h\x1b[>1u");
        pane.line_discipline_mut().canonical_mode = false;
        pane.line_discipline_mut().foreground_pgid = Some(200);
        pane.line_discipline_mut().shell_pgid = Some(100);
    }
    let r = cp.dispatch_method("agent.prompt", &obj(&[("selector", js("p2")), ("text", js("plan the driver\nwith two lines"))])).unwrap();
    assert_eq!(r.get("mode").and_then(JsonValue::as_str), Some("raw_bracketed_paste"));
    assert_eq!(last_write(&cp, 2), b"\x1b[200~plan the driver\nwith two lines\x1b[201~\r");
    // The paste terminator cannot be smuggled in to break out of the paste.
    cp.dispatch_method("agent.prompt", &obj(&[("selector", js("p2")), ("text", js("a\x1b[201~b"))])).unwrap();
    assert_eq!(last_write(&cp, 2), b"\x1b[200~ab\x1b[201~\r");
}

#[test]
fn prompt_injection_raw_tui_without_bracketed_paste_uses_plain_cr() {
    let mut cp = plane_with_panes(2);
    cp.wm.pane_mut(2).unwrap().process_pty_output(b"\x1b[?1049h");
    let r = cp.dispatch_method("agent.prompt", &obj(&[("selector", js("p2")), ("text", js("hello\nworld"))])).unwrap();
    assert_eq!(r.get("mode").and_then(JsonValue::as_str), Some("raw_plain"));
    assert_eq!(last_write(&cp, 2), b"hello\rworld\r");
}

#[test]
fn prompt_injection_is_guarded() {
    let mut cp = plane_with_panes(2);
    cp.wm.pane_mut(2).unwrap().set_read_only(true);
    let e = cp.dispatch_method("agent.prompt", &obj(&[("selector", js("p2")), ("text", js("x"))])).unwrap_err();
    assert!(e.contains("read-only"), "{e}");
    assert!(cp.dispatch_method("agent.prompt", &obj(&[("selector", js("p1"))])).is_err(), "text required");
    assert!(cp.dispatch_method("agent.prompt", &obj(&[("selector", js("p1")), ("text", js("x")), ("from_pane", JsonValue::Number(77))])).is_err());
}

#[test]
fn approve_requires_blocked_unless_forced_and_sends_enter() {
    let mut cp = plane_with_panes(2);
    cp.wm.update_agent_states(Instant::now());
    let p = obj(&[("selector", js("p2"))]);
    let e = cp.dispatch_method("agent.approve", &p).unwrap_err();
    assert!(e.contains("not blocked"), "{e}");
    cp.wm.report_agent_state(2, Some(AgentState::Blocked));
    let r = cp.dispatch_method("agent.approve", &p).unwrap();
    assert_eq!(r.get("status").and_then(JsonValue::as_str), Some("approved"));
    assert_eq!(last_write(&cp, 2), b"\r");
    cp.dispatch_method("agent.approve", &obj(&[("selector", js("p2")), ("answer", js("y"))])).unwrap();
    assert_eq!(last_write(&cp, 2), b"y\r");
    assert!(cp.dispatch_method("agent.approve", &obj(&[("selector", js("p2")), ("answer", js("rm"))])).is_err());
}

#[test]
fn last_turn_extracts_command_and_response() {
    let mut cp = plane_with_panes(2);
    cp.wm.pane_mut(2).unwrap().submit_command("echo marker").unwrap();
    cp.wm.pane_mut(2).unwrap().process_pty_output(b"$ echo marker\r\nanswer line one\r\nanswer line two\r\n");
    let r = cp.dispatch_method("agent.last_turn", &obj(&[("selector", js("p2"))])).unwrap();
    assert_eq!(r.get("command").and_then(JsonValue::as_str), Some("echo marker"));
    assert_eq!(r.get("anchored").and_then(JsonValue::as_bool), Some(true));
    let resp = r.get("response").and_then(JsonValue::as_str).unwrap();
    assert!(resp.contains("answer line one") && resp.contains("answer line two"), "{resp:?}");
    assert!(!resp.contains("echo marker"), "{resp:?}");
    let small = cp.dispatch_method("agent.last_turn", &obj(&[("selector", js("p2")), ("max_bytes", JsonValue::Number(8))])).unwrap();
    assert!(small.get("response").and_then(JsonValue::as_str).unwrap().len() <= 8);
}

// ------------------------------------------------------------------ pane / workspace RPC

#[test]
fn pane_and_workspace_rpc_lower_to_actions() {
    let mut cp = plane_with_panes(3);
    cp.dispatch_method("pane.focus", &obj(&[("selector", js("w1:p1"))])).unwrap();
    assert_eq!(cp.wm.active_pane_id(), 1);
    cp.dispatch_method("pane.zoom", &obj(&[("selector", js("p2"))])).unwrap();
    assert_eq!(cp.wm.zoomed_pane(), Some(2));
    cp.dispatch_method("pane.zoom", &obj(&[])).unwrap();
    assert_eq!(cp.wm.zoomed_pane(), None);
    cp.dispatch_method("pane.send", &obj(&[("selector", js("p3")), ("text", js("ls")), ("enter", JsonValue::Bool(true))])).unwrap();
    assert_eq!(last_write(&cp, 3), b"ls\r");
    let listed = cp.dispatch_method("pane.list", &obj(&[])).unwrap();
    assert_eq!(listed.get("panes").and_then(JsonValue::as_array).unwrap().len(), 3);
    let read = cp.dispatch_method("pane.read", &obj(&[("selector", js("p2"))])).unwrap();
    assert_eq!(read.get("pane_id").and_then(JsonValue::as_usize), Some(2));

    let ws = cp.dispatch_method("workspace.list", &obj(&[])).unwrap();
    let first = &ws.get("workspaces").and_then(JsonValue::as_array).unwrap()[0];
    assert_eq!(first.get("id").and_then(JsonValue::as_str), Some("w1"));
    cp.dispatch_method("workspace.rename", &obj(&[("name", js("herd-alpha"))])).unwrap();
    assert_eq!(cp.wm.config().history_session, "herd-alpha");
    assert!(cp.dispatch_method("workspace.rename", &obj(&[("name", js("../evil"))])).is_err());
    assert!(cp.dispatch_method("workspace.new", &obj(&[])).is_err(), "name is required");
    assert!(cp.dispatch_method("workspace.select", &obj(&[("name", js("w99999"))])).is_err());
}

// ------------------------------------------------------------------ HERDR-01 CLI parsing

fn env(pane: Option<usize>) -> CliEnv {
    CliEnv { pane_id: pane, socket: None }
}

fn call(a: &[&str], pane: Option<usize>) -> RpcCall {
    parse(&args(a), &env(pane)).unwrap_or_else(|e| panic!("{a:?}: {e}"))
}

fn p<'a>(c: &'a RpcCall, k: &str) -> Option<&'a str> {
    c.params.get(k).and_then(JsonValue::as_str)
}

#[test]
fn cli_agent_subcommands() {
    let c = call(&["agent", "list"], None);
    assert_eq!(c.method, "agents.list");
    let c = call(&["agent", "prompt", "@claude", "fix", "the", "bug"], Some(4));
    assert_eq!((c.method.as_str(), p(&c, "selector"), p(&c, "text")), ("agent.prompt", Some("@claude"), Some("fix the bug")));
    assert_eq!(c.params.get("from_pane").and_then(JsonValue::as_usize), Some(4), "BILTERM_PANE_ID becomes from_pane");
    let c = call(&["agent", "wait", "w1:p2", "--until", "blocked", "--timeout-ms", "1500"], None);
    assert_eq!((c.method.as_str(), p(&c, "until")), ("agent.wait", Some("blocked")));
    assert_eq!(c.poll.unwrap().timeout_ms, 1500);
    assert!(c.params.get("from_pane").is_none());
    assert_eq!(call(&["agent", "wait", "@idle"], None).poll.unwrap().timeout_ms, 60_000);
    let c = call(&["agent", "approve", "@blocked", "--answer", "y", "--force"], None);
    assert_eq!((c.method.as_str(), p(&c, "answer")), ("agent.approve", Some("y")));
    assert_eq!(c.params.get("force").and_then(JsonValue::as_bool), Some(true));
    let c = call(&["agent", "interrupt", "p3"], None);
    assert_eq!((c.method.as_str(), p(&c, "text")), ("pane.send", Some("\u{3}")));
    let c = call(&["agent", "last-turn", "@agy", "--max-bytes", "500"], None);
    assert_eq!(c.method, "agent.last_turn");
    assert_eq!(c.params.get("max_bytes").and_then(JsonValue::as_i64), Some(500));
}

#[test]
fn cli_pane_and_workspace_subcommands() {
    assert_eq!(call(&["pane", "list"], None).method, "pane.list");
    let c = call(&["pane", "read", "p2"], None);
    assert_eq!((c.method.as_str(), p(&c, "selector")), ("pane.read", Some("p2")));
    assert!(call(&["pane", "read"], None).params.get("selector").is_none());
    let c = call(&["pane", "split", "v", "p2"], None);
    assert_eq!((c.method.as_str(), p(&c, "direction"), p(&c, "selector")), ("pane.split", Some("v"), Some("p2")));
    let c = call(&["pane", "split"], None);
    assert_eq!(p(&c, "direction"), Some("h"));
    assert_eq!(call(&["pane", "focus", "@idle"], None).method, "pane.focus");
    assert_eq!(call(&["pane", "zoom"], None).method, "pane.zoom");
    let c = call(&["pane", "send", "p2", "ls", "-la", "--enter"], None);
    assert_eq!((c.method.as_str(), p(&c, "text")), ("pane.send", Some("ls -la")));
    assert_eq!(c.params.get("enter").and_then(JsonValue::as_bool), Some(true));
    assert_eq!(call(&["workspace", "list"], None).method, "workspace.list");
    for (sub, m) in [("select", "workspace.select"), ("new", "workspace.new"), ("rename", "workspace.rename")] {
        let c = call(&["workspace", sub, "alpha"], None);
        assert_eq!((c.method.as_str(), p(&c, "name")), (m, Some("alpha")));
    }
}

#[test]
fn cli_rejects_bad_input_with_usage() {
    for bad in [
        vec!["agent"],
        vec!["agent", "frobnicate"],
        vec!["agent", "prompt", "@claude"],
        vec!["agent", "prompt"],
        vec!["agent", "wait"],
        vec!["agent", "wait", "p1", "--timeout-ms", "soon"],
        vec!["agent", "wait", "p1", "--bogus"],
        vec!["pane", "focus"],
        vec!["pane", "send", "p1"],
        vec!["workspace", "new"],
        vec!["workspace", "teleport"],
    ] {
        assert!(parse(&args(&bad), &env(None)).is_err(), "{bad:?} should fail");
    }
    assert!(bilterm::cli::usage().contains("bilterm agent prompt"));
    assert!(bilterm::cli::is_cli_command(Some("agent")) && bilterm::cli::is_cli_command(Some("pane")) && bilterm::cli::is_cli_command(Some("workspace")));
    assert!(!bilterm::cli::is_cli_command(Some("ctl")) && !bilterm::cli::is_cli_command(None));
}

#[test]
fn cli_endpoint_prefers_env_socket_then_registry() {
    let rt = runtime_dir("cli");
    let e = CliEnv { pane_id: None, socket: Some("/run/x/9.sock".to_string()) };
    assert_eq!(resolve_endpoint(&e, Some(&rt)).as_deref(), Some("unix:/run/x/9.sock"));
    assert_eq!(resolve_endpoint(&env(None), Some(&rt)), None);
    let g = InstanceGuard::register(&rt, std::process::id()).unwrap();
    std::fs::write(g.socket_path(), b"").unwrap();
    assert_eq!(resolve_endpoint(&env(None), Some(&rt)), Some(format!("unix:{}", g.socket_path().display())));
    drop(g);
    let _ = std::fs::remove_dir_all(&rt);
}

#[tokio::test]
async fn cli_wait_polls_until_state_met() {
    // A tiny server that answers `pending` twice, then `met`: proves run_call polls and stops.
    let rt = runtime_dir("poll");
    let sock = rt.join("s.sock");
    let listener = tokio::net::UnixListener::bind(&sock).unwrap();
    tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        for status in ["pending", "pending", "met"] {
            let (s, _) = listener.accept().await.unwrap();
            let (rd, mut wr) = tokio::io::split(s);
            let mut line = String::new();
            BufReader::new(rd).read_line(&mut line).await.unwrap();
            let body = format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{{\"status\":\"{status}\"}}}}\n");
            wr.write_all(body.as_bytes()).await.unwrap();
        }
    });
    let c = call(&["agent", "wait", "p1", "--timeout-ms", "5000"], None);
    let out = bilterm::cli::run_call(&c, &format!("unix:{}", sock.display()), None).await.unwrap();
    assert!(out.contains("\"met\""), "{out}");
    let _ = std::fs::remove_dir_all(&rt);
}
