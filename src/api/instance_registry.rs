//! Multi-instance discovery for always-on foreground RPC sockets (`LIVE-03`).
//!
//! Every foreground Bilterm process owns `<runtime>/instances/<pid>.sock` (a private `0700`
//! directory under the secure runtime dir). The most recently started instance also repoints the
//! `<runtime>/bilterm.sock` symlink, so a bare `bilterm ctl` reaches the newest foreground
//! instance. A live headless daemon's socket at that path is never replaced. Liveness is checked
//! with [`SignalablePid`], so a recycled or forged pid file name can never signal pid `0`, `1`
//! or a negative process group.

use std::io;
use std::path::{Path, PathBuf};

use crate::daemon::{SignalablePid, check_signalable_pid_alive};

/// Directory holding per-instance sockets.
pub const INSTANCES_DIR: &str = "instances";
/// Name of the "newest instance" symlink (shares the headless daemon's default socket name).
pub const DEFAULT_SOCK: &str = "bilterm.sock";

/// One discovered live foreground instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceInfo {
    pub pid: u32,
    pub socket: PathBuf,
}

/// `<runtime>/instances`.
pub fn instances_dir(runtime: &Path) -> PathBuf {
    runtime.join(INSTANCES_DIR)
}

/// `<runtime>/instances/<pid>.sock`.
pub fn instance_socket_path(runtime: &Path, pid: u32) -> PathBuf {
    instances_dir(runtime).join(format!("{pid}.sock"))
}

/// `<runtime>/bilterm.sock`.
pub fn default_link_path(runtime: &Path) -> PathBuf {
    runtime.join(DEFAULT_SOCK)
}

/// Parses `<pid>.sock` into a verified signalable pid.
fn parse_instance_name(name: &str) -> Option<SignalablePid> {
    let stem = name.strip_suffix(".sock")?;
    if stem.is_empty() || !stem.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    SignalablePid::try_from(stem.parse::<i64>().ok()?).ok()
}

/// Enumerates live instances (oldest pid first). Entries whose process is gone are removed.
pub fn list_live_instances(runtime: &Path) -> Vec<InstanceInfo> {
    let Ok(rd) = std::fs::read_dir(instances_dir(runtime)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(pid) = parse_instance_name(&name) else {
            continue;
        };
        if check_signalable_pid_alive(pid) {
            out.push(InstanceInfo { pid: pid.as_raw(), socket: entry.path() });
        } else {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    out.sort_by_key(|i| i.pid);
    out
}

/// Resolves the socket a client should talk to: `BILTERM_SOCKET`, the `bilterm.sock` link, then
/// the highest-pid live instance.
pub fn resolve_client_socket(runtime: &Path, env_socket: Option<&str>) -> Option<PathBuf> {
    if let Some(s) = env_socket.filter(|s| !s.trim().is_empty()) {
        return Some(PathBuf::from(s.strip_prefix("unix:").unwrap_or(s)));
    }
    let link = default_link_path(runtime);
    if std::fs::symlink_metadata(&link).is_ok() {
        return Some(link);
    }
    list_live_instances(runtime).pop().map(|i| i.socket)
}

/// Owns this process's registered instance socket path; cleans up on drop.
#[derive(Debug)]
pub struct InstanceGuard {
    #[cfg_attr(not(unix), allow(dead_code))] // only the Unix default-link publisher reads it
    runtime: PathBuf,
    socket: PathBuf,
}

impl InstanceGuard {
    /// Creates the private instances directory and reserves `<pid>.sock` for `pid`.
    pub fn register(runtime: &Path, pid: u32) -> io::Result<Self> {
        SignalablePid::try_from(i64::from(pid))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        crate::secure_fs::ensure_private_dir(runtime)?;
        crate::secure_fs::ensure_private_dir(&instances_dir(runtime))?;
        Ok(Self { runtime: runtime.to_path_buf(), socket: instance_socket_path(runtime, pid) })
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket
    }

    /// Repoints `bilterm.sock` at this instance unless a live non-symlink socket owns the name.
    /// Returns whether the link now targets this instance.
    #[cfg(unix)]
    pub fn publish_default_link(&self) -> io::Result<bool> {
        use std::os::unix::fs::symlink;
        let link = default_link_path(&self.runtime);
        if let Ok(meta) = std::fs::symlink_metadata(&link) {
            let ft = meta.file_type();
            if !ft.is_symlink() {
                // A real socket (headless daemon) or stray file: never clobber it.
                return Ok(false);
            }
        }
        let tmp = self.runtime.join(format!(".bilterm.sock.{}", crate::secure_fs::unique_suffix()));
        symlink(&self.socket, &tmp)?;
        if let Err(e) = std::fs::rename(&tmp, &link) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        Ok(true)
    }

    #[cfg(not(unix))]
    pub fn publish_default_link(&self) -> io::Result<bool> {
        Ok(false)
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket);
        #[cfg(unix)]
        {
            let link = default_link_path(&self.runtime);
            if std::fs::read_link(&link).is_ok_and(|t| t == self.socket) {
                let _ = std::fs::remove_file(&link);
                // Fall back to the newest surviving instance, if any.
                if let Some(next) = list_live_instances(&self.runtime).pop() {
                    let _ = std::os::unix::fs::symlink(&next.socket, &link);
                }
            }
        }
    }
}
