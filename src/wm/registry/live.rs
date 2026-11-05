//! Live agents, panes and sessions (filled by plan 50.1-03). Skeleton: returns nothing.

use super::types::{FindKind, FindRow, Registry};
use crate::wm::WindowManager;

/// Everything the live workspace knows, no disk I/O.
pub fn snapshot(_wm: &WindowManager) -> Registry {
    Registry::default()
}

/// Live candidates for `kind` (Agent: agents; Session: session and pane rows; History: none).
pub fn candidates(_wm: &WindowManager, _kind: FindKind) -> Vec<FindRow> {
    Vec::new()
}
