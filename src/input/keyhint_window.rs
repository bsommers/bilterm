//! Window-mode sequence lookup for accelerator hints (`KEY-01`, Phase 63).
//!
//! `window_mode_sequence(&Action::LayoutMode(Stacked))` answers `"w s"`: the keys typed after the
//! leader to reach the action through Window mode. Both halves come from tables (the leader key
//! that enters Window mode from `LEADER_OVERRIDES`, the row from `WINDOW_MODE_BINDINGS`).

use super::keyprofile::leader_key_label;
use super::keyprofile_tables::LEADER_OVERRIDES;
use super::window_mode_tables::{WINDOW_MODE_BINDINGS, WinScope, is_mode_control};
use crate::action::{Action, WindowModeOp};

/// Keys typed after the leader to reach `action` through Window mode (`"w s"`), if any.
/// Mode-control actions have no sequence.
pub fn window_mode_sequence(action: &Action) -> Option<String> {
    if is_mode_control(action) {
        return None;
    }
    let enter = LEADER_OVERRIDES
        .iter()
        .find(|(_, a)| *a == Action::WindowMode(WindowModeOp::Enter))
        .map(|(code, _)| leader_key_label(*code))?;
    let (_, _, code, _) =
        WINDOW_MODE_BINDINGS.iter().find(|(scope, _, _, a)| *scope == WinScope::Focus && a == action)?;
    Some(format!("{enter} {}", leader_key_label(*code)))
}
