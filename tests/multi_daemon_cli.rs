//! Multi-daemon support: several named daemons coexist in one runtime dir, each on its own
//! socket, and are listed / targeted / stopped by name from the CLI.
#![cfg(unix)]

use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Private HOME + XDG dirs so the test never sees (or touches) a real bilterm.
struct Sandbox {
    root: tempfile::TempDir,
    hand_started: Vec<Child>,
}

impl Sandbox {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        for d in ["home", "run", "state", "config"] {
            std::fs::create_dir(root.path().join(d)).unwrap();
        }
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path().join("run"), std::fs::Permissions::from_mode(0o700)).unwrap();
        Self { root, hand_started: Vec::new() }
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let p = self.root.path();
        let mut c = Command::new(env!("CARGO_BIN_EXE_bilterm"));
        c.args(args)
            .env("HOME", p.join("home"))
            .env("XDG_RUNTIME_DIR", p.join("run"))
            .env("XDG_STATE_HOME", p.join("state"))
            .env("XDG_CONFIG_HOME", p.join("config"))
            .stdin(Stdio::null());
        for k in ["BILTERM_DAEMON", "BILTERM_SOCKET", "BILTERM_RUNTIME_DIR", "BILTERM_TOKEN_FILE", "BILTERM_PANE_ID"] {
            c.env_remove(k);
        }
        c
    }

    fn run(&self, args: &[&str]) -> Output {
        self.cmd(args).output().unwrap()
    }

    fn daemons_dir(&self) -> std::path::PathBuf {
        self.root.path().join("run/bilterm/daemons")
    }

    /// `bilterm --headless --name NAME` with no `-l`, as a user would type it.
    fn start_by_hand(&mut self, name: &str) {
        let child = self
            .cmd(&["--headless", "--name", name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        self.hand_started.push(child);
        let sock = self.daemons_dir().join(format!("daemon-{name}.sock"));
        wait_until(|| sock.exists(), &format!("socket for daemon '{name}'"));
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        // Stop every daemon this sandbox knows by name, then reap the ones we spawned directly.
        for name in ["alpha", "beta", "default"] {
            let _ = self.run(&["daemon", "stop", name, "--force"]);
        }
        for child in &mut self.hand_started {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn wait_until(mut cond: impl FnMut() -> bool, what: &str) {
    let start = Instant::now();
    while !cond() {
        assert!(start.elapsed() < Duration::from_secs(10), "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn out(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn assert_status_names(sb: &Sandbox, target: &str) {
    let o = sb.run(&["ctl", "--daemon-name", target, "daemon.status"]);
    let text = out(&o);
    assert!(o.status.success(), "ctl --daemon-name {target} failed: {text}");
    assert!(text.contains(&format!("\"name\":\"{target}\"")), "daemon.status via {target}: {text}");
    assert!(text.contains("\"status\":\"running\""), "{text}");
    assert!(text.contains("\"is_current\":true"), "{target} must answer for itself: {text}");
}

fn socket_of(dir: &Path, name: &str) -> String {
    dir.join(format!("daemon-{name}.sock")).display().to_string()
}

#[test]
fn two_named_daemons_coexist_and_are_managed_by_name() {
    let mut sb = Sandbox::new();

    // alpha is started by hand, beta through the CLI: both must come up side by side.
    sb.start_by_hand("alpha");
    let started = sb.run(&["daemon", "start", "beta"]);
    assert!(started.status.success(), "{}", out(&started));
    assert!(out(&started).contains("daemon 'beta' started (PID "), "{}", out(&started));
    let again = sb.run(&["daemon", "start", "beta"]);
    assert!(out(&again).contains("daemon 'beta' is already running"), "{}", out(&again));

    // Neither named daemon took the legacy default socket.
    assert!(!sb.root.path().join("run/bilterm/bilterm.sock").exists());

    // Both are listed, each on its own socket, by `daemon list` and by `ls`.
    let dir = sb.daemons_dir();
    let list = out(&sb.run(&["daemon", "list"]));
    for name in ["alpha", "beta"] {
        let row = list.lines().find(|l| l.starts_with(name)).unwrap_or_else(|| panic!("no {name} row: {list}"));
        assert!(row.contains("running") && row.contains(&socket_of(&dir, name)), "{row}");
    }
    let json = out(&sb.run(&["daemon", "list", "--json"]));
    assert!(json.contains("\"name\":\"alpha\"") && json.contains("\"name\":\"beta\""), "{json}");
    let ls = out(&sb.run(&["ls"]));
    assert!(ls.contains("Active Background Daemons (2)"), "{ls}");
    assert!(ls.contains("* alpha [LIVE DAEMON]") && ls.contains("* beta [LIVE DAEMON]"), "{ls}");

    // Each daemon is reachable by name and reports its own name.
    assert_status_names(&sb, "alpha");
    assert_status_names(&sb, "beta");
    let via_env = sb.cmd(&["ctl", "daemon.status"]).env("BILTERM_DAEMON", "beta").output().unwrap();
    assert!(out(&via_env).contains("\"name\":\"beta\""), "{}", out(&via_env));
    let st = sb.run(&["daemon", "status", "alpha"]);
    assert!(st.status.success() && out(&st).contains("daemon 'alpha': running (PID "), "{}", out(&st));

    // A second daemon with a taken name is refused by the lock, not by a socket clash.
    let dup = sb.run(&["--headless", "--name", "alpha"]);
    assert!(out(&dup).contains("daemon 'alpha' already running"), "{}", out(&dup));

    // Stop alpha by name: beta survives and still answers.
    let stop = sb.run(&["daemon", "stop", "alpha"]);
    assert!(stop.status.success() && out(&stop).contains("daemon 'alpha' stopped"), "{}", out(&stop));
    let st = sb.run(&["daemon", "status", "alpha"]);
    assert_eq!(st.status.code(), Some(3), "{}", out(&st));
    assert!(out(&st).contains("daemon 'alpha': not running"), "{}", out(&st));
    assert_status_names(&sb, "beta");
    let list = out(&sb.run(&["daemon", "list"]));
    assert!(list.lines().any(|l| l.starts_with("beta")) && !list.lines().any(|l| l.starts_with("alpha")), "{list}");

    // A stopped target is reported, never silently replaced by another daemon.
    let gone = sb.run(&["ctl", "--daemon-name", "alpha", "daemon.status"]);
    assert_eq!(gone.status.code(), Some(1), "{}", out(&gone));
    assert!(out(&gone).contains("daemon 'alpha' is not running"), "{}", out(&gone));
    assert!(!sb.root.path().join("run/bilterm/daemons/daemon-default.sock").exists());

    let stop = sb.run(&["daemon", "stop", "beta", "--force"]);
    assert!(out(&stop).contains("daemon 'beta' stopped"), "{}", out(&stop));
    let stop = sb.run(&["daemon", "stop", "beta"]);
    assert!(out(&stop).contains("daemon 'beta' was not running"), "{}", out(&stop));
    assert!(out(&sb.run(&["daemon", "list"])).starts_with("No bilterm daemons running"));
}

#[test]
fn default_daemon_keeps_legacy_socket_and_explicit_listen_wins() {
    let sb = Sandbox::new();
    let run_dir = sb.root.path().join("run/bilterm");

    // Unnamed daemon: legacy `<runtime>/bilterm.sock`, reachable by a plain `ctl`.
    let mut default = sb.cmd(&["--headless"]).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    wait_until(|| run_dir.join("bilterm.sock").exists(), "legacy default socket");
    let plain = sb.run(&["ctl", "daemon.status"]);
    assert!(out(&plain).contains("\"name\":\"default\""), "{}", out(&plain));

    // `-l unix:PATH` overrides the per-name default, and targeting by name still finds it.
    let custom = sb.root.path().join("run/custom.sock");
    let listen = format!("unix:{}", custom.display());
    let mut gamma = sb
        .cmd(&["--headless", "--name", "gamma", "-l", &listen])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_until(|| custom.exists(), "explicit listen socket");
    assert!(!run_dir.join("daemons/daemon-gamma.sock").exists());
    let st = sb.run(&["ctl", "--daemon-name", "gamma", "daemon.status"]);
    assert!(out(&st).contains("\"name\":\"gamma\""), "{}", out(&st));

    let bad = sb.run(&["ctl", "--daemon-name", "../x", "daemon.status"]);
    assert_eq!(bad.status.code(), Some(2), "{}", out(&bad));

    for (name, child) in [("gamma", &mut gamma), ("default", &mut default)] {
        let stop = sb.run(&["daemon", "stop", name]);
        assert!(out(&stop).contains(&format!("daemon '{name}' stopped")), "{}", out(&stop));
        let _ = child.kill();
        let _ = child.wait();
    }
}
