use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossterm::event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use thiserror::Error;

/// Unified error type for Bilterm subsystems.
#[derive(Debug, Error)]
pub enum BiltermError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("PTY error: {0}")]
    Pty(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Tmux integration error: {0}")]
    Tmux(String),
}

pub type Result<T> = std::result::Result<T, BiltermError>;

/// RAII guard and panic-safe restoration controller for host terminal state.
///
/// Ensures raw mode, mouse capture, bracketed paste, and the alternate screen
/// are cleanly torn down on normal exit, early `?` return, signal termination,
/// or thread panic.
#[derive(Clone, Debug)]
pub struct TerminalRestoreGuard {
    active: Arc<AtomicBool>,
}

impl Default for TerminalRestoreGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalRestoreGuard {
    /// Creates an inactive guard (used in headless tests or before entering raw mode).
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Enters host terminal raw mode, alternate screen, mouse capture, and bracketed paste.
    pub fn enter_host_tui() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste
        )?;
        stdout.flush()?;
        Ok(Self {
            active: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Creates a guard flagged as active without touching real stdout (for unit testing drop/restore semantics).
    pub fn new_simulated_active() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Returns whether the guard currently considers the host TUI active.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Idempotently restores host terminal state if active.
    pub fn restore(&self) {
        if self.active.swap(false, Ordering::SeqCst) {
            let _ = disable_raw_mode();
            let mut stdout = io::stdout();
            let _ = execute!(
                stdout,
                DisableBracketedPaste,
                DisableMouseCapture,
                LeaveAlternateScreen
            );
            let _ = stdout.flush();
        }
    }

    /// Marks the guard restored without writing escape sequences to stdout (used in tests).
    pub fn mark_restored_quiet(&self) {
        self.active.store(false, Ordering::SeqCst);
    }

    /// Installs a panic hook that invokes `restore()` before printing the `color_eyre` report.
    pub fn install_panic_hook(&self) -> color_eyre::Result<()> {
        let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::default()
            .display_env_section(false)
            .into_hooks();
        eyre_hook.install()?;

        let guard = self.clone();
        std::panic::set_hook(Box::new(move |panic_info| {
            guard.restore();
            eprintln!("{}", panic_hook.panic_report(panic_info));
        }));
        Ok(())
    }
}

impl Drop for TerminalRestoreGuard {
    fn drop(&mut self) {
        self.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_restore_is_idempotent() {
        let guard = TerminalRestoreGuard::new_simulated_active();
        assert!(guard.is_active());
        guard.mark_restored_quiet();
        assert!(!guard.is_active());
        guard.restore();
        assert!(!guard.is_active());
    }
}
