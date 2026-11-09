//! View state of the which-key popup (`KEY-01`, Phase 63-02).
//!
//! The popup shows while the leader is pending or Window mode is active, after
//! `keys.which_key_delay_ms` without a key or at once when pinned with `?`. State is private to
//! [`WindowManager`] (`ARCH-R02`); the renderer reads it through [`WindowManager::which_key_visible`].

use std::time::{Duration, Instant};

use super::WindowManager;

/// Idle timer and pin flag of the which-key popup.
#[derive(Clone, Copy, Debug)]
pub struct WhichKeyState {
    since: Instant,
    pinned: bool,
}

impl Default for WhichKeyState {
    fn default() -> Self {
        Self { since: Instant::now(), pinned: false }
    }
}

impl WindowManager {
    /// Restarts the idle timer (a key was handled while a leader or Window mode is active).
    pub(crate) fn touch_which_key(&mut self) {
        self.which_key.since = Instant::now();
    }

    /// Forgets the pin and restarts the idle timer (a prefix mode began or ended).
    pub(crate) fn reset_which_key(&mut self) {
        self.which_key = WhichKeyState::default();
    }

    /// Pins the popup open until the key mode ends (`?` in Window mode).
    pub fn pin_which_key(&mut self) {
        self.which_key.pinned = true;
    }

    /// `true` when the popup is pinned open.
    pub fn which_key_pinned(&self) -> bool {
        self.which_key.pinned
    }

    /// `true` when the which-key popup should be drawn now.
    pub fn which_key_visible(&self) -> bool {
        if !self.key_prefix_active() {
            return false;
        }
        if self.which_key.pinned {
            return true;
        }
        let keys = &self.config.keys;
        keys.which_key && self.which_key.since.elapsed() >= Duration::from_millis(u64::from(keys.which_key_delay_ms))
    }
}
