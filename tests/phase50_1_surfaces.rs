//! Phase 50.1-09: discoverability surfaces for the Find window.
//!
//! Completion (`:find` sub-words, `:finder` row), palette (three `:find` rows, exception E1 for
//! `:finder`), help text (reserved words, sources, masking wording) and the View menu items that
//! give the Find window its mouse entry path (UNI-R01). Every entry point must lower to the same
//! `Action::Find` / find-modal state as the colon path.

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::command_registry::catalog;
use bilterm::input::handle_wm_key_event;
use bilterm::input::mouse::handle_wm_mouse_event;
use bilterm::pane::completion::colon_command_table;
use bilterm::ui::modals::{HELP_LINES, PALETTE_CATALOG, menubar_category_item_count, menubar_dropdown_rect, menubar_item_command};
use bilterm::ui::shell::ShellLayout;
use bilterm::wm::find_cmd::{FindKind, FindOp};
use bilterm::wm::{SplitDirection, WindowManager, inset_window_frame};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

fn fixture(cols: u16, rows: u16) -> WindowManager {
    let mut wm = WindowManager::new_headless(rows, cols);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(Rect::new(0, 0, cols, rows)).unwrap();
    wm
}

fn parse(cmd: &str) -> Action {
    parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"))
}

fn run_colon(wm: &mut WindowManager, cmd: &str) {
    wm.apply(parse(cmd)).unwrap();
}

fn press(wm: &mut WindowManager, code: KeyCode) {
    handle_wm_key_event(wm, KeyEvent::new(code, KeyModifiers::NONE)).unwrap();
}

fn help_rows_for(head: &str) -> Vec<(&'static str, &'static str)> {
    HELP_LINES
        .iter()
        .copied()
        .filter(|(lhs, _)| lhs.split_whitespace().next() == Some(head))
        .collect()
}

/// (category, item) of the View-menu entry that runs `cmd`.
fn menu_slot(cmd: &str) -> (usize, usize) {
    for cat in 0..8 {
        for item in 0..menubar_category_item_count(cat) {
            if menubar_item_command(cat, item) == Some(cmd) {
                return (cat, item);
            }
        }
    }
    panic!("no menu bar item runs `{cmd}`");
}

const FIND_VERBS: [(&str, FindKind); 3] = [
    (":find agent", FindKind::Agent),
    (":find session", FindKind::Session),
    (":find history", FindKind::History),
];

// ---- Task 1: completion, palette, help, registry -------------------------------------------

#[test]
fn completion_offers_find_verbs_and_finder_row() {
    let names = bilterm::pane::completion::colon_command_names();
    assert!(names.contains(&":finder"), "`:finder` must be a completion command");
    let table = colon_command_table();
    let (_, subs) = table.iter().find(|(c, _)| *c == ":find").expect("`:find` completion row");
    for word in ["agent", "session", "history", "next", "prev", "off"] {
        assert!(subs.contains(&word), "`:find` completion lacks `{word}`: {subs:?}");
    }
    let (_, finder_subs) = table.iter().find(|(c, _)| *c == ":finder").expect("`:finder` completion row");
    for word in ["kind ", "query ", "move ", "select ", "enter", "prefill", "close"] {
        assert!(finder_subs.contains(&word), "`:finder` completion lacks `{word}`: {finder_subs:?}");
    }
}

#[test]
fn palette_has_three_find_entries_that_parse_to_find_actions() {
    for (text, kind) in FIND_VERBS {
        let rows: Vec<_> = PALETTE_CATALOG.iter().filter(|e| e.0 == text).collect();
        assert_eq!(rows.len(), 1, "palette must carry `{text}` exactly once");
        assert_eq!(
            parse(rows[0].0),
            Action::Find(FindOp::Open { kind, query: String::new() }),
            "`{text}` must run without arguments"
        );
    }
    let agent = PALETTE_CATALOG.iter().find(|e| e.0 == ":find agent").unwrap();
    assert_eq!(agent.1, "Ctrl+Alt+F", "the first find row carries the entry chord");
}

#[test]
fn typing_find_in_the_palette_lists_them() {
    let mut wm = fixture(120, 40);
    wm.open_palette_with_query("find");
    let items = wm.filtered_palette_items();
    for (text, _) in FIND_VERBS {
        assert!(items.iter().any(|e| e.0 == text), "typing `find` must list `{text}`");
    }
}

#[test]
fn help_mentions_reserved_words_sources_and_masking() {
    let find_lines = help_rows_for(":find");
    assert!(!find_lines.is_empty(), "HELP_LINES has no `:find` row");
    let all: String = find_lines.iter().map(|(l, r)| format!("{l} {r}\n")).collect();
    for word in ["agent", "session", "history", ":search", "scrollback"] {
        assert!(all.contains(word), "`:find` help lacks `{word}`:\n{all}");
    }
    let verbs = find_lines
        .iter()
        .find(|(l, _)| l.contains("agent") && l.contains("session") && l.contains("history"))
        .expect("help line for `:find agent|session|history`");
    assert!(verbs.1.contains("source"), "verb help must name the result source: {}", verbs.1);
    assert!(verbs.1.contains("masked"), "verb help must mention masking: {}", verbs.1);
    assert!(verbs.1.contains("never shown"), "masked secrets are counted, never shown (D-U2): {}", verbs.1);
}

