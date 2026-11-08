//! Window-mode sequence lookup for accelerator hints (`KEY-01`, Phase 63).
//!
//! 63-01 ships the extension point only. Plan 63-02 owns `WINDOW_MODE_BINDINGS` and replaces the
//! body so `hint_for_action(&Action::LayoutMode(Stacked), ..)` can answer `Ctrl+X w s`.

use crate::action::Action;

/// Keys typed after the leader to reach `action` through Window mode (`"w s"`), if any.
pub fn window_mode_sequence(_action: &Action) -> Option<String> {
    None
}
