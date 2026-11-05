//! Unified registry of agents, sessions, panes and command history (Phase 50.1).
//!
//! Plan 50.1-01 froze the types (`types`) and provider signatures; the providers are filled by
//! plans 03 (`live`), 04 (`rank`), 06 (`remote`), 10 (`capture`) and 11 (`saved`). This module
//! owns the glue: [`WindowManager::run_find`] gathers candidates per kind, dedupes sessions by
//! name (highest origin wins, D-U3), ranks, and reports the masked count (D-U2).
//!
//! `registry` is a child of `wm`, so its files read `WindowManager` private fields directly
//! without making any field public (ARCH-R02).

pub mod capture;
pub mod live;
pub mod rank;
pub mod remote;
pub mod saved;
pub mod types;

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

pub use rank::Ranked;
pub use types::*;

use crate::wm::WindowManager;

/// Registry state owned by the window manager: the command ring, the per-open candidate cache
/// and the last ranked results.
#[derive(Debug)]
pub struct RegistryState {
    /// Newest-last ring of captured commands, bounded by `CMD_RING_CAP` (50.1-10).
    #[allow(dead_code)] // filled by registry::capture (50.1-10)
    cmds: VecDeque<CapturedEntry>,
    /// Next sequence number for a captured command (50.1-10).
    #[allow(dead_code)] // filled by registry::capture (50.1-10)
    next_seq: u64,
    /// Secret-looking commands withheld since startup (count only, D-U2).
    masked_live: u64,
    disk: DiskCache,
    results: FindResults,
    /// Per-open candidate cache, one slot per kind (`FindKind::index`).
    cached: [Option<Vec<FindRow>>; 3],
    /// Masked count stored with each cached slot.
    cached_masked: [usize; 3],
}

impl Default for RegistryState {
    fn default() -> Self {
        Self::new()
    }
}

impl RegistryState {
    /// An empty registry.
    pub fn new() -> Self {
        Self {
            cmds: VecDeque::new(),
            next_seq: 0,
            masked_live: 0,
            disk: DiskCache::default(),
            results: FindResults::empty(FindKind::Agent, ""),
            cached: [None, None, None],
            cached_masked: [0; 3],
        }
    }

    /// Commands currently in the ring.
    pub fn history_len(&self) -> usize {
        self.cmds.len()
    }

    /// Secret-looking commands withheld since startup.
    pub fn masked_live(&self) -> u64 {
        self.masked_live
    }
}

/// Collapses Session rows that share a session name: the lowest-priority origin number wins and
/// the others move into its `also_in`. Pane rows and other kinds pass through in order.
fn dedupe_sessions(rows: Vec<FindRow>) -> Vec<FindRow> {
    let mut out: Vec<FindRow> = Vec::with_capacity(rows.len());
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for mut row in rows {
        let RowData::Session(s) = &row.data else {
            out.push(row);
            continue;
        };
        let Some(&at) = by_name.get(&s.name) else {
            by_name.insert(s.name.clone(), out.len());
            out.push(row);
            continue;
        };
        let keep = &mut out[at];
        if row.source.origin.priority() < keep.source.origin.priority() {
            std::mem::swap(keep, &mut row);
        }
        // `row` is now the loser; fold it and whatever it already merged into the winner.
        let mut losers = std::mem::take(&mut row.also_in);
        losers.push(row.source);
        for src in losers {
            if src != keep.source && !keep.also_in.contains(&src) {
                keep.also_in.push(src);
            }
        }
    }
    out
}

impl WindowManager {
    /// The registry state (command ring, cached candidates, last results).
    pub fn registry(&self) -> &RegistryState {
        &self.registry
    }

    /// Everything the live workspace knows, no disk I/O.
    pub fn registry_snapshot(&self) -> Registry {
        live::snapshot(self)
    }

    /// The last stored result list (what the Find window shows).
    pub fn find_results(&self) -> &FindResults {
        &self.registry.results
    }

    /// Stores `results` as what the Find window shows.
    pub fn set_find_results(&mut self, results: FindResults) {
        self.registry.results = results;
    }

    /// Drops cached candidates and the disk cache (a new open, or the window closing).
    pub fn invalidate_find_cache(&mut self) {
        self.registry.cached = [None, None, None];
        self.registry.cached_masked = [0; 3];
        self.registry.disk.invalidate();
    }

