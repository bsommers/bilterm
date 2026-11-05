//! Launch affordances of the AGENT TRAFFIC window: `:agents toggle`, the `Ctrl+Alt+A` global chord and
//! the top-menu entry (View > Agent Traffic). The Navigator footer button is covered in
//! `phase50_navigator_drawer.rs`. Every entry point lowers to `Action::Agents(AgentsOp::Toggle)`.

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::input::keymap::default_global_action;
use bilterm::input::handle_wm_key_event;
use bilterm::ui::modals::{menubar_category_item_count, menubar_item_command};
use bilterm::wm::agents_cmd::AgentsOp;
use bilterm::wm::{SplitDirection, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

fn fixture(cols: u16, rows: u16) -> WindowManager {
    let mut wm = WindowManager::new_headless(rows, cols);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(Rect::new(0, 0, cols, rows)).unwrap();
    wm
}

fn run_colon(wm: &mut WindowManager, cmd: &str) {
    let parsed = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"));
    wm.apply(parsed).unwrap();
}

fn press_mod(wm: &mut WindowManager, code: KeyCode, mods: KeyModifiers) {
    handle_wm_key_event(wm, KeyEvent::new(code, mods)).unwrap();
}

#[test]
fn toggle_colon_opens_then_closes_and_selects_pane() {
    let mut wm = fixture(120, 40);
    assert!(!wm.agents_window().active);
    run_colon(&mut wm, ":agents toggle");
    assert!(wm.agents_window().active, "first toggle opens");
    run_colon(&mut wm, ":agents toggle");
    assert!(!wm.agents_window().active, "second toggle closes");
    run_colon(&mut wm, ":agents toggle 2");
    assert!(wm.agents_window().active);
    assert_eq!(wm.agents_window().selection.node.as_ref().map(|k| k.0.as_str()), Some("pane:2"));
    run_colon(&mut wm, ":agents toggle 2");
    assert!(!wm.agents_window().active);
    let err = parse_colon_command(":agents toggle x").unwrap().unwrap_err();
    assert_eq!(err, "Usage: :agents toggle [pane-id]");
}

#[test]
fn toggle_closes_a_pinned_window_too() {
    let mut wm = fixture(120, 40);
    run_colon(&mut wm, ":agents zoom");
    wm.agents_window_mut().pinned = true;
    run_colon(&mut wm, ":agents toggle");
    assert!(!wm.agents_window().active && !wm.agents_window().pinned);
}

#[test]
fn toggle_global_chord_ctrl_alt_a_opens_and_closes() {
    let mut wm = fixture(120, 40);
    let chord = KeyModifiers::CONTROL | KeyModifiers::ALT;
    press_mod(&mut wm, KeyCode::Char('a'), chord);
    assert!(wm.agents_window().active, "Ctrl+Alt+A opens");
    press_mod(&mut wm, KeyCode::Char('a'), chord);
    assert!(!wm.agents_window().active, "Ctrl+Alt+A closes");
    press_mod(&mut wm, KeyCode::Char('A'), chord | KeyModifiers::SHIFT);
    assert!(wm.agents_window().active, "shifted form also opens");
}

#[test]
fn toggle_chord_lowers_to_the_toggle_action() {
    let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL | KeyModifiers::ALT);
    assert_eq!(default_global_action(key, false), Some(Action::Agents(AgentsOp::Toggle(None))));
}

#[test]
fn toggle_menu_entry_is_listed_and_toggles() {
    let found = (0..8)
        .flat_map(|c| (0..menubar_category_item_count(c).min(32)).map(move |i| (c, i)))
        .find(|&(c, i)| menubar_item_command(c, i) == Some(":agents toggle"));
    let (cat, item) = found.expect("a menubar category carries `:agents toggle`");
    let mut wm = fixture(120, 40);
    for expect_open in [true, false] {
        wm.set_menubar_open_category(Some(cat));
        for _ in 0..item {
            press_mod(&mut wm, KeyCode::Down, KeyModifiers::NONE);
        }
        press_mod(&mut wm, KeyCode::Enter, KeyModifiers::NONE);
        assert!(wm.menubar_state().is_none(), "the menu closes after the pick");
        assert_eq!(wm.agents_window().active, expect_open);
    }
}

/// Regression: a pane in raw PTY passthrough (alt-screen agent TUI, vim, less) used to swallow
/// `Ctrl+Alt+A`, so the launcher was dead exactly where it is needed.
#[test]
fn toggle_chord_wins_over_raw_pty_passthrough() {
    let mut wm = fixture(120, 40);
    wm.active_pane_mut().toggle_manual_raw_override();
    assert_eq!(
        wm.active_pane().effective_routing_mode(),
        bilterm::pty::line_discipline::InputRoutingMode::RawPtyPassthrough,
        "fixture must be in raw passthrough"
    );
    let chord = KeyModifiers::CONTROL | KeyModifiers::ALT;
    press_mod(&mut wm, KeyCode::Char('a'), chord);
    assert!(wm.agents_window().active, "Ctrl+Alt+A opens over a raw pane");
    press_mod(&mut wm, KeyCode::Char('a'), chord);
    assert!(!wm.agents_window().active, "and closes again");
}

#[test]
fn demo_fills_the_bus_with_pane_to_pane_events_and_opens_the_window() {
    let mut wm = fixture(120, 40);
    run_colon(&mut wm, ":agents demo");
    assert!(wm.agents_window().active);
    assert!(wm.pane_count() >= 3, "splits up to three panes");
    let evs: Vec<_> = wm.traffic().events_since(0).collect();
    assert!(evs.len() >= 10, "got {}", evs.len());
    assert!(evs.iter().all(|e| e.preview.starts_with("[demo]")));
    let pane_to_pane = evs.iter().filter(|e| matches!((&e.from, &e.to), (bilterm::wm::traffic::TrafficEndpoint::Pane(_), bilterm::wm::traffic::TrafficEndpoint::Pane(_)))).count();
    assert_eq!(pane_to_pane, evs.len(), "all demo events are pane to pane");
}
