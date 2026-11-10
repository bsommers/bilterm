//! Launches headless Chrome with `--remote-debugging-pipe` and owns its lifetime (`WEB-01`).
//!
//! Chrome reads commands on fd 3 and writes events on fd 4. The child runs in its own process
//! group so one `SIGKILL` to the group takes down the renderer, GPU and zygote processes too. The
//! profile lives in a per-process, per-pane directory inside the private runtime dir and is removed
//! when the browser is dropped. Child-process pipes, not terminal traffic.

use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, Once};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::pane::PaneId;
use crate::secure_fs::{private_dir_in, private_runtime_dir};

const PROFILE_PREFIX: &str = "profile-";
const STDERR_LOG: &str = "chrome-stderr.log";

/// A running Chrome and the directory it owns. Dropping it kills the process group.
pub struct ChromeProcess {
    child: Option<Child>,
    pgid: i32,
    profile_dir: PathBuf,
}

/// Pipe ends connected to Chrome: read events from `events`, write commands to `commands`.
pub struct ChromePipes {
    pub events: File,
    pub commands: File,
}

fn random_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    h.finish()
}

/// Directory that holds all browser profiles of this user (`<runtime>/browser`, mode 0700).
pub fn profiles_root() -> io::Result<PathBuf> {
    private_dir_in(&private_runtime_dir()?, "browser")
}

/// Creates a fresh private profile directory unique to this process and pane.
pub fn create_profile_dir(root: &Path, pane_id: PaneId) -> io::Result<PathBuf> {
    let name = format!("{PROFILE_PREFIX}{}-{pane_id}-{:016x}", std::process::id(), random_u64());
    let dir = root.join(&name);
    if dir.exists() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "browser profile dir already exists"));
    }
    private_dir_in(root, &name)
}

/// Owning pid encoded in a profile directory name, if it is one of ours.
pub fn profile_owner_pid(name: &str) -> Option<i32> {
    name.strip_prefix(PROFILE_PREFIX)?.split('-').next()?.parse().ok()
}

fn pid_is_alive(pid: i32) -> bool {
    // SAFETY: signal 0 only checks for existence.
    pid > 0 && (unsafe { libc::kill(pid, 0) } == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM))
}

/// Removes profile directories left behind by bilterm processes that no longer exist.
pub fn sweep_stale_profiles(root: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(root) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(profile_owner_pid) else { continue };
        let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
        if is_dir && !pid_is_alive(pid) && std::fs::remove_dir_all(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Duplicates `fd` to a descriptor >= 10 (close-on-exec) so it can never collide with fd 3 or 4.
fn dup_high(fd: RawFd) -> io::Result<OwnedFd> {
    // SAFETY: plain fcntl on a descriptor we own; the result is a new owned descriptor.
    let new = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 10) };
    if new < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `new` was just returned by fcntl and is owned by nobody else.
    Ok(unsafe { OwnedFd::from_raw_fd(new) })
}

/// Chrome arguments. The page URL is never passed on argv: navigation goes through CDP after
/// validation. The sandbox stays on (no `--no-sandbox`).
pub fn chrome_args(profile_dir: &Path, width_px: u32, height_px: u32) -> Vec<String> {
    vec![
        "--headless=new".to_string(),
        "--remote-debugging-pipe".to_string(),
        format!("--user-data-dir={}", profile_dir.to_string_lossy()),
        format!("--window-size={width_px},{height_px}"),
        "--hide-scrollbars".to_string(),
        "--mute-audio".to_string(),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-component-update".to_string(),
        "--disable-sync".to_string(),
        "--disable-breakpad".to_string(),
        "--disable-default-apps".to_string(),
        "--password-store=basic".to_string(),
        "about:blank".to_string(),
    ]
}

/// Launches `bin` with a new profile under `root`. Profile creation failure aborts the launch.
pub fn launch(bin: &Path, root: &Path, pane_id: PaneId, width_px: u32, height_px: u32) -> io::Result<(ChromeProcess, ChromePipes)> {
    let profile_dir = create_profile_dir(root, pane_id)?;
    let result = spawn_chrome(bin, &profile_dir, width_px, height_px);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&profile_dir);
    }
    let (child, pipes) = result?;
    let pgid = child.id() as i32;
    Ok((ChromeProcess { child: Some(child), pgid, profile_dir }, pipes))
}

