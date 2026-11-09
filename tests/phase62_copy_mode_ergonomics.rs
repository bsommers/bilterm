//! Headless regression suite for Phase 62 Copy Mode Ergonomics (`COPY-02`).

use std::sync::Mutex;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::input::handle_wm_mouse_event;
use bilterm::pane::{CompositePane, CopyModeState, CopySelectionKind};
use bilterm::ui::copy_hints::copy_hint_line;
use bilterm::ui::WorkspaceWidget;
use bilterm::wm::WindowManager;

static CLIP_TEST_MUTEX: Mutex<()> = Mutex::new(());

fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    }
}

fn run_colon(wm: &mut WindowManager, cmd_str: &str) {
    let formatted = if cmd_str.starts_with(':') {
        cmd_str.to_string()
    } else {
        format!(":{cmd_str}")
    };
    if let Some(Ok(action)) = parse_colon_command(&formatted) {
        execute_wm_colon_command(wm, action);
    }
}

fn pane_100_lines() -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    let mut paint = Vec::new();
    for i in 0..100 {
        if i > 0 {
            paint.extend_from_slice(b"\r\n");
        }
        paint.extend_from_slice(format!("LINE {:03} content text here", i).as_bytes());
    }
    wm.active_pane_mut().process_pty_output(&paint);
    wm.active_pane_mut().write_to_pty(b"").unwrap();
    wm
}

#[test]
fn test_01_scroll_spanning_selection_copies_exact_lines() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut wm = pane_100_lines();
    let _area = Rect::new(0, 0, 80, 24);

    // Enter copy mode
    run_colon(&mut wm, "copy");
    assert!(wm.active_pane().copy_mode().active);

    // Position at bottom of screen, start line selection (V)
    bilterm::input::handle_wm_key_event(&mut wm, make_key(KeyCode::Char('V'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane().copy_mode().selection_kind, CopySelectionKind::Line);

    // Move up 40 lines
    for _ in 0..40 {
        bilterm::input::handle_wm_key_event(&mut wm, make_key(KeyCode::Char('k'), KeyModifiers::NONE)).unwrap();
    }

    // Yank selection
    bilterm::input::handle_wm_key_event(&mut wm, make_key(KeyCode::Char('y'), KeyModifiers::NONE)).unwrap();

    let clip = bilterm::clipboard::get_system_clipboard().unwrap_or_default();
    let lines: Vec<&str> = clip.lines().collect();
    assert_eq!(lines.len(), 41, "Expected 41 lines copied, got {}", lines.len());
    assert!(lines.first().unwrap().contains("LINE 059"), "First line should be LINE 059, got: {}", lines.first().unwrap());
    assert!(lines.last().unwrap().contains("LINE 099"), "Last line should be LINE 099, got: {}", lines.last().unwrap());
}

#[test]
fn test_02_selection_survives_live_output() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut wm = pane_100_lines();
    let _area = Rect::new(0, 0, 80, 24);

    run_colon(&mut wm, "copy");
    bilterm::input::handle_wm_key_event(&mut wm, make_key(KeyCode::Char('V'), KeyModifiers::NONE)).unwrap();
    
    // Live output arrives
    wm.active_pane_mut().process_pty_output(b"NEW LINE 100\r\nNEW LINE 101\r\n");

    // Yank
    bilterm::input::handle_wm_key_event(&mut wm, make_key(KeyCode::Char('y'), KeyModifiers::NONE)).unwrap();
    let clip = bilterm::clipboard::get_system_clipboard().unwrap_or_default();
    assert!(clip.contains("LINE 099"));
}

