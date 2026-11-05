//! Transport facts about a pane: local, SSH, tmux, SSH+tmux (filled by plan 50.1-06).
//! Skeleton: every pane is local with a trusted cwd.

use super::types::TransportInfo;
use crate::pane::PaneId;
use crate::wm::WindowManager;

/// How `id` reaches its shell.
pub fn transport_of(_wm: &WindowManager, _id: PaneId) -> TransportInfo {
    TransportInfo::local()
}
