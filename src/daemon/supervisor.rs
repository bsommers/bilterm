//! Background Daemon Supervisor & Singleton/Multi-Daemon Lockfile (`DAEM-05`, `DAEM-06`, `SEC-R01..03`).
//!
//! Provides flock-based mutual exclusion with stale PID recovery (`libc::kill(pid, 0)`),
//! graceful stop, and auto-spawn capabilities for Bilterm background daemons.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, ErrorKind, Read, Write};
#[cfg(unix)]
use std::io::{Seek, SeekFrom};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::api::json::JsonValue;
use crate::secure_fs::{ensure_private_dir, validate_safe_name};
#[cfg(unix)]
use crate::secure_fs::private_open_options;

/// Lockfile record written to `<runtime_dir>/daemons/daemon-<name>.lock` or `daemon-singleton.lock`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonLockRecord {
    pub pid: u32,
    pub name: String,
    pub version: String,
    pub started_at_ms: u64,
    pub socket_path: PathBuf,
}

impl DaemonLockRecord {
    /// Serializes this record to compact JSON.
    pub fn to_json(&self) -> String {
        let mut map = BTreeMap::new();
        map.insert("pid".to_string(), JsonValue::Number(self.pid as i64));
        map.insert("name".to_string(), JsonValue::String(self.name.clone()));
        map.insert(
            "version".to_string(),
            JsonValue::String(self.version.clone()),
        );
        map.insert(
            "started_at_ms".to_string(),
            JsonValue::Number(self.started_at_ms as i64),
        );
        map.insert(
            "socket_path".to_string(),
            JsonValue::String(self.socket_path.to_string_lossy().into_owned()),
        );
        JsonValue::Object(map).to_json_string()
    }

    /// Deserializes a record from JSON text.
    pub fn from_json(s: &str) -> Result<Self, String> {
        let val = JsonValue::parse(s).ok_or_else(|| "invalid json in lock record".to_string())?;
        let pid = val
            .get("pid")
            .and_then(|v| v.as_i64())
            .and_then(parse_daemon_pid)
            .ok_or("missing or invalid pid")?;
        let name = val
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("missing name")?
            .to_string();
        let version = val
            .get("version")
            .and_then(|v| v.as_str())
            .ok_or("missing version")?
            .to_string();
        let started_at_ms = val
            .get("started_at_ms")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as u64;
        let socket_path = val
            .get("socket_path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .ok_or("missing socket_path")?;

        Ok(Self {
            pid,
            name,
            version,
            started_at_ms,
            socket_path,
        })
    }
}

/// Result of attempting to acquire a daemon lock.
#[derive(Debug)]
pub enum DaemonLockResult {
    /// Lock was successfully acquired; the caller owns the daemon lifecycle.
    Acquired(DaemonLock),
    /// Lock is already held by a living daemon process.
    AlreadyHeld(DaemonLockRecord),
}

/// RAII handle guarding the open lockfile and its active `flock`.
#[derive(Debug)]
pub struct DaemonLock {
    file: Option<File>,
    record: DaemonLockRecord,
    lock_path: PathBuf,
}

impl DaemonLock {
    /// Returns the active lock record.
    pub fn record(&self) -> &DaemonLockRecord {
        &self.record
    }

    /// Returns the filesystem path to the lockfile.
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    /// Explicitly releases the lock, unlinking the lockfile and releasing `flock`.
    pub fn release(mut self) -> io::Result<()> {
        let _ = fs::remove_file(&self.lock_path);
        #[cfg(unix)]
        {
            if let Some(ref f) = self.file {
                unsafe {
                    libc::flock(f.as_raw_fd(), libc::LOCK_UN);
                }
            }
        }
        if let Some(mut f) = self.file.take() {
            f.flush()?;
        }
        Ok(())
    }
}

impl Drop for DaemonLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            if let Some(ref f) = self.file {
                unsafe {
                    libc::flock(f.as_raw_fd(), libc::LOCK_UN);
                }
            }
        }
    }
}

/// Result of ensuring a daemon is running.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaemonStartResult {
    /// Fresh daemon was spawned and is ready.
    Started { pid: u32, socket_path: PathBuf },
    /// Daemon was already running and ready.
    AlreadyRunning { pid: u32, socket_path: PathBuf },
}

