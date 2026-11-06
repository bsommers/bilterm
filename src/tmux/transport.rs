//! PTY-backed link transport: `ssh -tt ... tmux -CC` behind `LinkDriver`/`LinkConnector`
//! (Phase 51.1, plan 03 fills the bodies; only plan 03 may start a child process).
//!
//! Seam state (plan 01): no process is ever started.

use crate::tmux::link::{DriverPoll, LinkConnector, LinkDriver, LinkTarget, LinkTransport};

/// Driver over a PTY child (plan 03).
#[derive(Debug, Default)]
pub struct PtyLinkDriver {}

impl LinkDriver for PtyLinkDriver {
    fn pump(&mut self) -> DriverPoll {
        DriverPoll::default()
    }
    fn write_raw(&mut self, _bytes: &[u8]) -> std::io::Result<()> {
        Ok(())
    }
    fn resize(&mut self, _rows: u16, _cols: u16) {}
    fn kill(&mut self) {}
}

/// Connector that spawns the real ssh PTY child (plan 03).
#[derive(Debug, Default)]
pub struct PtyLinkConnector {}

impl LinkConnector for PtyLinkConnector {
    fn connect(&mut self, _target: &LinkTarget, _size: (u16, u16)) -> std::io::Result<LinkTransport> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "PTY link transport not implemented yet",
        ))
    }
}
