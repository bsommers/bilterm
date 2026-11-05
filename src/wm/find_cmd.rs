//! `:find agent|session|history` modal state (Phase 50.1-01, FND-R01/R02).
//!
//! `FindOp` is the single vocabulary every surface (key, mouse, colon, palette, RPC, MCP) lowers
//! to through `Action::Find` and `WindowManager::apply` (ARCH-R01, UNI-R01). The modal itself is
//! `Modal::Find { kind, query, selected }`; ranked results live in `registry` (`find_results()`).

use super::{Modal, WindowManager};

/// Maximum characters of a Find query (T-50.1-01); mirrors `registry::FIND_QUERY_CAP`.
pub const QUERY_CAP: usize = 128;

/// Which registry a Find window searches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FindKind {
    /// Live agents (panes with an agent state).
    Agent,
    /// Terminal sessions and their panes (live first, then saved sessions).
    Session,
    /// Command history (live ring, then `history.log`).
    History,
}

impl FindKind {
    /// Wire/UI name: `agent`, `session` or `history`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Session => "session",
            Self::History => "history",
        }
    }

    /// Parses `agent|agents|session|sessions|history` (the reserved first words of `:find`, D-U1).
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "agent" | "agents" => Some(Self::Agent),
            "session" | "sessions" => Some(Self::Session),
            "history" => Some(Self::History),
            _ => None,
        }
    }

    /// Next kind in tab order (wraps).
    pub fn next(self) -> Self {
        match self {
            Self::Agent => Self::Session,
            Self::Session => Self::History,
            Self::History => Self::Agent,
        }
    }

    /// Previous kind in tab order (wraps).
    pub fn prev(self) -> Self {
        match self {
            Self::Agent => Self::History,
            Self::Session => Self::Agent,
            Self::History => Self::Session,
        }
    }

    /// Stable index (`Agent` 0, `Session` 1, `History` 2) used for per-kind caches and tab chips.
    pub fn index(self) -> usize {
        match self {
            Self::Agent => 0,
            Self::Session => 1,
            Self::History => 2,
        }
    }
}

/// One Find operation; every surface lowers to exactly one of these (`Action::Find`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FindOp {
    /// Open (or re-target) the Find window on `kind` with `query`.
    Open { kind: FindKind, query: String },
    /// Switch to `kind` (resets the selection).
    SetKind(FindKind),
    /// Next (`true`) or previous (`false`) kind.
    CycleKind(bool),
    /// Replace the whole query.
    SetQuery(String),
    /// Append one typed character.
    Push(char),
    /// Delete the last query character.
    Pop,
    /// Empty the query.
    ClearQuery,
    /// Move the selection by `n` rows; `i16::MIN` is top, `i16::MAX` is bottom.
    Move(i16),
    /// Select the 0-based row `n` (clamped).
    Select(usize),
    /// Jump to the selected result.
    Activate,
    /// Put the selected result on the prompt.
    Prefill,
    /// Close the window.
    Close,
}

/// Truncates `text` to [`QUERY_CAP`] characters.
pub fn cap_query(text: &str) -> String {
    text.chars().take(QUERY_CAP).collect()
}

impl WindowManager {
    /// Whether the Find modal is open.
    pub fn is_find_visible(&self) -> bool {
        matches!(self.modal, Modal::Find { .. })
    }

    /// `(kind, query, selected)` while the Find modal is open.
    pub fn find_modal_state(&self) -> Option<(FindKind, &str, usize)> {
        match &self.modal {
            Modal::Find { kind, query, selected } => Some((*kind, query.as_str(), *selected)),
            _ => None,
        }
    }

    /// Opens the Find modal (replaces any other modal; `Modal` is exclusive).
    pub fn open_find(&mut self, kind: FindKind, query: &str) {
        self.modal = Modal::Find { kind, query: cap_query(query), selected: 0 };
    }

    /// Mutable access to the modal fields; only the Find executor uses this.
    pub(crate) fn find_modal_mut_state(&mut self) -> Option<(&mut FindKind, &mut String, &mut usize)> {
        match &mut self.modal {
            Modal::Find { kind, query, selected } => Some((kind, query, selected)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_parse_cycle_and_names() {
        assert_eq!(FindKind::parse("agents"), Some(FindKind::Agent));
        assert_eq!(FindKind::parse("sessions"), Some(FindKind::Session));
        assert_eq!(FindKind::parse("history"), Some(FindKind::History));
        assert_eq!(FindKind::parse("foo"), None);
        assert_eq!(FindKind::Agent.next().next().next(), FindKind::Agent);
        assert_eq!(FindKind::Agent.prev(), FindKind::History);
        assert_eq!(FindKind::Session.as_str(), "session");
    }

    #[test]
    fn open_caps_the_query() {
        let mut wm = WindowManager::new_headless(24, 80);
        wm.open_find(FindKind::History, &"x".repeat(10_000));
        let (kind, q, sel) = wm.find_modal_state().unwrap();
        assert_eq!((kind, q.chars().count(), sel), (FindKind::History, QUERY_CAP, 0));
    }
}