fn spawn_chrome(bin: &Path, profile_dir: &Path, width_px: u32, height_px: u32) -> io::Result<(Child, ChromePipes)> {
    // commands: we write, Chrome reads on fd 3. events: Chrome writes on fd 4, we read.
    let (cmd_read, cmd_write) = io::pipe()?;
    let (evt_read, evt_write) = io::pipe()?;
    let child_in = dup_high(cmd_read.as_raw_fd())?;
    let child_out = dup_high(evt_write.as_raw_fd())?;
    drop((cmd_read, evt_write));

    let stderr = crate::secure_fs::create_private_file(&profile_dir.join(STDERR_LOG)).map_or_else(|_| Stdio::null(), Stdio::from);
    let mut cmd = Command::new(bin);
    cmd.args(chrome_args(profile_dir, width_px, height_px))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .process_group(0);
    let (fd_in, fd_out) = (child_in.as_raw_fd(), child_out.as_raw_fd());
    // SAFETY: only async-signal-safe dup2 runs between fork and exec; both source descriptors are
    // >= 10, so neither dup2 can clobber the other's source. dup2 clears close-on-exec on 3 and 4.
    unsafe {
        cmd.pre_exec(move || {
            if libc::dup2(fd_in, 3) < 0 || libc::dup2(fd_out, 4) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = cmd.spawn()?;
    drop((child_in, child_out));
    Ok((child, ChromePipes { events: File::from(OwnedFd::from(evt_read)), commands: File::from(OwnedFd::from(cmd_write)) }))
}

impl ChromeProcess {
    pub fn pgid(&self) -> i32 {
        self.pgid
    }

    pub fn profile_dir(&self) -> &Path {
        &self.profile_dir
    }

    /// Last line Chrome wrote to stderr (bounded), for launch-failure messages.
    pub fn stderr_tail(&self) -> Option<String> {
        use std::io::{Read, Seek, SeekFrom};
        let mut f = File::open(self.profile_dir.join(STDERR_LOG)).ok()?;
        let len = f.metadata().ok()?.len();
        f.seek(SeekFrom::Start(len.saturating_sub(2048))).ok()?;
        let mut buf = Vec::new();
        f.take(2048).read_to_end(&mut buf).ok()?;
        let text = String::from_utf8_lossy(&buf);
        let line = text.lines().rev().find(|l| !l.trim().is_empty())?.trim();
        Some(line.chars().filter(|c| !c.is_control()).take(160).collect())
    }

    /// Sends `SIGKILL` to the whole process group. Safe to call more than once.
    pub fn kill_group(&self) {
        if self.child.is_some() && self.pgid > 1 {
            // SAFETY: the group id is our own child's pid (it leads its group); the child has not
            // been reaped yet, so the id cannot have been reused.
            unsafe { libc::kill(-self.pgid, libc::SIGKILL) };
        }
    }
}

impl Drop for ChromeProcess {
    fn drop(&mut self) {
        self.kill_group();
        let Some(mut child) = self.child.take() else { return };
        let dir = std::mem::take(&mut self.profile_dir);
        let pgid = self.pgid;
        // Reap and delete off the calling (UI) thread: wait() and remove_dir_all can take a while.
        let reap = move || {
            // A member that was in the middle of fork() when the first SIGKILL went out can miss
            // it and outlive the group leader (seen on macOS). Signal again a few times BEFORE the
            // reap: until `wait()` the leader's pid, and so the group id, cannot be reused. The
            // group is never signalled after the reap.
            for _ in 0..REKILL_ROUNDS {
                std::thread::sleep(REKILL_PAUSE);
                if pgid > 1 {
                    // SAFETY: see above; the unreaped leader pins the group id to our child.
                    unsafe { libc::kill(-pgid, libc::SIGKILL) };
                }
            }
            let _ = child.wait();
            for _ in 0..50 {
                if std::fs::remove_dir_all(&dir).is_ok() || !dir.exists() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        };
        match std::thread::Builder::new().name("bilterm-chrome-reaper".into()).spawn(reap) {
            Ok(handle) => track_reaper(handle),
            Err(e) => tracing::warn!("browser: could not spawn reaper thread: {e}"),
        }
    }
}

/// Reaper threads still running. Process exit would cut them off and leave a profile directory
/// behind, so an `atexit` hook gives them a short, bounded time to finish.
static REAPERS: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());
const EXIT_GRACE: Duration = Duration::from_secs(2);
/// Extra SIGKILL rounds sent to the group before the leader is reaped (3 x 20 ms, off the UI thread).
const REKILL_ROUNDS: u32 = 3;
const REKILL_PAUSE: Duration = Duration::from_millis(20);

fn track_reaper(handle: JoinHandle<()>) {
    static HOOK: Once = Once::new();
    // SAFETY: registers a plain `extern "C"` function with no captured state.
    HOOK.call_once(|| unsafe {
        libc::atexit(finish_reapers_at_exit);
    });
    let mut reapers = REAPERS.lock().unwrap_or_else(|e| e.into_inner());
    reapers.retain(|h| !h.is_finished());
    reapers.push(handle);
}

/// Waits (bounded) for pending reaper threads. Returns `true` when all finished in time.
pub fn finish_reapers(grace: Duration) -> bool {
    let deadline = Instant::now() + grace;
    loop {
        let pending = {
            let mut reapers = REAPERS.lock().unwrap_or_else(|e| e.into_inner());
            reapers.retain(|h| !h.is_finished());
            reapers.len()
        };
        if pending == 0 {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

extern "C" fn finish_reapers_at_exit() {
    finish_reapers(EXIT_GRACE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_dirs_are_unique_private_and_carry_the_pid() {
        let tmp = tempfile::tempdir().unwrap();
        let a = create_profile_dir(tmp.path(), 7).unwrap();
        let b = create_profile_dir(tmp.path(), 7).unwrap();
        assert_ne!(a, b);
        let name = a.file_name().unwrap().to_str().unwrap();
        assert_eq!(profile_owner_pid(name), Some(std::process::id() as i32));
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&a).unwrap().permissions().mode() & 0o777, 0o700);
    }

    #[test]
    fn profile_creation_failure_aborts_the_launch() {
        let missing = Path::new("/nonexistent-bilterm-browser-root");
        assert!(create_profile_dir(missing, 1).is_err());
        assert!(launch(Path::new("/bin/true"), missing, 1, 800, 600).is_err());
    }

    #[test]
    fn sweep_removes_only_profiles_of_dead_processes() {
        let tmp = tempfile::tempdir().unwrap();
        let live = create_profile_dir(tmp.path(), 1).unwrap();
        // pid_max is at most 2^22 on Linux, so this pid cannot exist.
        let dead = tmp.path().join(format!("{PROFILE_PREFIX}2000000000-1-00"));
        std::fs::create_dir(&dead).unwrap();
        let other = tmp.path().join("unrelated");
        std::fs::create_dir(&other).unwrap();
        assert_eq!(sweep_stale_profiles(tmp.path()), 1);
        assert!(live.exists() && other.exists() && !dead.exists());
        assert_eq!(profile_owner_pid("unrelated"), None);
    }

    #[test]
    fn args_keep_the_sandbox_and_never_carry_a_page_url() {
        let args = chrome_args(Path::new("/run/x/profile"), 800, 460);
        assert!(args.iter().any(|a| a == "--remote-debugging-pipe"));
        assert!(args.iter().any(|a| a == "--headless=new"));
        assert!(!args.iter().any(|a| a.contains("no-sandbox") || a.contains("remote-debugging-port")));
        assert_eq!(args.last().map(String::as_str), Some("about:blank"));
    }

    #[test]
    fn pipes_reach_the_child_on_fd_3_and_4_and_drop_kills_the_group() {
        // `sh` stands in for Chrome: it echoes fd 3 to fd 4, proving the descriptor plumbing.
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("fake-chrome.sh");
        std::fs::write(&script, "#!/bin/sh\nread line <&3\nprintf '%s!' \"$line\" >&4\nsleep 60\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let (proc_, mut pipes) = launch(&script, tmp.path(), 3, 800, 600).unwrap();
        use std::io::{Read, Write};
        pipes.commands.write_all(b"ping\n").unwrap();
        let mut got = [0u8; 5];
        pipes.events.read_exact(&mut got).unwrap();
        assert_eq!(&got, b"ping!");
        let (pgid, dir) = (proc_.pgid(), proc_.profile_dir().to_path_buf());
        assert!(dir.exists());
        drop(proc_);
        assert!(finish_reapers(Duration::from_secs(5)), "reaper finished");
        assert!(!dir.exists(), "profile dir removed after drop");
        // The group leader is reaped above, but its SIGKILLed children are reaped by init, and a
        // not-yet-reaped zombie still counts as a group member. Wait (bounded) for that reap.
        let deadline = Instant::now() + Duration::from_secs(5);
        // SAFETY: signal 0 only probes for existence; nothing is delivered.
        while unsafe { libc::kill(-pgid, 0) } == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        // SAFETY: as above.
        assert_ne!(unsafe { libc::kill(-pgid, 0) }, 0, "process group is gone");
    }
}
