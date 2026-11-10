//! Live integration test of the persistent tmux link against a REAL tmux server.
//!
//! Gated: runs only with `BILTERM_TMUX_LIVE=1` (CI without tmux skips it).
//!
//! ```text
//! BILTERM_TMUX_LIVE=1 cargo test --test tmux_link_live -- --nocapture
//! ```
//!
//! Isolation: the tmux server lives on a private socket (`TMUX_TMPDIR` inside a fresh temp dir,
//! with `HOME` there too, so no `~/.tmux.conf` and no default-socket server is touched). The
//! transport is an `ssh` stand-in: a script that receives Bilterm's real ssh argv, ignores the
//! options and the host, and runs the remote command locally in a PTY. Everything after the
//! process boundary is real: `tmux -CC new-session -A`, control mode, `%output`, replies.
//!
//! Scenario: connect, render check, size check, kill the transport, assert the remote session
//! and its counter loop are alive, reconnect, assert continuity (same session, same pane, output
//! produced while away is on screen, nothing duplicated).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use bilterm::pane::PaneId;
use bilterm::tmux::hydrate::HydratePhase;
use bilterm::tmux::link::{LinkState, ReconnectPolicy};
use bilterm::tmux::transport::PtyLinkConnector;
use bilterm::wm::WindowManager;

const SESSION: &str = "bilterm-live";

struct Live {
    dir: PathBuf,
    wm: WindowManager,
    start: Instant,
}

