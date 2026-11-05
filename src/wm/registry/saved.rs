//! Saved sessions and `history.log` reads (filled by plan 50.1-11). Skeleton: returns nothing.

use super::types::{DiskCache, FindKind, FindRow};
use crate::wm::WindowManager;

/// Disk candidates for Session or History; adds secret-looking lines to the cache's masked count.
pub fn candidates(_wm: &WindowManager, _kind: FindKind, _cache: &mut DiskCache) -> Vec<FindRow> {
    Vec::new()
}