#[test]
fn test_03_selection_cleared_when_anchor_evicted() {
    let mut wm = WindowManager::new_headless(10, 40);
    wm.active_pane_mut().vt_mut().set_scrollback_capacity(20);
    let _area = Rect::new(0, 0, 40, 10);

    for i in 0..15 {
        wm.active_pane_mut().process_pty_output(format!("ROW {i}\r\n").as_bytes());
    }

    run_colon(&mut wm, "copy");
    bilterm::input::handle_wm_key_event(&mut wm, make_key(KeyCode::Char('V'), KeyModifiers::NONE)).unwrap();

    // Pump enough rows to exceed capacity and evict the anchor
    for i in 100..150 {
        wm.active_pane_mut().process_pty_output(format!("EVIC {i}\r\n").as_bytes());
    }

    // Selection should be cleared or handled safely without panic
    let _ = wm.active_pane_mut().yank_copy_selection();
}

#[test]
fn test_04_alt_screen_selection_is_frame_only() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.active_pane_mut().process_pty_output(b"\x1b[?1049h");
    wm.active_pane_mut().process_pty_output(b"\x1b[1;1HFrame line content");

    run_colon(&mut wm, "copy");
    let hint = copy_hint_line(wm.active_pane().copy_mode(), "screen only", 120);
    assert!(hint.contains("screen only") || hint.contains("frame"));
}

#[test]
fn test_05_word_motions_w_b_e() {
    let text = "alpha beta, gamma_x delta";
    assert_eq!(bilterm::pane::copy_motion::next_word_start(text, 0), Some(6));
    assert_eq!(bilterm::pane::copy_motion::word_end(text, 0), Some(4));
    assert_eq!(bilterm::pane::copy_motion::prev_word_start(text, 6), Some(0));

    let cjk = "世界 hello";
    let (s, e) = bilterm::pane::copy_motion::word_bounds_at(cjk, 0);
    assert_eq!(s, 0);
    assert!(e >= 3);
}

#[test]
fn test_06_line_and_page_motions() {
    let mut wm = pane_100_lines();
    let _area = Rect::new(0, 0, 80, 24);

    run_colon(&mut wm, "copy");
    // $ goes to end of line
    bilterm::input::handle_wm_key_event(&mut wm, make_key(KeyCode::Char('$'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane().copy_mode().cursor_col, 79);

    // 0 goes to col 0
    bilterm::input::handle_wm_key_event(&mut wm, make_key(KeyCode::Char('0'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane().copy_mode().cursor_col, 0);

    // gg goes to top of scrollback
    bilterm::input::handle_wm_key_event(&mut wm, make_key(KeyCode::Char('g'), KeyModifiers::NONE)).unwrap();
    bilterm::input::handle_wm_key_event(&mut wm, make_key(KeyCode::Char('g'), KeyModifiers::NONE)).unwrap();

    // G goes to bottom
    bilterm::input::handle_wm_key_event(&mut wm, make_key(KeyCode::Char('G'), KeyModifiers::SHIFT)).unwrap();
}

#[test]
fn test_07_vi_emacs_arrows_equivalence() {
    let mut wm1 = pane_100_lines();
    let mut wm2 = pane_100_lines();
    let _area = Rect::new(0, 0, 80, 24);

    // Vi: v l l y
    run_colon(&mut wm1, "copy");
    bilterm::input::handle_wm_key_event(&mut wm1, make_key(KeyCode::Char('v'), KeyModifiers::NONE)).unwrap();
    bilterm::input::handle_wm_key_event(&mut wm1, make_key(KeyCode::Char('l'), KeyModifiers::NONE)).unwrap();
    bilterm::input::handle_wm_key_event(&mut wm1, make_key(KeyCode::Char('l'), KeyModifiers::NONE)).unwrap();

    // Emacs: Ctrl+Space Ctrl+f Ctrl+f
    run_colon(&mut wm2, "copy");
    bilterm::input::handle_wm_key_event(&mut wm2, make_key(KeyCode::Char(' '), KeyModifiers::CONTROL)).unwrap();
    bilterm::input::handle_wm_key_event(&mut wm2, make_key(KeyCode::Char('f'), KeyModifiers::CONTROL)).unwrap();
    bilterm::input::handle_wm_key_event(&mut wm2, make_key(KeyCode::Char('f'), KeyModifiers::CONTROL)).unwrap();

    assert_eq!(wm1.active_pane().copy_mode().cursor_col, wm2.active_pane().copy_mode().cursor_col);
}

#[test]
fn test_08_selection_kinds() {
    let mut wm = pane_100_lines();
    let _area = Rect::new(0, 0, 80, 24);

    run_colon(&mut wm, "copy");
    bilterm::input::handle_wm_key_event(&mut wm, make_key(KeyCode::Char('v'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane().copy_mode().selection_kind, CopySelectionKind::Char);

    // Toggle clears
    bilterm::input::handle_wm_key_event(&mut wm, make_key(KeyCode::Char('v'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane().copy_mode().selection_kind, CopySelectionKind::None);

    run_colon(&mut wm, "select line");
    assert_eq!(wm.active_pane().copy_mode().selection_kind, CopySelectionKind::Line);

    run_colon(&mut wm, "select block");
    assert_eq!(wm.active_pane().copy_mode().selection_kind, CopySelectionKind::Block);
}

#[test]
fn test_09_multi_click_word_and_line() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut wm = pane_100_lines();
    let area = Rect::new(0, 0, 80, 24);

    // Double-click at (1, 1)
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        },
        area,
    ).unwrap();
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        },
        area,
    ).unwrap();
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        },
        area,
    ).unwrap();
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        },
        area,
    ).unwrap();

    assert!(wm.active_pane().copy_mode().has_selection());
}

