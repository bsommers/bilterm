//! Fuzzy ranking for `:find` (Phase 50.1-04, FND-R02).
//!
//! Same matcher as the command palette (`WindowManager::filtered_palette_items`):
//! nucleo `Pattern::parse` (ignore-case, smart normalisation) over the row haystack,
//! guarded by `catch_unwind`, with the palette's substring (100) / subsequence (10) fallback.
//! Rows sort by origin group first (Live, SavedSession, HistoryLog; D-U3 / P-05), then score
//! descending, then `order` descending, then `id` ascending.

use std::cmp::Reverse;
use std::panic::{AssertUnwindSafe, catch_unwind};

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use super::types::{FIND_QUERY_CAP, FIND_RESULTS_CAP, FindRow};

/// A ranked, truncated candidate list.
#[derive(Clone, Debug)]
pub struct Ranked {
    pub rows: Vec<FindRow>,
    /// Matching rows before truncation.
    pub total: usize,
    pub truncated: bool,
}

/// Ranks `rows` against `query` and returns at most `limit.clamp(1, FIND_RESULTS_CAP)` of them.
///
/// An empty or whitespace query scores nothing and keeps every row in group/order sequence.
/// The query is cut to `FIND_QUERY_CAP` chars. `matched` holds char indices into `label` only.
pub fn search(rows: Vec<FindRow>, query: &str, limit: usize) -> Ranked {
    let limit = limit.clamp(1, FIND_RESULTS_CAP);
    let q: String = query.trim().chars().take(FIND_QUERY_CAP).collect();
    let q_lower = q.trim().to_lowercase();
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(&q_lower, CaseMatching::Ignore, Normalization::Smart);

    let mut scored: Vec<FindRow> = Vec::with_capacity(rows.len());
    for mut row in rows {
        row.matched.clear();
        if q_lower.is_empty() {
            row.score = 0;
        } else {
            match score_row(&pattern, &mut matcher, &row.haystack, &q_lower) {
                Some(score) => row.score = score,
                None => continue,
            }
        }
        scored.push(row);
    }
    scored.sort_by(|a, b| {
        (a.source.origin.priority(), Reverse(a.score), Reverse(a.order), &a.id).cmp(&(
            b.source.origin.priority(),
            Reverse(b.score),
            Reverse(b.order),
            &b.id,
        ))
    });
    let total = scored.len();
    scored.truncate(limit);
    if !q_lower.is_empty() {
        for row in &mut scored {
            row.matched = label_indices(&pattern, &mut matcher, row);
        }
    }
    Ranked { truncated: total > scored.len(), rows: scored, total }
}

fn score_row(pattern: &Pattern, matcher: &mut Matcher, haystack: &str, q_lower: &str) -> Option<u32> {
    catch_unwind(AssertUnwindSafe(|| {
        let mut buf = Vec::new();
        pattern.score(Utf32Str::new(haystack, &mut buf), matcher)
    }))
    .ok()
    .flatten()
    .or_else(|| fallback_score(haystack, q_lower))
}

/// Palette fallback: substring scores 100, subsequence scores 10.
fn fallback_score(haystack: &str, needle_lower: &str) -> Option<u32> {
    let hay_lower = haystack.to_lowercase();
    if hay_lower.contains(needle_lower) {
        return Some(100);
    }
    let mut hay_chars = hay_lower.chars();
    needle_lower.chars().all(|nc| hay_chars.any(|hc| hc == nc)).then_some(10)
}

/// Sorted, deduped char indices of the match that fall inside `label`.
fn label_indices(pattern: &Pattern, matcher: &mut Matcher, row: &FindRow) -> Vec<u32> {
    let label_chars = row.label.chars().count() as u32;
    let mut idx: Vec<u32> = catch_unwind(AssertUnwindSafe(|| {
        let mut buf = Vec::new();
        let mut out = Vec::new();
        pattern.indices(Utf32Str::new(&row.haystack, &mut buf), matcher, &mut out);
        out
    }))
    .unwrap_or_default();
    idx.retain(|&i| i < label_chars);
    idx.sort_unstable();
    idx.dedup();
    idx
}
