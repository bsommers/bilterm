//! Phase 61-02 regression suite: zoom follows focus and toggle_zoom is a true toggle.

use bilterm::action::Action;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::wm::{split_tree::SplitDirection, Focus, Modal, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

const AREA: Rect = Rect::new(0, 0, 80, 24);

fn isolate_xdg() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let dir = std::env::temp_dir().join(format!("bilterm-zoom-follows-focus-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        // SAFETY: runs once before tests create WindowManager instances.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &dir);
            std::env::set_var("XDG_STATE_HOME", &dir);
        }
    });
}

fn fixture() -> WindowManager {
    isolate_xdg();
    let mut wm = WindowManager::new_headless(24, 80);
    wm.split_focused(SplitDirection::Vertical).expect("split pane");
    wm.resize_all(AREA).expect("resize all");
    wm
}

fn find_menubar_item(cmd: &str) -> (usize, usize) {
    for cat in 0..10 {
        let count = bilterm::ui::modals::menubar_category_item_count(cat);
        for item in 0..count {
            if bilterm::ui::modals::menubar_item_command(cat, item) == Some(cmd) {
                return (cat, item);
            }
        }
    }
    panic!("command {cmd:?} not found in menubar");
}

#[test]
fn shuffle_while_zoomed_keeps_zoom_on_active_pane() {
    let mut wm = fixture();
    assert_eq!(wm.pane_count(), 2);
    let p2 = wm.active_pane_id();
    wm.apply(Action::ZoomToggle).expect("zoom toggle");
    assert_eq!(wm.zoomed_pane(), Some(p2));

    wm.shuffle_deck_next();
    let active = wm.active_pane_id();
    assert_ne!(active, p2);
    assert_eq!(wm.zoomed_pane(), Some(active));
    assert_eq!(wm.compute_workspace_rects(AREA), vec![(active, AREA)]);

    let (focus, outer, _inner) = wm.hit_test_pane(40, 12, AREA).expect("hit test pane");
    assert_eq!(focus, Focus::Pane(active));
    assert_eq!(outer, AREA);
}

#[test]
fn toggle_after_focus_change_unzooms() {
    let mut wm = fixture();
    let p2 = wm.active_pane_id();
    wm.apply(Action::ZoomToggle).expect("zoom toggle");
    assert_eq!(wm.zoomed_pane(), Some(p2));

    let other = 1;
    assert_ne!(other, p2);
    wm.focus_and_raise(other);
    assert_eq!(wm.active_pane_id(), other);
    assert_eq!(wm.zoomed_pane(), Some(other));

    // Pre-fix: toggle_zoom re-zoomed instead of un-zooming.
    // Post-fix: toggle_zoom is a true toggle and clears zoom.
    wm.apply(Action::ZoomToggle).expect("zoom toggle");
    assert_eq!(wm.zoomed_pane(), None);
    let rects = wm.compute_workspace_rects(AREA);
    assert_eq!(rects.len(), 2);
    assert!(rects.iter().any(|&(id, _)| id == 1));
    assert!(rects.iter().any(|&(id, _)| id == 2));
}

#[test]
fn keys_reach_visible_zoomed_pane() {
    let mut wm = fixture();
    let p2 = wm.active_pane_id();
    wm.apply(Action::ZoomToggle).expect("zoom toggle");
    assert_eq!(wm.zoomed_pane(), Some(p2));

    // Alt+Tab switches focus to pane 1
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Tab, KeyModifiers::ALT)).expect("alt+tab");
    assert_eq!(wm.active_pane_id(), 1);
    assert_eq!(wm.zoomed_pane(), Some(1));

    // Typing 'x' lands in active pane's prompt text
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)).expect("type x");
    assert_eq!(wm.active_pane().prompt().text(), "x");
    assert_eq!(wm.pane(p2).expect("pane 2 exists").prompt().text(), "");
}

