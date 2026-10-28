//! Live Background OS Process Daemon Spawner & Socket Discovery (`DAEM-03`, `DAEM-04`, `SEC-R01..03`).
//!
//! Manages per-session background OS daemon processes (`bilterm --daemon --session <name>`)
//! listening on private `0700`/`0600` Unix domain sockets (`<runtime_dir>/daemons/daemon-<session>.sock`).

use std::fs;
use std::io::{self, ErrorKind};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Sanitizes an arbitrary session name into a safe `[A-Za-z0-9_-]+` identifier,
/// defaulting to `"default"` if empty (`SEC-R01`).
pub fn sanitize_session_name(session: &str) -> String {
    let clean: String = session
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect();
    if clean.is_empty() {
        "default".to_string()
    } else {
        clean
    }
}

/// Resolves `<base_dir>/daemons/daemon-<safe_session>.sock` inside a verified `0700` private directory (`SEC-R03`).
pub fn session_daemon_socket_path_in(base_dir: &Path, session: &str) -> io::Result<PathBuf> {
    let dir = crate::secure_fs::private_dir_in(base_dir, "daemons")?;
    let safe = sanitize_session_name(session);
    Ok(dir.join(format!("daemon-{safe}.sock")))
}

/// Resolves the private Unix domain socket path for a named background session daemon (`DAEM-03`).
pub fn session_daemon_socket_path(session: &str) -> io::Result<PathBuf> {
    crate::secure_fs::private_runtime_dir()
        .and_then(|d| session_daemon_socket_path_in(&d, session))
}

/// Probes `sock_path` to check if a live daemon is listening (`true`), or unlinks a stale
/// socket file when connection fails with `ConnectionRefused` or `NotFound` (`false`).
///
/// **Security Invariant (`SEC-R03`):** Never unlinks `sock_path` on `PermissionDenied`.
pub fn probe_or_cleanup_stale_socket(sock_path: &Path) -> bool {
    if fs::symlink_metadata(sock_path).is_err() {
        return false;
    }

    #[cfg(unix)]
    {
        match UnixStream::connect(sock_path) {
            Ok(_stream) => true,
            Err(e)
                if matches!(
                    e.kind(),
                    ErrorKind::ConnectionRefused | ErrorKind::NotFound
                ) =>
            {
                let _ = fs::remove_file(sock_path);
                false
            }
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        sock_path.exists()
    }
}

/// Spawns a detached background OS daemon process (`bilterm --daemon --session <name> -l unix:<sock>`)
/// if one is not already listening on the session's private socket (`DAEM-03`).
pub fn spawn_background_daemon_for_session(session: &str) -> io::Result<PathBuf> {
    let sock_path = session_daemon_socket_path(session)?;
    if probe_or_cleanup_stale_socket(&sock_path) {
        return Ok(sock_path);
    }

    let safe = sanitize_session_name(session);
    let exe = std::env::current_exe()?;
    let listen_arg = format!("unix:{}", sock_path.display());

    let mut cmd = Command::new(exe);
    cmd.args(["--daemon", "--session", &safe, "-l", &listen_arg])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(unix)]
    {
        cmd.process_group(0);
    }

    let _child = cmd.spawn()?;
    Ok(sock_path)
}

/// Scans `<base_dir>/daemons` for live `daemon-<session>.sock` endpoints, cleaning up stale sockets
/// and returning sorted `(session_name, socket_path)` pairs (`DAEM-03`).
pub fn list_active_daemon_sessions_in(base_dir: &Path) -> Vec<(String, PathBuf)> {
    let dir = base_dir.join("daemons");
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut active = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(fname) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Some(rest) = fname.strip_prefix("daemon-")
            && let Some(session_name) = rest.strip_suffix(".sock")
            && !session_name.is_empty()
            && probe_or_cleanup_stale_socket(&path)
        {
            active.push((session_name.to_string(), path));
        }
    }
    active.sort_by(|a, b| a.0.cmp(&b.0));
    active
}

/// Returns all live background daemon `(session_name, socket_path)` pairs in the private runtime directory.
pub fn list_active_daemon_sessions() -> Vec<(String, PathBuf)> {
    crate::secure_fs::private_runtime_dir()
        .map(|d| list_active_daemon_sessions_in(&d))
        .unwrap_or_default()
}
