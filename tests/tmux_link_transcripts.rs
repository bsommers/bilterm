//! Recorded `tmux -CC` control-mode transcripts fed through the real splitter, bridge and pumps
//! into a headless pane, with assertions on the resulting screen and on every command Bilterm
//! writes back. Each test pins one defect found by driving a real tmux over a real ssh transport
//! (see `tests/tmux_link_live.rs` for the live counterpart).

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::pane::PaneId;
use bilterm::remote::RemoteHostProfile;
use bilterm::tmux::TmuxControlSubprocess;
use bilterm::tmux::link::{DisconnectCause, ExitClass, LinkState, TmuxDisconnect};
use bilterm::tmux::reconnect::{disconnect_text, jittered};
use bilterm::tmux::splitter::{ControlLineSplitter, SplitOut};
use bilterm::wm::WindowManager;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// A linked headless workspace plus a scripted tmux server on the other end of the channels.
struct Rig {
    wm: WindowManager,
    pane: PaneId,
    line_tx: UnboundedSender<String>,
    cmd_rx: UnboundedReceiver<String>,
    splitter: ControlLineSplitter,
    now: u64,
    block: u64,
    /// Every command Bilterm wrote, in order.
    log: Vec<String>,
    /// Reply to the next `display-message` (pane state) and `capture-pane`.
    state: String,
    screen: Vec<String>,
}

impl Rig {
    fn connect() -> Self {
        let mut wm = WindowManager::new_headless(24, 80);
        let pane = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        wm.attach_tmux_control(TmuxControlSubprocess::from_channels("main", cmd_tx, line_rx));
        let mut rig = Rig {
            wm,
            pane,
            line_tx,
            cmd_rx,
            splitter: ControlLineSplitter::new(),
            now: 0,
            block: 100,
            log: Vec::new(),
            state: "0 0 0 80 20 1 0 0 0 0 0".into(),
            screen: vec!["$ ".into()],
        };
        // What a real server sends first: the DCS sentinel, the greeting block, the session.
        rig.feed(b"\x1bP1000p%begin 1 1 0\r\n%end 1 1 0\r\n%session-changed $0 main\r\n");
        rig.settle();
        assert!(matches!(rig.wm.tmux_link_state(), LinkState::Connected { .. }));
        rig
    }

    /// Raw transport bytes, split exactly as the PTY driver does.
    fn feed(&mut self, raw: &[u8]) {
        for piece in self.splitter.push(raw) {
            if let SplitOut::Line(line) = piece {
                self.line_tx.send(line).unwrap();
            }
        }
        self.tick();
    }

    fn tick(&mut self) {
        self.now += 50;
        self.wm.tick_tmux_link(self.now);
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            self.answer(&cmd);
            self.log.push(cmd);
        }
    }

    /// Replies the way tmux does: one `%begin`..`%end` block per command, in order.
    fn answer(&mut self, cmd: &str) {
        self.block += 1;
        let n = self.block;
        let mut body: Vec<String> = Vec::new();
        if cmd.starts_with("list-panes") {
            body.push("@0 %0 bash".into());
        } else if cmd.starts_with("display-message") {
            body.push(format!("bilterm-state [%0 {}]", self.state));
        } else if cmd.starts_with("capture-pane") {
            body.extend(self.screen.iter().cloned());
        } else if cmd.starts_with("refresh-client -A") {
            body.push("%continue %0".into());
        }
        let _ = self.line_tx.send(format!("%begin 1 {n} 1"));
        for l in body {
            let _ = self.line_tx.send(l);
        }
        let _ = self.line_tx.send(format!("%end 1 {n} 1"));
    }

    fn settle(&mut self) {
        for _ in 0..40 {
            self.tick();
        }
    }

    fn text(&self) -> String {
        self.wm.pane(self.pane).unwrap().vt().contents()
    }

    fn sent(&self, prefix: &str) -> Vec<&String> {
        self.log.iter().filter(|c| c.starts_with(prefix)).collect()
    }
}

#[test]
fn utf8_split_across_output_lines_renders_intact() {
    let mut rig = Rig::connect();
    // tmux cuts `%output` at arbitrary bytes and leaves bytes >= 0x80 raw: "漢" = e6 bc a2 and
    // "😀" = f0 9f 98 80 are each split over two lines here, and the chunks over two reads.
    rig.feed(b"%output %0 a\xe6\r\n%output %0 \xbc\xa2b \xf0\x9f");
    rig.feed(b"\r\n%output %0 \x98\x80 c\\015\\012\r\n");
    let text = rig.text();
    assert!(text.contains("a漢b 😀 c"), "got {text:?}");
    assert!(!text.contains('\u{fffd}'), "no replacement characters: {text:?}");
}

