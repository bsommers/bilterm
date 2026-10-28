//! Phase 35 (`DAEM-03`, `DAEM-04`) Verification Suite:
//! Live Background OS Process Daemonization for `:detach` / `bilterm attach`.
//!
//! Covers:
//! 1. `sanitize_session_name` and `session_daemon_socket_path_in` path-traversal confinement (`SEC-R01`, `SEC-R03`).
//! 2. `probe_or_cleanup_stale_socket` unlinking a stale bound-and-dropped Unix socket (`ConnectionRefused`)
//!    while keeping a live `UnixListener` socket intact and discoverable via `list_active_daemon_sessions_in`.
//! 3. `"session.save"` and `"session.restore"` JSON-RPC methods via `AutomationControlPlane` preserving
//!    multi-pane `SplitTree`, `ZStack` custom window geometry, prompt input, and pane output across detach/restore.

use ratatui::layout::Rect;

use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::config::BiltermConfig;
use bilterm::daemon::{
    list_active_daemon_sessions_in, probe_or_cleanup_stale_socket, sanitize_session_name,
    session_daemon_socket_path_in,
};
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::{WindowManager, WorkspaceLayoutMode};

#[test]
fn sanitize_session_name_and_socket_path_confinement() {
    assert_eq!(sanitize_session_name("work-01_prod"), "work-01_prod");
    assert_eq!(sanitize_session_name("  ../../etc/passwd  "), "etcpasswd");
    assert_eq!(sanitize_session_name("../../../"), "default");
    assert_eq!(sanitize_session_name(""), "default");

    let tmp = tempfile::tempdir().unwrap();
    let sock = session_daemon_socket_path_in(tmp.path(), "../evil/session\0name").unwrap();
    assert_eq!(
        sock,
        tmp.path().join("daemons").join("daemon-evilsessionname.sock")
    );
    assert!(sock.starts_with(tmp.path().join("daemons")));
}

#[test]
#[cfg(unix)]
fn probe_and_list_active_daemon_sessions_unlinks_stale_and_keeps_live_sockets() {
    use std::os::unix::net::UnixListener;

    let tmp = tempfile::tempdir().unwrap();
    let live_sock = session_daemon_socket_path_in(tmp.path(), "alpha-live").unwrap();
    let stale_sock = session_daemon_socket_path_in(tmp.path(), "beta-stale").unwrap();

    let live_listener = UnixListener::bind(&live_sock).unwrap();
    let stale_listener = UnixListener::bind(&stale_sock).unwrap();
    drop(stale_listener);

    assert!(
        probe_or_cleanup_stale_socket(&live_sock),
        "live UnixListener socket must be detected as active"
    );
    assert!(live_sock.exists(), "live socket file must remain on disk");

    assert!(
        !probe_or_cleanup_stale_socket(&stale_sock),
        "dropped UnixListener socket (ConnectionRefused) must report inactive"
    );
    assert!(
        !stale_sock.exists(),
        "stale socket file must be unlinked on ConnectionRefused"
    );

    // Re-create a second stale socket and verify list_active_daemon_sessions_in filters & cleans it
    let stale_sock_2 = session_daemon_socket_path_in(tmp.path(), "gamma-stale").unwrap();
    let stale_listener_2 = UnixListener::bind(&stale_sock_2).unwrap();
    drop(stale_listener_2);

    let active = list_active_daemon_sessions_in(tmp.path());
    assert_eq!(active, vec![("alpha-live".to_string(), live_sock.clone())]);
    assert!(
        !stale_sock_2.exists(),
        "list_active_daemon_sessions_in must clean up stale sockets during scan"
    );

    drop(live_listener);
}

