use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use bilterm::input::handle_key_event;
use bilterm::pane::CompositePane;
use bilterm::pty::line_discipline::{InputRoutingMode, LineDisciplineState};
use bilterm::ui::{buffer_to_snapshot_string, CompositePaneWidget};

#[test]
fn tab_completes_unique_filesystem_path_and_colon_commands() {
    let mut pane = CompositePane::new_headless(1, 20, 80);

    // 1. Colon command completion: ":pr" + Tab -> ":prompt "
    for ch in ":pr".chars() {
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)).unwrap();
    assert_eq!(pane.prompt.text(), ":prompt ");

    // 2. Sub-argument colon completion: ":prompt bo" + Tab -> ":prompt bottom"
    for ch in "bo".chars() {
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)).unwrap();
    assert_eq!(pane.prompt.text(), ":prompt bottom");

    // 3. Relative filesystem directory completion: "echo sr" + Tab -> "echo src/"
    pane.prompt.clear_line();
    for ch in "echo sr".chars() {
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)).unwrap();
    assert_eq!(pane.prompt.text(), "echo src/");
}

#[test]
fn tab_and_shift_tab_cycle_multiple_candidates_and_render_completion_bar() {
    let mut pane = CompositePane::new_headless(1, 20, 80);

    // "Cargo." matches Cargo.lock and Cargo.toml in the repo root
    for ch in "cat Cargo.".chars() {
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }

    // First Tab: expands LCP ("Cargo.") and opens completion candidates
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)).unwrap();
    assert!(pane.prompt.completion.is_active());
    assert!(pane.prompt.completion.candidates.len() >= 2);

    // Render pane and verify completion candidates bar appears inside the floating prompt
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    CompositePaneWidget::new(&pane, true).render(area, &mut buf);
    let snap = buffer_to_snapshot_string(&buf);
    assert!(snap.contains("Cargo.lock"));
    assert!(snap.contains("Cargo.toml"));

    // Next Tab cycles to candidate 0 ("Cargo.lock"), next Tab cycles to candidate 1 ("Cargo.toml")
    let first_text = pane.prompt.text();
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)).unwrap();
    let second_text = pane.prompt.text();
    assert_ne!(first_text, second_text);

    // Shift+Tab (BackTab) cycles backward to the previous candidate
    handle_key_event(
        &mut pane,
        KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
    )
    .unwrap();
    assert_eq!(pane.prompt.text(), first_text);
}

#[test]
fn streaming_foreground_command_keeps_floating_prompt_visible_over_scrolling_background() {
    let mut pane = CompositePane::new_headless(1, 12, 60);

    // Simulate `tail -f /dev/random` running in foreground:
    // - foreground_pgid (4200) != shell_pgid (4100)
    // - canonical_mode = true, echo_enabled = true (standard cooked streaming process)
    // - osc133_executing = true (OSC 133;C emitted when command started)
    pane.line_discipline = LineDisciplineState {
        echo_enabled: true,
        canonical_mode: true,
        foreground_pgid: Some(4200),
        shell_pgid: Some(4100),
        osc133_executing: true,
    };

    // Floating command prompt MUST remain visible and accept typing while streaming!
    assert_eq!(pane.effective_routing_mode(), InputRoutingMode::CommandPrompt);
    assert!(pane.is_prompt_visible());

    // Stream binary/random garbage including C0/C1 control codes, UTF-8 multibyte, and newlines
    for chunk_idx in 0..20u8 {
        let mut noisy = Vec::new();
        noisy.extend_from_slice(format!("STREAM_LINE_{chunk_idx:02} ").as_bytes());
        // Include C1 control bytes (0x80..0x9F), bidi overrides, and random high bytes
        noisy.extend_from_slice(&[0x84, 0x85, 0x8d, 0x9b, b'A', 0xe2, 0x80, 0xae, b'\r', b'\n']);
        pane.vt.process(&noisy);
    }

    // Type a new command into the floating prompt while background is scrolling
    for ch in "ps aux".chars() {
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }
    assert_eq!(pane.prompt.text(), "ps aux");

    // Render and verify:
    // 1. Floating prompt stays intact on top with "==> ps aux"
    // 2. Underlying streaming lines are visible below the prompt
    // 3. No C0/C1 control characters or bidi overrides leak into any Ratatui Buffer cell
    let area = Rect::new(0, 0, 60, 12);
    let mut buf = Buffer::empty(area);
    CompositePaneWidget::new(&pane, true).render(area, &mut buf);

    let snap = buffer_to_snapshot_string(&buf);
    assert!(snap.contains("BILTERM 3270 // CMD [TOP]"));
    assert!(snap.contains("==> ps aux"));
    assert!(snap.contains("STREAM_LINE_"));

    for y in 0..area.height {
        for x in 0..area.width {
            let sym = buf.cell((x, y)).unwrap().symbol();
            for ch in sym.chars() {
                assert!(
                    !ch.is_control(),
                    "Buffer cell ({x},{y}) leaked control character U+{:04X}",
                    ch as u32
                );
            }
        }
    }
}

