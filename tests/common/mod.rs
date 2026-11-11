//! Shared, platform-neutral test support (Linux / macOS / Windows, x86_64 / ARM64).
//!
//! Include from an integration test with `mod common;`. Everything here avoids hard-coded
//! `/bin/sh`, `/tmp`, `sleep` and `:`-separated paths so the same test body runs on every CI cell.
#![allow(dead_code)]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Reports a runtime skip so it is visible in the test log even when the test "passes".
///
/// Writes straight to the stderr handle (the libtest capture only intercepts `print!`/`eprintln!`),
/// so `SKIPPED <test>: <reason>` always reaches the log; `scripts/verify.sh` collects these lines
/// into `target/verify/skips.txt`. Prefer `#[cfg_attr(not(unix), ignore = "reason")]` when the skip
/// is known at compile time: libtest then prints `ignored, reason` itself.
pub fn skip(reason: &str) {
    let name = std::thread::current().name().unwrap_or("<unnamed>").to_string();
    // Leading newline: libtest's progress dots share the line, and the collector matches line starts.
    let _ = writeln!(std::io::stderr(), "\nSKIPPED {name}: {reason}");
}

/// Polls `cond` every 10 ms until it holds or `timeout` elapses. Returns whether it held.
/// Use instead of a fixed `sleep`: fast machines finish early, slow ones get the full deadline.
pub fn wait_until(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Generous deadline for anything that waits on another process (CI ARM / Windows runners are slow).
pub const PROCESS_DEADLINE: Duration = Duration::from_secs(30);

/// `(program, args)` running `script` in the platform's stock non-interactive shell:
/// `/bin/sh -c` on Unix, `cmd.exe /C` on Windows.
pub fn shell_command(script: &str) -> (String, Vec<String>) {
    if cfg!(windows) {
        let comspec = std::env::var("COMSPEC").ok().filter(|s| !s.trim().is_empty());
        (comspec.unwrap_or_else(|| "cmd.exe".to_string()), vec!["/C".to_string(), script.to_string()])
    } else {
        ("/bin/sh".to_string(), vec!["-c".to_string(), script.to_string()])
    }
}

/// A shell line that prints `marker` and exits; valid for both `sh` and `cmd.exe`.
pub fn echo_script(marker: &str) -> String {
    format!("echo {marker}")
}

/// Whether `program` can be started at all (used for optional tools such as `git`).
pub fn has_program(program: &str) -> bool {
    Command::new(program).arg("--version").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

/// A loopback TCP port that was free a moment ago.
pub fn free_loopback_port() -> u16 {
    let l = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback port 0");
    l.local_addr().expect("local addr").port()
}

/// Deterministic xorshift64* PRNG: fixed seeds give identical streams on every platform, no deps.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // SplitMix64 scramble so small seeds (0, 1, 2 ...) still start far apart.
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Self((z ^ (z >> 31)) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform-ish value in `0..n` (`n > 0`).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    pub fn byte(&mut self) -> u8 {
        (self.next_u64() >> 32) as u8
    }

    pub fn chance(&mut self, one_in: usize) -> bool {
        self.below(one_in) == 0
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    pub fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.byte()).collect()
    }
}

/// Throwaway HOME / XDG / runtime tree so a spawned `bilterm` never reads or writes the developer's
/// profile, sockets, daemons or sessions (and never talks to a running instance).
pub struct IsolatedEnv {
    root: tempfile::TempDir,
}

impl IsolatedEnv {
    pub fn new() -> Self {
        let root = tempfile::tempdir().expect("create isolated env root");
        for d in ["home", "config", "state", "data", "runtime", "appdata/roaming", "appdata/local", "tmp"] {
            let dir = root.path().join(d);
            std::fs::create_dir_all(&dir).expect("create isolated env dir");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
            }
        }
        Self { root }
    }

    pub fn path(&self) -> &Path {
        self.root.path()
    }

    pub fn dir(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }

    /// Points `cmd` at the isolated tree and strips every variable that could reach a live instance.
    pub fn apply<'a>(&self, cmd: &'a mut Command) -> &'a mut Command {
        let p = |n: &str| self.root.path().join(n);
        cmd.env("HOME", p("home"))
            .env("USERPROFILE", p("home"))
            .env("APPDATA", p("appdata/roaming"))
            .env("LOCALAPPDATA", p("appdata/local"))
            .env("XDG_CONFIG_HOME", p("config"))
            .env("XDG_STATE_HOME", p("state"))
            .env("XDG_DATA_HOME", p("data"))
            .env("XDG_RUNTIME_DIR", p("runtime"))
            .env("BILTERM_RUNTIME_DIR", p("runtime"))
            .env("TMPDIR", p("tmp"))
            .env("TEMP", p("tmp"))
            .env("TMP", p("tmp"))
            // Never let a non-TTY launch pop a terminal emulator window on the host desktop.
            .env("BILTERM_RELAUNCHED", "1")
            .env_remove("BILTERM_SOCKET")
            .env_remove("BILTERM_INSTANCE_PID")
            .env_remove("BILTERM_WORKSPACE_ID")
            .env_remove("BILTERM_PANE_ID")
            .env_remove("BILTERM_TOKEN_FILE")
            .env_remove("BILTERM_GUI")
            .env_remove("BILTERM_TERMINAL")
            .env_remove("TERMINAL")
            .env_remove("TMUX")
    }

    /// `bilterm` with `args`, isolated, stdin closed.
    pub fn bilterm(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bilterm"));
        self.apply(&mut cmd).args(args).stdin(Stdio::null());
        cmd
    }

    /// Runs `bilterm args` to completion (killed and failed after [`PROCESS_DEADLINE`]).
    pub fn run(&self, args: &[&str]) -> Output {
        run_with_deadline(self.bilterm(args), PROCESS_DEADLINE)
    }

    /// Writes a bearer-token file with owner-only permissions and returns its path.
    pub fn token_file(&self, token: &str) -> PathBuf {
        let path = self.root.path().join("rpc.token");
        std::fs::write(&path, format!("{token}\n")).expect("write token file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("chmod token file");
        }
        path
    }
}

/// Kills the child on drop so a failing assertion never leaks a process.
pub struct KillOnDrop(pub Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Runs `cmd` capturing stdout/stderr; panics (after killing it) if it outlives `deadline`.
pub fn run_with_deadline(mut cmd: Command, deadline: Duration) -> Output {
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("spawn child process");
    let mut out = child.stdout.take().expect("child stdout");
    let mut err = child.stderr.take().expect("child stderr");
    let out_t = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = out.read_to_end(&mut b);
        b
    });
    let err_t = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = err.read_to_end(&mut b);
        b
    });
    let mut guard = KillOnDrop(child);
    let mut status = None;
    let finished = wait_until(deadline, || {
        status = guard.0.try_wait().expect("poll child");
        status.is_some()
    });
    if !finished {
        let _ = guard.0.kill();
    }
    let status = status.unwrap_or_else(|| guard.0.wait().expect("reap child"));
    let stdout = out_t.join().unwrap_or_default();
    let stderr = err_t.join().unwrap_or_default();
    assert!(
        finished,
        "process outlived its {deadline:?} deadline; stdout: {} stderr: {}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    );
    Output { status, stdout, stderr }
}

pub fn stdout_of(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

pub fn stderr_of(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}