impl Live {
    /// Private-socket tmux command (never the default socket).
    fn tmux(&self, args: &[&str]) -> (bool, String) {
        let out = Command::new("tmux")
            .args(args)
            .env("TMUX_TMPDIR", &self.dir)
            .env("HOME", &self.dir)
            .env_remove("TMUX")
            .output()
            .expect("run tmux");
        (out.status.success(), String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn tick(&mut self) {
        let now = self.start.elapsed().as_millis() as u64;
        self.wm.tick_tmux_link(now);
        std::thread::sleep(Duration::from_millis(15));
    }

    fn wait(&mut self, what: &str, secs: u64, mut done: impl FnMut(&mut Live) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < deadline {
            self.tick();
            if done(self) {
                return;
            }
        }
        panic!("timed out after {secs}s waiting for: {what} (state {:?})", self.wm.tmux_link_state());
    }

    fn connected_and_hydrated(&self) -> bool {
        matches!(self.wm.tmux_link_state(), LinkState::Connected { .. })
            && self.wm.link_hydrate_phase() == HydratePhase::Done
    }

    fn text(&self, pane: PaneId) -> String {
        self.wm.pane(pane).unwrap().vt().contents()
    }

    fn counter(&self) -> u64 {
        std::fs::read_to_string(self.dir.join("counter"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    }
}

fn write_ssh_stand_in(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("ssh-stand-in");
    let script = format!(
        "#!/bin/sh\n# Receives Bilterm's ssh argv; the last argument is the remote command.\n\
         for a in \"$@\"; do last=\"$a\"; done\n\
         unset TMUX TMUX_PANE\n\
         export TMUX_TMPDIR='{d}' HOME='{d}' SHELL=/bin/sh PS1='live$ '\n\
         cd '{d}' && exec /bin/sh -c \"$last\"\n",
        d = dir.display()
    );
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[test]
fn live_link_survives_transport_loss_and_reattaches_to_the_same_session() {
    if std::env::var("BILTERM_TMUX_LIVE").as_deref() != Ok("1") {
        eprintln!("tmux_link_live: skipped (set BILTERM_TMUX_LIVE=1 to run against a real tmux)");
        return;
    }
    if Command::new("tmux").arg("-V").output().is_err() {
        eprintln!("tmux_link_live: skipped (no tmux binary)");
        return;
    }
    // Short path: the tmux socket lives below it and unix socket paths are limited to ~100 bytes.
    let dir = PathBuf::from(format!("/tmp/btl-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let stand_in = write_ssh_stand_in(&dir);

    let mut wm = WindowManager::new_headless(24, 80);
    wm.set_link_connector(Box::new(PtyLinkConnector::with_program(stand_in.to_string_lossy())));
    wm.set_link_policy(ReconnectPolicy { base_ms: 300, ..ReconnectPolicy::default() });
    let pane = wm.connect_ssh_tmux("live@rig", SESSION, vec![]).unwrap();
    let mut live = Live { dir: dir.clone(), wm, start: Instant::now() };

    // 1. Connect: a new remote session is created and attached.
    live.wait("first connect + hydrate", 20, |l| l.connected_and_hydrated());
    let (ok, sessions) = live.tmux(&["list-sessions", "-F", "#{session_name}"]);
    assert!(ok && sessions == SESSION, "one session named {SESSION}: {sessions:?}");
    eprintln!("[live] connected: session={sessions} panes={:?}", live.wm.tmux_link_summary().panes);

    // 2. Size: the remote window is as large as the linked pane.
    let (rows, cols) = live.wm.pane(pane).unwrap().vt().size();
    live.wait("remote window takes the pane size", 10, |l| {
        l.tmux(&["list-panes", "-t", SESSION, "-F", "#{pane_width}x#{pane_height}"]).1
            == format!("{cols}x{rows}")
    });
    eprintln!("[live] remote pane size {cols}x{rows} == linked pane");

    // 3. Render: typed input reaches the remote shell, output (with SGR and UTF-8) comes back.
    let typed = "i=0; while :; do i=$((i+1)); echo $i > $HOME/counter; sleep 0.2; done &\r\
                 printf '\\033[1;32mMARK-%s\\033[0m \\346\\274\\242\\345\\255\\227\\n' $((6*7))\r";
    live.wm.pane_mut(pane).unwrap().write_to_pty(typed.as_bytes()).unwrap();
    live.wait("command output rendered", 10, |l| l.text(pane).contains("MARK-42 漢字"));
    live.wait("counter loop running", 10, |l| l.counter() >= 2);
    let mapping_before = live.wm.tmux_link_summary().panes;
    let panes_before = live.wm.pane_count();
    eprintln!("[live] rendered 'MARK-42 漢字'; counter={}", live.counter());

    // 4. Disconnect: kill the transport (the tmux control client behind the stand-in).
    let (_, client_pid) = live.tmux(&["list-clients", "-F", "#{client_pid}"]);
    let status = Command::new("kill").args(["-9", client_pid.trim()]).status().unwrap();
    assert!(status.success(), "killed transport pid {client_pid}");
    live.wait("link notices the loss", 10, |l| {
        !matches!(l.wm.tmux_link_state(), LinkState::Connected { .. })
    });
    eprintln!("[live] transport pid {client_pid} killed -> state {}", live.wm.tmux_link_state().as_str());

    // 5. THE requirement: the remote session and its programs are still alive.
    assert!(live.tmux(&["has-session", "-t", SESSION]).0, "remote session survives the disconnect");
    let c1 = live.counter();
    std::thread::sleep(Duration::from_millis(900));
    let c2 = live.counter();
    assert!(c2 > c1, "counter keeps counting while disconnected ({c1} -> {c2})");
    // Output produced while Bilterm is away.
    assert!(live.tmux(&["send-keys", "-t", SESSION, "echo AWAY-$((40+2))", "Enter"]).0);
    eprintln!("[live] remote session alive while disconnected; counter {c1} -> {c2}");

    // 6. Reconnect (automatic, backoff): same session, same pane, nothing duplicated or lost.
    live.wait("automatic reconnect + re-hydrate", 20, |l| l.connected_and_hydrated());
    live.wait("output produced while away is on screen", 10, |l| l.text(pane).contains("AWAY-42"));
    assert!(live.text(pane).contains("MARK-42 漢字"), "earlier output is still there");
    assert_eq!(live.text(pane).matches("MARK-42 漢字").count(), 1, "re-hydrate does not duplicate output");
    assert_eq!(live.wm.tmux_link_summary().panes, mapping_before, "same pane mapping");
    assert_eq!(live.wm.pane_count(), panes_before, "no duplicated panes");
    let (_, sessions) = live.tmux(&["list-sessions", "-F", "#{session_name}"]);
    assert_eq!(sessions, SESSION, "re-attached to the same session, no second one");
    let c3 = live.counter();
    assert!(c3 > c2, "the counter never stopped ({c2} -> {c3})");
    eprintln!("[live] reconnected: same session, mapping {mapping_before:?}, 'AWAY-42' hydrated, counter {c3}");

    // 7. User disconnect keeps the session too.
    live.wm.link_disconnect();
    for _ in 0..20 {
        live.tick();
    }
    assert!(live.tmux(&["has-session", "-t", SESSION]).0, "session kept after :ssh-tmux disconnect");
    eprintln!("[live] :ssh-tmux disconnect -> remote session still alive");

    // Cleanup of this test's own private server and directory.
    let _ = live.tmux(&["kill-server"]);
    let _ = std::fs::remove_dir_all(&dir);
}