#[test]
fn extended_output_is_rendered_and_a_pause_recaptures_then_continues() {
    let mut rig = Rig::connect();
    assert_eq!(rig.sent("refresh-client -f pause-after=").len(), 1, "flow control is requested once per attach");
    assert_eq!(
        rig.sent("set-option destroy-unattached off").len(),
        1,
        "a remote tmux.conf with destroy-unattached on must not destroy the session on disconnect"
    );
    rig.feed(b"%extended-output %0 1500 : STALE-LINE\\015\\012\r\n");
    assert!(rig.text().contains("STALE-LINE"));

    rig.log.clear();
    rig.state = "0 4 1 80 20 1 0 0 0 0 0".into();
    rig.screen = vec!["FRESH-ONE".into(), "$ ok".into()];
    // The pane fell behind: tmux pauses it; output that was already in flight must not be drawn
    // on top of the re-captured screen.
    rig.feed(b"%pause %0\r\n%extended-output %0 9000 : LATE-BACKLOG\r\n");
    rig.settle();
    let order: Vec<&str> = rig.log.iter().map(|c| c.split(' ').next().unwrap()).collect();
    assert_eq!(order, ["display-message", "capture-pane", "refresh-client"], "{:?}", rig.log);
    assert_eq!(rig.log[2], "refresh-client -A '%0:continue'\n", "quoted: tmux rejects a bare leading %");
    let text = rig.text();
    assert!(text.contains("FRESH-ONE") && text.contains("$ ok"), "{text:?}");
    assert!(!text.contains("STALE-LINE") && !text.contains("LATE-BACKLOG"), "{text:?}");
    assert_eq!(rig.wm.pane(rig.pane).unwrap().vt().cursor_position(), (1, 4));

    // Live output flows again after the continue.
    rig.feed(b"%extended-output %0 0 : \\015\\012AFTER\r\n");
    assert!(rig.text().contains("AFTER"));
}

#[test]
fn hydrate_restores_alternate_screen_cursor_and_mouse_mode() {
    let mut rig = Rig::connect();
    rig.settle();
    // Reconnect while `vim` runs remotely: alternate screen on, cursor at column 8 of row 0.
    rig.state = "1 8 0 80 20 1 0 0 0 0 1".into();
    rig.screen = vec!["HELLO-VIM".into(), "~".into(), "~".into()];
    rig.feed(b"%pause %0\r\n");
    rig.settle();
    let vt = rig.wm.pane(rig.pane).unwrap().vt();
    assert!(vt.is_alternate_screen(), "the remote is on the alternate screen");
    assert_eq!(vt.cursor_position(), (0, 8));
    assert!(vt.contents().starts_with("HELLO-VIM"));

    // The application leaves the alternate screen: the primary screen must not show its rows.
    rig.feed(b"%output %0 \\033[?1049l$ done\r\n");
    let vt = rig.wm.pane(rig.pane).unwrap().vt();
    assert!(!vt.is_alternate_screen());
    assert!(!vt.contents().contains("HELLO-VIM"), "{:?}", vt.contents());
}

#[test]
fn remote_client_size_is_the_linked_pane_not_the_whole_window() {
    let mut rig = Rig::connect();
    rig.settle();
    let (rows, cols) = rig.wm.pane(rig.pane).unwrap().vt().size();
    assert!(cols < 80, "the linked pane is one half of an 80-column workspace (got {cols})");
    let sizes = rig.sent("refresh-client -C ");
    assert_eq!(sizes.len(), 1, "{sizes:?}");
    assert_eq!(*sizes[0], format!("refresh-client -C {cols}x{rows}\n"));
}

#[test]
fn a_100_kb_paste_reaches_tmux_complete_and_in_order() {
    let mut rig = Rig::connect();
    rig.settle();
    rig.log.clear();
    let paste: Vec<u8> = (0..100_000u32).map(|i| b"abcdefghij\n"[(i % 11) as usize]).collect();
    rig.wm.pane_mut(rig.pane).unwrap().write_to_pty(&paste).unwrap();
    rig.settle();
    let mut got = Vec::new();
    for cmd in rig.sent("send-keys -t %0 -H") {
        for hex in cmd.trim_end().split(' ').skip(4) {
            got.push(u8::from_str_radix(hex, 16).unwrap());
        }
    }
    assert_eq!(got.len(), paste.len(), "no byte of the paste is dropped");
    assert!(got == paste, "bytes arrive in order");
}

#[test]
fn malformed_remote_input_never_panics_or_breaks_the_link() {
    let mut rig = Rig::connect();
    let mut junk: Vec<u8> = (0u8..=255).cycle().take(4096).collect();
    junk.extend_from_slice(b"\r\n%output\r\n%output %x y\r\n%extended-output %0\r\n%pause\r\n%pause %99999999999999999999\r\n");
    junk.extend_from_slice(b"%layout-change @1 ,,,{{[[\r\n%begin x y z\r\n%end 1 2\r\n%future-notification @1 %2 $3\r\n%output %0 \\03\r\n");
    junk.extend_from_slice(&vec![b'A'; 300 * 1024]);
    junk.extend_from_slice(b"\r\n%output %0 STILL-ALIVE\r\n");
    for chunk in junk.chunks(777) {
        rig.feed(chunk);
    }
    assert!(matches!(rig.wm.tmux_link_state(), LinkState::Connected { .. }));
    assert!(rig.text().contains("STILL-ALIVE"));
}

