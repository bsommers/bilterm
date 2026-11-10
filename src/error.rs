use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement, EnterAlternateScreen,
    LeaveAlternateScreen,
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

/// Process-wide host TUI state: set by whichever guard entered raw mode / the alternate screen,
/// read by every guard (the panic-hook guard is created before the TUI is entered).
static HOST_TUI_ACTIVE: AtomicBool = AtomicBool::new(false);
static HOST_KITTY_PUSHED: AtomicBool = AtomicBool::new(false);

/// RAII guard and panic-safe restoration controller for host terminal state.
///
/// Ensures raw mode, mouse capture, bracketed paste, and the alternate screen
/// are cleanly torn down on normal exit, early `?` return, signal termination,
/// or thread panic.
#[derive(Clone, Debug)]
pub struct TerminalRestoreGuard {
    active: Arc<AtomicBool>,
    /// The kitty keyboard enhancement flags were pushed and must be popped on restore (Phase 63).
    kitty_pushed: Arc<AtomicBool>,
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
            kitty_pushed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Enters host terminal raw mode, alternate screen, mouse capture, and bracketed paste.
    /// Pushes the kitty keyboard protocol when the terminal supports it (`keys.kitty_protocol = "auto"`).
    pub fn enter_host_tui() -> Result<Self> {
        Self::enter_host_tui_with(crate::config::keys::KittyProtocol::Auto)
    }

    /// [`Self::enter_host_tui`] with an explicit `keys.kitty_protocol` policy.
    ///
    /// With `Auto` and `supports_keyboard_enhancement() == Ok(true)` this pushes
    /// `DISAMBIGUATE_ESCAPE_CODES` after raw mode is on (so `Ctrl+Shift+<key>` and `Primary`
    /// chords become visible) and records it in [`crate::input::keyenv::kitty_active`]; the flags are
    /// popped on restore. Children still receive legacy key bytes (see `encode_key_to_pty_bytes_full`).
    pub fn enter_host_tui_with(kitty: crate::config::keys::KittyProtocol) -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste
        )?;
        let push_kitty = kitty == crate::config::keys::KittyProtocol::Auto
            && matches!(supports_keyboard_enhancement(), Ok(true));
        if push_kitty {
            execute!(
                stdout,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )?;
            crate::input::keyenv::set_kitty_active(true);
        }
        stdout.flush()?;
        HOST_TUI_ACTIVE.store(true, Ordering::SeqCst);
        HOST_KITTY_PUSHED.store(push_kitty, Ordering::SeqCst);
        Ok(Self {
            active: Arc::new(AtomicBool::new(true)),
            kitty_pushed: Arc::new(AtomicBool::new(push_kitty)),
        })
    }

    /// Like [`Self::new_simulated_active`], but also sets the process-wide host state the way
    /// [`Self::enter_host_tui_with`] does (no real stdout; for testing hook-side guards).
    pub fn new_simulated_host_active() -> Self {
        HOST_TUI_ACTIVE.store(true, Ordering::SeqCst);
        Self::new_simulated_active()
    }

    /// Creates a guard flagged as active without touching real stdout (for unit testing drop/restore semantics).
    pub fn new_simulated_active() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(true)),
            kitty_pushed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns whether the guard currently considers the host TUI active.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst) || HOST_TUI_ACTIVE.load(Ordering::SeqCst)
    }

    /// Idempotently restores host terminal state if active.
    pub fn restore(&self) {
        let own = self.active.swap(false, Ordering::SeqCst);
        let host = HOST_TUI_ACTIVE.swap(false, Ordering::SeqCst);
        if own || host {
            let _ = disable_raw_mode();
            let mut stdout = io::stdout();
            let own_kitty = self.kitty_pushed.swap(false, Ordering::SeqCst);
            let host_kitty = HOST_KITTY_PUSHED.swap(false, Ordering::SeqCst);
            if own_kitty || host_kitty {
                let _ = execute!(stdout, PopKeyboardEnhancementFlags);
                crate::input::keyenv::set_kitty_active(false);
            }
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
        HOST_TUI_ACTIVE.store(false, Ordering::SeqCst);
        HOST_KITTY_PUSHED.store(false, Ordering::SeqCst);
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