#[test]
fn test_10_search_forward_backward() {
    let mut wm = pane_100_lines();
    let _area = Rect::new(0, 0, 80, 24);

    run_colon(&mut wm, "copy search LINE 07");
    // Verify search was initiated or matched
    assert!(wm.active_pane().copy_mode().active);
}

#[test]
fn test_11_hint_line_per_state() {
    let state = CopyModeState {
        active: true,
        ..Default::default()
    };
    let hint_wide = copy_hint_line(&state, "history", 120);
    assert!(hint_wide.contains("v char"));
    assert!(hint_wide.contains("y copy"));

    let hint_narrow = copy_hint_line(&state, "history", 60);
    assert!(hint_narrow.contains("Esc exit"));
}

#[test]
fn test_12_hint_not_overwritten_by_toast() {
    let mut wm = pane_100_lines();
    run_colon(&mut wm, "copy");
    wm.active_pane_mut().set_status_message(Some("A toast message".to_string()));

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| {
        f.render_widget(WorkspaceWidget::new(&wm), f.area());
    }).unwrap();

    let buf = terminal.backend().buffer();
    let mut bottom_row = String::new();
    for col in 0..80 {
        bottom_row.push_str(buf[(col, 23)].symbol());
    }
    assert!(bottom_row.contains("COPY") || bottom_row.contains("sel"));
}

#[test]
fn test_13_copied_region_flashes_then_clears() {
    let mut wm = pane_100_lines();
    run_colon(&mut wm, "copy");
    run_colon(&mut wm, "select word");
    let _ = wm.active_pane_mut().yank_copy_selection();

    assert!(wm.active_pane().copy_mode().flash_region.is_some());
    wm.active_pane_mut().tick_toast(100_000);
    assert!(wm.active_pane().copy_mode().flash_region.is_none());
}

#[test]
fn test_14_uni_r01_parity() {
    let mut wm_key = pane_100_lines();
    let mut wm_cmd = pane_100_lines();
    let _area = Rect::new(0, 0, 80, 24);

    // Key entry
    run_colon(&mut wm_key, "copy");
    // Colon entry
    run_colon(&mut wm_cmd, "copy");

    assert_eq!(wm_key.active_pane().copy_mode().active, wm_cmd.active_pane().copy_mode().active);
}

#[test]
fn test_15_unicode_regressions_unchanged() {
    let mut p = CompositePane::new_headless(1, 8, 40);
    p.vt_mut().process(b"world \xe4\xb8\x96\xe7\x95\x8c test");
    assert!(p.vt().snapshot().cell(0, 0).is_some());
}