#[test]
fn rpc_session_save_and_restore_preserves_split_tree_zstack_and_pane_output() {
    let tmp = tempfile::tempdir().unwrap();
    let sess_dir = tmp.path().join("sessions");
    std::fs::create_dir_all(&sess_dir).unwrap();

    let area = Rect::new(0, 0, 100, 30);
    let mut wm = WindowManager::new_headless(30, 100);
    let cfg = BiltermConfig {
        session_dir: Some(sess_dir.to_string_lossy().into_owned()),
        history_session: "ops-cluster".to_string(),
        ..Default::default()
    };
    wm.apply_config(cfg.clone());
    wm.resize_all(area).unwrap();

    // Populate pane #1 output and prompt text
    wm.active_pane_mut().set_title("MAINFRAME-1".to_string());
    wm.active_pane_mut()
        .process_pty_output(b"READY_ON_PANE_1\r\n");
    wm.active_pane_mut()
        .prompt_mut()
        .set_text("status --all");

    // Split vertically into pane #2 and horizontally into pane #3
    let pane2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.pane_mut(pane2).unwrap().set_title("WORKER-2".to_string());
    wm.pane_mut(pane2)
        .unwrap()
        .process_pty_output(b"BUILD_RUNNING_PANE_2\r\n");

    let pane3 = wm.split_focused(SplitDirection::Horizontal).unwrap();
    wm.pane_mut(pane3)
        .unwrap()
        .set_title("TELEMETRY-3".to_string());
    wm.pane_mut(pane3)
        .unwrap()
        .process_pty_output(b"METRICS_OK_PANE_3\r\n");

    // Switch to Stacked Z-order mode and assign custom floating window geometry to pane #2
    wm.set_layout_mode(WorkspaceLayoutMode::Stacked);
    let custom_rect = Rect::new(8, 4, 54, 16);
    wm.z_stack_mut().set_window_rect(pane2, custom_rect, area);
    wm.z_stack_mut().raise(pane2);

    let mut plane = AutomationControlPlane::new(wm, area);

    // 1. Save session via JSON-RPC "session.save"
    let save_res = plane
        .dispatch_method(
            "session.save",
            &JsonValue::obj([("name", JsonValue::String("ops-cluster".to_string()))]),
        )
        .unwrap();
    assert_eq!(save_res.get("saved").and_then(JsonValue::as_bool), Some(true));
    assert_eq!(
        save_res.get("session").and_then(JsonValue::as_str),
        Some("ops-cluster")
    );
    assert_eq!(save_res.get("panes").and_then(JsonValue::as_i64), Some(3));

    // Also test detach_client marking the session as [DETACHED]
    assert!(plane.wm.detach_client());
    assert!(plane.wm.has_detached_session("ops-cluster"));

    // 2. Create a fresh single-pane control plane and restore via JSON-RPC "session.restore"
    let mut fresh_wm = WindowManager::new_headless(30, 100);
    fresh_wm.apply_config(cfg);
    let mut fresh_plane = AutomationControlPlane::new(fresh_wm, area);
    assert_eq!(fresh_plane.wm.pane_count(), 1);

    let restore_res = fresh_plane
        .dispatch_method(
            "session.restore",
            &JsonValue::obj([("name", JsonValue::String("ops-cluster".to_string()))]),
        )
        .unwrap();
    assert_eq!(
        restore_res.get("restored").and_then(JsonValue::as_bool),
        Some(true)
    );
    assert_eq!(
        restore_res.get("session").and_then(JsonValue::as_str),
        Some("ops-cluster")
    );
    assert_eq!(
        restore_res.get("panes").and_then(JsonValue::as_i64),
        Some(3)
    );

    // Verify full multi-pane SplitTree, ZStack custom geometry, prompt input, and canvas output
    assert_eq!(fresh_plane.wm.pane_count(), 3);
    assert_eq!(fresh_plane.wm.layout_mode(), WorkspaceLayoutMode::Stacked);
    assert_eq!(fresh_plane.wm.active_pane_id(), pane2);
    assert_eq!(
        fresh_plane.wm.z_stack().rect_for(pane2, area),
        Some(custom_rect)
    );
    assert_eq!(
        fresh_plane.wm.pane(1).unwrap().prompt().text(),
        "status --all"
    );
    assert!(
        fresh_plane
            .wm
            .pane(1)
            .unwrap()
            .vt()
            .contents()
            .contains("READY_ON_PANE_1")
    );
    assert!(
        fresh_plane
            .wm
            .pane(pane2)
            .unwrap()
            .vt()
            .contents()
            .contains("BUILD_RUNNING_PANE_2")
    );
    assert!(
        fresh_plane
            .wm
            .pane(pane3)
            .unwrap()
            .vt()
            .contents()
            .contains("METRICS_OK_PANE_3")
    );
    assert!(
        !fresh_plane.wm.has_detached_session("ops-cluster"),
        "restoring a detached session must clear the detached flag"
    );
}