/// Resolves the lockfile path for `name` inside `<runtime_dir>/daemons/`.
pub fn daemon_lock_path_in(runtime_dir: &Path, name: &str) -> io::Result<PathBuf> {
    validate_safe_name(name)
        .map_err(|e| io::Error::new(ErrorKind::InvalidInput, format!("invalid daemon name: {e}")))?;

    let daemons_dir = runtime_dir.join("daemons");
    ensure_private_dir(&daemons_dir)?;

    let filename = if name == "default" {
        "daemon-singleton.lock".to_string()
    } else {
        format!("daemon-{name}.lock")
    };

    Ok(daemons_dir.join(filename))
}

/// Validates a lockfile PID as a signalable process id: `1 < pid <= i32::MAX`.
///
/// Rejects `0`/negative values (which `kill(2)` interprets as process groups or "all
/// processes") and `1` (init), plus anything that would overflow `pid_t`.
pub fn parse_daemon_pid(raw: i64) -> Option<u32> {
    if raw > 1 && raw <= i64::from(i32::MAX) {
        Some(raw as u32)
    } else {
        None
    }
}

/// Probes whether `pid` is currently alive (`libc::kill(pid, 0)`).
pub fn check_pid_alive(pid: u32) -> bool {
    if parse_daemon_pid(i64::from(pid)).is_none() {
        return false;
    }
    #[cfg(unix)]
    {
        let ret = unsafe { libc::kill(pid as libc::pid_t, 0) };
        if ret == 0 {
            true
        } else {
            let err = io::Error::last_os_error();
            err.raw_os_error() != Some(libc::ESRCH)
        }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Returns the current unix time in milliseconds.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Attempts to acquire the exclusive lock for daemon `name` in `runtime_dir`.
///
/// If the lock is held by a live PID, returns [`DaemonLockResult::AlreadyHeld`].
/// If the lockfile exists but the recorded PID is dead (`ESRCH`), performs stale-PID
/// lock recovery: cleans up any stale socket and re-acquires the lock (`DAEM-05`).
pub fn acquire_lock(
    runtime_dir: &Path,
    name: &str,
    socket_path: PathBuf,
) -> io::Result<DaemonLockResult> {
    let lock_path = daemon_lock_path_in(runtime_dir, name)?;

    #[cfg(unix)]
    {
        let mut open_opts = private_open_options();
        open_opts.read(true).write(true).create(true);
        open_opts.custom_flags(libc::O_NOFOLLOW);

        let mut file = open_opts.open(&lock_path)?;
        let fd = file.as_raw_fd();
        let flock_res = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
        if flock_res == 0 {
            // Flock acquired. Check if prior dead daemon left a stale socket.
            let mut old_content = String::new();
            let _ = file.read_to_string(&mut old_content);
            if let Ok(old_rec) = DaemonLockRecord::from_json(&old_content)
                && fs::symlink_metadata(&old_rec.socket_path).is_ok()
            {
                let _ = fs::remove_file(&old_rec.socket_path);
            }
            if fs::symlink_metadata(&socket_path).is_ok() {
                let _ = fs::remove_file(&socket_path);
            }

            // Truncate and write fresh record.
            let record = DaemonLockRecord {
                pid: std::process::id(),
                name: name.to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                started_at_ms: now_ms(),
                socket_path,
            };
            file.set_len(0)?;
            file.seek(SeekFrom::Start(0))?;
            file.write_all(record.to_json().as_bytes())?;
            file.sync_all()?;
            return Ok(DaemonLockResult::Acquired(DaemonLock {
                file: Some(file),
                record,
                lock_path,
            }));
        }

        // Flock blocked. Inspect lockfile contents.
        let mut content = String::new();
        let _ = file.read_to_string(&mut content);

        if let Ok(rec) = DaemonLockRecord::from_json(&content) {
            if check_pid_alive(rec.pid) {
                return Ok(DaemonLockResult::AlreadyHeld(rec));
            }
            // Stale lock detected: PID is dead!
            if fs::symlink_metadata(&rec.socket_path).is_ok() {
                let _ = fs::remove_file(&rec.socket_path);
            }
            // Retry flock after brief backoff
            std::thread::sleep(Duration::from_millis(20));
            let retry_flock = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
            if retry_flock == 0 {
                let record = DaemonLockRecord {
                    pid: std::process::id(),
                    name: name.to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    started_at_ms: now_ms(),
                    socket_path,
                };
                file.set_len(0)?;
                file.seek(SeekFrom::Start(0))?;
                file.write_all(record.to_json().as_bytes())?;
                file.sync_all()?;
                return Ok(DaemonLockResult::Acquired(DaemonLock {
                    file: Some(file),
                    record,
                    lock_path,
                }));
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = &socket_path;
    }

    Err(io::Error::new(
        ErrorKind::WouldBlock,
        format!("unable to acquire daemon lock on {:?}", lock_path),
    ))
}

/// Resolves the daemon runtime dir: `BILTERM_RUNTIME_DIR` (absolute) if set, else the
/// process-wide private runtime dir. Lets a spawned child share its parent's `runtime_dir`.
pub fn resolve_runtime_dir() -> io::Result<PathBuf> {
    if let Some(v) = std::env::var_os("BILTERM_RUNTIME_DIR") {
        let p = PathBuf::from(v);
        if p.is_absolute() {
            ensure_private_dir(&p)?;
            return Ok(p);
        }
    }
    crate::secure_fs::private_runtime_dir()
}

/// Spawns a background detached daemon process for `name` using the default runtime dir.
pub fn spawn_daemon(name: &str) -> io::Result<PathBuf> {
    let dir = resolve_runtime_dir()?;
    spawn_daemon_in(&dir, name).map(|(_, p)| p)
}

/// Convenience wrapper returning the child PID and socket path (default runtime dir).
pub fn spawn_daemon_with_pid(name: &str) -> io::Result<(u32, PathBuf)> {
    let dir = resolve_runtime_dir()?;
    spawn_daemon_in(&dir, name)
}

/// Spawns a detached daemon for `name` bound to `runtime_dir`, returning `(child_pid, socket_path)`.
pub fn spawn_daemon_in(runtime_dir: &Path, name: &str) -> io::Result<(u32, PathBuf)> {
    validate_safe_name(name)
        .map_err(|e| io::Error::new(ErrorKind::InvalidInput, format!("invalid daemon name: {e}")))?;

    let sock_path = crate::daemon::process::session_daemon_socket_path_in(runtime_dir, name)?;
    let exe = std::env::current_exe()?;
    let listen_arg = format!("unix:{}", sock_path.display());

    let mut cmd = Command::new(exe);
    cmd.args(["--daemon", "--name", name, "-l", &listen_arg])
        .env("BILTERM_RUNTIME_DIR", runtime_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(unix)]
    {
        cmd.process_group(0);
    }

    let child = cmd.spawn()?;
    Ok((child.id(), sock_path))
}

fn read_lock_record(lock_path: &Path) -> Option<DaemonLockRecord> {
    let mut s = String::new();
    File::open(lock_path).ok()?.read_to_string(&mut s).ok()?;
    DaemonLockRecord::from_json(&s).ok()
}

/// Ensures the daemon for `name` is running. Returns `AlreadyRunning` or `Started`.
/// The returned PID is always a real signalable PID (`> 1`), never `0`.
pub fn ensure_started(runtime_dir: &Path, name: &str) -> io::Result<DaemonStartResult> {
    validate_safe_name(name)
        .map_err(|e| io::Error::new(ErrorKind::InvalidInput, format!("invalid daemon name: {e}")))?;

    let sock_path = crate::daemon::process::session_daemon_socket_path_in(runtime_dir, name)?;
    let lock_path = daemon_lock_path_in(runtime_dir, name)?;

    // Already running per lockfile?
    if let Some(record) = read_lock_record(&lock_path)
        && check_pid_alive(record.pid)
        && crate::daemon::process::probe_or_cleanup_stale_socket(&sock_path)
    {
        return Ok(DaemonStartResult::AlreadyRunning {
            pid: record.pid,
            socket_path: sock_path,
        });
    }
    // Already running per registry (json or synthesized)?
    if let Some(e) = crate::daemon::registry::DaemonRegistry::list_active(runtime_dir)
        .into_iter()
        .find(|e| e.name == name && parse_daemon_pid(i64::from(e.pid)).is_some())
        && crate::daemon::process::probe_or_cleanup_stale_socket(&e.socket_path)
    {
        return Ok(DaemonStartResult::AlreadyRunning {
            pid: e.pid,
            socket_path: e.socket_path,
        });
    }

    let (spawned_pid, _) = spawn_daemon_in(runtime_dir, name)?;
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(3000) {
        std::thread::sleep(Duration::from_millis(25));
        if !crate::daemon::process::probe_or_cleanup_stale_socket(&sock_path) {
            continue;
        }
        let pid = read_lock_record(&lock_path)
            .map(|r| r.pid)
            .filter(|p| check_pid_alive(*p))
            .or_else(|| {
                crate::daemon::registry::DaemonRegistry::list_active(runtime_dir)
                    .into_iter()
                    .find(|e| e.name == name && parse_daemon_pid(i64::from(e.pid)).is_some())
                    .map(|e| e.pid)
            })
            .unwrap_or(spawned_pid);
        debug_assert!(pid > 1);
        // Make sure the registry holds a real record for this daemon.
        let registered = crate::daemon::registry::DaemonRegistry::list_active(runtime_dir)
            .iter()
            .any(|e| e.name == name && e.pid == pid);
        if !registered {
            let _ = crate::daemon::registry::DaemonRegistry::register(
                runtime_dir,
                &crate::daemon::registry::DaemonRegistryEntry {
                    id: format!("daemon-{name}-{pid}"),
                    name: name.to_string(),
                    pid,
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    started_at_ms: now_ms(),
                    socket_path: sock_path.clone(),
                    sessions_count: 1,
                    panes_count: 0,
                    agents_count: 0,
                    last_heartbeat_ms: now_ms(),
                },
            );
        }
        return Ok(DaemonStartResult::Started {
            pid,
            socket_path: sock_path,
        });
    }

    Err(io::Error::new(
        ErrorKind::TimedOut,
        format!("timed out waiting for daemon '{name}' socket to become ready"),
    ))
}

/// Registers the running daemon in the registry, returning a guard that unregisters on drop.
pub struct RegistrationGuard {
    runtime_dir: PathBuf,
    name: String,
}

impl Drop for RegistrationGuard {
    fn drop(&mut self) {
        let _ = crate::daemon::registry::DaemonRegistry::unregister(&self.runtime_dir, &self.name);
    }
}

/// Registers the current process as daemon `name` (called by the headless daemon before serving).
pub fn register_current_daemon(
    runtime_dir: &Path,
    name: &str,
    socket_path: PathBuf,
    sessions: usize,
    panes: usize,
) -> io::Result<RegistrationGuard> {
    let pid = std::process::id();
    crate::daemon::registry::DaemonRegistry::register(
        runtime_dir,
        &crate::daemon::registry::DaemonRegistryEntry {
            id: format!("daemon-{name}-{pid}"),
            name: name.to_string(),
            pid,
            version: env!("CARGO_PKG_VERSION").to_string(),
            started_at_ms: now_ms(),
            socket_path,
            sessions_count: sessions.max(1),
            panes_count: panes,
            agents_count: 0,
            last_heartbeat_ms: now_ms(),
        },
    )?;
    Ok(RegistrationGuard {
        runtime_dir: runtime_dir.to_path_buf(),
        name: name.to_string(),
    })
}

#[cfg(unix)]
fn terminate_pid(pid: u32, force: bool) {
    let sig = if force { libc::SIGKILL } else { libc::SIGTERM };
    unsafe {
        libc::kill(pid as libc::pid_t, sig);
    }
    for _ in 0..10 {
        std::thread::sleep(Duration::from_millis(20));
        if !check_pid_alive(pid) {
            return;
        }
    }
    if !force {
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGKILL);
        }
    }
}

/// Tries a non-blocking exclusive `flock` on `lock_path`.
///
/// Returns `Ok(Some(file))` (lock now held by the caller) when no daemon holds it, i.e. the
/// lockfile is stale, and `Ok(None)` when a live daemon holds it.
#[cfg(unix)]
fn claim_unheld_lock(lock_path: &Path) -> io::Result<Option<File>> {
    let file = private_open_options().read(true).open(lock_path)?;
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Ok(Some(file));
    }
    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::EWOULDBLOCK) {
        Ok(None)
    } else {
        Err(err)
    }
}