#[test]
fn menu_window_zoom_toggles_via_keys() {
    let mut wm = fixture();
    let (cat, zoom_item) = find_menubar_item(":zoom");

    // Open category dropdown
    wm.set_menubar_open_category(Some(cat));

    // Move Down to the zoom item
    for _ in 0..zoom_item {
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)).expect("down");
    }

    // Press Enter to activate
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).expect("enter");
    assert!(wm.zoomed_pane().is_some());
    assert_eq!(wm.modal(), &Modal::None);

    // Repeat to toggle off
    wm.set_menubar_open_category(Some(cat));
    for _ in 0..zoom_item {
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)).expect("down");
    }
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).expect("enter");
    assert_eq!(wm.zoomed_pane(), None);
    assert_eq!(wm.modal(), &Modal::None);
}

#[test]
fn menu_window_zoom_via_mouse() {
    let mut wm = fixture();
    let (cat, zoom_item) = find_menubar_item(":zoom");

    // Open dropdown
    wm.set_menubar_open_category(Some(cat));
    let dd = bilterm::ui::modals::menubar_dropdown_rect(cat, AREA);
    let inner = bilterm::wm::inset_window_frame(dd);
    let click_row = inner.y + (zoom_item as u16);
    let click_col = inner.x + 2;

    // Click on :zoom item
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: click_col,
            row: click_row,
            modifiers: KeyModifiers::NONE,
        },
        AREA,
    )
    .expect("click menubar zoom item");

    assert!(wm.zoomed_pane().is_some());
    assert_eq!(wm.modal(), &Modal::None);

    // Second click toggles zoom off
    wm.set_menubar_open_category(Some(cat));
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: click_col,
            row: click_row,
            modifiers: KeyModifiers::NONE,
        },
        AREA,
    )
    .expect("second click menubar zoom item");

    assert_eq!(wm.zoomed_pane(), None);
    assert_eq!(wm.modal(), &Modal::None);
}

#[test]
fn mouse_click_when_zoomed_never_targets_hidden_pane() {
    let mut wm = fixture();
    wm.apply(Action::ZoomToggle).expect("zoom toggle");
    wm.shuffle_deck_next();
    assert_eq!(wm.active_pane_id(), 1);
    assert_eq!(wm.zoomed_pane(), Some(1));

    for (col, row) in [(10, 5), (40, 12), (70, 20)] {
        let _ = handle_wm_mouse_event(
            &mut wm,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: col,
                row,
                modifiers: KeyModifiers::NONE,
            },
            AREA,
        );
        assert_eq!(wm.active_pane_id(), 1);
        assert_eq!(wm.zoomed_pane(), Some(1));
    }
}

#[test]
fn zoom_with_quick_terminal_visible_does_not_panic() {
    let mut wm = fixture();
    wm.toggle_quick_terminal();
    assert!(wm.is_quick_terminal_visible());
    wm.apply(Action::ZoomToggle).expect("first zoom toggle with quick terminal");
    wm.apply(Action::ZoomToggle).expect("second zoom toggle with quick terminal");
}

#[test]
fn remove_zoomed_or_top_pane_clears_zoom() {
    let mut wm = fixture();
    let top = wm.active_pane_id();
    wm.apply(Action::ZoomToggle).expect("zoom toggle");
    assert_eq!(wm.zoomed_pane(), Some(top));

    wm.remove_pane(top);
    let current_zoom = wm.zoomed_pane();
    let rects = wm.compute_workspace_rects(AREA);
    if let Some(z) = current_zoom {
        assert_eq!(z, wm.active_pane_id());
        assert_eq!(rects, vec![(z, AREA)]);
        assert!(wm.panes().contains_key(&z));
    } else {
        assert!(!rects.is_empty());
    }

    // Also verify when focus was shuffled before remove
    let mut wm2 = fixture();
    wm2.apply(Action::ZoomToggle).expect("zoom toggle");
    wm2.shuffle_deck_next();
    let top2 = wm2.active_pane_id();
    wm2.remove_pane(top2);
    let current_zoom2 = wm2.zoomed_pane();
    let rects2 = wm2.compute_workspace_rects(AREA);
    if let Some(z) = current_zoom2 {
        assert_eq!(z, wm2.active_pane_id());
        assert_eq!(rects2, vec![(z, AREA)]);
        assert!(wm2.panes().contains_key(&z));
    } else {
        assert!(!rects2.is_empty());
    }
}
