//! PTY-backed link transport: `ssh -tt ... tmux -CC` behind `LinkDriver`/`LinkConnector`
//! (Phase 51.1, plan 03; only this plan may start a child process).
//!
//! `tmux -CC` needs a tty on the client side, so the child runs in a PTY. Output is split by
//! `ControlLineSplitter`: control lines go into the channel the window manager polls, everything
//! else (ssh banner, host-key and password prompts, errors) is returned as `raw_output` plus
//! sanitised `diag` lines. Commands queued on the channel are written back to the PTY.

use std::time::Duration;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::error::Result;
use crate::pty::{PtyConfig, PtyEvent, PtySession};
use crate::tmux::link::{
    DriverExit, DriverPoll, LinkConnector, LinkDriver, LinkTarget, LinkTransport,
    LINK_SESSION_DEFAULT,
};
use crate::tmux::splitter::{diag_lines, ControlLineSplitter, SplitOut};
use crate::tmux::TmuxControlSubprocess;

/// Time given to `try_wait` after the PTY stream ended, to learn the exit code.
const EXIT_CODE_PATIENCE: Duration = Duration::from_millis(50);

/// Driver over a PTY child.
pub struct PtyLinkDriver {
    pty: Option<PtySession>,
    splitter: ControlLineSplitter,
    line_tx: Option<UnboundedSender<String>>,
    cmd_rx: Option<UnboundedReceiver<String>>,
    exit_reported: bool,
}

impl Default for PtyLinkDriver {
    /// A driver without a child: pumps nothing, accepts and drops writes.
    fn default() -> Self {
        Self {
            pty: None,
            splitter: ControlLineSplitter::new(),
            line_tx: None,
            cmd_rx: None,
            exit_reported: false,
        }
    }
}

impl std::fmt::Debug for PtyLinkDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtyLinkDriver")
            .field("live", &self.pty.is_some())
            .field("in_control", &self.splitter.in_control())
            .field("exit_reported", &self.exit_reported)
            .finish()
    }
}

impl PtyLinkDriver {
    /// Starts `cfg` in a PTY; the control channel is named `bilterm`.
    pub fn spawn(cfg: PtyConfig) -> Result<(TmuxControlSubprocess, PtyLinkDriver)> {
        Self::spawn_named(LINK_SESSION_DEFAULT, cfg)
    }

    /// Starts `cfg` in a PTY and returns the bridge-facing control handle (channel pair) plus
    /// the driver that feeds it.
    pub fn spawn_named(
        name: &str,
        cfg: PtyConfig,
    ) -> Result<(TmuxControlSubprocess, PtyLinkDriver)> {
        let pty = PtySession::spawn(cfg)?;
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let control = TmuxControlSubprocess::from_channels(name, cmd_tx, line_rx);
        let driver = PtyLinkDriver {
            pty: Some(pty),
            splitter: ControlLineSplitter::new(),
            line_tx: Some(line_tx),
            cmd_rx: Some(cmd_rx),
            exit_reported: false,
        };
        Ok((control, driver))
    }

    fn absorb(&mut self, pieces: Vec<SplitOut>, poll: &mut DriverPoll) {
        for piece in pieces {
            match piece {
                SplitOut::Raw(bytes) => {
                    poll.diag.extend(diag_lines(&bytes));
                    poll.raw_output.extend_from_slice(&bytes);
                }
                SplitOut::ControlStart => poll.control_started = true,
                SplitOut::Line(line) => {
                    if let Some(tx) = self.line_tx.as_ref() {
                        let _ = tx.send(line);
                    }
                }
                SplitOut::ControlEnd => {}
                SplitOut::Dropped(_) => poll.diag.push("control line too long, dropped".into()),
            }
        }
    }

    fn write_queued_commands(&mut self) {
        let (Some(rx), Some(pty)) = (self.cmd_rx.as_mut(), self.pty.as_ref()) else {
            return;
        };
        while let Ok(mut cmd) = rx.try_recv() {
            if !cmd.ends_with('\n') {
                cmd.push('\n');
            }
            if pty.write_all(cmd.as_bytes()).is_err() {
                break;
            }
        }
    }
}

impl LinkDriver for PtyLinkDriver {
    fn pump(&mut self) -> DriverPoll {
        let mut poll = DriverPoll::default();
        if self.exit_reported {
            return poll;
        }
        let events = match self.pty.as_mut() {
            Some(pty) => pty.drain_events(),
            None => return poll,
        };
        let mut exited = false;
        for ev in events {
            match ev {
                PtyEvent::Output(bytes) => {
                    let pieces = self.splitter.push(&bytes);
                    self.absorb(pieces, &mut poll);
                }
                PtyEvent::Exited(_) => {
                    exited = true;
                    break;
                }
            }
        }
        if exited {
            let pieces = self.splitter.flush_partial();
            self.absorb(pieces, &mut poll);
            let code = self
                .pty
                .as_ref()
                .and_then(|pty| pty.exit_code(EXIT_CODE_PATIENCE));
            self.exit_reported = true;
            poll.exited = Some(DriverExit { code });
        } else {
            self.write_queued_commands();
        }
        poll
    }

    fn write_raw(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        match self.pty.as_ref() {
            Some(pty) => pty.write_all(bytes).map_err(std::io::Error::other),
            None => Ok(()),
        }
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        if let Some(pty) = self.pty.as_ref() {
            let _ = pty.resize(rows, cols);
        }
    }

    fn kill(&mut self) {
        // Dropping the session kills and reaps the child on its reaper thread.
        self.pty = None;
        self.cmd_rx = None;
    }
}

/// Connector that spawns the real `ssh -tt` PTY child.
#[derive(Debug, Default)]
pub struct PtyLinkConnector {}

impl LinkConnector for PtyLinkConnector {
    fn connect(&mut self, target: &LinkTarget, size: (u16, u16)) -> std::io::Result<LinkTransport> {
        let args = target
            .profile
            .build_ssh_tmux_cc_args(&target.session, None)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let cfg = PtyConfig {
            program: "ssh".into(),
            args,
            rows: size.0,
            cols: size.1,
            ..PtyConfig::default()
        };
        let (control, driver) = PtyLinkDriver::spawn_named(&target.session, cfg)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(LinkTransport {
            control,
            driver: Some(Box::new(driver)),
        })
    }
}
