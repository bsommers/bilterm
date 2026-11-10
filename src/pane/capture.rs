//! Per-pane command capture outbox types (Phase 50.1, D-U2).
//!
//! 50.1-01 declares the shape; 50.1-10 fills the capture path in `CompositePane`: the decision
//! made before each submit, the metadata written with the command, the bounded outbox and the
//! exit-code annotation. Pane layer: imports nothing outside `std`/`crate::vt`/`crate::pane`.

use std::time::Instant;

use super::CompositePane;
use super::history_store::HistoryMeta;
use super::redact::{Exclusion, classify};
use crate::vt::sanitize_display;

/// Characters per stored command (`sanitize_display`).
pub const CMD_CAP: usize = 512;
/// Captured commands kept per pane between registry ticks.
pub const PANE_OUTBOX_CAP: usize = 64;

/// One submitted command waiting to be drained by `registry::capture::tick`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedCommand {
    /// Sanitised, `<= CMD_CAP` chars, never secret-looking.
    pub command: String,
    pub at: Instant,
    pub cwd: Option<String>,
    pub host: Option<String>,
    pub block_row: Option<usize>,
}

/// What `CompositePane::submit_command` records for one submission (D-U2, P-04).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Decision {
    /// Input to a password prompt: recorded nowhere, only written to the PTY.
    PasswordInput,
    /// Blank or `:colon` text: recorded as before, never captured.
    Legacy,
    /// Secret-looking: kept in memory as before, never written to disk or captured.
    Secret,
    /// A normal command: written with host/transport/cwd and captured.
    Capture,
}

impl CompositePane {
    /// Decides what to record. Must run before the PTY write, since `is_password_prompt()`
    /// describes the program that is waiting for this input.
    pub(super) fn capture_decision(&self, cmd: &str) -> Decision {
        if self.is_password_prompt() {
            return Decision::PasswordInput;
        }
        match classify(cmd) {
            Some(Exclusion::Secret) => Decision::Secret,
            Some(Exclusion::Empty | Exclusion::Colon) => Decision::Legacy,
            None => Decision::Capture,
        }
    }

    /// `(host, transport, cwd)` of the pane. A remote pane never reports the local cwd: only
    /// the OSC 7 reported one (gap G8).
    fn capture_context(&self) -> (Option<String>, Option<&'static str>, Option<String>) {
        match self.remote_target.as_deref() {
            Some(target) => {
                let (host, transport) = match target.split_once('/') {
                    Some((host, _)) => {
                        if !self.transport.is_remote() {
                            let cwd = self.vt.reported_cwd().or(self.tracked_cwd.as_deref());
                            return (None, None, cwd.map(|p| p.display().to_string()));
                        }
                        (host, "ssh-tmux")
                    }
                    None => (target, "ssh"),
                };
                let cwd = self.vt.reported_cwd().map(|p| p.display().to_string());
                (Some(host.to_string()), Some(transport), cwd)
            }
            None => {
                let cwd = self.vt.reported_cwd().or(self.tracked_cwd.as_deref());
                (None, None, cwd.map(|p| p.display().to_string()))
            }
        }
    }

    /// Writes `cmd` to history.log with its metadata and queues it for the registry.
    pub(super) fn record_captured(&mut self, cmd: &str) {
        let (host, transport, cwd) = self.capture_context();
        let meta = HistoryMeta {
            host: host.clone(),
            transport: transport.map(str::to_string),
            cwd: cwd.clone(),
        };
        let _ = self.history_store.record_command_meta(cmd, &meta);
        if self.captured.len() >= PANE_OUTBOX_CAP {
            self.captured.remove(0);
        }
        self.captured.push(CapturedCommand {
            command: sanitize_display(cmd, CMD_CAP),
            at: Instant::now(),
            cwd: cwd.map(|c| sanitize_display(&c, 256)),
            host: host.map(|h| sanitize_display(&h, 64)),
            block_row: self.vt.command_blocks().last().map(|b| b.absolute_row),
        });
    }

    /// Counts a withheld secret-looking command (a number, never text).
    pub(super) fn note_masked(&mut self) {
        self.captured_masked = self.captured_masked.saturating_add(1);
    }

    /// Takes the commands captured since the last drain plus the count of secret-looking commands
    /// withheld meanwhile (Phase 50.1, D-U2).
    pub(crate) fn drain_captured(&mut self) -> (Vec<CapturedCommand>, u32) {
        (std::mem::take(&mut self.captured), std::mem::take(&mut self.captured_masked))
    }

    /// Appends one exit annotation for a finished `command` to history.log (hash only).
    pub(crate) fn record_history_exit(&mut self, command: &str, exit_code: i32) {
        let _ = self.history_store.record_exit(command, exit_code);
    }
}

#[cfg(test)]
mod tests {
    use crate::pane::CompositePane;

    #[test]
    fn fresh_pane_outbox_is_empty_and_drain_resets() {
        let mut pane = CompositePane::new_headless(1, 10, 40);
        assert_eq!(pane.drain_captured(), (Vec::new(), 0));
        assert_eq!(pane.drain_captured(), (Vec::new(), 0));
    }

    #[test]
    fn honest_ssh_tmux_capture_context() {
        let mut pane = CompositePane::new_headless(1, 10, 40);
        pane.set_remote_target(Some("user@host/session".to_string()));
        // Local fallback / unlinked: remote_input_tap is false -> treated as local
        assert_eq!(pane.capture_context().0, None);
        assert_eq!(pane.capture_context().1, None);

        // Connected tmux link: remote_input_tap is true -> treated as ssh-tmux
        pane.set_remote_input_tap(true);
        assert_eq!(pane.capture_context().0.as_deref(), Some("user@host"));
        assert_eq!(pane.capture_context().1, Some("ssh-tmux"));

        // Pure SSH target (no slash) -> always ssh
        pane.set_remote_target(Some("user@host".to_string()));
        pane.set_remote_input_tap(false);
        assert_eq!(pane.capture_context().0.as_deref(), Some("user@host"));
        assert_eq!(pane.capture_context().1, Some("ssh"));
    }
}
