//! Pane byte transport (`GUARD-02`): a pane is exactly one of a local PTY, a remote link
//! bridge (PTY-less `:ssh-tmux` panes), or nothing (headless / held / disconnected).
//!
//! Making this a sum type removes the contradictory states the old `pty` +
//! `remote_input_tap` + `remote_input` fields allowed (`T-TRN-01`): typed bytes can never be
//! delivered to a local PTY while Remote, and the remote tap cannot exist while Local.

use crate::error::Result;
use crate::pty::PtySession;

/// Remote-link side of a pane: bytes typed into the pane, buffered for the tmux link pump.
#[derive(Debug, Default)]
pub struct RemotePaneBridge {
    /// Tap buffer, bounded by `REMOTE_INPUT_CAP`; drained once per link tick.
    remote_input: Vec<u8>,
}

impl RemotePaneBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Buffers `bytes`, dropping whatever exceeds `REMOTE_INPUT_CAP` until the next drain.
    pub fn push_input(&mut self, bytes: &[u8]) {
        let room = crate::tmux::link::REMOTE_INPUT_CAP.saturating_sub(self.remote_input.len());
        self.remote_input.extend_from_slice(&bytes[..bytes.len().min(room)]);
    }

    /// Drains the buffered bytes.
    pub fn take_input(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.remote_input)
    }

    pub fn pending_len(&self) -> usize {
        self.remote_input.len()
    }
}

/// Sends one terminal query reply (DA, DSR, ...) and logs it when the PTY does not take it
/// (input backlog full or the write side failed); replies are answers to the child, so there is
/// nobody to show the error to.
pub(crate) fn send_query_reply(pty: &PtySession, reply: &[u8]) {
    if let Err(err) = pty.write_all(reply) {
        tracing::warn!(error = %err, bytes = reply.len(), "terminal query reply not sent");
    }
}

/// How a pane's input bytes leave the process.
#[derive(Default)]
pub enum PaneTransport {
    /// Backed by a live local `PtySession`.
    Local(PtySession),
    /// Mirrored to the tmux link; never touches a local PTY.
    Remote(RemotePaneBridge),
    /// Nothing attached: headless, held after exit, or orphaned. Writes are dropped.
    #[default]
    None,
}

impl std::fmt::Debug for PaneTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Local(_) => "Local",
            Self::Remote(_) => "Remote",
            Self::None => "None",
        })
    }
}

impl PaneTransport {
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local(_))
    }

    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Remote(_))
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Single byte-delivery point: PTY when Local, tap buffer when Remote, dropped when None.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        match self {
            Self::Local(pty) => pty.write_all(bytes),
            Self::Remote(bridge) => {
                bridge.push_input(bytes);
                Ok(())
            }
            Self::None => Ok(()),
        }
    }

    /// Resizes the local PTY; `Ok(false)` when there is none (or no SIGWINCH was needed).
    pub fn resize(&self, rows: u16, cols: u16) -> Result<bool> {
        match self {
            Self::Local(pty) => pty.resize(rows, cols),
            Self::Remote(_) | Self::None => Ok(false),
        }
    }

    pub fn local_pty(&self) -> Option<&PtySession> {
        match self {
            Self::Local(pty) => Some(pty),
            _ => None,
        }
    }

    pub fn local_pty_mut(&mut self) -> Option<&mut PtySession> {
        match self {
            Self::Local(pty) => Some(pty),
            _ => None,
        }
    }

    pub fn remote_bridge(&self) -> Option<&RemotePaneBridge> {
        match self {
            Self::Remote(b) => Some(b),
            _ => None,
        }
    }

    pub fn remote_bridge_mut(&mut self) -> Option<&mut RemotePaneBridge> {
        match self {
            Self::Remote(b) => Some(b),
            _ => None,
        }
    }

    /// Links to the remote bridge (dropping any local PTY, which is reaped in the background).
    /// Already-Remote transports keep their buffer.
    pub fn link_remote(&mut self) {
        if !self.is_remote() {
            *self = Self::Remote(RemotePaneBridge::new());
        }
    }

    /// Unlinks the remote bridge (buffered bytes are dropped). Local/None are unaffected.
    pub fn unlink_remote(&mut self) {
        if self.is_remote() {
            *self = Self::None;
        }
    }

    /// Drops a local PTY (hold-on-exit); Remote is unaffected.
    pub fn drop_local(&mut self) {
        if self.is_local() {
            *self = Self::None;
        }
    }
}
