//! Coverage for `src/input/modal.rs`: pane-level modal key handlers (unsafe paste, copy mode,
//! hints, fuzzy search, history popup) and WindowManager modals (drawer, MCP console, exit
//! guard, help, inspector, palette, menubar, held exited shell).

use std::sync::Once;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use bilterm::input::{AppAction, handle_key_event};
use bilterm::pane::{CompositePane, CopySelectionKind};

static ENV: Once = Once::new();

fn isolate() {
    ENV.call_once(|| {
        let dir = std::env::temp_dir().join(format!("bilterm-modal-cov-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        // SAFETY: executed exactly once, before any test in this binary reads the environment.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
    });
}

fn k(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
fn km(code: KeyCode, m: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, m)
}
fn ch(c: char) -> KeyEvent {
    k(KeyCode::Char(c))
}

fn pane() -> CompositePane {
    isolate();
    CompositePane::new_headless(1, 12, 40)
}


// ---------------------------------------------------------------------------------------------
// Unsafe paste modal
// ---------------------------------------------------------------------------------------------

#[test]
fn unsafe_paste_cancel_via_n_and_esc_clears_pending_and_sets_status() {
    for cancel in [ch('n'), ch('N'), k(KeyCode::Esc)] {
        let mut p = pane();
        p.set_pending_unsafe_paste(Some("a\nb".to_string()));
        let act = handle_key_event(&mut p, cancel).unwrap();
        assert_eq!(act, AppAction::Handled);
        assert!(p.pending_unsafe_paste().is_none());
        assert_eq!(p.status_message(), Some("Cancelled unsafe multi-line paste"));
    }
}

#[test]
fn unsafe_paste_other_keys_keep_pending_paste() {
    let mut p = pane();
    p.set_pending_unsafe_paste(Some("a\nb".to_string()));
    let act = handle_key_event(&mut p, ch('x')).unwrap();
    assert_eq!(act, AppAction::Handled);
    assert_eq!(p.pending_unsafe_paste(), Some("a\nb"));
}

#[test]
fn unsafe_paste_confirm_consumes_payload_and_reports_length() {
    for confirm in [ch('y'), ch('Y'), k(KeyCode::Enter)] {
        let mut p = pane();
        p.set_pending_unsafe_paste(Some("ls\nid".to_string()));
        let _ = handle_key_event(&mut p, confirm);
        assert!(p.pending_unsafe_paste().is_none(), "payload must be consumed");
        let status = p.status_message().unwrap_or_default().to_string();
        assert!(status.starts_with("Confirmed paste ("), "status was {status:?}");
    }
}

#[test]
fn unsafe_paste_confirm_with_nothing_pending_is_noop() {
    let mut p = pane();
    // pending_unsafe_paste is None so the router never reaches the modal; Enter goes to prompt.
    let act = handle_key_event(&mut p, ch('y')).unwrap();
    assert_eq!(act, AppAction::Handled);
    assert!(p.pending_unsafe_paste().is_none());
}

// ---------------------------------------------------------------------------------------------
// Hints mode
// ---------------------------------------------------------------------------------------------

#[test]
fn hints_mode_with_no_hints_exits_on_any_key() {
    let mut p = pane();
    p.hints_mode_mut().active = true;
    assert!(p.hints_mode().hints.is_empty());
    let act = handle_key_event(&mut p, ch('a')).unwrap();
    assert_eq!(act, AppAction::Handled);
    assert!(!p.hints_mode().active);
    assert_eq!(p.status_message(), Some("Exited 3270 Field Hints"));
}

fn pane_with_hints() -> CompositePane {
    let mut p = pane();
    p.process_pty_output(b"visit https://example.com/one and https://example.org/two now\r\n");
    p.enter_hints_mode();
    p
}

#[test]
fn hints_mode_navigation_wraps_and_esc_exits() {
    let mut p = pane_with_hints();
    let n = p.hints_mode().hints.len();
    assert!(n >= 2, "expected at least two hints, got {n}");
    assert!(p.hints_mode().active);
    assert_eq!(p.hints_mode().selected, 0);

    handle_key_event(&mut p, k(KeyCode::Down)).unwrap();
    assert_eq!(p.hints_mode().selected, 1);
    handle_key_event(&mut p, k(KeyCode::Tab)).unwrap();
    assert_eq!(p.hints_mode().selected, 2 % n);

    // Up from 0 wraps to the last hint.
    p.hints_mode_mut().selected = 0;
    handle_key_event(&mut p, k(KeyCode::Up)).unwrap();
    assert_eq!(p.hints_mode().selected, n - 1);
    handle_key_event(&mut p, k(KeyCode::BackTab)).unwrap();
    assert_eq!(p.hints_mode().selected, n - 2);

    // Down from the last wraps to 0.
    p.hints_mode_mut().selected = n - 1;
    handle_key_event(&mut p, k(KeyCode::Down)).unwrap();
    assert_eq!(p.hints_mode().selected, 0);

    handle_key_event(&mut p, k(KeyCode::Esc)).unwrap();
    assert!(!p.hints_mode().active);
}

#[test]
fn hints_mode_enter_accepts_selected_hint_into_prompt() {
    let mut p = pane_with_hints();
    let want = p.hints_mode().hints[0].text.clone();
    handle_key_event(&mut p, k(KeyCode::Enter)).unwrap();
    assert!(!p.hints_mode().active);
    assert_eq!(p.prompt().text(), want);
    assert!(p.status_message().unwrap().starts_with("Selected hint ["));
}

#[test]
fn hints_mode_typed_label_selects_and_backspace_trims_prefix() {
    let mut p = pane_with_hints();
    // A non-matching prefix stays pending; Backspace removes it.
    p.hints_mode_mut().typed_prefix.push('z');
    handle_key_event(&mut p, k(KeyCode::Backspace)).unwrap();
    assert!(p.hints_mode().typed_prefix.is_empty());
    // A digit is not a label char: ignored.
    handle_key_event(&mut p, ch('7')).unwrap();
    assert!(p.hints_mode().typed_prefix.is_empty());
    // Typing the first hint label accepts it.
    let label = p.hints_mode().hints[0].label.clone();
    let text = p.hints_mode().hints[0].text.clone();
    for c in label.chars() {
        handle_key_event(&mut p, ch(c)).unwrap();
    }
    assert!(!p.hints_mode().active);
    assert_eq!(p.prompt().text(), text);
    // Unmapped key in hints mode (when inactive it would go elsewhere) -> covered via re-enter.
    let mut p = pane_with_hints();
    handle_key_event(&mut p, k(KeyCode::F(3))).unwrap();
    assert!(p.hints_mode().active, "unrelated keys leave hints mode open");
}

// ---------------------------------------------------------------------------------------------
// Copy mode
// ---------------------------------------------------------------------------------------------

fn copy_pane() -> CompositePane {
    let mut p = pane();
    p.process_pty_output(b"hello world line one\r\nsecond line here\r\n");
    p.enter_copy_mode();
    p.copy_mode_mut().cursor_row = 3;
    p.copy_mode_mut().cursor_col = 5;
    p
}

#[test]
fn copy_mode_ctrl_g_and_f12_toggle_raw_override() {
    let mut p = copy_pane();
    assert!(!p.prompt().manual_raw_override);
    handle_key_event(&mut p, km(KeyCode::Char('g'), KeyModifiers::CONTROL)).unwrap();
    let mid = p.prompt().manual_raw_override;
    handle_key_event(&mut p, k(KeyCode::F(12))).unwrap();
    let after = p.prompt().manual_raw_override;
    assert!(p.copy_mode().active, "toggle must not leave copy mode");
    assert!(mid, "Ctrl+G enables manual raw override");
    assert!(!after, "F12 toggles it back off");
}

#[test]
fn copy_mode_f4_and_alt_e_cycle_prompt_mode() {
    let mut p = copy_pane();
    let m0 = p.prompt_mode();
    handle_key_event(&mut p, k(KeyCode::F(4))).unwrap();
    let m1 = p.prompt_mode();
    assert_ne!(m0, m1);
    handle_key_event(&mut p, km(KeyCode::Char('e'), KeyModifiers::ALT)).unwrap();
    let m2 = p.prompt_mode();
    assert_ne!(m1, m2);
    assert!(p.copy_mode().active);
}

#[test]
fn copy_mode_alt_c_exits_and_alt_r_toggles_rect() {
    let mut p = copy_pane();
    handle_key_event(&mut p, km(KeyCode::Char('r'), KeyModifiers::ALT)).unwrap();
    assert!(p.copy_mode().rect_mode, "Alt+R turns on rectangular mode");
    assert_eq!(p.copy_mode().selection_kind, CopySelectionKind::Block);
    handle_key_event(&mut p, km(KeyCode::Char('c'), KeyModifiers::ALT)).unwrap();
    assert!(!p.copy_mode().active);
}

#[test]
fn copy_mode_ctrl_down_exits() {
    let mut p = copy_pane();
    handle_key_event(&mut p, km(KeyCode::Down, KeyModifiers::CONTROL)).unwrap();
    assert!(!p.copy_mode().active);
}

#[test]
fn copy_mode_ctrl_v_starts_visual_block_at_cursor() {
    let mut p = copy_pane();
    handle_key_event(&mut p, km(KeyCode::Char('v'), KeyModifiers::CONTROL)).unwrap();
    assert_eq!(p.copy_mode().anchor, Some((3, 5)));
    assert_eq!(p.copy_mode().selection_kind, CopySelectionKind::Block);
    assert!(p.copy_mode().visual_lock);
    assert_eq!(p.status_message(), Some("COPY MODE: VISUAL BLOCK (rectangular)"));
}

#[test]
fn copy_mode_ctrl_c_yanks_and_ctrl_x_cuts_selection() {
    let mut p = copy_pane();
    p.copy_mode_mut().cursor_row = 0;
    p.copy_mode_mut().cursor_col = 0;
    handle_key_event(&mut p, ch('v')).unwrap();
    for _ in 0..4 {
        handle_key_event(&mut p, ch('l')).unwrap();
    }
    handle_key_event(&mut p, km(KeyCode::Char('c'), KeyModifiers::CONTROL)).unwrap();
    assert!(!p.copy_mode().active, "yank leaves copy mode");
    assert!(p.copy_mode().anchor.is_none());
    assert_eq!(p.status_message(), Some("Yanked 5 chars to clipboard"));

    let mut p = copy_pane();
    p.copy_mode_mut().cursor_row = 0;
    p.copy_mode_mut().cursor_col = 0;
    handle_key_event(&mut p, ch('v')).unwrap();
    for _ in 0..4 {
        handle_key_event(&mut p, ch('l')).unwrap();
    }
    handle_key_event(&mut p, km(KeyCode::Char('x'), KeyModifiers::CONTROL)).unwrap();
    assert!(!p.copy_mode().active);
    assert_eq!(p.prompt().text(), "hello");
}

#[test]
fn copy_mode_unmapped_ctrl_key_falls_through_without_state_change() {
    let mut p = copy_pane();
    let (r, c) = (p.copy_mode().cursor_row, p.copy_mode().cursor_col);
    handle_key_event(&mut p, km(KeyCode::Char('z'), KeyModifiers::CONTROL)).unwrap();
    assert!(p.copy_mode().active);
    assert_eq!((p.copy_mode().cursor_row, p.copy_mode().cursor_col), (r, c));
}

#[test]
fn copy_mode_shift_arrow_starts_char_selection_then_block_when_alt() {
    let mut p = copy_pane();
    handle_key_event(&mut p, km(KeyCode::Right, KeyModifiers::SHIFT)).unwrap();
    assert_eq!(p.copy_mode().anchor, Some((3, 5)));
    assert_eq!(p.copy_mode().selection_kind, CopySelectionKind::Char);
    assert!(!p.copy_mode().visual_lock);
    assert!(p.status_message().unwrap().starts_with("CANVAS SELECT"));
    assert_eq!(p.copy_mode().cursor_col, 6);

    // Existing Char anchor + Shift+Alt arrow upgrades to Block with lock.
    handle_key_event(
        &mut p,
        km(KeyCode::Right, KeyModifiers::SHIFT | KeyModifiers::ALT),
    )
    .unwrap();
    assert_eq!(p.copy_mode().selection_kind, CopySelectionKind::Block);
    assert!(p.copy_mode().visual_lock);
}

#[test]
fn copy_mode_shift_alt_arrow_without_anchor_starts_block_with_rect_message() {
    let mut p = copy_pane();
    handle_key_event(
        &mut p,
        km(KeyCode::Down, KeyModifiers::SHIFT | KeyModifiers::ALT),
    )
    .unwrap();
    assert_eq!(p.copy_mode().selection_kind, CopySelectionKind::Block);
    assert!(p.copy_mode().visual_lock);
    assert!(p.status_message().unwrap().starts_with("RECT SELECT"));
}

#[test]
fn copy_mode_plain_nav_without_visual_lock_drops_selection() {
    let mut p = copy_pane();
    handle_key_event(&mut p, km(KeyCode::Right, KeyModifiers::SHIFT)).unwrap();
    assert!(p.copy_mode().anchor.is_some());
    handle_key_event(&mut p, k(KeyCode::Left)).unwrap();
    assert!(p.copy_mode().anchor.is_none());
    assert_eq!(p.copy_mode().selection_kind, CopySelectionKind::None);
}

#[test]
fn copy_mode_exit_keys() {
    for exit in [
        k(KeyCode::Esc),
        ch('q'),
        ch('i'),
        k(KeyCode::Tab),
        k(KeyCode::F(8)),
    ] {
        let mut p = copy_pane();
        handle_key_event(&mut p, exit).unwrap();
        assert!(!p.copy_mode().active, "{exit:?} must exit copy mode");
    }
}

#[test]
fn copy_mode_horizontal_motion_clamps_at_edges() {
    let mut p = copy_pane();
    let (_, cols) = p.vt().size();
    let max_c = cols - 1;
    p.copy_mode_mut().cursor_col = 1;
    handle_key_event(&mut p, k(KeyCode::Left)).unwrap();
    assert_eq!(p.copy_mode().cursor_col, 0);
    handle_key_event(&mut p, ch('h')).unwrap();
    assert_eq!(p.copy_mode().cursor_col, 0, "clamped at 0");
    handle_key_event(&mut p, k(KeyCode::Right)).unwrap();
    assert_eq!(p.copy_mode().cursor_col, 1);
    // word modifier steps 5 columns
    handle_key_event(&mut p, km(KeyCode::Right, KeyModifiers::ALT)).unwrap();
    assert_eq!(p.copy_mode().cursor_col, 6);
    handle_key_event(&mut p, km(KeyCode::Left, KeyModifiers::ALT)).unwrap();
    assert_eq!(p.copy_mode().cursor_col, 1);
    handle_key_event(&mut p, k(KeyCode::End)).unwrap();
    assert_eq!(p.copy_mode().cursor_col, max_c);
    handle_key_event(&mut p, ch('l')).unwrap();
    assert_eq!(p.copy_mode().cursor_col, max_c, "clamped at last column");
    handle_key_event(&mut p, k(KeyCode::Home)).unwrap();
    assert_eq!(p.copy_mode().cursor_col, 0);
    handle_key_event(&mut p, ch('$')).unwrap();
    assert_eq!(p.copy_mode().cursor_col, max_c);
    handle_key_event(&mut p, ch('0')).unwrap();
    assert_eq!(p.copy_mode().cursor_col, 0);
    handle_key_event(&mut p, ch('w')).unwrap();
    assert_eq!(p.copy_mode().cursor_col, 5);
    handle_key_event(&mut p, ch('B')).unwrap();
    assert_eq!(p.copy_mode().cursor_col, 0);
    p.copy_mode_mut().cursor_col = max_c - 2;
    handle_key_event(&mut p, ch('w')).unwrap();
    assert_eq!(p.copy_mode().cursor_col, max_c);
}

#[test]
fn copy_mode_vertical_motion_moves_cursor_then_scrolls_scrollback() {
    let mut p = pane();
    for i in 0..60 {
        p.process_pty_output(format!("line {i}\r\n").as_bytes());
    }
    p.enter_copy_mode();
    let (rows, _) = p.vt().size();
    let max_r = rows - 1;
    p.copy_mode_mut().cursor_row = 1;
    handle_key_event(&mut p, k(KeyCode::Up)).unwrap();
    assert_eq!(p.copy_mode().cursor_row, 0);
    assert_eq!(p.vt().scrollback_offset(), 0);
    // At the top row, Up/k scroll the scrollback instead.
    handle_key_event(&mut p, k(KeyCode::Up)).unwrap();
    assert_eq!(p.copy_mode().cursor_row, 0);
    assert_eq!(p.vt().scrollback_offset(), 1);
    handle_key_event(&mut p, ch('k')).unwrap();
    assert_eq!(p.vt().scrollback_offset(), 2);
    // Down at the bottom scrolls back toward live output.
    p.copy_mode_mut().cursor_row = max_r;
    handle_key_event(&mut p, k(KeyCode::Down)).unwrap();
    assert_eq!(p.vt().scrollback_offset(), 1);
    handle_key_event(&mut p, ch('j')).unwrap();
    assert_eq!(p.vt().scrollback_offset(), 0);
    assert_eq!(p.copy_mode().cursor_row, max_r);
    // Down from inside moves the cursor.
    p.copy_mode_mut().cursor_row = 2;
    handle_key_event(&mut p, ch('j')).unwrap();
    assert_eq!(p.copy_mode().cursor_row, 3);
}

#[test]
fn copy_mode_page_keys_move_by_eight_or_scroll_at_limits() {
    let mut p = pane();
    for i in 0..80 {
        p.process_pty_output(format!("line {i}\r\n").as_bytes());
    }
    p.enter_copy_mode();
    let (rows, _) = p.vt().size();
    let max_r = rows - 1;
    p.copy_mode_mut().cursor_row = max_r;
    handle_key_event(&mut p, k(KeyCode::PageUp)).unwrap();
    assert_eq!(p.copy_mode().cursor_row, max_r - 8);
    // Not enough room: pins to row 0 and scrolls 8 lines back.
    p.copy_mode_mut().cursor_row = 3;
    handle_key_event(&mut p, k(KeyCode::PageUp)).unwrap();
    assert_eq!(p.copy_mode().cursor_row, 0);
    assert_eq!(p.vt().scrollback_offset(), 8);
    // PageDown with room moves the cursor.
    p.copy_mode_mut().cursor_row = 0;
    if max_r >= 8 {
        handle_key_event(&mut p, k(KeyCode::PageDown)).unwrap();
        assert_eq!(p.copy_mode().cursor_row, 8);
    }
    // PageDown without room pins to bottom row and scrolls back toward live.
    p.copy_mode_mut().cursor_row = max_r;
    handle_key_event(&mut p, k(KeyCode::PageDown)).unwrap();
    assert_eq!(p.copy_mode().cursor_row, max_r);
    assert_eq!(p.vt().scrollback_offset(), 0);
}

#[test]
fn copy_mode_r_toggles_rect_selection_on_and_off() {
    let mut p = copy_pane();
    handle_key_event(&mut p, ch('r')).unwrap();
    assert_eq!(p.copy_mode().selection_kind, CopySelectionKind::Block);
    assert!(p.copy_mode().visual_lock);
    assert_eq!(p.copy_mode().anchor, Some((3, 5)));
    assert!(p.status_message().unwrap().contains("VISUAL BLOCK"));
    handle_key_event(&mut p, ch('R')).unwrap();
    assert_eq!(p.copy_mode().selection_kind, CopySelectionKind::None);
    assert!(!p.copy_mode().visual_lock);
    assert!(p.copy_mode().anchor.is_none());
    assert_eq!(
        p.status_message(),
        Some("COPY MODE: rectangular selection cleared")
    );
}

#[test]
fn copy_mode_r_keeps_existing_anchor_when_starting_block() {
    let mut p = copy_pane();
    p.copy_mode_mut().anchor = Some((1, 1));
    p.copy_mode_mut().cursor_row = 4;
    handle_key_event(&mut p, ch('r')).unwrap();
    assert_eq!(p.copy_mode().anchor, Some((1, 1)));
    assert_eq!(p.copy_mode().selection_kind, CopySelectionKind::Block);
}

#[test]
fn copy_mode_b_starts_block_then_moves_back_five_columns() {
    let mut p = copy_pane();
    handle_key_event(&mut p, ch('b')).unwrap();
    assert_eq!(p.copy_mode().selection_kind, CopySelectionKind::Block);
    assert_eq!(p.copy_mode().anchor, Some((3, 5)));
    p.copy_mode_mut().cursor_col = 12;
    handle_key_event(&mut p, ch('b')).unwrap();
    assert_eq!(p.copy_mode().cursor_col, 7);
    p.copy_mode_mut().cursor_col = 2;
    handle_key_event(&mut p, ch('b')).unwrap();
    assert_eq!(p.copy_mode().cursor_col, 0);
}

#[test]
fn copy_mode_v_and_space_toggle_char_selection() {
    let mut p = copy_pane();
    handle_key_event(&mut p, ch('v')).unwrap();
    assert_eq!(p.copy_mode().selection_kind, CopySelectionKind::Char);
    assert!(p.copy_mode().visual_lock);
    assert!(p.status_message().unwrap().contains("VISUAL CHAR"));
    handle_key_event(&mut p, ch(' ')).unwrap();
    assert_eq!(p.copy_mode().selection_kind, CopySelectionKind::None);
    assert!(!p.copy_mode().visual_lock);
    assert_eq!(p.status_message(), Some("COPY MODE: selection cleared"));
}

#[test]
fn copy_mode_capital_v_selects_line() {
    let mut p = copy_pane();
    handle_key_event(&mut p, ch('V')).unwrap();
    assert_eq!(p.copy_mode().selection_kind, CopySelectionKind::Line);
    assert!(p.copy_mode().visual_lock);
    assert_eq!(p.copy_mode().anchor, Some((3, 5)));
    assert_eq!(p.status_message(), Some("COPY MODE: VISUAL LINE"));
}

#[test]
fn copy_mode_y_enter_c_yank_and_x_cuts_to_prompt() {
    for yank in [ch('y'), ch('c'), k(KeyCode::Enter)] {
        let mut p = copy_pane();
        p.copy_mode_mut().cursor_row = 0;
        p.copy_mode_mut().cursor_col = 0;
        handle_key_event(&mut p, ch('V')).unwrap();
        handle_key_event(&mut p, yank).unwrap();
        assert!(!p.copy_mode().active, "{yank:?} yanks and exits");
        assert!(p.copy_mode().anchor.is_none());
    }
    // y with no selection reports an empty selection.
    let mut p = copy_pane();
    handle_key_event(&mut p, ch('y')).unwrap();
    assert_eq!(p.status_message(), Some("Copy Mode: empty selection"));

    let mut p = copy_pane();
    p.copy_mode_mut().cursor_row = 0;
    p.copy_mode_mut().cursor_col = 0;
    handle_key_event(&mut p, ch('v')).unwrap();
    for _ in 0..4 {
        handle_key_event(&mut p, ch('l')).unwrap();
    }
    handle_key_event(&mut p, ch('x')).unwrap();
    assert_eq!(p.prompt().text(), "hello");
}

#[test]
fn copy_mode_unmapped_key_is_ignored() {
    let mut p = copy_pane();
    let (r, c) = (p.copy_mode().cursor_row, p.copy_mode().cursor_col);
    let act = handle_key_event(&mut p, ch('~')).unwrap();
    assert_eq!(act, AppAction::Handled);
    assert!(p.copy_mode().active);
    assert_eq!((p.copy_mode().cursor_row, p.copy_mode().cursor_col), (r, c));
}

// ---------------------------------------------------------------------------------------------
// Fuzzy history search
// ---------------------------------------------------------------------------------------------

fn fuzzy_pane() -> CompositePane {
    let mut p = pane();
    p.prompt_mut().history = vec![
        "cargo build".to_string(),
        "cargo test".to_string(),
        "git status".to_string(),
    ];
    p.prompt_mut().open_fuzzy_search();
    p
}

#[test]
fn fuzzy_search_ctrl_cancel_and_navigation() {
    let mut p = fuzzy_pane();
    assert!(p.prompt().fuzzy_search.active);
    handle_key_event(&mut p, km(KeyCode::Char('g'), KeyModifiers::CONTROL)).unwrap();
    assert!(!p.prompt().fuzzy_search.active);

    let mut p = fuzzy_pane();
    handle_key_event(&mut p, km(KeyCode::Char('c'), KeyModifiers::CONTROL)).unwrap();
    assert!(!p.prompt().fuzzy_search.active);

    let mut p = fuzzy_pane();
    let s0 = p.prompt().fuzzy_search.selected;
    handle_key_event(&mut p, km(KeyCode::Char('r'), KeyModifiers::CONTROL)).unwrap();
    let s1 = p.prompt().fuzzy_search.selected;
    assert_ne!(s0, s1);
    handle_key_event(&mut p, km(KeyCode::Char('n'), KeyModifiers::CONTROL)).unwrap();
    let s2 = p.prompt().fuzzy_search.selected;
    assert_ne!(s1, s2);
    handle_key_event(&mut p, km(KeyCode::Char('p'), KeyModifiers::CONTROL)).unwrap();
    assert_eq!(p.prompt().fuzzy_search.selected, s1);
    // Unmapped control key: still active, state unchanged.
    handle_key_event(&mut p, km(KeyCode::Char('z'), KeyModifiers::CONTROL)).unwrap();
    assert!(p.prompt().fuzzy_search.active);
    assert_eq!(p.prompt().fuzzy_search.selected, s1);
}

#[test]
fn fuzzy_search_typing_backspace_arrows_enter_esc() {
    let mut p = fuzzy_pane();
    handle_key_event(&mut p, ch('g')).unwrap();
    handle_key_event(&mut p, ch('i')).unwrap();
    assert_eq!(p.prompt().fuzzy_search.query, "gi");
    handle_key_event(&mut p, k(KeyCode::Backspace)).unwrap();
    assert_eq!(p.prompt().fuzzy_search.query, "g");
    handle_key_event(&mut p, k(KeyCode::Down)).unwrap();
    handle_key_event(&mut p, k(KeyCode::Up)).unwrap();
    // Unmapped key (F2) is ignored.
    handle_key_event(&mut p, k(KeyCode::F(2))).unwrap();
    assert!(p.prompt().fuzzy_search.active);

    // Accept selection.
    let mut p = fuzzy_pane();
    for c in "status".chars() {
        handle_key_event(&mut p, ch(c)).unwrap();
    }
    handle_key_event(&mut p, k(KeyCode::Enter)).unwrap();
    assert!(!p.prompt().fuzzy_search.active);
    assert_eq!(p.prompt().text(), "git status");

    // Esc cancels without modifying the prompt.
    let mut p = fuzzy_pane();
    handle_key_event(&mut p, k(KeyCode::Esc)).unwrap();
    assert!(!p.prompt().fuzzy_search.active);
    assert_eq!(p.prompt().text(), "");
}

// ---------------------------------------------------------------------------------------------
// History popup
// ---------------------------------------------------------------------------------------------

fn popup_pane() -> CompositePane {
    let mut p = pane();
    p.prompt_mut().history = vec!["one".into(), "two".into(), "three".into()];
    p.prompt_mut().history_popup_open = true;
    p.prompt_mut().history_popup_selected = 0;
    p
}

#[test]
fn history_popup_navigation_clamps_and_accepts() {
    let mut p = popup_pane();
    handle_key_event(&mut p, k(KeyCode::Up)).unwrap();
    assert_eq!(p.prompt().history_popup_selected, 0, "clamped at top");
    handle_key_event(&mut p, ch('j')).unwrap();
    handle_key_event(&mut p, k(KeyCode::Down)).unwrap();
    assert_eq!(p.prompt().history_popup_selected, 2);
    handle_key_event(&mut p, k(KeyCode::Down)).unwrap();
    assert_eq!(p.prompt().history_popup_selected, 2, "clamped at bottom");
    handle_key_event(&mut p, ch('k')).unwrap();
    assert_eq!(p.prompt().history_popup_selected, 1);
    handle_key_event(&mut p, k(KeyCode::Enter)).unwrap();
    assert!(!p.prompt().history_popup_open);
    assert_eq!(p.prompt().text(), "two");
}

#[test]
fn history_popup_esc_and_q_close_without_editing_prompt() {
    for close in [k(KeyCode::Esc), ch('q')] {
        let mut p = popup_pane();
        handle_key_event(&mut p, close).unwrap();
        assert!(!p.prompt().history_popup_open);
        assert_eq!(p.prompt().text(), "");
    }
}

#[test]
fn history_popup_s_saves_history_and_reports_path() {
    let mut p = popup_pane();
    handle_key_event(&mut p, ch('s')).unwrap();
    assert!(p.prompt().history_popup_open, "saving keeps the popup open");
    if let Some(msg) = p.status_message() {
        assert!(msg.starts_with("Saved history to "), "status was {msg:?}");
    }
    // Unmapped key is a no-op.
    handle_key_event(&mut p, ch('?')).unwrap();
    assert!(p.prompt().history_popup_open);
}