#[test]
fn dev_random_binary_flood_with_embedded_escape_traps_never_hides_prompt_or_locks_scroll_region() {
    let mut pane = CompositePane::new_headless(1, 12, 60);
    pane.prompt.set_text("echo floating-over-random");

    // Construct a 64KB `/dev/random`-like binary stream containing embedded VT100 traps:
    // - `\x1b[?1049h` and `\x1b[?47h` (would enter alt-screen and hide the floating prompt!)
    // - `\x1b[1;2r` (DECSTBM: would lock scroll region to rows 1..2 and freeze background scrolling!)
    // - `\x1b[H` (CUP: would home cursor and stop bottom scrolling!)
    // - Random binary bytes 0x00..=0xFF
    let mut random_stream = Vec::with_capacity(65536);
    for i in 0..512u32 {
        // Mix in binary control bytes (0x00, 0x01, 0x02, 0x1b) and invalid UTF-8 (0xff, 0xfe)
        random_stream.extend_from_slice(&[0x00, 0x01, 0xff, 0xfe, 0x02, 0x03]);
        if i == 10 {
            random_stream.extend_from_slice(b"\x1b[?1049h\x1b[?47h");
        }
        if i == 20 {
            random_stream.extend_from_slice(b"\x1b[1;2r\x1b[H");
        }
        random_stream.extend_from_slice(format!("RND_BLOCK_{i:04}\n").as_bytes());
    }

    pane.process_pty_output(&random_stream);

    // 1. Binary noise stream MUST NOT trigger alternate screen or hide the floating prompt
    assert!(
        !pane.vt.is_alternate_screen(),
        "Random binary stream falsely triggered DECSET alternate screen"
    );
    assert!(
        pane.is_prompt_visible(),
        "Floating prompt must remain visible during /dev/random stream"
    );

    // 2. Render pane and verify both the floating prompt AND the latest scrolled block (RND_BLOCK_0511) are visible
    let area = Rect::new(0, 0, 60, 12);
    let mut buf = Buffer::empty(area);
    CompositePaneWidget::new(&pane, true).render(area, &mut buf);
    let snap = buffer_to_snapshot_string(&buf);

    assert!(snap.contains("==> echo floating-over-random"));
    assert!(
        snap.contains("RND_BLOCK_0511"),
        "Background canvas failed to scroll to latest block (scroll region locked by binary noise):\n{snap}"
    );
}

