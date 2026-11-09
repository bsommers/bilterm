//! Phase 62 Wave 1 integration tests: Nested TUI copy fix (`COPY-01`).

use std::sync::Mutex;

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use bilterm::api::AutomationControlPlane;
use bilterm::clipboard::{
    clear_process_clipboard, get_system_clipboard, set_system_clipboard, ClipboardSource,
};
use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::pane::{CopyOrigin, CopySelectionKind};
use bilterm::ui::modals::PALETTE_CATALOG;
use bilterm::ui::WorkspaceWidget;
use bilterm::vt::encode_osc52_clipboard;
use bilterm::wm::WindowManager;

static CLIP_TEST_MUTEX: Mutex<()> = Mutex::new(());

fn key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
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

fn herdr_wm() -> WindowManager {
    let mut wm = WindowManager::new_headless(25, 80);
    // Alternate screen + SGR mouse tracking (?1000h ?1002h ?1006h) + bracketed paste (?2004h)
    let setup = b"\x1b[?1049h\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?2004h";
    wm.active_pane_mut().process_pty_output(setup);

    // Paint 10 lines via CUP so the canvas contains recognizable text
    let mut paint = Vec::new();
    for row in 1..=10 {
        paint.extend_from_slice(format!("\x1b[{row};1HLine {row:02} content text here\r\n").as_bytes());
    }
    wm.active_pane_mut().process_pty_output(&paint);
    wm.active_pane_mut().write_to_pty(b"").unwrap();
    wm
}

fn render_to_string(wm: &WindowManager, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            frame.render_widget(WorkspaceWidget::new(wm), frame.area());
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..height {
        for x in 0..width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn test_01_plain_drag_is_forwarded_as_sgr() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut wm = herdr_wm();
    let area = Rect::new(0, 0, 80, 25);

    // Down at col 5, row 2
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();

    // Drag to col 15, row 2
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 15,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();

    let pty_write = String::from_utf8_lossy(wm.active_pane().last_pty_write()).into_owned();
    assert!(
        pty_write.starts_with("\x1b[<") && pty_write.ends_with('M'),
        "Expected SGR mouse drag report, got: {pty_write:?}"
    );
    assert!(!wm.active_pane().copy_mode().active);
}

#[test]
fn test_02_shift_drag_selects_locally_and_writes_nothing() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut wm = herdr_wm();
    let area = Rect::new(0, 0, 80, 25);

    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 2,
            modifiers: KeyModifiers::SHIFT,
        },
        area,
    )
    .unwrap();

    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 10,
            row: 2,
            modifiers: KeyModifiers::SHIFT,
        },
        area,
    )
    .unwrap();

    assert!(
        wm.active_pane().last_pty_write().is_empty(),
        "Expected nothing written to PTY on Shift-drag, got: {:?}",
        String::from_utf8_lossy(wm.active_pane().last_pty_write())
    );
    assert!(wm.active_pane().copy_mode().has_selection());
    assert_eq!(
        wm.active_pane().copy_mode().selection_kind,
        CopySelectionKind::Char
    );
}

#[test]
fn test_03_alt_drag_selects_block_locally() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut wm = herdr_wm();
    let area = Rect::new(0, 0, 80, 25);

    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 2,
            modifiers: KeyModifiers::ALT,
        },
        area,
    )
    .unwrap();

    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 8,
            row: 4,
            modifiers: KeyModifiers::ALT,
        },
        area,
    )
    .unwrap();

    assert!(wm.active_pane().last_pty_write().is_empty());
    assert!(wm.active_pane().copy_mode().has_selection());
    assert_eq!(
        wm.active_pane().copy_mode().selection_kind,
        CopySelectionKind::Block
    );
}

#[test]
fn test_04_ctrl_click_forwards_modifier_bit() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut wm = herdr_wm();
    let area = Rect::new(0, 0, 80, 25);

    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 10,
            row: 5,
            modifiers: KeyModifiers::CONTROL,
        },
        area,
    )
    .unwrap();

    let pty_write = String::from_utf8_lossy(wm.active_pane().last_pty_write()).into_owned();
    // In SGR 1006 mouse: button 0 + 16 (Ctrl) = 16
    assert!(
        pty_write.starts_with("\x1b[<16;"),
        "Expected SGR report starting with \\x1b[<16;, got: {pty_write:?}"
    );

    // Shift + Right click remains paste bypass (Phase 57 guard)
    clear_process_clipboard();
    set_system_clipboard("pasted_token");
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: 10,
            row: 5,
            modifiers: KeyModifiers::SHIFT,
        },
        area,
    )
    .unwrap();
    let paste_write = String::from_utf8_lossy(wm.active_pane().last_pty_write()).into_owned();
    assert_eq!(paste_write, "\x1b[200~pasted_token\x1b[201~");
}

