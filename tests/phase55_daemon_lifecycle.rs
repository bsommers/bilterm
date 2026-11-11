//! Phase 55 Wave 2 Test Suite: Daemon Lifecycle, RPC, MCP, Colon Commands, and Version Handshake (`DAEM-05`, `DAEM-07`, `DAEM-08`).

use std::fs;
use std::path::PathBuf;

use bilterm::action::{Action, DaemonSupervisorOp};
use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::daemon::registry::{DaemonRegistry, DaemonRegistryEntry};
use bilterm::daemon::supervisor::{daemon_lock_path_in, now_ms, stop_daemon};
use bilterm::daemon::{DaemonRequest, DaemonResponse, DaemonServerState};
use bilterm::secure_fs::ensure_private_dir;
use bilterm::wm::WindowManager;

#[test]
fn test_colon_daemon_commands_parse_and_execute() {
    let mut wm = WindowManager::new_headless(24, 80);

    // 1. :daemon list
    let cmd = parse_colon_command(":daemon list").unwrap().unwrap();
    assert_eq!(cmd, Action::DaemonSupervisor(DaemonSupervisorOp::List));
    assert!(!execute_wm_colon_command(&mut wm, cmd));
    assert!(wm.active_pane().status_message().is_some());

    // 2. :daemon status
    let cmd = parse_colon_command(":daemon status").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::DaemonSupervisor(DaemonSupervisorOp::Status { name: None })
    );
    assert!(!execute_wm_colon_command(&mut wm, cmd));

    // 3. :daemon status worker-1
    let cmd = parse_colon_command(":daemon status worker-1").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::DaemonSupervisor(DaemonSupervisorOp::Status {
            name: Some("worker-1".to_string())
        })
    );
    assert!(!execute_wm_colon_command(&mut wm, cmd));

    // 4. :daemon stop worker-1 --force
    let cmd = parse_colon_command(":daemon stop worker-1 --force").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::DaemonSupervisor(DaemonSupervisorOp::Stop {
            name: Some("worker-1".to_string()),
            force: true
        })
    );
    assert!(!execute_wm_colon_command(&mut wm, cmd));

    // 5. :daemon switch worker-2
    let cmd = parse_colon_command(":daemon switch worker-2").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::DaemonSupervisor(DaemonSupervisorOp::Switch {
            name: "worker-2".to_string()
        })
    );
    assert!(!execute_wm_colon_command(&mut wm, cmd));

    // 6. :daemon prune
    let cmd = parse_colon_command(":daemon prune").unwrap().unwrap();
    assert_eq!(cmd, Action::DaemonSupervisor(DaemonSupervisorOp::Prune));
    assert!(!execute_wm_colon_command(&mut wm, cmd));
}

#[test]
fn test_rpc_daemon_handlers() {
    let mut cp = AutomationControlPlane::new_headless(24, 80);

    // daemon.list
    let res = cp
        .dispatch_method("daemon.list", &JsonValue::Null)
        .expect("daemon.list");
    assert!(res.get("daemons").is_some());

    // daemon.status
    let status_res = cp
        .dispatch_method(
            "daemon.status",
            &JsonValue::obj([("name", JsonValue::String("default".to_string()))]),
        )
        .expect("daemon.status");
    assert_eq!(
        status_res.get("name").and_then(JsonValue::as_str),
        Some("default")
    );

    // daemon.switch
    let switch_res = cp
        .dispatch_method(
            "daemon.switch",
            &JsonValue::obj([("name", JsonValue::String("worker-42".to_string()))]),
        )
        .expect("daemon.switch");
    assert_eq!(
        switch_res.get("status").and_then(JsonValue::as_str),
        Some("switched")
    );
}

#[test]
fn test_mcp_bilterm_daemon_tool() {
    let mut cp = AutomationControlPlane::new_headless(24, 80);

    let req = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"bilterm_daemon","arguments":{"action":"list"}}}"#;
    let resp_str = cp.handle_mcp_line(req).expect("mcp response");
    let resp = JsonValue::parse(&resp_str).expect("json parse");
    assert_eq!(resp.get("id").and_then(JsonValue::as_i64), Some(1));
    let content = resp
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(JsonValue::as_array)
        .expect("content array");
    assert!(!content.is_empty());
}

#[test]
fn test_version_mismatch_diagnostic() {
    let mut state = DaemonServerState::new();
    state.socket_path = Some(PathBuf::from("/run/user/1000/bilterm/daemons/daemon-default.sock"));

    // 1. Handshake with matching version succeeds
    let ok_req = DaemonRequest::Handshake {
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        client_id: 1,
    };
    let ok_resp = state.handle_request(ok_req);
    match ok_resp {
        DaemonResponse::HandshakeOk { server_version, daemon_name, .. } => {
            assert_eq!(server_version, env!("CARGO_PKG_VERSION"));
            assert_eq!(daemon_name, "default");
        }
        _ => panic!("expected HandshakeOk"),
    }

    // 2. Handshake with mismatching version returns structured diagnostic
    let bad_req = DaemonRequest::Handshake {
        client_version: "99.0.0".to_string(),
        client_id: 2,
    };
    let bad_resp = state.handle_request(bad_req);
    match bad_resp {
        DaemonResponse::VersionMismatch { server_version, client_version, diagnostic } => {
            assert_eq!(server_version, env!("CARGO_PKG_VERSION"));
            assert_eq!(client_version, "99.0.0");
            assert!(diagnostic.contains("bilterm: daemon version mismatch"));
            assert!(diagnostic.contains("client version: 99.0.0"));
            assert!(diagnostic.contains("action required: restart daemon"));
        }
        _ => panic!("expected VersionMismatch"),
    }
}

#[test]
fn test_graceful_stop_and_cleanup() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let runtime_dir = tmp.path();

    // Setup a dummy registered daemon with lockfile
    let entry = DaemonRegistryEntry {
        id: "daem_stop_test".to_string(),
        name: "test-stop".to_string(),
        pid: 9_999_991, // dead PID
        version: env!("CARGO_PKG_VERSION").to_string(),
        started_at_ms: now_ms(),
        socket_path: runtime_dir.join("daemons/daemon-test-stop.sock"),
        sessions_count: 1,
        panes_count: 1,
        agents_count: 0,
        last_heartbeat_ms: now_ms(),
    };
    ensure_private_dir(&runtime_dir.join("daemons")).unwrap();
    fs::write(&entry.socket_path, b"sock").unwrap();
    DaemonRegistry::register(runtime_dir, &entry).unwrap();

    let lock_path = daemon_lock_path_in(runtime_dir, "test-stop").unwrap();
    fs::write(&lock_path, b"lock content").unwrap();

    // Call stop_daemon
    let stopped = stop_daemon(runtime_dir, "test-stop", false).expect("stop daemon");
    assert!(!stopped); // dead PID, returns false for active process, but cleans resources

    // Socket and registry file are removed; the lockfile is kept (never unlinked, another
    // starter may hold it open) but its stale content is cleared.
    assert!(!entry.socket_path.exists());
    assert_eq!(fs::read(&lock_path).unwrap().len(), 0);
    let reg_path = DaemonRegistry::entry_path_in(runtime_dir, "test-stop").unwrap();
    assert!(!reg_path.exists());
}
