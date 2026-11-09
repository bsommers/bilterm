//! Daemon lifecycle regression tests: registry synthesis, non-zero PIDs, stop fallback.
#![cfg(unix)]

use std::os::unix::net::UnixListener;
use std::process::Command;

use bilterm::daemon::registry::{DaemonRegistry, DaemonRegistryEntry};
use bilterm::daemon::supervisor::{
    acquire_lock, daemon_lock_path_in, ensure_started, stop_daemon, DaemonLockRecord,
    DaemonLockResult, DaemonStartResult,
};

fn rec(pid: u32, name: &str, sock: std::path::PathBuf) -> DaemonLockRecord {
    DaemonLockRecord {
        pid,
        name: name.to_string(),
        version: "0".to_string(),
        started_at_ms: 1,
        socket_path: sock,
    }
}

#[test]
fn list_active_synthesizes_from_live_lock() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let sock = dir.join("daemons").join("daemon-w1.sock");
    let DaemonLockResult::Acquired(_lock) = acquire_lock(dir, "w1", sock).unwrap() else {
        panic!("lock");
    };
    let list = DaemonRegistry::list_active(dir);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "w1");
    assert_eq!(list[0].pid, std::process::id());
}

#[test]
fn list_active_dedupes_json_and_lock() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let sock = dir.join("daemons").join("daemon-default.sock");
    let DaemonLockResult::Acquired(_lock) = acquire_lock(dir, "default", sock.clone()).unwrap()
    else {
        panic!("lock");
    };
    DaemonRegistry::register(
        dir,
        &DaemonRegistryEntry {
            id: "x".into(),
            name: "default".into(),
            pid: std::process::id(),
            version: "0".into(),
            started_at_ms: 1,
            socket_path: sock,
            sessions_count: 1,
            panes_count: 0,
            agents_count: 0,
            last_heartbeat_ms: 1,
        },
    )
    .unwrap();
    assert_eq!(DaemonRegistry::list_active(dir).len(), 1);
}

#[test]
fn list_active_ignores_dead_lock_and_stale_socket() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let lock = daemon_lock_path_in(dir, "ghost").unwrap();
    std::fs::write(&lock, rec(4_000_000, "ghost", dir.join("g.sock")).to_json()).unwrap();
    // Stale (unbound) socket file: bind then drop listener leaves a refusing socket.
    let sp = dir.join("daemons").join("daemon-stale.sock");
    drop(UnixListener::bind(&sp).unwrap());
    assert!(DaemonRegistry::list_active(dir).is_empty());
}

#[test]
fn list_active_synthesizes_from_live_socket() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let sp = dir.join("daemons").join("daemon-sockonly.sock");
    std::fs::create_dir_all(sp.parent().unwrap()).unwrap();
    let _l = UnixListener::bind(&sp).unwrap();
    let list = DaemonRegistry::list_active(dir);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "sockonly");
}

#[test]
fn ensure_started_reports_nonzero_pid_when_already_running() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let sock = bilterm::daemon::process::session_daemon_socket_path_in(dir, "live").unwrap();
    let DaemonLockResult::Acquired(_lock) = acquire_lock(dir, "live", sock.clone()).unwrap() else {
        panic!("lock");
    };
    let _l = UnixListener::bind(&sock).unwrap();
    match ensure_started(dir, "live").unwrap() {
        DaemonStartResult::AlreadyRunning { pid, .. } => assert!(pid > 1),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn ensure_started_uses_registry_when_lock_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let sock = bilterm::daemon::process::session_daemon_socket_path_in(dir, "reg").unwrap();
    let _l = UnixListener::bind(&sock).unwrap();
    DaemonRegistry::register(
        dir,
        &DaemonRegistryEntry {
            id: "r".into(),
            name: "reg".into(),
            pid: std::process::id(),
            version: "0".into(),
            started_at_ms: 1,
            socket_path: sock,
            sessions_count: 1,
            panes_count: 0,
            agents_count: 0,
            last_heartbeat_ms: 1,
        },
    )
    .unwrap();
    match ensure_started(dir, "reg").unwrap() {
        DaemonStartResult::AlreadyRunning { pid, .. } => assert_eq!(pid, std::process::id()),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn stop_daemon_falls_back_to_registry_and_kills_live_pid() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let mut child = Command::new("sleep").arg("30").spawn().unwrap();
    let sock = bilterm::daemon::process::session_daemon_socket_path_in(dir, "orphan").unwrap();
    let _l = UnixListener::bind(&sock).unwrap();
    DaemonRegistry::register(
        dir,
        &DaemonRegistryEntry {
            id: "o".into(),
            name: "orphan".into(),
            pid: child.id(),
            version: "0".into(),
            started_at_ms: 1,
            socket_path: sock.clone(),
            sessions_count: 1,
            panes_count: 0,
            agents_count: 0,
            last_heartbeat_ms: 1,
        },
    )
    .unwrap();
    assert!(stop_daemon(dir, "orphan", true).unwrap());
    let _ = child.wait();
    assert!(!sock.exists());
    assert!(DaemonRegistry::list_active(dir).is_empty());
}
