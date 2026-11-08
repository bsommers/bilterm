//! Key-mode state machine of the workspace (`KEY-01`, Phase 63). API FROZEN by plan 63-01.
//!
//! Replaces the old `leader_pending: bool`: the workspace is in exactly one of `Normal`,
//! `LeaderPending` (a leader key awaits its next chord) or `Window(sub)` (sticky Window mode).
//! The field stays private to [`WindowManager`] (`ARCH-R02`); `is_leader_pending()` /
//! `set_leader_pending()` remain as thin accessors so existing modal guards keep compiling.

use crate::action::{KeysOp, WinSub, WindowModeOp};
use crate::config::keys::KeyProfileName;
use crate::input::keyenv::{KeyEnv, Surface};
use crate::input::keyprofile::{EffectiveKeymap, KeyProfile};

use super::WindowManager;

/// The workspace key mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyMode {
    #[default]
    Normal,
    /// The leader key was pressed; the next chord is looked up in the leader table.
    LeaderPending,
    /// Sticky Window mode with its active sub-mode.
    Window(WinSub),
}

impl KeyMode {
    pub const fn is_window(self) -> bool {
        matches!(self, Self::Window(_))
    }

    pub const fn is_leader_pending(self) -> bool {
        matches!(self, Self::LeaderPending)
    }

    /// The Window-mode sub-mode, when Window mode is active.
    pub const fn window_sub(self) -> Option<WinSub> {
        match self {
            Self::Window(s) => Some(s),
            _ => None,
        }
    }
}

impl WindowManager {
    /// Current key mode.
    pub fn key_mode(&self) -> KeyMode {
        self.key_mode
    }

    /// `true` while a leader or Window mode awaits input (modal guards use this).
    pub fn key_prefix_active(&self) -> bool {
        self.key_mode != KeyMode::Normal
    }

    /// Enters sticky Window mode (Focus sub-mode), cancelling a pending leader.
    pub fn enter_window_mode(&mut self) {
        self.key_mode = KeyMode::Window(WinSub::Focus);
    }

    /// Leaves Window mode (a pending leader is untouched).
    pub fn exit_window_mode(&mut self) {
        if self.key_mode.is_window() {
            self.key_mode = KeyMode::Normal;
        }
    }

    /// Switches the Window-mode sub-mode, entering Window mode when it is not active.
    pub fn set_window_sub(&mut self, sub: WinSub) {
        self.key_mode = KeyMode::Window(sub);
    }

    /// `true` while the leader key awaits its next chord.
    pub fn is_leader_pending(&self) -> bool {
        self.key_mode.is_leader_pending()
    }

    /// Marks the leader pending (`true`) or cancels it (`false`; Window mode is never clobbered).
    pub fn set_leader_pending(&mut self, pending: bool) {
        if pending {
            self.key_mode = KeyMode::LeaderPending;
        } else if self.key_mode.is_leader_pending() {
            self.key_mode = KeyMode::Normal;
        }
    }

    /// The key environment, with the surface derived from the GUI flag and the kitty keyboard
    /// protocol OR-ed in from the live terminal guard.
    pub fn key_env(&self) -> KeyEnv {
        let mut env = self.key_env;
        env.surface = if self.gui_surface { Surface::Gui } else { env.surface };
        env.kitty_kb |= crate::input::keyenv::kitty_active();
        env
    }

    /// Overrides the key environment (tests, embedders).
    pub fn set_key_env(&mut self, env: KeyEnv) {
        self.key_env = env;
    }

    /// Selects the key profile (same as `:keys profile <name>`).
    pub fn set_key_profile(&mut self, name: KeyProfileName) {
        self.config.keys.profile = name;
    }

    /// The effective keymap for the current config and env.
    pub fn keymap(&self) -> EffectiveKeymap {
        KeyProfile::resolve(&self.config.keys, &self.key_env())
    }

    /// Accelerator text for `action` under this workspace's profile, env and leader key.
    pub fn hint_for_action(&self, action: &crate::action::Action) -> Option<String> {
        let leader = self.config.leader_key.clone();
        self.keymap().hint_for_action_with_leader(action, &leader)
    }

    /// Every chord bound to `action` in this workspace's active profile.
    pub fn keys_for_action(&self, action: &crate::action::Action) -> Vec<crate::input::chord::KeyChord> {
        self.keymap().keys_for_action(action)
    }

    /// Applies a `WindowModeOp` (the `apply` arm).
    pub(crate) fn apply_window_mode(&mut self, op: WindowModeOp) {
        match op {
            WindowModeOp::Enter => self.enter_window_mode(),
            WindowModeOp::Exit => self.exit_window_mode(),
            WindowModeOp::Toggle => {
                if self.key_mode.is_window() {
                    self.exit_window_mode();
                } else {
                    self.enter_window_mode();
                }
            }
            WindowModeOp::Sub(sub) => self.set_window_sub(sub),
        }
        let msg = match self.key_mode {
            KeyMode::Window(sub) => format!("WINDOW mode [{}]: Esc exits", sub.name()),
            _ => "Window mode off".to_string(),
        };
        self.active_pane_mut().set_status_message(Some(msg));
    }

    /// Opens the menu bar from the keyboard (`Action::MenuOpen`), revealing the bar when hidden.
    /// The category is stored unclamped; the menu renderer and key handler clamp it to the table.
    pub(crate) fn open_menu_from_keyboard(&mut self, category: Option<usize>) {
        self.config.show_top_menubar = true;
        self.set_menubar_open_category(Some(category.unwrap_or(0)));
    }

    /// Applies a `KeysOp` (the `apply` arm): list, internal check, explain, profile.
    pub(crate) fn apply_keys_op(&mut self, op: &KeysOp) {
        let msg = crate::input::keyprofile_ops::run_keys_op(self, op);
        self.active_pane_mut().set_status_message(Some(msg));
    }
}
