//! `:ssh-tmux prefill|reconnect|disconnect|info` operations (Phase 51.1, UNI-R01).
//!
//! One enum lowered to [`crate::action::Action::RemoteLink`]; keys, the menu, the banner click,
//! the colon grammar and `remote.link` all reach `WindowManager::apply` through it.

/// One link verb. `Prefill` is UI-only (uniformity exception E2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteLinkOp {
    /// Open the prompt prefilled with `:ssh-tmux ` (connect takes a free-text host, E1).
    Prefill,
    /// Reconnect now (restart the transport, reset the attempt counter).
    Reconnect,
    /// Stop the link; the remote tmux session is left running.
    Disconnect,
    /// Report the link state (status line and structured data).
    Info,
}

impl RemoteLinkOp {
    /// Canonical colon form (round-trips through `parse_colon_command`).
    pub fn colon_form(&self) -> &'static str {
        match self {
            RemoteLinkOp::Prefill => ":ssh-tmux prefill",
            RemoteLinkOp::Reconnect => ":ssh-tmux reconnect",
            RemoteLinkOp::Disconnect => ":ssh-tmux disconnect",
            RemoteLinkOp::Info => ":ssh-tmux info",
        }
    }

    /// The closed set of reserved first words (P-13), lowercase.
    pub const WORDS: [&'static str; 4] = ["prefill", "reconnect", "disconnect", "info"];

    /// Parses a reserved word, case-insensitively.
    pub fn from_word(word: &str) -> Option<Self> {
        match word.to_ascii_lowercase().as_str() {
            "prefill" => Some(RemoteLinkOp::Prefill),
            "reconnect" => Some(RemoteLinkOp::Reconnect),
            "disconnect" => Some(RemoteLinkOp::Disconnect),
            "info" => Some(RemoteLinkOp::Info),
            _ => None,
        }
    }
}