/// Gracefully stops daemon `name`.
pub fn stop_daemon(runtime_dir: &Path, name: &str, force: bool) -> io::Result<bool> {
    #[cfg(not(unix))]
    let _ = force;
    validate_safe_name(name)
        .map_err(|e| io::Error::new(ErrorKind::InvalidInput, format!("invalid daemon name: {e}")))?;

    let lock_path = daemon_lock_path_in(runtime_dir, name)?;
    let record = if let Ok(mut f) = File::open(&lock_path) {
        let mut s = String::new();
        if f.read_to_string(&mut s).is_ok() {
            DaemonLockRecord::from_json(&s).ok()
        } else {
            None
        }
    } else {
        None
    };

    let Some(rec) = record else {
        // No lock record: fall back to the registry (json / synthesized) for a live daemon.
        let mut stopped = false;
        let sock = crate::daemon::process::session_daemon_socket_path_in(runtime_dir, name)?;
        if let Some(e) = crate::daemon::registry::DaemonRegistry::list_active(runtime_dir)
            .into_iter()
            .find(|e| e.name == name)
        {
            #[cfg(unix)]
            if parse_daemon_pid(i64::from(e.pid)).is_some()
                && e.pid != std::process::id()
                && check_pid_alive(e.pid)
                && crate::daemon::process::probe_or_cleanup_stale_socket(&e.socket_path)
            {
                terminate_pid(e.pid, force);
                stopped = true;
            }
            let _ = fs::remove_file(&e.socket_path);
        }
        let _ = fs::remove_file(&lock_path);
        let _ = fs::remove_file(sock);
        let _ = crate::daemon::registry::DaemonRegistry::unregister(runtime_dir, name);
        return Ok(stopped);
    };

    // Only signal a PID whose daemon still holds the flock: if we can claim the lock
    // ourselves it is stale, and the recorded PID may now belong to an unrelated process.
    #[cfg(unix)]
    match claim_unheld_lock(&lock_path) {
        Ok(None) => {}
        Ok(Some(_stale_guard)) => {
            let _ = fs::remove_file(&lock_path);
            let _ = fs::remove_file(&rec.socket_path);
            let _ = crate::daemon::registry::DaemonRegistry::unregister(runtime_dir, name);
            return Ok(false);
        }
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    }

    if rec.pid != std::process::id() && check_pid_alive(rec.pid) {
        #[cfg(unix)]
        terminate_pid(rec.pid, force);
    }

    let _ = fs::remove_file(&lock_path);
    let _ = fs::remove_file(&rec.socket_path);
    let _ = crate::daemon::registry::DaemonRegistry::unregister(runtime_dir, name);

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_json(pid: &str) -> String {
        format!(r#"{{"pid":{pid},"name":"t","version":"0","started_at_ms":0,"socket_path":"/x.sock"}}"#)
    }

    #[test]
    fn parse_daemon_pid_rejects_unsafe_values() {
        assert_eq!(parse_daemon_pid(0), None);
        assert_eq!(parse_daemon_pid(-1), None);
        assert_eq!(parse_daemon_pid(1), None);
        assert_eq!(parse_daemon_pid(i64::from(i32::MAX) + 1), None);
        assert_eq!(parse_daemon_pid(i64::MAX), None);
        assert_eq!(parse_daemon_pid(4242), Some(4242));
        assert_eq!(parse_daemon_pid(i64::from(i32::MAX)), Some(i32::MAX as u32));
        assert!(!check_pid_alive(u32::MAX));
    }

    #[test]
    fn lock_record_rejects_invalid_pids() {
        for bad in ["0", "-1", "1", "4294967295", "99999999999", "\"garbage\"", "null"] {
            assert!(DaemonLockRecord::from_json(&record_json(bad)).is_err(), "pid {bad}");
        }
        assert_eq!(DaemonLockRecord::from_json(&record_json("4242")).unwrap().pid, 4242);
    }

    #[cfg(unix)]
    #[test]
    fn stop_daemon_does_not_signal_pid_from_stale_lock() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime_dir = tmp.path();
        // An unrelated live process whose PID ended up in a lockfile no daemon holds.
        let mut victim = Command::new("sleep").arg("30").spawn().unwrap();
        let lock_path = daemon_lock_path_in(runtime_dir, "stale").unwrap();
        let rec = DaemonLockRecord {
            pid: victim.id(),
            name: "stale".to_string(),
            version: "0".to_string(),
            started_at_ms: 0,
            socket_path: runtime_dir.join("stale.sock"),
        };
        fs::write(&lock_path, rec.to_json()).unwrap();

        let stopped = stop_daemon(runtime_dir, "stale", true).unwrap();
        let survived = victim.try_wait().unwrap().is_none();
        let _ = victim.kill();
        let _ = victim.wait();
        assert!(!stopped, "stale lock must report not_running");
        assert!(survived, "unrelated process must not be signaled");
        assert!(!lock_path.exists(), "stale lockfile is cleaned up");
    }

    #[cfg(unix)]
    #[test]
    fn stop_daemon_treats_flocked_lock_as_live() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime_dir = tmp.path();
        let sock = runtime_dir.join("live.sock");
        let DaemonLockResult::Acquired(_lock) = acquire_lock(runtime_dir, "live", sock).unwrap() else {
            panic!("expected to acquire fresh lock");
        };
        // Held by this process: recognized as live (not stale), and never self-signaled.
        assert!(stop_daemon(runtime_dir, "live", false).unwrap());
    }
}
