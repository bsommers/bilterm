//! Operand types of the Phase 63 keyboard actions (`KEY-01..03`). API FROZEN by plan 63-01.
//!
//! Re-exported from [`crate::action`]; kept in their own module so `action.rs` stays under the
//! `ARCH-R04` line budget.

use serde::{Deserialize, Serialize};

/// Which Window-mode sub-mode is active.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WinSub {
    /// hjkl / arrows move focus.
    #[default]
    Focus,
    /// hjkl / arrows resize the focused window.
    Resize,
    /// hjkl / arrows move the focused window.
    Move,
}

impl WinSub {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::Resize => "resize",
            Self::Move => "move",
        }
    }
}

/// `:wmode [on|off|focus|resize|move]` operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowModeOp {
    /// Enter sticky Window mode (Focus sub-mode).
    Enter,
    /// Leave Window mode.
    Exit,
    /// Enter when inactive, leave when active.
    Toggle,
    /// Switch to a sub-mode (entering Window mode when it is not active).
    Sub(WinSub),
}

impl WindowModeOp {
    /// Canonical `:wmode` colon form.
    pub fn colon_form(&self) -> String {
        match self {
            Self::Enter => ":wmode on".to_string(),
            Self::Exit => ":wmode off".to_string(),
            Self::Toggle => ":wmode".to_string(),
            Self::Sub(s) => format!(":wmode {}", s.name()),
        }
    }
}

/// `:keys [list|check|explain <chord>|profile [name]]` operations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeysOp {
    /// List the active profile's chord -> action table.
    List,
    /// Report key binding conflicts (internal duplicates in 63-01; host/consumer in 63-03).
    Check,
    /// Explain what a chord does in the active profile.
    Explain(String),
    /// Show (`None`) or switch (`Some(name)`) the active key profile.
    Profile(Option<String>),
}

impl KeysOp {
    /// Canonical `:keys` colon form.
    pub fn colon_form(&self) -> String {
        match self {
            Self::List => ":keys list".to_string(),
            Self::Check => ":keys check".to_string(),
            Self::Explain(c) => format!(":keys explain {c}"),
            Self::Profile(None) => ":keys profile".to_string(),
            Self::Profile(Some(n)) => format!(":keys profile {n}"),
        }
    }
}
