use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use bilterm::input::{handle_key_event, handle_mouse_event, handle_paste_event};
use bilterm::pane::CompositePane;
use bilterm::pane::colon_command::{execute_colon_command, parse_colon_command};
use bilterm::ui::{CompositePaneWidget, buffer_to_snapshot_string};

#[test]
fn v020_end_to_end_canvas_search_vi_copy_url_yank_and_exit_telemetry() {
    let mut pane = CompositePane::new_headless(1, 16, 72);
    let area = Rect::new(0, 0, 72, 16);

    // 1. Emit OSC 133 command execution + exit telemetry (VAL-01, VAL-06)
    pane.prompt.running_command = Some("cargo test".to_string());
    pane.vt.record_command_submission("cargo test");
    pane.process_pty_output(
        b"\x1b]133;C\x07\r\n\r\n\r\n\r\nDocs: https://docs.rs/ratatui/latest\r\nGRID_COL_A  RECT_1234\r\nGRID_COL_B  RECT_5678\r\n\x1b]133;D;0\x07",
    );
    assert_eq!(pane.vt.last_exit_code(), Some(0));
    assert!(pane.vt.last_duration_ms().is_some());

    // 2. Execute :search RECT_ and verify match count & navigation (VAL-02)
    let search_cmd = parse_colon_command(":search RECT_").unwrap().unwrap();
    execute_colon_command(&mut pane, search_cmd);
    assert!(pane.canvas_search.active);
    assert_eq!(pane.canvas_search.matches.len(), 2);

    handle_key_event(
        &mut pane,
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::ALT),
    )
    .unwrap();
    assert_eq!(pane.canvas_search.selected, 1);

    // 3. Ctrl+Click on the URL on row 4 -> yanks URL via OSC 52 (VAL-03)
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 10,
            row: 4,
            modifiers: KeyModifiers::CONTROL,
        },
        area,
    )
    .unwrap();
    assert_eq!(pane.prompt.yank_buffer, "https://docs.rs/ratatui/latest");

    // 4. Alt+LeftDrag rectangular selection across "1234\n5678" on rows 5..=6, cols 17..=20 (VAL-04)
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 17,
            row: 5,
            modifiers: KeyModifiers::ALT,
        },
        area,
    )
    .unwrap();
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 20,
            row: 6,
            modifiers: KeyModifiers::ALT,
        },
        area,
    )
    .unwrap();
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 20,
            row: 6,
            modifiers: KeyModifiers::ALT,
        },
        area,
    )
    .unwrap();
    assert_eq!(pane.prompt.yank_buffer, "1234\n5678");

    // 5. Trigger unsafe paste modal in RawPtyPassthrough and render it (FND-01)
    pane.toggle_manual_raw_override();
    handle_paste_event(&mut pane, "echo line1\necho line2").unwrap();
    assert!(pane.pending_unsafe_paste.is_some());

    let mut buf = Buffer::empty(area);
    CompositePaneWidget::new(&pane, true).render(area, &mut buf);
    let rendered = buffer_to_snapshot_string(&buf);
    assert!(rendered.contains("UNSAFE PASTE GUARD"));

    // Confirm paste with Enter
    handle_key_event(
        &mut pane,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    )
    .unwrap();
    assert!(pane.pending_unsafe_paste.is_none());
}
