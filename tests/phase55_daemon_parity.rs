//! Phase 55 Wave 3 Test Suite: Surface Uniformity Parity Matrix (`DAEM-07`, `UNI-R01`).
//!
//! Validates that every daemon capability has parity across:
//! - Mouse click on top menubar dropdown
//! - Key chord (`Ctrl+X D ...`)
//! - Colon command (`:daemon ...`)
//! - Palette catalog entry (`PALETTE_CATALOG`)
//! - JSON-RPC method (`daemon.*`)
//! - MCP tool (`bilterm_daemon`)
//! - Cross-daemon `:find` session discovery

use std::path::PathBuf;

use bilterm::action::{Action, DaemonSupervisorOp};
use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::parse_colon_command;
use bilterm::daemon::registry::{DaemonRegistry, DaemonRegistryEntry};
use bilterm::daemon::supervisor::now_ms;
use bilterm::input::keymap::resolve_daemon_subchord;
use bilterm::ui::modals::{menubar_dropdown_rect, menubar_item_command, PALETTE_CATALOG};
use bilterm::wm::find_cmd::FindKind;
use bilterm::wm::registry::saved;
use crossterm::event::KeyCode;
use ratatui::layout::Rect;

const AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: 120,
    height: 40,
};

/// Points the runtime dir at a throwaway directory for the duration of a test, so nothing here
/// can lock, signal or unlink anything in the user's real `$XDG_RUNTIME_DIR/bilterm`.
fn sandbox_runtime() -> (std::sync::MutexGuard<'static, ()>, tempfile::TempDir) {
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().expect("sandbox runtime dir");
    // SAFETY: serialized by ENV_LOCK; every test in this file goes through this function.
    unsafe {
        std::env::set_var("XDG_RUNTIME_DIR", dir.path());
        std::env::remove_var("BILTERM_RUNTIME_DIR");
    }
    (guard, dir)
}

#[test]
fn test_daemon_surface_parity() {
    // This test takes the "default" daemon lock and sends `daemon.stop`.
    let (_env, _sandbox) = sandbox_runtime();
    let runtime_dir = bilterm::daemon::supervisor::resolve_runtime_dir().expect("runtime dir");
    let sock_path = bilterm::daemon::process::session_daemon_socket_path_in(&runtime_dir, "default").unwrap();
    let mut lock = Some(bilterm::daemon::supervisor::acquire_lock(&runtime_dir, "default", sock_path.clone()).unwrap());
    #[cfg(unix)]
    let _listener = std::os::unix::net::UnixListener::bind(&sock_path).ok();

    let matrix = vec![
        (
            "list",
            ":daemon list",
            "Ctrl+X D L",
            KeyCode::Char('l'),
            "daemon.list",
            "list",
            Action::DaemonSupervisor(DaemonSupervisorOp::List),
        ),
        (
            "status",
            ":daemon status",
            "Ctrl+X D S",
            KeyCode::Char('s'),
            "daemon.status",
            "status",
            Action::DaemonSupervisor(DaemonSupervisorOp::Status { name: None }),
        ),
        (
            "start",
            ":daemon start",
            "Ctrl+X D N",
            KeyCode::Char('n'),
            "daemon.start",
            "start",
            Action::DaemonSupervisor(DaemonSupervisorOp::Start { name: None }),
        ),
        (
            "stop",
            ":daemon stop",
            "Ctrl+X D Q",
            KeyCode::Char('q'),
            "daemon.stop",
            "stop",
            Action::DaemonSupervisor(DaemonSupervisorOp::Stop {
                name: None,
                force: false,
            }),
        ),
        (
            "switch",
            ":daemon switch default",
            "Ctrl+X D W",
            KeyCode::Char('w'),
            "daemon.switch",
            "switch",
            Action::DaemonSupervisor(DaemonSupervisorOp::Switch {
                name: "default".to_string(),
            }),
        ),
    ];

    let mut cp = AutomationControlPlane::new_headless(24, 80);

    for (name, colon, chord_str, subchord_code, rpc_method, mcp_action, canonical_action) in matrix {
        // A daemon refuses to stop its own process, so release the in-process lock first.
        if name == "stop" {
            lock.take();
        }
        // 1. Colon command parsing parity
        let parsed = parse_colon_command(colon)
            .unwrap_or_else(|| panic!("failed to recognize {colon}"))
            .unwrap_or_else(|e| panic!("error parsing {colon}: {e}"));
        assert_eq!(
            parsed, canonical_action,
            "colon command '{colon}' did not parse to expected action"
        );

        // 2. Key chord mapping parity (Ctrl+X D <key>)
        let resolved_chord = resolve_daemon_subchord(subchord_code)
            .unwrap_or_else(|| panic!("subchord {subchord_code:?} ({chord_str}) not resolved"));
        assert_eq!(
            resolved_chord, canonical_action,
            "chord '{chord_str}' did not map to expected action"
        );

        // 3. Palette catalog entry parity
        let palette_entry = PALETTE_CATALOG
            .iter()
            .find(|(cmd, _, _)| *cmd == colon || (*cmd).starts_with(colon.split_whitespace().next().unwrap()))
            .unwrap_or_else(|| panic!("no PALETTE_CATALOG entry for {colon}"));
        assert!(!palette_entry.2.is_empty(), "palette description must not be empty");

        // 4. JSON-RPC parity
        let rpc_params = match rpc_method {
            "daemon.switch" => JsonValue::obj([("name", JsonValue::String("default".to_string()))]),
            _ => JsonValue::Null,
        };
        let rpc_res = cp
            .dispatch_method(rpc_method, &rpc_params)
            .unwrap_or_else(|e| panic!("RPC method {rpc_method} failed: {e}"));
        assert!(!matches!(rpc_res, JsonValue::Null));

        // 5. MCP tool parity
        let mcp_call = format!(
            r#"{{"jsonrpc":"2.0","id":100,"method":"tools/call","params":{{"name":"bilterm_daemon","arguments":{{"action":"{mcp_action}","name":"default"}}}}}}"#
        );
        let mcp_res_str = cp.handle_mcp_line(&mcp_call).expect("mcp response");
        let mcp_res = JsonValue::parse(&mcp_res_str).expect("json parse");
        let is_err = mcp_res
            .get("result")
            .and_then(|r| r.get("isError"))
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);
        assert!(!is_err, "MCP bilterm_daemon action {mcp_action} returned error");

        // 6. Menubar item parity (Tools category)
        let found_menu = (0..8)
            .flat_map(|cat| (0..10).map(move |item| (cat, item)))
            .any(|(cat, item)| menubar_item_command(cat, item) == Some(colon));
        assert!(found_menu, "menubar entry missing for {name} ({colon})");
    }

    // Verify Menubar Tools dropdown rect computation
    let dropdown_rect = menubar_dropdown_rect(5, AREA);
    assert!(dropdown_rect.width > 0);
    assert!(dropdown_rect.height > 0);
}

