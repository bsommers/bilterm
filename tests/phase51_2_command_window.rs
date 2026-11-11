//! Phase 51.2-02: Pinnable COMMAND PALETTE first-class window (`WIN-02`).

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::WorkspaceWidget;
use bilterm::wm::command_window::{CommandWindowBadge, CommandWindowHit, CommandWindowOp};
use bilterm::wm::WindowManager;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

const COLS: u16 = 120;
const ROWS: u16 = 40;

fn fixture() -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    let area = Rect::new(0, 0, COLS, ROWS);
    wm.resize_all(area).unwrap();
    (wm, area)
}

fn line(buf: &Buffer, r: Rect, y: u16) -> String {
    (r.x..r.x + r.width)
        .map(|x| buf[(x, y)].symbol().to_string())
        .collect()
}

fn all_text(buf: &Buffer) -> String {
    let a = buf.area;
    (a.y..a.y + a.height)
        .map(|y| line(buf, a, y))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn test_01_colon_commands_open_close_toggle() {
    let (mut wm, _) = fixture();

    assert!(!wm.command_window().active);

    // :commands open
    let cmd = parse_colon_command(":commands open").unwrap().unwrap();
    wm.apply(cmd).unwrap();
    assert!(wm.command_window().active);
    assert!(wm.command_window().focused);

    // :commands toggle -> closes
    let cmd = parse_colon_command(":commands toggle").unwrap().unwrap();
    wm.apply(cmd).unwrap();
    assert!(!wm.command_window().active);

    // bare :commands -> toggles open
    let cmd = parse_colon_command(":commands").unwrap().unwrap();
    wm.apply(cmd).unwrap();
    assert!(wm.command_window().active);

    // :commands close
    let cmd = parse_colon_command(":commands close").unwrap().unwrap();
    wm.apply(cmd).unwrap();
    assert!(!wm.command_window().active);
}

#[test]
fn test_02_colon_palette_open_and_close() {
    let (mut wm, _) = fixture();

    // :palette open opens first-class command window
    let cmd = parse_colon_command(":palette open").unwrap().unwrap();
    wm.apply(cmd).unwrap();
    assert!(wm.command_window().active);
    assert!(wm.command_window().focused);

    // :palette close closes it
    let cmd = parse_colon_command(":palette close").unwrap().unwrap();
    wm.apply(cmd).unwrap();
    assert!(!wm.command_window().active);
}

#[test]
fn test_03_unpinned_execute_selected_auto_closes() {
    let (mut wm, _) = fixture();

    wm.open_command_window();
    assert!(wm.command_window().active);
    assert!(!wm.command_window().pinned);

    // Execute selected item (e.g. :split h)
    wm.apply(Action::CommandWindow(CommandWindowOp::ExecuteSelected))
        .unwrap();

    // Must be auto-closed because unpinned
    assert!(!wm.command_window().active);
    assert_eq!(wm.command_window().history_log.len(), 1);
    assert!(wm.command_window().history_log[0].ok);
}

#[test]
fn test_04_pinned_execute_selected_keeps_open_and_logs() {
    let (mut wm, _) = fixture();

    wm.open_command_window();
    let cmd = parse_colon_command(":commands pin").unwrap().unwrap();
    wm.apply(cmd).unwrap();
    assert!(wm.command_window().pinned);

    // Execute selected item
    wm.apply(Action::CommandWindow(CommandWindowOp::ExecuteSelected))
        .unwrap();

    // Must remain open and focused
    assert!(wm.command_window().active);
    assert!(wm.command_window().focused);
    assert_eq!(wm.command_window().history_log.len(), 1);
    let entry = &wm.command_window().history_log[0];
    assert!(entry.ok);
    assert!(!entry.status.is_empty());
}

#[test]
fn test_05_direct_colon_repl_in_query() {
    let (mut wm, _) = fixture();

    wm.open_command_window();
    wm.command_window_mut().pinned = true;

    // Direct :colon command entry in query
    wm.apply(Action::CommandWindow(CommandWindowOp::SetQuery(
        ":split h".to_string(),
    )))
    .unwrap();
    let initial_panes = wm.pane_count();

    wm.apply(Action::CommandWindow(CommandWindowOp::ExecuteSelected))
        .unwrap();

    assert_eq!(wm.pane_count(), initial_panes + 1);
    assert_eq!(wm.command_window().history_log.len(), 1);
    assert_eq!(
        wm.command_window().history_log[0].command,
        ":split h"
    );
    assert!(wm.command_window().history_log[0].ok);
}

#[test]
fn test_06_promoting_modal_palette_via_alt_p() {
    let (mut wm, _) = fixture();

    // Open modal palette
    wm.toggle_palette();
    assert!(wm.is_palette_visible());

    // Press Alt+P inside modal palette
    let key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::ALT);
    handle_wm_key_event(&mut wm, key).unwrap();

    // Modal closed, command window open & pinned!
    assert!(!wm.is_palette_visible());
    assert!(wm.command_window().active);
    assert!(wm.command_window().focused);
    assert!(wm.command_window().pinned);
}

#[test]
fn test_07_promoting_modal_palette_via_asterisk() {
    let (mut wm, _) = fixture();

    // Open modal palette and type some query
    wm.toggle_palette();
    assert!(wm.is_palette_visible());

    // Press '*'
    let key = KeyEvent::new(KeyCode::Char('*'), KeyModifiers::NONE);
    handle_wm_key_event(&mut wm, key).unwrap();

    assert!(!wm.is_palette_visible());
    assert!(wm.command_window().active);
    assert!(wm.command_window().focused);
    assert!(wm.command_window().pinned);
}