    /// Fresh search: clears the per-open cache, gathers candidates of `kind` and ranks them.
    pub fn run_find(&mut self, kind: FindKind, query: &str, limit: usize) -> FindResults {
        self.invalidate_find_cache();
        self.gather_find(kind);
        self.rank_cached(kind, query, limit)
    }

    /// Ranks the cached candidates of `kind`, gathering once (lazily) when it has none. Used for
    /// every keystroke and kind switch.
    pub fn rerank_find(&mut self, kind: FindKind, query: &str, limit: usize) -> FindResults {
        if self.registry.cached[kind.index()].is_none() {
            self.gather_find(kind);
        }
        self.rank_cached(kind, query, limit)
    }

    fn gather_find(&mut self, kind: FindKind) {
        if !self.agents.has_updated() {
            self.update_agent_states(Instant::now());
        }
        let mut cache = std::mem::take(&mut self.registry.disk);
        let mut rows = live::candidates(self, kind);
        if kind == FindKind::History {
            rows.extend(capture::candidates(self));
        }
        if matches!(kind, FindKind::Session | FindKind::History) {
            rows.extend(saved::candidates(self, kind, &mut cache));
        }
        if kind == FindKind::Session {
            rows = dedupe_sessions(rows);
        }
        let masked = usize::try_from(self.registry.masked_live)
            .unwrap_or(usize::MAX)
            .saturating_add(cache.masked());
        self.registry.disk = cache;
        self.registry.cached[kind.index()] = Some(rows);
        self.registry.cached_masked[kind.index()] = masked;
    }

    fn rank_cached(&mut self, kind: FindKind, query: &str, limit: usize) -> FindResults {
        let query: String = query.chars().take(FIND_QUERY_CAP).collect();
        let rows = self.registry.cached[kind.index()].clone().unwrap_or_default();
        let ranked = rank::search(rows, &query, limit.clamp(1, FIND_RESULTS_CAP));
        FindResults {
            kind,
            query,
            rows: ranked.rows,
            total: ranked.total,
            truncated: ranked.truncated,
            masked: self.registry.cached_masked[kind.index()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_row(name: &str, origin: Origin) -> FindRow {
        let mut r = FindRow::empty(FindKind::Session);
        r.data = RowData::Session(SessionRow { name: name.to_string(), active: false, parent: None, pane_count: 1 });
        r.source = ResultSource { origin, name: name.to_string(), path: None };
        r.session = name.to_string();
        r
    }

    #[test]
    fn dedupe_keeps_highest_origin_and_folds_the_rest_into_also_in() {
        let rows = vec![
            session_row("a", Origin::HistoryLog),
            session_row("b", Origin::Live),
            session_row("a", Origin::SavedSession),
            session_row("a", Origin::Live),
        ];
        let out = dedupe_sessions(rows);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].source.origin, Origin::Live);
        assert_eq!(out[0].session, "a");
        let mut also: Vec<Origin> = out[0].also_in.iter().map(|s| s.origin).collect();
        also.sort_by_key(|o| o.priority());
        assert_eq!(also, vec![Origin::SavedSession, Origin::HistoryLog]);
        assert!(out[1].also_in.is_empty());
    }

    #[test]
    fn pane_rows_pass_through_dedupe_untouched() {
        let mut pane = FindRow::empty(FindKind::Session);
        pane.data = RowData::Pane(PaneRow { pane: 1, title: "t".into(), cwd: None, cwd_trusted: true, running_command: None });
        let out = dedupe_sessions(vec![pane.clone(), pane]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn run_find_with_empty_providers_is_empty_and_clamps_query() {
        let mut wm = WindowManager::new_headless(24, 80);
        let r = wm.run_find(FindKind::History, &"q".repeat(1000), 0);
        assert_eq!((r.rows.len(), r.total, r.truncated, r.masked), (0, 0, false, 0));
        assert_eq!(r.query.chars().count(), FIND_QUERY_CAP);
        wm.set_find_results(r);
        assert_eq!(wm.find_results().kind, FindKind::History);
        assert_eq!(wm.registry().history_len(), 0);
        assert_eq!(wm.registry_snapshot().masked, 0);
        let again = wm.rerank_find(FindKind::Session, "x", 10);
        assert_eq!(again.kind, FindKind::Session);
        wm.invalidate_find_cache();
    }
}
