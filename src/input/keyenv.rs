//! Key environment and the platform-aware `Primary` modifier (`KEY-02`, Phase 63).
//!
//! A [`KeyEnv`] describes where keystrokes come from (host OS, TUI or native GUI, kitty keyboard
//! protocol available). It is resolved once and injected, never read from `cfg!` deep in routing,
//! so every OS path is testable from a single machine.

use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::event::KeyModifiers;

/// Host operating system as far as key semantics are concerned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HostOs {
    MacOs,
    Windows,
    Linux,
}

impl HostOs {
    /// The OS this binary was compiled for.
    pub const fn detect() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }

    /// Lowercase name (`macos`, `windows`, `linux`).
    pub const fn name(self) -> &'static str {
        match self {
            Self::MacOs => "macos",
            Self::Windows => "windows",
            Self::Linux => "linux",
        }
    }
}

/// Where key events originate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Surface {
    /// Host terminal emulator through crossterm.
    Tui,
    /// Bilterm's own native window (`--gui`): Cocoa, Win32 or X11.
    Gui,
}

/// Immutable description of the keyboard environment (`KEY-02`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEnv {
    pub os: HostOs,
    pub surface: Surface,
    /// The outer terminal accepted the kitty keyboard protocol (so Ctrl+Shift and Super arrive).
    pub kitty_kb: bool,
    /// Bilterm runs inside an outer `tmux` (the `Ctrl+B` leader alias is then left to tmux).
    pub inside_tmux: bool,
}

static KITTY_KB_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Records that the host terminal enhancement flags were pushed (set by `TerminalRestoreGuard`).
pub fn set_kitty_active(on: bool) {
    KITTY_KB_ACTIVE.store(on, Ordering::SeqCst);
}

/// `true` while the kitty keyboard protocol is pushed on the host terminal.
pub fn kitty_active() -> bool {
    KITTY_KB_ACTIVE.load(Ordering::SeqCst)
}

impl KeyEnv {
    /// A plain env: no kitty protocol, not inside tmux.
    pub const fn new(os: HostOs, surface: Surface) -> Self {
        Self { os, surface, kitty_kb: false, inside_tmux: false }
    }

    /// Same env with the kitty keyboard flag set.
    pub const fn with_kitty(mut self, kitty_kb: bool) -> Self {
        self.kitty_kb = kitty_kb;
        self
    }

    /// Deterministic env used by headless workspaces and tests: Linux, TUI, no kitty protocol.
    pub const fn headless() -> Self {
        Self::new(HostOs::Linux, Surface::Tui)
    }

    /// Resolves the live environment for an interactive TUI process.
    pub fn detect() -> Self {
        Self {
            os: HostOs::detect(),
            surface: Surface::Tui,
            kitty_kb: kitty_active(),
            inside_tmux: crate::tmux::TmuxEnv::detect().is_some(),
        }
    }
}

/// The chord modifier set that `Primary` resolves to, or `None` when `Primary` chords are not
/// available (legacy TUI: leader, Window mode and F-keys only).
///
/// Shift is part of `Primary` on Windows/Linux, so no table row may be `Primary+Shift+x`.
pub fn primary_mods(env: &KeyEnv) -> Option<KeyModifiers> {
    match (env.surface, env.os, env.kitty_kb) {
        (Surface::Gui, HostOs::MacOs, _) => Some(KeyModifiers::SUPER),
        (Surface::Gui, _, _) => Some(KeyModifiers::CONTROL | KeyModifiers::SHIFT),
        // Never SUPER in a TUI: the host terminal owns Cmd.
        (Surface::Tui, _, true) => Some(KeyModifiers::CONTROL | KeyModifiers::SHIFT),
        (Surface::Tui, _, false) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_resolves_per_env_table() {
        let cs = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        let rows = [
            (KeyEnv::new(HostOs::MacOs, Surface::Gui), Some(KeyModifiers::SUPER)),
            (KeyEnv::new(HostOs::Windows, Surface::Gui), Some(cs)),
            (KeyEnv::new(HostOs::Linux, Surface::Gui), Some(cs)),
            (KeyEnv::new(HostOs::MacOs, Surface::Tui).with_kitty(true), Some(cs)),
            (KeyEnv::new(HostOs::Linux, Surface::Tui).with_kitty(true), Some(cs)),
            (KeyEnv::new(HostOs::Linux, Surface::Tui), None),
            (KeyEnv::new(HostOs::MacOs, Surface::Tui), None),
            (KeyEnv::new(HostOs::Windows, Surface::Tui), None),
        ];
        for (env, want) in rows {
            assert_eq!(primary_mods(&env), want, "{env:?}");
        }
    }

    #[test]
    fn headless_env_is_deterministic() {
        assert_eq!(KeyEnv::headless(), KeyEnv::new(HostOs::Linux, Surface::Tui));
        assert_eq!(HostOs::MacOs.name(), "macos");
    }
}
