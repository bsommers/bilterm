use super::*;

#[test]
fn large_pty_bursts_are_processed_without_dropping_mode_changes() {
    let mut pane = CompositePane::new_headless(1, 25, 80);
    // A mode switch at the front of a >256 KB burst must never be discarded.
    let mut events = vec![PtyEvent::Output(b"\x1b[?1049h".to_vec())];
    for _ in 0..40 {
        events.push(PtyEvent::Output(vec![b'x'; 8192]));
    }
    pane.ingest_pty_events(events);
    assert!(pane.vt.is_alternate_screen());
    assert!(pane.vt.total_bytes_processed() >= 40 * 8192);
}

#[test]
fn moving_or_hiding_prompt_never_resizes_canvas_or_emits_sigwinch() {
    let mut pane = CompositePane::new_headless(1, 25, 80);
    let viewport = Rect::new(0, 0, 80, 25);
    assert_eq!(pane.canvas_rect(viewport), Rect::new(0, 0, 80, 24));

    // Move prompt Top -> Bottom -> Floating: canvas size never changes
    pane.set_prompt_anchor(PromptAnchor::Bottom);
    assert_eq!(pane.prompt_rect(viewport).y, 21);
    assert!(!pane.resize_viewport(viewport).unwrap());

    pane.move_prompt_vertical(-5, 25);
    assert_eq!(pane.prompt.anchor, PromptAnchor::Floating { y_offset: 16 });
    assert!(!pane.resize_viewport(viewport).unwrap());

    pane.toggle_manual_raw_override();
    assert!(!pane.is_prompt_visible());
    assert!(!pane.resize_viewport(viewport).unwrap());
}

#[test]
fn alternate_screen_and_password_lock_auto_hide_floating_prompt() {
    let mut pane = CompositePane::new_headless(1, 25, 80);
    assert!(pane.is_prompt_visible());
    assert_eq!(pane.effective_routing_mode(), InputRoutingMode::CommandPrompt);

    // Enter DECSET 1049 alternate screen (e.g., vim / htop)
    pane.process_pty_output(b"\x1b[?1049h");
    assert!(!pane.is_prompt_visible());
    assert_eq!(
        pane.effective_routing_mode(),
        InputRoutingMode::RawPtyPassthrough
    );

    // Exit DECSET 1049 alternate screen -> prompt automatically restores
    pane.process_pty_output(b"\x1b[?1049l");
    assert!(pane.is_prompt_visible());

    // Simulate sudo !ECHO password prompt -> prompt hides and routes to RawPtyPassthrough
    pane.line_discipline.echo_enabled = false;
    assert!(!pane.is_prompt_visible());
    assert_eq!(
        pane.effective_routing_mode(),
        InputRoutingMode::RawPtyPassthrough
    );
}

#[test]
fn multiline_editing_undo_redo_and_word_navigation_work() {
    let mut prompt = FloatingPromptState::default();
    prompt.insert_str("echo first");
    prompt.insert_newline();
    prompt.insert_str("echo second");
    assert_eq!(prompt.lines.len(), 2);
    assert_eq!(prompt.text(), "echo first\necho second");

    // Undo removes "echo second"
    assert!(prompt.undo());
    assert_eq!(prompt.text(), "echo first\n");
    assert!(prompt.redo());
    assert_eq!(prompt.text(), "echo first\necho second");

    // Word navigation and kill/yank
    prompt.move_word_left();
    assert_eq!(prompt.cursor_col, 5);
    prompt.kill_to_end_of_line();
    assert_eq!(prompt.lines[1], "echo ");
    assert_eq!(prompt.yank_buffer, "second");
    prompt.yank();
    assert_eq!(prompt.lines[1], "echo second");
}

#[test]
fn submit_command_and_submit_prompt_enforce_identical_invariants() {
    let mut pane = CompositePane::new_headless(1, 25, 80);

    pane.prompt_mut().set_text("echo from_prompt");
    let submitted = pane.submit_prompt().expect("submit_prompt");
    assert_eq!(submitted, "echo from_prompt");
    assert_eq!(pane.prompt().text(), "");
    assert_eq!(pane.prompt().history, vec!["echo from_prompt".to_string()]);
    assert_eq!(pane.prompt().running_command.as_deref(), Some("echo from_prompt"));
    assert_eq!(pane.vt().command_blocks().len(), 1);

    pane.submit_command("echo programmatic").expect("submit_command");
    assert_eq!(
        pane.prompt().history,
        vec![
            "echo from_prompt".to_string(),
            "echo programmatic".to_string()
        ]
    );
    assert_eq!(
        pane.prompt().running_command.as_deref(),
        Some("echo programmatic")
    );
    assert_eq!(pane.vt().command_blocks().len(), 2);

    pane.set_read_only(true);
    assert!(pane.submit_command("echo blocked").is_err());
    pane.prompt_mut().set_text("echo blocked_prompt");
    assert!(pane.submit_prompt().is_err());
    assert_eq!(pane.vt().command_blocks().len(), 2);
}