#[test]
fn readline_non_canonical_no_echo_never_flashes_prompt_and_three_way_mode_toggle_works() {
    use bilterm::input::handle_mouse_event;
    use bilterm::pane::{PromptAnchor, PromptMode};
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    let mut pane = CompositePane::new_headless(1, 20, 80);

    // 1. Simulate Bash Readline / Zsh ZLE / Oh-My-Bash prompt subshell:
    // Readline sets !ICANON and !ECHO simultaneously while the shell itself owns the TTY (fg == sh).
    // This MUST NOT be mistaken for a `sudo` password prompt and MUST NOT hide or flash the command prompt!
    pane.line_discipline = LineDisciplineState {
        echo_enabled: false,
        canonical_mode: false,
        foreground_pgid: Some(5000),
        shell_pgid: Some(5000),
        osc133_executing: false,
    };
    assert!(!pane.line_discipline.is_password_prompt());
    assert!(pane.is_prompt_visible());
    assert_eq!(pane.effective_routing_mode(), InputRoutingMode::CommandPrompt);

    // 2. Simulate `sudo` / `sudo-rs` / `ssh` password prompt:
    // `sudo` runs in its own foreground process group (fg != sh) and disables BOTH ECHO and ICANON!
    // This MUST engage `!ECHO LOCK` and switch to RawPtyPassthrough so the password prompt captures keyboard input!
    pane.line_discipline = LineDisciplineState {
        echo_enabled: false,
        canonical_mode: false,
        foreground_pgid: Some(5001),
        shell_pgid: Some(5000),
        osc133_executing: true,
    };
    assert!(pane.line_discipline.is_password_prompt());
    assert!(!pane.is_prompt_visible());
    assert_eq!(
        pane.effective_routing_mode(),
        InputRoutingMode::RawPtyPassthrough
    );

    // 3. Genuine canonical password prompt (`ICANON && !ECHO`, e.g., `read -s` or `getpass`) also hides prompt:
    pane.line_discipline = LineDisciplineState {
        echo_enabled: false,
        canonical_mode: true,
        foreground_pgid: Some(5000),
        shell_pgid: Some(5000),
        osc133_executing: true,
    };
    assert!(pane.line_discipline.is_password_prompt());
    assert!(!pane.is_prompt_visible());

    // 4. Defense-in-depth canvas password prompt detection (`[sudo: authenticate] Password: `)
    pane.line_discipline = LineDisciplineState::default();
    pane.prompt.running_command = Some("sudo apt update".to_string());
    pane.process_pty_output(b"[sudo: authenticate] Password: ");
    assert!(pane.line_discipline.is_password_prompt());
    assert_eq!(
        pane.effective_routing_mode(),
        InputRoutingMode::RawPtyPassthrough
    );

    // Restore normal shell state
    pane.prompt.running_command = None;
    pane.line_discipline = LineDisciplineState::default();
    assert!(pane.is_prompt_visible());
    assert_eq!(pane.prompt_mode(), PromptMode::Floating);

    // 2. Test 3-way mode cycle via F4: Floating -> Fixed -> RegularTerminal -> Floating
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE)).unwrap();
    assert_eq!(pane.prompt_mode(), PromptMode::Fixed);
    assert_eq!(pane.prompt.anchor, PromptAnchor::SplitTop);
    assert!(pane.is_prompt_visible());

    handle_key_event(&mut pane, KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE)).unwrap();
    assert_eq!(pane.prompt_mode(), PromptMode::RegularTerminal);
    assert!(!pane.is_prompt_visible());
    assert_eq!(
        pane.effective_routing_mode(),
        InputRoutingMode::RawPtyPassthrough
    );

    // Pressing F4 (or Alt+E) while in RegularTerminal mode restores Floating Prompt!
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE)).unwrap();
    assert_eq!(pane.prompt_mode(), PromptMode::Floating);
    assert_eq!(pane.prompt.anchor, PromptAnchor::Top);
    assert!(pane.is_prompt_visible());

    // 3. Test :mode fixed, :mode terminal, :mode float, and OIA status bar click
    for ch in ":mode fixed".chars() {
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert_eq!(pane.prompt_mode(), PromptMode::Fixed);

    // Clicking the bottom OIA status bar (row 19) cycles Fixed -> RegularTerminal -> Floating
    let viewport = Rect::new(0, 0, 80, 20);
    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 10,
            row: 19,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    assert_eq!(pane.prompt_mode(), PromptMode::RegularTerminal);

    handle_mouse_event(
        &mut pane,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 10,
            row: 19,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    assert_eq!(pane.prompt_mode(), PromptMode::Floating);
}

