//! Unit tests for the navigator row geometry helpers (two-line rows).

use super::*;

#[test]
fn metrics_report_tallest_row_and_footer() {
    assert_eq!(navigator_layout_metrics(10, 1), (0, 1, 1));
    assert_eq!(navigator_layout_metrics(10, 2), (0, 1, 2));
    assert_eq!(navigator_layout_metrics(3, 2), (0, 0, 2), "no footer below 4 inner rows");
    assert_eq!(navigator_layout_metrics(10, 0), (0, 1, 2), "anything but 1 means 2");
    assert_eq!(navigator_layout_metrics(10, 9), (0, 1, 2));
}

fn triples(placed: &[PlacedRow]) -> Vec<(usize, u16, u16)> {
    placed.iter().map(|p| (p.row_index, p.y, p.height)).collect()
}

#[test]
fn place_rows_one_line_matches_old_layout() {
    let placed = place_rows(&[1, 1, 1, 1, 1], 1, 5, 3);
    assert_eq!(triples(&placed), vec![(1, 5, 1), (2, 6, 1), (3, 7, 1)]);
}

#[test]
fn place_rows_mixed_heights_stack_and_clip() {
    let placed = place_rows(&[1, 2, 2, 1], 0, 0, 5);
    assert_eq!(triples(&placed), vec![(0, 0, 1), (1, 1, 2), (2, 3, 2)], "rows fill the 5 lines exactly");
    let clipped = place_rows(&[2, 2, 2], 0, 10, 3);
    assert_eq!(triples(&clipped), vec![(0, 10, 2), (1, 12, 1)], "the last row is clipped to the lines left");
}

#[test]
fn place_rows_empty_and_past_end() {
    assert!(place_rows(&[], 0, 0, 4).is_empty());
    assert!(place_rows(&[1, 1], 5, 0, 4).is_empty());
}

#[test]
fn scroll_for_keeps_selected_row_fully_visible() {
    let heights = [2u16; 10];
    // A list of 5 lines holds two 2-line rows: selecting row 9 needs scroll 8.
    assert_eq!(scroll_for(0, 9, &heights, 5), 8);
    assert_eq!(scroll_for(0, 1, &heights, 5), 0, "already visible");
    assert_eq!(scroll_for(3, 1, &heights, 5), 3, "selected above the viewport is left to the caller");
    let ones = [1u16; 10];
    assert_eq!(scroll_for(5, 9, &ones, 5), 5, "single-line rows add nothing to the base scroll");
}

fn meta(level: NavigatorLevel, detail: Option<&str>) -> crate::wm::drawer::NavigatorRowMeta {
    crate::wm::drawer::NavigatorRowMeta {
        depth: 1,
        level,
        collapsed: false,
        has_children: false,
        live: true,
        pane_count: 0,
        agent: None,
        counts: None,
        comms: None,
        comms_active: false,
        detail: detail.map(str::to_string),
    }
}

fn row(kind: DrawerRowKind, m: crate::wm::drawer::NavigatorRowMeta) -> DrawerRow {
    DrawerRow {
        kind,
        connector: "",
        focus_rank_badge: String::new(),
        spinner: ' ',
        sparkline: String::new(),
        cmd_badge: String::new(),
        mode_badges: Vec::new(),
        transport_badges: Vec::new(),
        title: "t".to_string(),
        is_focused_pane: false,
        navigator: Some(m),
    }
}

#[test]
fn row_height_is_two_only_for_pane_rows_with_detail() {
    assert_eq!(row_height(&row(DrawerRowKind::Pane(1), meta(NavigatorLevel::Pane, Some("u@h  ~")))), 2);
    assert_eq!(row_height(&row(DrawerRowKind::Pane(1), meta(NavigatorLevel::Pane, None))), 1);
    assert_eq!(
        row_height(&row(DrawerRowKind::SessionHeader("s".into()), meta(NavigatorLevel::Session, Some("x")))),
        1
    );
    assert_eq!(row_height(&row(DrawerRowKind::QuickTerminal, meta(NavigatorLevel::Other, Some("x")))), 1);
}