#[test]
fn help_has_finder_keys_and_mouse_lines() {
    assert!(!help_rows_for(":finder").is_empty(), "HELP_LINES has no `:finder` row");
    let keys = HELP_LINES.iter().find(|(l, _)| l.contains("Find window keys")).expect("Find window keys line");
    for k in ["Ctrl+Alt+F", "Tab", "Enter", "Ctrl+Y", "Esc"] {
        assert!(keys.1.contains(k), "Find window keys line lacks `{k}`: {}", keys.1);
    }
    let mouse = HELP_LINES.iter().find(|(l, _)| l.contains("Find window mouse")).expect("Find window mouse line");
    for k in ["tab", "row", "wheel", "footer"] {
        assert!(mouse.1.contains(k), "Find window mouse line lacks `{k}`: {}", mouse.1);
    }
}

#[test]
fn finder_has_help_and_no_palette_entry() {
    assert!(!help_rows_for(":finder").is_empty(), "HELP_LINES has no `:finder` row");
    assert!(
        PALETTE_CATALOG.iter().all(|e| e.0.split_whitespace().next() != Some(":finder")),
        "exception E1: `:finder` verbs are modal-only and must not be palette entries"
    );
}

#[test]
fn catalog_lists_find_and_finder() {
    let cat = catalog();
    let find = cat.iter().find(|c| c.name == ":find").expect("`:find` in the command catalog");
    assert!(find.examples.len() >= 3, "`:find` needs its three examples: {:?}", find.examples);
    for (text, _) in FIND_VERBS {
        assert!(find.examples.contains(&text), "`:find` catalog lacks `{text}`");
    }
    let finder = cat.iter().find(|c| c.name == ":finder").expect("`:finder` in the command catalog");
    assert!(!finder.summary.is_empty(), "`:finder` needs a summary");
}

// ---- Task 2: View menu items (mouse entry) and click/keyboard parity -----------------------

#[test]
fn menu_items_are_listed() {
    let (cat, _) = menu_slot(":find agent");
    let items: Vec<_> = (0..menubar_category_item_count(cat)).filter_map(|i| menubar_item_command(cat, i)).collect();
    for (text, _) in FIND_VERBS {
        assert!(items.contains(&text), "menu category {cat} lacks `{text}`: {items:?}");
    }
    let (agent_cat, _) = menu_slot(":agents toggle");
    assert_eq!(cat, agent_cat, "Find items sit in the View category with Agent Traffic");
}

#[test]
fn menu_item_keyboard_opens_find() {
    for (text, kind) in FIND_VERBS {
        let mut wm = fixture(120, 40);
        let (cat, item) = menu_slot(text);
        wm.set_menubar_open_category(Some(cat));
        for _ in 0..item {
            press(&mut wm, KeyCode::Down);
        }
        press(&mut wm, KeyCode::Enter);
        assert!(wm.menubar_state().is_none(), "the menu closes after the pick");
        assert_eq!(wm.find_modal_state().map(|s| s.0), Some(kind), "`{text}` menu item opens the matching kind");
    }
}

#[test]
fn menu_item_mouse_click_opens_find() {
    let screen = Rect::new(0, 0, 120, 40);
    for (text, kind) in FIND_VERBS {
        let mut wm = fixture(120, 40);
        let (cat, item) = menu_slot(text);
        wm.set_menubar_open_category(Some(cat));
        let shell = ShellLayout::compute(screen, wm.config(), wm.pane_count());
        let inner = inset_window_frame(menubar_dropdown_rect(cat, shell.workspace));
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: inner.x + 1,
            row: inner.y + item as u16,
            modifiers: KeyModifiers::NONE,
        };
        handle_wm_mouse_event(&mut wm, click, screen).unwrap();
        assert_eq!(wm.find_modal_state().map(|s| s.0), Some(kind), "click on `{text}` opens the matching kind");
    }
}

#[test]
fn menu_click_state_equals_colon_state() {
    let screen = Rect::new(0, 0, 120, 40);
    let mut colon = fixture(120, 40);
    run_colon(&mut colon, ":find session");
    let want = colon.find_modal_state().map(|(k, q, s)| (k, q.to_string(), s));

    let mut clicked = fixture(120, 40);
    let (cat, item) = menu_slot(":find session");
    clicked.set_menubar_open_category(Some(cat));
    let shell = ShellLayout::compute(screen, clicked.config(), clicked.pane_count());
    let inner = inset_window_frame(menubar_dropdown_rect(cat, shell.workspace));
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: inner.x + 1,
        row: inner.y + item as u16,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut clicked, click, screen).unwrap();
    let got = clicked.find_modal_state().map(|(k, q, s)| (k, q.to_string(), s));
    assert!(want.is_some(), "the colon path opens the Find window");
    assert_eq!(got, want, "menu click and `:find session` reach the same modal state");
}
