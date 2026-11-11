//! Daemon lifecycle hardening: reaped children, zombie/PID-reuse-proof liveness, and a lockfile
//! that is never unlinked (`flock` is the only source of truth).
#![cfg(unix)]

use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};

use bilterm::daemon::registry::{DaemonRegistry, DaemonRegistryEntry};
use bilterm::daemon::supervisor::{
    acquire_lock, daemon_lock_path_in, spawn_daemon_in, stop_daemon, DaemonLockRecord, DaemonLockResult,
};

fn proc_state(pid: u32) -> Option<char> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit_once(')')?.1.trim_start().chars().next()
}

fn entry(name: &str, pid: u32, dir: &std::path::Path) -> DaemonRegistryEntry {
    DaemonRegistryEntry {
        id: format!("daemon-{name}-{pid}"),
        name: name.to_string(),
        pid,
        version: "0".into(),
        started_at_ms: 0,
        socket_path: dir.join(format!("{name}.sock")),
        sessions_count: 1,
        panes_count: 0,
        agents_count: 0,
        last_heartbeat_ms: 0,
    }
}

fn record(name: &str, pid: u32, dir: &std::path::Path) -> DaemonLockRecord {
    DaemonLockRecord {
        pid,
        name: name.to_string(),
        version: "0".into(),
        started_at_ms: 0,
        socket_path: dir.join(format!("{name}.sock")),
    }
}

#[test]
fn release_and_stop_never_unlink_the_lockfile() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let lock_path = daemon_lock_path_in(dir, "keep").unwrap();
    let DaemonLockResult::Acquired(lock) = acquire_lock(dir, "keep", dir.join("keep.sock")).unwrap() else {
        panic!("fresh lock must be acquired");
    };
    lock.release().unwrap();
    assert!(lock_path.exists(), "release must not unlink a lockfile another starter may hold open");

    // A crashed daemon's record: stop clears it but keeps the file.
    let mut victim = Command::new("sleep").arg("30").spawn().unwrap();
    fs::write(&lock_path, record("keep", victim.id(), dir).to_json()).unwrap();
    assert!(!stop_daemon(dir, "keep", true).unwrap());
    let alive = victim.try_wait().unwrap().is_none();
    let _ = victim.kill();
    let _ = victim.wait();
    assert!(alive, "a PID from an unheld lock must never be signalled");
    assert!(lock_path.exists(), "stop must not unlink the lockfile");
}

// Quarantined off Linux: zombie detection reads /proc (`daemon::liveness::pid_is_zombie` is Linux-only),
// so on macOS a registry entry whose process is an unreaped zombie is still listed as active.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "product gap: pid_is_zombie is Linux-only (/proc); zombie daemon entries are not detected on this OS")]
fn zombie_registry_entry_is_not_listed_active() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let mut child = Command::new("true").spawn().unwrap();
    let pid = child.id();
    let deadline = Instant::now() + Duration::from_secs(5);
    while proc_state(pid) != Some('Z') && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(proc_state(pid), Some('Z'), "child should be an unreaped zombie");
    DaemonRegistry::register(dir, &entry("zed", pid, dir)).unwrap();
    let active = DaemonRegistry::list_active(dir);
    let _ = child.wait();
    assert!(active.iter().all(|e| e.name != "zed"), "a zombie is not a running daemon: {active:?}");
}

#[test]
fn unheld_lock_record_with_a_live_reused_pid_is_not_active() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    // The recorded PID now belongs to an unrelated live process; nobody holds the flock.
    let mut other = Command::new("sleep").arg("30").spawn().unwrap();
    let lock_path = daemon_lock_path_in(dir, "reused").unwrap();
    fs::write(&lock_path, record("reused", other.id(), dir).to_json()).unwrap();
    DaemonRegistry::register(dir, &entry("reused", other.id(), dir)).unwrap();
    let active = DaemonRegistry::list_active(dir);
    let _ = other.kill();
    let _ = other.wait();
    assert!(active.iter().all(|e| e.name != "reused"), "crashed daemon listed as active: {active:?}");
}

#[test]
fn spawned_daemon_child_is_reaped() {
    let tmp = tempfile::tempdir().unwrap();
    // `current_exe` is this test binary: it rejects the daemon flags and exits at once.
    let (pid, _sock) = spawn_daemon_in(tmp.path(), "reap").unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while proc_state(pid).is_some() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(proc_state(pid), None, "exited daemon child must be reaped, not left as a zombie");
}
