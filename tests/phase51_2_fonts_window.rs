//! Comprehensive test suite for Plan 51.2-01: First-Class `FONTS` Window (`WIN-01`).

use bilterm::colon_command::parse_colon_command;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event, AppAction};
use bilterm::pane::FontFamilyPreset;
use bilterm::ui::WorkspaceWidget;
use bilterm::wm::{SplitDirection, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

const COLS: u16 = 100;
const ROWS: u16 = 30;

fn fixture() -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    let area = Rect::new(0, 0, COLS, ROWS);
    wm.resize_all(area).unwrap();
    (wm, area)
}

fn run_cmd(wm: &mut WindowManager, cmd: &str) {
    let act = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("failed to parse colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("syntax error in colon command: {cmd} -> {e}"));
    wm.apply(act).unwrap();
}

fn buf_to_string(buf: &Buffer) -> String {
    let a = buf.area;
    let mut out = String::new();
    for y in a.y..a.y + a.height {
        for x in a.x..a.x + a.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn test_01_font_list_pops_up_fonts_window() {
    let (mut wm, _) = fixture();
    assert!(!wm.fonts_window().active);

    run_cmd(&mut wm, ":font list");
    assert!(wm.fonts_window().active, "Window should be active after :font list");
    assert!(wm.fonts_window().focused, "Window should be focused after :font list");
    assert!(wm.fonts_window().captures_input());
    assert!(wm.fonts_window().last_status.is_some());
}

#[test]
fn test_02_font_scan_pops_up_fonts_window() {
    let (mut wm, _) = fixture();
    run_cmd(&mut wm, ":font scan");
    assert!(wm.fonts_window().active);
    assert!(wm.fonts_window().focused);
    assert!(
        wm.fonts_window()
            .last_status
            .as_ref()
            .is_some_and(|s| s.contains("SCANNED FONTS"))
    );
}

#[test]
fn test_03_fonts_open_close_toggle() {
    let (mut wm, _) = fixture();

    run_cmd(&mut wm, ":fonts open");
    assert!(wm.fonts_window().active && wm.fonts_window().focused);

    run_cmd(&mut wm, ":fonts close");
    assert!(!wm.fonts_window().active && !wm.fonts_window().focused);

    run_cmd(&mut wm, ":fonts toggle");
    assert!(wm.fonts_window().active);

    run_cmd(&mut wm, ":fonts toggle");
    assert!(!wm.fonts_window().active);
}

#[test]
fn test_04_unpinned_apply_selected_changes_font_and_auto_closes() {
    let (mut wm, _) = fixture();
    run_cmd(&mut wm, ":fonts open");
    assert!(!wm.fonts_window().pinned);

    // Find row for VT323
    let rows = wm.fonts_window_rows();
    let vt323_idx = rows
        .iter()
        .position(|r| r.family == FontFamilyPreset::Vt323)
        .expect("VT323 must be in presets");

    run_cmd(&mut wm, &format!(":fonts select {vt323_idx}"));
    assert_eq!(wm.fonts_window().selected, vt323_idx);

    run_cmd(&mut wm, ":fonts apply");
    // Active pane should now use VT323
    assert_eq!(wm.active_pane().font_options().family, FontFamilyPreset::Vt323);
    // Unpinned window auto-closes!
    assert!(!wm.fonts_window().active, "Unpinned window must auto-close on apply");
}

#[test]
fn test_05_pinned_apply_selected_keeps_window_open_for_live_preview() {
    let (mut wm, _) = fixture();
    run_cmd(&mut wm, ":fonts open");
    run_cmd(&mut wm, ":fonts pin on");
    assert!(wm.fonts_window().pinned);

    let rows = wm.fonts_window_rows();
    let lazenby_idx = rows
        .iter()
        .position(|r| r.family == FontFamilyPreset::LazenbyComputer)
        .expect("LazenbyComputer must be in presets");

    run_cmd(&mut wm, &format!(":fonts select {lazenby_idx}"));
    run_cmd(&mut wm, ":fonts apply");

    assert_eq!(
        wm.active_pane().font_options().family,
        FontFamilyPreset::LazenbyComputer
    );
    // Pinned window stays open and focused for live comparison!
    assert!(wm.fonts_window().active, "Pinned window must stay active");
    assert!(wm.fonts_window().focused, "Pinned window must stay focused");
}

#[test]
fn test_06_apply_all_applies_font_to_all_panes() {
    let (mut wm, _) = fixture();
    wm.split_focused(SplitDirection::Vertical).unwrap();
    assert_eq!(wm.panes().len(), 2);

    run_cmd(&mut wm, ":fonts open");
    let rows = wm.fonts_window_rows();
    let vga_idx = rows
        .iter()
        .position(|r| r.family == FontFamilyPreset::BorlandDosVga)
        .expect("BorlandDosVga must be in presets");

    run_cmd(&mut wm, &format!(":fonts select {vga_idx}"));
    run_cmd(&mut wm, ":fonts all");

    for pane in wm.panes().values() {
        assert_eq!(pane.font_options().family, FontFamilyPreset::BorlandDosVga);
    }
}

#[test]
fn test_07_size_step_and_scale_step() {
    let (mut wm, _) = fixture();
    run_cmd(&mut wm, ":fonts open");

    let orig_pt = wm.active_pane().font_options().size_pt.unwrap_or(wm.config().font_size_pt);
    run_cmd(&mut wm, ":fonts size-step 2");
    let new_pt = wm.active_pane().font_options().size_pt.unwrap();
    assert_eq!(new_pt, (orig_pt + 2).clamp(6, 72));

    run_cmd(&mut wm, ":fonts size-step -1");
    let new_pt2 = wm.active_pane().font_options().size_pt.unwrap();
    assert_eq!(new_pt2, new_pt - 1);

    let orig_pct = wm.active_pane().font_options().scale_pct.unwrap_or(wm.config().font_scale_pct);
    run_cmd(&mut wm, ":fonts scale-step 20");
    let new_pct = wm.active_pane().font_options().scale_pct.unwrap();
    assert_eq!(new_pct, (orig_pct + 20).clamp(50, 400));
}

#[test]
fn test_08_move_resize_and_set_rect() {
    let (mut wm, ws) = fixture();
    run_cmd(&mut wm, ":fonts open");

    let initial = wm.fonts_window_rect(ws).unwrap();
    run_cmd(&mut wm, ":fonts move 5 3");
    let moved = wm.fonts_window_rect(ws).unwrap();
    assert_eq!(moved.x, initial.x + 5);
    assert_eq!(moved.y, initial.y + 3);

    run_cmd(&mut wm, ":fonts resize 6 4");
    let resized = wm.fonts_window_rect(ws).unwrap();
    assert_eq!(resized.width, initial.width + 6);
    assert_eq!(resized.height, initial.height + 4);

    run_cmd(&mut wm, ":fonts rect 15 8 50 16");
    let custom = wm.fonts_window_rect(ws).unwrap();
    assert_eq!(custom.x, 15);
    assert_eq!(custom.y, 8);
    assert_eq!(custom.width, 50);
    assert_eq!(custom.height, 16);
}

#[test]
fn test_09_keyboard_shortcuts_routing() {
    let (mut wm, _) = fixture();
    run_cmd(&mut wm, ":fonts open");
    assert!(wm.fonts_window().captures_input());

    // 'j' steps down
    let initial_sel = wm.fonts_window().selected;
    let key_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    handle_wm_key_event(&mut wm, key_j).unwrap();
    assert_eq!(wm.fonts_window().selected, initial_sel + 1);

    // 'k' steps up
    let key_k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
    handle_wm_key_event(&mut wm, key_k).unwrap();
    assert_eq!(wm.fonts_window().selected, initial_sel);

    // '*' toggles pin
    let key_star = KeyEvent::new(KeyCode::Char('*'), KeyModifiers::NONE);
    handle_wm_key_event(&mut wm, key_star).unwrap();
    assert!(wm.fonts_window().pinned);

    // 'Esc' dismisses window
    let key_esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    handle_wm_key_event(&mut wm, key_esc).unwrap();
    assert!(!wm.fonts_window().active);
}

#[test]
fn test_10_mouse_click_select_and_apply() {
    let (mut wm, ws) = fixture();
    run_cmd(&mut wm, ":fonts open");
    let rect = wm.fonts_window_rect(ws).unwrap();

    // Row 0 is at rect.y + 3
    // Row 2 is at rect.y + 5
    let click_row_2 = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x + 5,
        row: rect.y + 5,
        modifiers: KeyModifiers::NONE,
    };

    // First click selects row 2
    let res = handle_wm_mouse_event(&mut wm, click_row_2, ws).unwrap();
    assert_eq!(res, AppAction::Handled);
    assert_eq!(wm.fonts_window().selected, 2);

    // Second click on the already selected row applies it
    let res2 = handle_wm_mouse_event(&mut wm, click_row_2, ws).unwrap();
    assert_eq!(res2, AppAction::Handled);
    assert!(!wm.fonts_window().active, "Unpinned apply should close window");
}

#[test]
fn test_11_mouse_click_badges_close_dock_pin() {
    let (mut wm, ws) = fixture();
    run_cmd(&mut wm, ":fonts open");
    let rect = wm.fonts_window_rect(ws).unwrap();
    let r_edge = rect.x + rect.width;

    // Pin badge at r_edge - 11
    let click_pin = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: r_edge - 11,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, click_pin, ws).unwrap();
    assert!(wm.fonts_window().pinned, "Pin badge click should toggle pin on");

    // Close badge at r_edge - 3
    let click_close = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: r_edge - 3,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, click_close, ws).unwrap();
    assert!(!wm.fonts_window().active, "Close badge click should close window");
}

