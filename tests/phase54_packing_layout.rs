//! Tests for pure deterministic 2D treemap session packing layout (Phase 54, VIEW-01).

use ratatui::layout::Rect;
use bilterm::wm::{
    compute_inner_rect, pack_sessions_treemap, SessionPackInput, WindowPackInput,
};

fn rect_intersects(a: Rect, b: Rect) -> bool {
    if a.width == 0 || a.height == 0 || b.width == 0 || b.height == 0 {
        return false;
    }
    let left = a.x.max(b.x);
    let right = a.right().min(b.right());
    let top = a.y.max(b.y);
    let bottom = a.bottom().min(b.bottom());
    right > left && bottom > top
}

fn rect_contains_rect(parent: Rect, child: Rect) -> bool {
    if child.width == 0 || child.height == 0 {
        return true;
    }
    child.x >= parent.x
        && child.y >= parent.y
        && child.right() <= parent.right()
        && child.bottom() <= parent.bottom()
}

#[test]
fn test_packing_empty_sessions() {
    let area = Rect::new(0, 0, 80, 24);
    let res = pack_sessions_treemap(area, &[]);
    assert!(res.is_empty());
}

#[test]
fn test_packing_single_session_single_window() {
    let area = Rect::new(0, 0, 80, 24);
    let sessions = vec![SessionPackInput {
        session_name: "default".into(),
        parent_session: None,
        windows: vec![WindowPackInput {
            pane_id: 1,
            title: "shell".into(),
            weight: 10,
            focused: true,
        }],
        active: true,
    }];

    let res = pack_sessions_treemap(area, &sessions);
    assert_eq!(res.len(), 1);
    let s = &res[0];
    assert_eq!(s.outer, area);
    assert_eq!(s.inner, compute_inner_rect(area));
    assert_eq!(s.windows.len(), 1);
    assert_eq!(s.windows[0].rect, s.inner);
    assert!(rect_contains_rect(s.inner, s.windows[0].rect));
}

#[test]
fn test_packing_determinism() {
    let area = Rect::new(0, 0, 120, 40);
    let sessions = vec![
        SessionPackInput {
            session_name: "alpha".into(),
            parent_session: None,
            windows: vec![
                WindowPackInput { pane_id: 1, title: "editor".into(), weight: 20, focused: false },
                WindowPackInput { pane_id: 2, title: "compiler".into(), weight: 10, focused: true },
            ],
            active: true,
        },
        SessionPackInput {
            session_name: "beta".into(),
            parent_session: None,
            windows: vec![
                WindowPackInput { pane_id: 3, title: "server".into(), weight: 15, focused: false },
            ],
            active: false,
        },
        SessionPackInput {
            session_name: "gamma".into(),
            parent_session: Some("alpha".into()),
            windows: vec![
                WindowPackInput { pane_id: 4, title: "sub-worker".into(), weight: 5, focused: false },
            ],
            active: false,
        },
    ];

    let run1 = pack_sessions_treemap(area, &sessions);
    let run2 = pack_sessions_treemap(area, &sessions);

    assert_eq!(run1, run2);
}

