//! Bounded command ring, tick drain and exit-code join (filled by plan 50.1-10).
//! Skeleton: nothing is captured.

use std::time::Instant;

use super::types::FindRow;
use crate::wm::WindowManager;

/// History rows for the live ring.
pub fn candidates(_wm: &WindowManager) -> Vec<FindRow> {
    Vec::new()
}

/// Drains pane outboxes into the ring and joins exit codes (called from `WindowManager::tick`).
pub fn tick(_wm: &mut WindowManager, _now: Instant) {}