#[test]
fn test_cross_daemon_find_indexing() {
    let (_env, _sandbox) = sandbox_runtime();
    let runtime_dir = bilterm::secure_fs::private_runtime_dir().expect("runtime dir");

    // Register 2 live peer daemons
    let peer_1 = DaemonRegistryEntry {
        id: "daem_find_01".to_string(),
        name: "worker-alpha".to_string(),
        pid: std::process::id(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        started_at_ms: now_ms(),
        socket_path: PathBuf::from("/tmp/sock1"),
        sessions_count: 2,
        panes_count: 4,
        agents_count: 1,
        last_heartbeat_ms: now_ms(),
    };

    let peer_2 = DaemonRegistryEntry {
        id: "daem_find_02".to_string(),
        name: "worker-beta".to_string(),
        pid: std::process::id(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        started_at_ms: now_ms(),
        socket_path: PathBuf::from("/tmp/sock2"),
        sessions_count: 3,
        panes_count: 6,
        agents_count: 2,
        last_heartbeat_ms: now_ms(),
    };

    DaemonRegistry::register(&runtime_dir, &peer_1).unwrap();
    DaemonRegistry::register(&runtime_dir, &peer_2).unwrap();

    let cp = AutomationControlPlane::new_headless(24, 80);
    let mut cache = bilterm::wm::registry::DiskCache::default();
    let candidates = saved::candidates(&cp.wm, FindKind::Session, &mut cache);

    // Candidates should include peer daemons
    let alpha_row = candidates.iter().find(|r| r.id == "daemon:worker-alpha");
    assert!(alpha_row.is_some(), "worker-alpha must appear in session find index");
    assert_eq!(alpha_row.unwrap().source.origin.as_str(), "daemon");

    let beta_row = candidates.iter().find(|r| r.id == "daemon:worker-beta");
    assert!(beta_row.is_some(), "worker-beta must appear in session find index");
    assert_eq!(beta_row.unwrap().source.origin.as_str(), "daemon");

    let _ = DaemonRegistry::unregister(&runtime_dir, "worker-alpha");
    let _ = DaemonRegistry::unregister(&runtime_dir, "worker-beta");
}