#[test]
fn test_05_release_copies_by_default_and_toasts() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    clear_process_clipboard();
    let mut wm = herdr_wm();
    let area = Rect::new(0, 0, 80, 25);

    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 2,
            modifiers: KeyModifiers::SHIFT,
        },
        area,
    )
    .unwrap();

    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 10,
            row: 2,
            modifiers: KeyModifiers::SHIFT,
        },
        area,
    )
    .unwrap();

    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 10,
            row: 2,
            modifiers: KeyModifiers::SHIFT,
        },
        area,
    )
    .unwrap();

    let clip = get_system_clipboard().unwrap_or_default();
    assert!(!clip.is_empty(), "Clipboard should contain copied text");
    assert!(
        wm.clipboard_registry()
            .entries()
            .iter()
            .any(|e| e.source == ClipboardSource::CopyModeYank)
    );

    let status = wm.active_pane().status_message().unwrap_or_default();
    assert!(status.contains("Copied"), "Status message should contain 'Copied', got: {status}");

    // Toast expiration
    wm.active_pane_mut().tick_toast(100_000);
    assert!(
        wm.active_pane().status_message().is_none(),
        "Toast should have expired"
    );
}

#[test]
fn test_06_explicit_mode_does_not_copy_on_release() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    clear_process_clipboard();
    let mut wm = herdr_wm();
    let area = Rect::new(0, 0, 80, 25);

    run_colon(&mut wm, "copy auto off");

    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 2,
            modifiers: KeyModifiers::SHIFT,
        },
        area,
    )
    .unwrap();

    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 10,
            row: 2,
            modifiers: KeyModifiers::SHIFT,
        },
        area,
    )
    .unwrap();

    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 10,
            row: 2,
            modifiers: KeyModifiers::SHIFT,
        },
        area,
    )
    .unwrap();

    assert_eq!(
        get_system_clipboard(),
        None,
        "Clipboard must remain empty when copy auto is off"
    );
}

#[test]
fn test_07_mouse_selection_is_transient() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut wm = herdr_wm();
    let area = Rect::new(0, 0, 80, 25);

    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 2,
            modifiers: KeyModifiers::SHIFT,
        },
        area,
    )
    .unwrap();

    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 10,
            row: 2,
            modifiers: KeyModifiers::SHIFT,
        },
        area,
    )
    .unwrap();

    wm.active_pane_mut().write_to_pty(b"").unwrap();

    // Typing a non-copy key 'a' must clear selection and be delivered to the child PTY
    handle_wm_key_event(&mut wm, key_event(KeyCode::Char('a'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane().last_pty_write(), b"a");
    assert!(!wm.active_pane().copy_mode().active);
}

#[test]
fn test_08_keyboard_copy_mode_stays_modal() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut wm = herdr_wm();

    // Leader chord: Ctrl+B then [
    handle_wm_key_event(
        &mut wm,
        key_event(KeyCode::Char('b'), KeyModifiers::CONTROL),
    )
    .unwrap();
    handle_wm_key_event(&mut wm, key_event(KeyCode::Char('['), KeyModifiers::NONE)).unwrap();

    assert!(wm.active_pane().copy_mode().active);
    assert_eq!(wm.active_pane().copy_mode().origin, CopyOrigin::Keyboard);

    wm.active_pane_mut().write_to_pty(b"").unwrap();
    handle_wm_key_event(&mut wm, key_event(KeyCode::Char('a'), KeyModifiers::NONE)).unwrap();
    assert!(
        wm.active_pane().last_pty_write().is_empty(),
        "Key 'a' must NOT reach child PTY while in keyboard copy mode"
    );
    assert!(wm.active_pane().copy_mode().active);
}

#[test]
fn test_09_sticky_toggle_makes_plain_drag_local() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut wm = herdr_wm();
    let area = Rect::new(0, 0, 80, 25);

    run_colon(&mut wm, "copy mouse on");
    assert!(wm.active_pane().mouse_select_sticky());

    // Plain drag without Shift/Alt
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();

    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 10,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();

    assert!(
        wm.active_pane().last_pty_write().is_empty(),
        "Sticky mode: plain drag must select locally and write nothing to child"
    );
    assert!(wm.active_pane().copy_mode().has_selection());

    run_colon(&mut wm, "copy mouse off");
    assert!(!wm.active_pane().mouse_select_sticky());
}