#[test]
fn nothing_bilterm_writes_ever_kills_a_remote_session() {
    let mut rig = Rig::connect();
    rig.feed(b"%pause %0\r\n");
    rig.settle();
    rig.wm.link_disconnect();
    rig.settle();
    rig.wm.link_reconnect();
    rig.settle();
    assert!(!rig.log.is_empty());
    for cmd in &rig.log {
        assert!(!cmd.contains("kill-"), "{cmd}");
    }
    // No link source file can even spell a kill command.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join("src/tmux"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    for f in ["src/wm/tmux_link.rs", "src/wm/tmux_io.rs", "src/wm/tmux_flow.rs", "src/wm/tmux_remote.rs", "src/remote/mod.rs"] {
        files.push(root.join(f));
    }
    for f in files {
        let src = std::fs::read_to_string(&f).unwrap();
        for word in ["kill-session", "kill-server", "kill-window"] {
            assert!(!src.contains(word), "{} mentions {word}", f.display());
        }
    }
}

#[test]
fn removing_the_link_node_stops_the_transport_instead_of_masking_the_state() {
    let mut rig = Rig::connect();
    assert!(rig.wm.is_tmux_control_attached());
    assert!(rig.wm.disconnect_fleet_node("tmux-me@box-main"));
    // Before: the state read `idle` while the control transport was still attached.
    assert_eq!(*rig.wm.tmux_link_state(), LinkState::Idle);
    assert!(!rig.wm.is_tmux_control_attached(), "idle means the transport is gone");
}

#[test]
fn double_dash_reaches_hosts_named_like_link_verbs() {
    for word in ["info", "reconnect", "disconnect", "prefill"] {
        let parsed = parse_colon_command(&format!(":ssh-tmux -- {word} work")).unwrap().unwrap();
        assert_eq!(parsed, Action::SshTmuxConnect { target: word.into(), session: "work".into() });
        assert!(matches!(
            parse_colon_command(&format!(":ssh-tmux {word}")).unwrap().unwrap(),
            Action::RemoteLink(_)
        ));
    }
    assert_eq!(
        parse_colon_command(":ssh-tmux -- box").unwrap().unwrap(),
        Action::SshTmuxConnect { target: "box".into(), session: "bilterm".into() }
    );
    assert!(parse_colon_command(":ssh-tmux -- -oProxyCommand=x").unwrap().is_err());
    assert!(parse_colon_command(":ssh-tmux --").unwrap().is_err());
}

#[test]
fn link_transport_detects_a_dead_peer_in_seconds_and_stays_attach_or_create() {
    let prof = RemoteHostProfile::new("box", "me@box");
    let link = prof.build_ssh_tmux_cc_args("my session;rm -rf", None).unwrap();
    assert!(link.contains(&"ServerAliveInterval=5".to_string()), "{link:?}");
    assert!(link.contains(&"ServerAliveCountMax=3".to_string()));
    assert!(link.contains(&"ConnectTimeout=15".to_string()));
    let remote = link.last().unwrap();
    assert!(remote.ends_with("tmux -CC new-session -A -s mysessionrm-rf'"), "{remote}");
    // An interactive `:ssh` pane keeps the gentler interval.
    let shell = prof.build_ssh_args(None).unwrap();
    assert!(shell.contains(&"ServerAliveInterval=15".to_string()));
}

#[test]
fn backoff_jitter_is_bounded_and_spread() {
    let mut seen = std::collections::BTreeSet::new();
    for now in 0..2000u64 {
        let d = jittered(1000, 25, now * 37);
        assert!((1000..=1250).contains(&d), "{d}");
        seen.insert(d);
    }
    assert!(seen.len() > 50, "delays are spread, not constant ({})", seen.len());
    assert_eq!(jittered(1000, 0, 12345), 1000);
    assert_eq!(jittered(0, 25, 12345), 0);
}

#[test]
fn missing_or_old_tmux_is_named_in_the_failure_text() {
    let down = |tail: &str| TmuxDisconnect {
        cause: DisconnectCause::ProcessExit(Some(127)),
        reason: None,
        code: Some(127),
        tail: vec!["Connection to box closed.".into(), tail.into()],
        at_ms: 0,
        class: ExitClass::Fatal,
    };
    // The exact line a real remote printed (recorded over ssh), plus the shell variants.
    for line in ["env: 'tmux': No such file or directory", "env: tmux: No such file or directory", "bash: line 1: exec: tmux: not found"] {
        let missing = disconnect_text(&down(line));
        assert!(missing.contains("tmux is not installed on the remote host"), "{line} -> {missing}");
    }
    let old = disconnect_text(&down("tmux: unknown option -- C"));
    assert!(old.contains("too old for control mode"), "{old}");
    // Authentication errors keep ssh's own line (it already says what happened).
    let mut d = down("me@box: Permission denied (publickey).");
    d.tail.remove(0);
    let auth = disconnect_text(&d);
    assert!(auth.contains("Permission denied (publickey)"), "{auth}");
}