#[test]
fn test_08_palette_toggle_focuses_pinned_command_window() {
    let (mut wm, _) = fixture();

    wm.open_command_window();
    wm.command_window_mut().pinned = true;
    wm.command_window_mut().blur();
    assert!(!wm.command_window().focused);

    // Ctrl+P / Action::PaletteToggle
    wm.apply(Action::PaletteToggle).unwrap();

    // Does NOT open ephemeral modal, instead focuses pinned window
    assert!(!wm.is_palette_visible());
    assert!(wm.command_window().active);
    assert!(wm.command_window().focused);
}

#[test]
fn test_09_move_and_resize() {
    let (mut wm, area) = fixture();

    wm.open_command_window();
    let initial_rect = wm.command_window_rect(area).unwrap();

    wm.apply(Action::CommandWindow(CommandWindowOp::Move { dx: 4, dy: 2 }))
        .unwrap();
    let moved_rect = wm.command_window_rect(area).unwrap();
    assert_eq!(moved_rect.x, initial_rect.x + 4);
    assert_eq!(moved_rect.y, initial_rect.y + 2);

    wm.apply(Action::CommandWindow(CommandWindowOp::Resize { dw: 6, dh: 3 }))
        .unwrap();
    let resized_rect = wm.command_window_rect(area).unwrap();
    assert_eq!(resized_rect.width, initial_rect.width + 6);
    assert_eq!(resized_rect.height, initial_rect.height + 3);
}

#[test]
fn test_10_mouse_click_pin_dock_close_badges() {
    let (mut wm, area) = fixture();

    wm.open_command_window();
    let r = wm.command_window_rect(area).unwrap();

    // Hit test badges
    let pin_x = r.x + r.width - 11;
    let dock_x = r.x + r.width - 7;
    let close_x = r.x + r.width - 3;

    assert_eq!(
        wm.command_window_hit(area, pin_x, r.y),
        CommandWindowHit::Badge(CommandWindowBadge::Pin)
    );
    assert_eq!(
        wm.command_window_hit(area, dock_x, r.y),
        CommandWindowHit::Badge(CommandWindowBadge::Dock)
    );
    assert_eq!(
        wm.command_window_hit(area, close_x, r.y),
        CommandWindowHit::Badge(CommandWindowBadge::Close)
    );

    // Click Pin badge
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: pin_x,
        row: r.y,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, mouse, area).unwrap();
    assert!(wm.command_window().pinned);

    // Click Dock badge
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: dock_x,
        row: r.y,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, mouse, area).unwrap();
    assert!(wm.command_window().docked);

    // Click Close badge
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: close_x,
        row: r.y,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, mouse, area).unwrap();
    assert!(!wm.command_window().active);
}

#[test]
fn test_11_mouse_row_click_and_scroll() {
    let (mut wm, area) = fixture();

    wm.open_command_window();
    let r = wm.command_window_rect(area).unwrap();

    // Click row index 2 inside window
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: r.x + 5,
        row: r.y + 3,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, mouse, area).unwrap();
    assert_eq!(wm.command_window().selected, 1);

    // Scroll wheel down
    let mouse_scroll = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: r.x + 5,
        row: r.y + 3,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, mouse_scroll, area).unwrap();
    assert_eq!(wm.command_window().scroll, 3);
}

#[test]
fn test_12_rendering_command_palette_and_output_history() {
    let (mut wm, area) = fixture();

    wm.open_command_window();
    wm.command_window_mut().pinned = true;
    wm.command_window_mut().push_log(
        ":font vt323".to_string(),
        "PANE FONT: family='VT323'".to_string(),
        true,
    );

    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(&wm).render(area, &mut buf);
    let text = all_text(&buf);

    assert!(text.contains("COMMAND PALETTE"), "title missing: {text}");
    assert!(text.contains("OUTPUT / HISTORY"), "output/history missing: {text}");
    assert!(
        text.contains("Enter:Run  Alt+P:Pin  Ctrl+L:ClearLog  Esc:Back"),
        "footer hint missing: {text}"
    );
    assert!(text.contains(":font vt323"), "log entry missing: {text}");
}

#[test]
fn test_13_keyboard_navigation_and_query_editing() {
    let (mut wm, _) = fixture();

    wm.open_command_window();
    assert!(wm.command_window().captures_input());

    // Type 's', 'p', 'l', 'i', 't'
    for c in ['s', 'p', 'l', 'i', 't'] {
        let key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        handle_wm_key_event(&mut wm, key).unwrap();
    }
    assert_eq!(wm.command_window().query, "split");

    // Backspace
    let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
    handle_wm_key_event(&mut wm, key).unwrap();
    assert_eq!(wm.command_window().query, "spli");

    // Ctrl+U (clear query)
    let key = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
    handle_wm_key_event(&mut wm, key).unwrap();
    assert_eq!(wm.command_window().query, "");

    // Down arrow steps selection
    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    handle_wm_key_event(&mut wm, key).unwrap();
    assert_eq!(wm.command_window().selected, 1);

    // Up arrow steps back
    let key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
    handle_wm_key_event(&mut wm, key).unwrap();
    assert_eq!(wm.command_window().selected, 0);

    // FOCUS-01: with no query typed, Esc hands the keyboard back and leaves the window open.
    assert!(wm.command_window().query.is_empty());
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    handle_wm_key_event(&mut wm, key).unwrap();
    assert!(wm.command_window().active && !wm.command_window().captures_input());
}
