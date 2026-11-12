//! `bilterm ls` must list a daemon that holds a lock record even when it has no
//! `daemon-<name>.sock` (the default-listener case).
use bilterm::daemon::supervisor::{acquire_lock, DaemonLockResult};
use bilterm::daemon::list_active_daemon_sessions_in;

#[test]
fn ls_lists_daemon_with_lock_record_only() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("bilterm.sock");
    let lock = match acquire_lock(dir.path(), "default", sock.clone()).unwrap() {
        DaemonLockResult::Acquired(l) => l,
        _ => panic!("expected to acquire the lock"),
    };
    let listed = list_active_daemon_sessions_in(dir.path());
    assert_eq!(listed, vec![("default".to_string(), sock)]);
    drop(lock);
}
