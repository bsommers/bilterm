//! Fuzzy ranking (filled by plan 50.1-04). Skeleton: keeps input order and truncates to `limit`.

use super::types::FindRow;

/// A ranked, truncated candidate list.
#[derive(Clone, Debug)]
pub struct Ranked {
    pub rows: Vec<FindRow>,
    /// Matching rows before truncation.
    pub total: usize,
    pub truncated: bool,
}

/// Ranks `rows` against `query` and returns at most `limit` of them.
pub fn search(mut rows: Vec<FindRow>, _query: &str, limit: usize) -> Ranked {
    let total = rows.len();
    rows.truncate(limit);
    Ranked { truncated: total > rows.len(), rows, total }
}