#[test]
fn test_10_osc52_split_at_every_offset_decodes_once() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let payload = "OSC52_CLIP_PAYLOAD";
    let full_osc = encode_osc52_clipboard(payload).into_bytes();

    for i in 1..full_osc.len() {
        clear_process_clipboard();
        let mut wm = herdr_wm();
        let chunk1 = &full_osc[..i];
        let chunk2 = &full_osc[i..];

        wm.active_pane_mut().process_pty_output(chunk1);
        wm.active_pane_mut().process_pty_output(chunk2);
        wm.sync_clipboard_records();

        assert_eq!(
            get_system_clipboard().as_deref(),
            Some(payload),
            "Failed at split index {i}"
        );
        let osc_entries = wm
            .clipboard_registry()
            .entries()
            .iter()
            .filter(|e| e.source == ClipboardSource::Osc52 && e.content == payload)
            .count();
        assert_eq!(
            osc_entries, 1,
            "Expected exactly 1 OSC52 entry at split index {i}, got {osc_entries}"
        );
    }
}

#[test]
fn test_11_osc52_9000_byte_payload_split_at_8192() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    clear_process_clipboard();
    let large_payload = "A".repeat(9000);
    let full_osc = encode_osc52_clipboard(&large_payload).into_bytes();
    assert!(full_osc.len() > 8192);

    let mut wm = herdr_wm();
    let chunk1 = &full_osc[..8192];
    let chunk2 = &full_osc[8192..];

    wm.active_pane_mut().process_pty_output(chunk1);
    wm.active_pane_mut().process_pty_output(chunk2);
    wm.sync_clipboard_records();

    assert_eq!(
        get_system_clipboard().as_deref(),
        Some(large_payload.as_str())
    );
}

#[test]
fn test_12_osc52_carry_is_bounded_and_safe() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    clear_process_clipboard();
    let mut wm = herdr_wm();

    // Unterminated OSC 52 followed by 2 MiB of base64 data
    let mut huge = b"\x1b]52;c;".to_vec();
    huge.extend(vec![b'A'; 2 * 1024 * 1024]);
    wm.active_pane_mut().process_pty_output(&huge);
    assert!(
        wm.active_pane().vt().osc52_carry_len() <= 1_048_576 + 64,
        "Carry must be bounded <= 1MB + 64"
    );

    // Split '?' query rejection
    let mut wm_query = herdr_wm();
    wm_query.active_pane_mut().process_pty_output(b"\x1b]52;c;");
    wm_query.active_pane_mut().process_pty_output(b"?\x07");
    wm_query.sync_clipboard_records();
    assert_eq!(get_system_clipboard(), None);

    // Smuggled CSI mid-way
    let mut wm_smuggle = herdr_wm();
    wm_smuggle.active_pane_mut().process_pty_output(b"\x1b]52;c;QQ==");
    wm_smuggle.active_pane_mut().process_pty_output(b"\x1b[2J\x07");
    wm_smuggle.sync_clipboard_records();
    // Smuggled escape aborted the sequence safely
    assert_eq!(get_system_clipboard(), None);
}

#[test]
fn test_13_oia_shows_mouse_capture_badge() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let wm_herdr = herdr_wm();
    let rendered = render_to_string(&wm_herdr, 120, 25);
    assert!(
        rendered.contains("MOUSE->APP") || rendered.contains("MOUSE"),
        "OIA should show MOUSE capture badge when child has mouse tracking. Got:\n{rendered}"
    );

    let wm_plain = WindowManager::new_headless(25, 80);
    let rendered_plain = render_to_string(&wm_plain, 120, 25);
    assert!(
        !rendered_plain.contains("MOUSE->APP"),
        "Plain pane should not show MOUSE->APP badge"
    );
}

#[test]
fn test_14_ctrl_shift_c_gui_chord() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut wm = herdr_wm();

    // In RawPtyPassthrough without selection: Ctrl+Shift+C enters copy mode
    handle_wm_key_event(
        &mut wm,
        key_event(KeyCode::Char('C'), KeyModifiers::CONTROL | KeyModifiers::SHIFT),
    )
    .unwrap();
    assert!(wm.active_pane().copy_mode().active);
}

#[test]
fn test_15_parity_test_sticky_toggle() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut wm = herdr_wm();

    // 1. Colon command: :copy mouse on
    run_colon(&mut wm, "copy mouse on");
    assert!(wm.active_pane().mouse_select_sticky());

    // 2. Colon command: :copy mouse off
    run_colon(&mut wm, "copy mouse off");
    assert!(!wm.active_pane().mouse_select_sticky());

    // 3. AutomationControlPlane (JSON-RPC wm.command)
    let mut plane = AutomationControlPlane::new_headless(25, 80);
    let rpc_line = r#"{"jsonrpc":"2.0","id":1,"method":"wm.command","params":{"command":"copy mouse on"}}"#;
    let _ = plane.handle_jsonrpc_line(rpc_line);
    assert!(plane.wm.active_pane().mouse_select_sticky());

    // 4. Command Palette catalog item exists
    let palette_found = PALETTE_CATALOG.iter().any(|item| {
        item.0.contains("copy mouse") || item.2.to_lowercase().contains("mouse select")
    });
    assert!(palette_found, "Palette catalog must contain mouse select toggle entry");
}