#[test]
fn test_packing_containment_and_non_overlap() {
    let area = Rect::new(5, 5, 100, 30);
    let sessions = vec![
        SessionPackInput {
            session_name: "s1".into(),
            parent_session: None,
            windows: vec![
                WindowPackInput { pane_id: 1, title: "w1".into(), weight: 10, focused: true },
                WindowPackInput { pane_id: 2, title: "w2".into(), weight: 10, focused: false },
                WindowPackInput { pane_id: 3, title: "w3".into(), weight: 20, focused: false },
            ],
            active: true,
        },
        SessionPackInput {
            session_name: "s2".into(),
            parent_session: None,
            windows: vec![
                WindowPackInput { pane_id: 4, title: "w4".into(), weight: 15, focused: false },
                WindowPackInput { pane_id: 5, title: "w5".into(), weight: 15, focused: false },
            ],
            active: false,
        },
        SessionPackInput {
            session_name: "s3".into(),
            parent_session: None,
            windows: vec![
                WindowPackInput { pane_id: 6, title: "w6".into(), weight: 5, focused: false },
            ],
            active: false,
        },
    ];

    let layout = pack_sessions_treemap(area, &sessions);
    assert_eq!(layout.len(), 3);

    // Root sessions containment inside area and pairwise non-overlap
    for i in 0..layout.len() {
        assert!(
            rect_contains_rect(area, layout[i].outer),
            "Session outer must be within area"
        );
        assert!(
            rect_contains_rect(layout[i].outer, layout[i].inner),
            "Session inner must be within outer"
        );

        for j in (i + 1)..layout.len() {
            assert!(
                !rect_intersects(layout[i].outer, layout[j].outer),
                "Root sessions must not overlap"
            );
        }

        // Window containment and pairwise non-overlap within session
        for (w_i, w1) in layout[i].windows.iter().enumerate() {
            assert!(
                rect_contains_rect(layout[i].inner, w1.rect),
                "Window rect must be within session inner"
            );
            for (w_j, w2) in layout[i].windows.iter().enumerate() {
                if w_i != w_j {
                    assert!(
                        !rect_intersects(w1.rect, w2.rect),
                        "Sibling windows within session must not overlap"
                    );
                }
            }
        }
    }
}

#[test]
fn test_packing_nested_sessions() {
    let area = Rect::new(0, 0, 100, 30);
    let sessions = vec![
        SessionPackInput {
            session_name: "parent".into(),
            parent_session: None,
            windows: vec![
                WindowPackInput { pane_id: 10, title: "p_win".into(), weight: 10, focused: true },
            ],
            active: true,
        },
        SessionPackInput {
            session_name: "child".into(),
            parent_session: Some("parent".into()),
            windows: vec![
                WindowPackInput { pane_id: 20, title: "c_win".into(), weight: 10, focused: false },
            ],
            active: false,
        },
    ];

    let layout = pack_sessions_treemap(area, &sessions);
    assert_eq!(layout.len(), 2);

    let parent = &layout[0];
    let child = &layout[1];

    assert_eq!(parent.session_name, "parent");
    assert_eq!(child.session_name, "child");

    // Parent is root
    assert!(rect_contains_rect(area, parent.outer));
    assert!(rect_contains_rect(parent.outer, parent.inner));

    // Child is inside parent inner
    assert!(
        rect_contains_rect(parent.inner, child.outer),
        "Child outer must be inside parent inner"
    );
    assert!(rect_contains_rect(child.outer, child.inner));

    // Windows inside respective inner rects
    assert!(rect_contains_rect(parent.inner, parent.windows[0].rect));
    assert!(rect_contains_rect(child.inner, child.windows[0].rect));

    // Parent direct window and child container do not overlap
    assert!(!rect_intersects(parent.windows[0].rect, child.outer));
}

#[test]
fn test_packing_tiny_and_degenerate_bounds() {
    let test_areas = [
        Rect::new(0, 0, 0, 0),
        Rect::new(0, 0, 1, 1),
        Rect::new(0, 0, 2, 2),
        Rect::new(0, 0, 4, 2),
        Rect::new(0, 0, 7, 3),
        Rect::new(0, 0, 100, 1),
        Rect::new(0, 0, 1, 100),
    ];

    let sessions = vec![
        SessionPackInput {
            session_name: "s1".into(),
            parent_session: None,
            windows: vec![
                WindowPackInput { pane_id: 1, title: "w1".into(), weight: 10, focused: true },
                WindowPackInput { pane_id: 2, title: "w2".into(), weight: 10, focused: false },
            ],
            active: true,
        },
        SessionPackInput {
            session_name: "s2".into(),
            parent_session: None,
            windows: vec![
                WindowPackInput { pane_id: 3, title: "w3".into(), weight: 10, focused: false },
            ],
            active: false,
        },
    ];

    for area in test_areas {
        let layout = pack_sessions_treemap(area, &sessions);
        assert_eq!(layout.len(), 2);
        for s in &layout {
            assert!(rect_contains_rect(area, s.outer));
            assert!(rect_contains_rect(s.outer, s.inner));
            for w in &s.windows {
                assert!(rect_contains_rect(s.inner, w.rect));
            }
        }
    }
}
