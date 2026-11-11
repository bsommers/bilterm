//! Daemon liveness derived from the lockfile `flock` (`DAEM-05`).
//!
//! The kernel drops a `flock` when its holder dies, so "is the lock held?" cannot be fooled by
//! a zombie or by a PID that was reused by an unrelated process. The lockfile is never unlinked:
//! a clean exit truncates the record, so a record nobody holds means the daemon crashed.

// The flock probe is Unix-only; elsewhere the helpers and their imports are unused.
#![cfg_attr(not(unix), allow(unused_imports, dead_code))]

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::daemon::supervisor::{daemon_lock_path_in, DaemonLockRecord};

/// What the lockfile says about daemon `name`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DaemonLiveness {
    /// A live process holds the lock.
    Running(DaemonLockRecord),
    /// A record is present but nobody holds the lock: the daemon died without cleaning up.
    Crashed(DaemonLockRecord),
    /// No lockfile, or an empty one left by a clean exit.
    Stopped,
}

impl DaemonLiveness {
    /// Status word used by `:daemon status`, `daemon.status` and the daemon window.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Running(_) => "running",
            Self::Crashed(_) => "crashed",
            Self::Stopped => "stopped",
        }
    }
}

/// `true` when `pid` is a zombie (exited, not yet reaped). Linux only; `false` elsewhere.
pub fn pid_is_zombie(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        // `/proc/<pid>/stat` is `pid (comm) state ...`; `comm` may contain spaces and parens.
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|s| s.rsplit_once(')').map(|(_, rest)| rest.trim_start().starts_with(['Z', 'X'])))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        false
    }
}

#[cfg(unix)]
fn try_flock(file: &File, op: libc::c_int) -> io::Result<bool> {
    use std::os::unix::io::AsRawFd;
    if unsafe { libc::flock(file.as_raw_fd(), op | libc::LOCK_NB) } == 0 {
        return Ok(true);
    }
    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::EWOULDBLOCK) { Ok(false) } else { Err(err) }
}

#[cfg(unix)]
fn unlock(file: &File) {
    use std::os::unix::io::AsRawFd;
    unsafe {
        libc::flock(file.as_raw_fd(), libc::LOCK_UN);
    }
}

fn read_record(lock_path: &Path) -> Option<DaemonLockRecord> {
    let mut s = String::new();
    File::open(lock_path).ok()?.read_to_string(&mut s).ok()?;
    DaemonLockRecord::from_json(&s).ok()
}

/// Reads the liveness of daemon `name` from its lockfile. Never signals, never unlinks.
pub fn daemon_liveness(runtime_dir: &Path, name: &str) -> DaemonLiveness {
    let Ok(lock_path) = daemon_lock_path_in(runtime_dir, name) else {
        return DaemonLiveness::Stopped;
    };
    lock_liveness(&lock_path)
}

/// [`daemon_liveness`] for an explicit lockfile path.
pub fn lock_liveness(lock_path: &Path) -> DaemonLiveness {
    #[cfg(unix)]
    {
        // A starting daemon holds the lock a moment before its record is written: retry briefly.
        for _ in 0..10 {
            let Ok(file) = crate::secure_fs::private_open_options().read(true).open(lock_path) else {
                return DaemonLiveness::Stopped;
            };
            match try_flock(&file, libc::LOCK_SH) {
                Ok(true) => {
                    let rec = read_record(lock_path);
                    unlock(&file);
                    return rec.map_or(DaemonLiveness::Stopped, DaemonLiveness::Crashed);
                }
                Ok(false) => {
                    if let Some(rec) = read_record(lock_path) {
                        return DaemonLiveness::Running(rec);
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => return DaemonLiveness::Stopped,
            }
        }
        DaemonLiveness::Stopped
    }
    #[cfg(not(unix))]
    {
        let _ = lock_path;
        DaemonLiveness::Stopped
    }
}

/// Truncates the record of a lockfile nobody holds, waiting up to `wait` for the holder to die.
/// Returns `true` when the record was cleared (or there was none). The file itself stays.
pub fn clear_stale_record(lock_path: &Path, wait: Duration) -> bool {
    #[cfg(unix)]
    {
        let deadline = Instant::now() + wait;
        loop {
            let file = match crate::secure_fs::private_open_options().read(true).write(true).open(lock_path) {
                Ok(f) => f,
                Err(e) => return e.kind() == io::ErrorKind::NotFound,
            };
            if try_flock(&file, libc::LOCK_EX).unwrap_or(false) {
                let ok = file.set_len(0).and_then(|()| file.sync_all()).is_ok();
                unlock(&file);
                return ok;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (lock_path, wait);
        false
    }
}
