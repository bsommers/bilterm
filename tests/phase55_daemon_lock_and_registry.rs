//! Phase 55 Wave 1 Test Suite: Singleton Lock, Stale-Lock Recovery, and Daemon Registry (`DAEM-05`, `DAEM-06`, `SEC-R01..03`).

use std::fs;
use std::path::PathBuf;

use bilterm::daemon::registry::{DaemonRegistry, DaemonRegistryEntry};
use bilterm::daemon::supervisor::{
    acquire_lock, check_pid_alive, daemon_lock_path_in, now_ms, DaemonLockRecord, DaemonLockResult,
};
use bilterm::secure_fs::{ensure_private_dir, validate_safe_name};

#[test]
fn test_validate_safe_name_boundaries() {
    assert!(validate_safe_name("default").is_ok());
    assert!(validate_safe_name("worker-1").is_ok());
    assert!(validate_safe_name("sess_prod_42").is_ok());

    // Rejections
    assert!(validate_safe_name("").is_err());
    assert!(validate_safe_name(".").is_err());
    assert!(validate_safe_name("..").is_err());
    assert!(validate_safe_name("../traversal").is_err());
    assert!(validate_safe_name("/etc/passwd").is_err());
    assert!(validate_safe_name("daemon name").is_err());
    assert!(validate_safe_name("null\0byte").is_err());
    assert!(validate_safe_name(&"x".repeat(65)).is_err());
}

#[test]
fn test_singleton_lock_and_recovery() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let runtime_dir = tmp.path();
    let sock_path = runtime_dir.join("daemons/daemon-default.sock");

    // 1. Acquire singleton lock for "default"
    let lock_res = acquire_lock(runtime_dir, "default", sock_path.clone())
        .expect("acquire lock success");
    let lock = match lock_res {
        DaemonLockResult::Acquired(l) => l,
        DaemonLockResult::AlreadyHeld(_) => panic!("expected lock to be acquired"),
    };

    assert_eq!(lock.record().name, "default");
    assert_eq!(lock.record().pid, std::process::id());
    assert!(lock.lock_path().ends_with("daemon-singleton.lock"));
    assert!(fs::metadata(lock.lock_path()).is_ok());

    // 2. Double-acquire attempt in same or another process while flock is held
    let second_res = acquire_lock(runtime_dir, "default", sock_path.clone())
        .expect("second acquire call");
    match second_res {
        DaemonLockResult::AlreadyHeld(rec) => {
            assert_eq!(rec.pid, std::process::id());
            assert_eq!(rec.name, "default");
        }
        DaemonLockResult::Acquired(_) => panic!("expected AlreadyHeld for held lock"),
    }

    // 3. Release lock cleanly
    lock.release().expect("clean release");
    let lock_path = daemon_lock_path_in(runtime_dir, "default").unwrap();
    assert_eq!(
        fs::read(&lock_path).unwrap().len(),
        0,
        "clean release clears the record and keeps the lockfile (flock is the only truth)"
    );

    // 4. Stale lock recovery: write a fake lockfile with a dead PID (e.g. 9999999)
    let fake_record = DaemonLockRecord {
        pid: 9_999_999, // Highly improbable PID
        name: "default".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        started_at_ms: now_ms(),
        socket_path: sock_path.clone(),
    };
    ensure_private_dir(&runtime_dir.join("daemons")).unwrap();
    // Plant stale socket
    fs::write(&sock_path, b"stale socket content").unwrap();
    fs::write(&lock_path, fake_record.to_json()).unwrap();

    assert!(!check_pid_alive(9_999_999));

    // Stale lock should be reclaimed automatically
    let recovered_res = acquire_lock(runtime_dir, "default", sock_path.clone())
        .expect("acquire should recover stale lock");
    let recovered_lock = match recovered_res {
        DaemonLockResult::Acquired(l) => l,
        DaemonLockResult::AlreadyHeld(_) => panic!("expected lock acquisition after recovering dead PID"),
    };
    assert_eq!(recovered_lock.record().pid, std::process::id());
    // Stale socket should have been unlinked
    assert!(!sock_path.exists(), "stale socket must be unlinked during recovery");
    recovered_lock.release().unwrap();
}

#[test]
fn test_multi_daemon_isolation() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let runtime_dir = tmp.path();

    let sock_1 = runtime_dir.join("daemons/daemon-worker-1.sock");
    let sock_2 = runtime_dir.join("daemons/daemon-worker-2.sock");

    let lock_1 = match acquire_lock(runtime_dir, "worker-1", sock_1.clone()).unwrap() {
        DaemonLockResult::Acquired(l) => l,
        _ => panic!("lock 1 failed"),
    };

    let lock_2 = match acquire_lock(runtime_dir, "worker-2", sock_2.clone()).unwrap() {
        DaemonLockResult::Acquired(l) => l,
        _ => panic!("lock 2 failed"),
    };

    assert_eq!(lock_1.record().name, "worker-1");
    assert_eq!(lock_2.record().name, "worker-2");
    assert_ne!(lock_1.lock_path(), lock_2.lock_path());
    assert!(lock_1.lock_path().ends_with("daemon-worker-1.lock"));
    assert!(lock_2.lock_path().ends_with("daemon-worker-2.lock"));

    lock_1.release().unwrap();
    lock_2.release().unwrap();
}

#[test]
fn test_daemon_registry_lifecycle_and_pruning() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let runtime_dir = tmp.path();

    let entry_1 = DaemonRegistryEntry {
        id: "daem_live_01".to_string(),
        name: "worker-1".to_string(),
        pid: std::process::id(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        started_at_ms: now_ms(),
        socket_path: PathBuf::from("/tmp/sock1"),
        sessions_count: 2,
        panes_count: 5,
        agents_count: 1,
        last_heartbeat_ms: now_ms(),
    };

    let entry_dead = DaemonRegistryEntry {
        id: "daem_dead_99".to_string(),
        name: "dead-worker".to_string(),
        pid: 9_999_998,
        version: env!("CARGO_PKG_VERSION").to_string(),
        started_at_ms: now_ms() - 100_000,
        socket_path: runtime_dir.join("daemons/dead.sock"),
        sessions_count: 0,
        panes_count: 0,
        agents_count: 0,
        last_heartbeat_ms: now_ms() - 50_000,
    };

    // Plant dead socket
    ensure_private_dir(&runtime_dir.join("daemons")).unwrap();
    fs::write(&entry_dead.socket_path, b"dead").unwrap();

    // Register both
    DaemonRegistry::register(runtime_dir, &entry_1).expect("register 1");
    DaemonRegistry::register(runtime_dir, &entry_dead).expect("register dead");

    // Verify heartbeat update
    DaemonRegistry::update_heartbeat(runtime_dir, "worker-1", 3, 7, 2).expect("update heartbeat");

    // list_active should automatically prune the dead entry
    let active = DaemonRegistry::list_active(runtime_dir);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].name, "worker-1");
    assert_eq!(active[0].sessions_count, 3);
    assert_eq!(active[0].panes_count, 7);
    assert_eq!(active[0].agents_count, 2);

    // Dead registry file and dead socket should be removed
    assert!(!entry_dead.socket_path.exists());
    let dead_json = DaemonRegistry::entry_path_in(runtime_dir, "dead-worker").unwrap();
    assert!(!dead_json.exists());

    // Clean unregister of live worker
    DaemonRegistry::unregister(runtime_dir, "worker-1").expect("unregister");
    let active_after = DaemonRegistry::list_active(runtime_dir);
    assert!(active_after.is_empty());
}