#[test]
fn test_12_render_fonts_window_contains_title_and_fonts() {
    let (mut wm, ws) = fixture();
    run_cmd(&mut wm, ":fonts open");

    let mut buf = Buffer::empty(ws);
    WorkspaceWidget::new(&wm).render(ws, &mut buf);
    let text = buf_to_string(&buf);

    assert!(text.contains("FONTS"), "Rendered text should contain 'FONTS'");
    assert!(
        text.contains("Lazenby Computer"),
        "Rendered text should contain 'Lazenby Computer'"
    );
    assert!(text.contains("VT323"), "Rendered text should contain 'VT323'");
    assert!(
        text.contains("Enter:Apply"),
        "Rendered text should contain 'Enter:Apply'"
    );
    assert!(text.contains("[s:Scan]"), "Rendered text should contain '[s:Scan]'");
}

#[test]
fn test_13_row_badges_and_samples() {
    let (wm, _) = fixture();
    let rows = wm.fonts_window_rows();

    for row in &rows {
        assert_eq!(row.sample, "AaBbCc 0123 ─│┌┐");
        match row.family {
            FontFamilyPreset::JetBrainsMono
            | FontFamilyPreset::LazenbyComputer
            | FontFamilyPreset::LazenbyLiquid
            | FontFamilyPreset::Vt323 => {
                assert_eq!(row.badge, "BUNDLED");
            }
            FontFamilyPreset::Ibm3270
            | FontFamilyPreset::BorlandDosVga
            | FontFamilyPreset::Tektronix4014 => {
                assert_eq!(row.badge, "RETRO");
            }
            FontFamilyPreset::Custom(_) => {
                assert_eq!(row.badge, "CUSTOM");
            }
            _ => {
                assert_eq!(row.badge, "PRESET");
            }
        }
    }
}

#[test]
fn test_14_blur_releases_keyboard_ownership() {
    let (mut wm, _) = fixture();
    run_cmd(&mut wm, ":fonts open");
    assert!(wm.fonts_window().captures_input());

    run_cmd(&mut wm, ":fonts blur");
    assert!(wm.fonts_window().active, "Window stays active");
    assert!(!wm.fonts_window().focused, "Window lost focus");
    assert!(!wm.fonts_window().captures_input(), "No longer captures input");
}
