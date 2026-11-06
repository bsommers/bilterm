//! Phase 50.1-04: shared fuzzy ranker (`registry::rank::search`): group order, scoring,
//! highlights, bounds, and ranking parity with the command palette.

use bilterm::ui::modals::PALETTE_CATALOG;
use bilterm::wm::WindowManager;
use bilterm::wm::registry::rank::search;
use bilterm::wm::registry::{FIND_QUERY_CAP, FIND_RESULTS_CAP, FindKind, FindRow, Origin, ResultSource};

fn row(id: &str, label: &str, origin: Origin, order: u64) -> FindRow {
    let mut r = FindRow::empty(FindKind::History);
    r.id = id.to_string();
    r.label = label.to_string();
    r.haystack = label.to_string();
    r.order = order;
    r.source = ResultSource { origin, name: "s".to_string(), path: None };
    r
}

fn ids(rows: &[FindRow]) -> Vec<&str> {
    rows.iter().map(|r| r.id.as_str()).collect()
}

#[test]
fn live_rows_outrank_saved_and_log_even_with_lower_score() {
    let rows = vec![
        row("log", "cargo build", Origin::HistoryLog, 1),
        row("saved", "cargo build --release", Origin::SavedSession, 1),
        row("live", "c a r g o", Origin::Live, 1),
    ];
    let out = search(rows, "cargo", 50);
    assert_eq!(ids(&out.rows), vec!["live", "saved", "log"]);
}

#[test]
fn score_orders_within_a_group() {
    let rows = vec![row("weak", "x c x a x r x g x o", Origin::Live, 9), row("strong", "cargo", Origin::Live, 1)];
    let out = search(rows, "cargo", 50);
    assert_eq!(ids(&out.rows), vec!["strong", "weak"]);
    assert!(out.rows[0].score > out.rows[1].score);
}

#[test]
fn order_breaks_score_ties() {
    let rows = vec![row("old", "cargo", Origin::Live, 1), row("new", "cargo", Origin::Live, 5)];
    let out = search(rows, "cargo", 50);
    assert_eq!(ids(&out.rows), vec!["new", "old"]);
}

#[test]
fn id_breaks_order_ties() {
    let rows = vec![row("b", "cargo", Origin::Live, 3), row("a", "cargo", Origin::Live, 3)];
    let out = search(rows, "cargo", 50);
    assert_eq!(ids(&out.rows), vec!["a", "b"]);
}

#[test]
fn empty_query_keeps_group_and_order_and_never_drops_rows() {
    let rows = vec![
        row("log", "zzz", Origin::HistoryLog, 9),
        row("live-old", "aaa", Origin::Live, 1),
        row("live-new", "bbb", Origin::Live, 2),
    ];
    for q in ["", "   "] {
        let out = search(rows.clone(), q, 50);
        assert_eq!(ids(&out.rows), vec!["live-new", "live-old", "log"]);
        assert_eq!(out.total, 3);
        assert!(out.rows.iter().all(|r| r.score == 0 && r.matched.is_empty()));
    }
}

#[test]
fn matched_indices_are_char_indices_into_label_only() {
    let mut r = row("a", "label", Origin::Live, 1);
    r.haystack = "label zzqq".to_string();
    let out = search(vec![r], "zq", 50);
    assert_eq!(out.rows.len(), 1);
    assert!(out.rows[0].matched.is_empty(), "match only in the non-label part: {:?}", out.rows[0].matched);

    let mut u = row("u", "é😀x", Origin::Live, 1);
    u.haystack = "é😀x tail".to_string();
    let out = search(vec![u], "x", 50);
    assert_eq!(out.rows[0].matched, vec![2], "char index, not byte index");
}

#[test]
fn limit_truncates_and_reports_total() {
    let rows: Vec<FindRow> = (0..500).map(|i| row(&format!("r{i:04}"), "cargo", Origin::Live, i)).collect();
    let out = search(rows.clone(), "cargo", 50);
    assert_eq!((out.rows.len(), out.total, out.truncated), (50, 500, true));
    assert_eq!(search(rows.clone(), "cargo", 0).rows.len(), 1);
    assert_eq!(search(rows, "cargo", 10_000).rows.len(), FIND_RESULTS_CAP);
    assert_eq!(FIND_RESULTS_CAP, 200);
}

#[test]
fn query_is_capped_to_128_chars() {
    let long: String = "a".repeat(FIND_QUERY_CAP) + &"b".repeat(500);
    let capped = row("x", &"a".repeat(FIND_QUERY_CAP), Origin::Live, 1);
    let out = search(vec![capped], &long, 50);
    assert_eq!(out.rows.len(), 1, "capped query matches; uncapped would not");
}

#[test]
fn non_matching_rows_are_excluded_when_query_non_empty() {
    let rows = vec![row("hit", "cargo test", Origin::Live, 1), row("miss", "ls -la", Origin::Live, 2)];
    let out = search(rows, "cargo", 50);
    assert_eq!(ids(&out.rows), vec!["hit"]);
    assert_eq!(out.total, 1);
    assert!(!out.truncated);
}

#[test]
fn ranking_parity_with_palette() {
    let queries = ["split", "theme", "agents", "find", "sess", "zoom", "ssh", "xyz-no-match"];
    let n = PALETTE_CATALOG.len() as u64;
    let base: Vec<FindRow> = PALETTE_CATALOG
        .iter()
        .enumerate()
        .map(|(i, (cmd, key, desc))| {
            let mut r = row(&format!("{i:05}"), cmd, Origin::Live, n - i as u64);
            r.haystack = format!("{cmd} {key} {desc}");
            r
        })
        .collect();
    for q in queries {
        let mut wm = WindowManager::new_headless(24, 80);
        wm.open_palette_with_query(q);
        let expect: Vec<&str> = wm.filtered_palette_items().iter().map(|i| i.0).collect();
        let out = search(base.clone(), q, 10_000);
        let got: Vec<&str> = out.rows.iter().map(|r| r.label.as_str()).collect();
        // Result cap is 200; the palette itself is uncapped.
        let k = got.len().min(expect.len());
        assert_eq!(got.len(), expect.len().min(FIND_RESULTS_CAP), "len for {q:?}");
        assert_eq!(got[..k], expect[..k], "order for {q:?}");
    }
}

#[test]
fn pathological_inputs_do_not_panic() {
    let mut big = row("big", "x", Origin::Live, 1);
    big.haystack = "ab".repeat(50_000);
    let q = "a".repeat(128);
    let _ = search(vec![big], &q, 50);

    let _ = search(vec![row("m", "cargo", Origin::Live, 1)], "\u{301}\u{301}\u{301}", 50);
    let _ = search(vec![row("m", "ca\u{0}rgo\u{1b}[0m", Origin::Live, 1)], "\u{0}\u{1b}", 50);

    let rows: Vec<FindRow> = (0..10_000).map(|i| row(&format!("r{i}"), "cargo run", Origin::Live, i)).collect();
    let out = search(rows, "cargo", 10_000);
    assert!(out.rows.len() <= 200);
    assert_eq!(out.total, 10_000);
}
