use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use insta::assert_snapshot;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use bilterm::input::handle_key_event;
use bilterm::pane::{CompositePane, PromptAnchor};
use bilterm::pty::PtyConfig;
use bilterm::ui::{buffer_to_snapshot_string, CompositePaneWidget};

fn render_pane_to_string(pane: &CompositePane, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| {
            let area = frame.area();
            frame.render_widget(CompositePaneWidget::new(pane, true), area);
        })
        .expect("draw");
    buffer_to_snapshot_string(terminal.backend().buffer())
}

#[test]
fn snapshot_top_pinned_prompt_over_scrolling_output_canvas() {
    let mut pane = CompositePane::new_headless(1, 10, 64);
    for idx in 0..9 {
        pane.process_pty_output(format!("OUTPUT_ROW_{idx:02}: system telemetry nominal\r\n").as_bytes());
    }
    pane.prompt_mut().input = "status --all".to_string();
    pane.prompt_mut().cursor_char_idx = pane.prompt().input.len();

    let rendered = render_pane_to_string(&pane, 64, 10);
    // Rows 0..3 are occluded by the top-pinned floating prompt; rows 3..9 remain visible underneath
    assert!(!rendered.contains("OUTPUT_ROW_00"));
    assert!(rendered.contains("OUTPUT_ROW_04"));
    assert!(rendered.contains("OUTPUT_ROW_08"));
    assert_snapshot!("top_pinned_composite_pane", rendered);
}

#[test]
fn snapshot_bottom_pinned_and_alternate_screen_auto_hide() {
    let mut pane = CompositePane::new_headless(1, 10, 64);
    for idx in 0..9 {
        pane.process_pty_output(format!("CANVAS_ROW_{idx:02}: scrolling underneath\r\n").as_bytes());
    }
    pane.set_prompt_anchor(PromptAnchor::Bottom);
    pane.prompt_mut().input = "vim Cargo.toml".to_string();

    let bottom_rendered = render_pane_to_string(&pane, 64, 10);
    // Top rows are now visible while bottom rows are occluded by the bottom-pinned prompt
    assert!(bottom_rendered.contains("CANVAS_ROW_01"));
    assert_snapshot!("bottom_pinned_composite_pane", bottom_rendered);

    // Enter DECSET 1049 alternate screen: floating prompt auto-hides and OIA switches to ALT SCR + RAW PTY
    pane.process_pty_output(b"\x1b[?1049h\x1b[2J\x1b[H~ VIM - Vi IMproved (Alternate Screen)\r\n~");
    let alt_rendered = render_pane_to_string(&pane, 64, 10);
    assert!(!alt_rendered.contains("BILTERM 3270 // CMD"));
    assert!(alt_rendered.contains("ALT SCR"));
    assert!(alt_rendered.contains("INPUT: RAW PTY"));
    assert_snapshot!("alternate_screen_auto_hide_pane", alt_rendered);

    // Exit DECSET 1049 alternate screen: floating prompt automatically reappears
    pane.process_pty_output(b"\x1b[?1049l");
    let restored = render_pane_to_string(&pane, 64, 10);
    assert!(restored.contains("BILTERM 3270 // CMD [BOT]"));
    assert!(restored.contains("INPUT: CMD BOX"));
}

#[test]
fn snapshot_password_no_echo_lock_hides_prompt_and_protects_input() {
    let mut pane = CompositePane::new_headless(1, 8, 64);
    pane.process_pty_output(b"[sudo] password for operator: ");
    pane.line_discipline_mut().echo_enabled = false;

    for ch in "hunter2".chars() {
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }

    let rendered = render_pane_to_string(&pane, 64, 8);
    assert!(!rendered.contains("hunter2"));
    assert!(rendered.contains("!ECHO LOCK"));
    assert!(rendered.contains("INPUT: RAW PTY"));
    assert_snapshot!("password_no_echo_lock_pane", rendered);
}

#[test]
fn moving_and_hiding_prompt_on_live_pty_pane_emits_zero_sigwinch() {
    let mut cfg = PtyConfig::default();
    if !cfg!(windows) {
        cfg.program = "/bin/sh".to_string();
    }
    cfg.rows = 25;
    cfg.cols = 80;

    let mut pane = CompositePane::spawn(1, cfg).expect("spawn composite pane");
    let viewport = Rect::new(0, 0, 80, 25);
    assert_eq!(pane.pty().unwrap().sigwinch_count(), 0);

    // Move floating prompt across Top, Bottom, Floating, and toggle visibility
    pane.set_prompt_anchor(PromptAnchor::Bottom);
    pane.resize_viewport(viewport).unwrap();
    pane.move_prompt_vertical(-6, 25);
    pane.resize_viewport(viewport).unwrap();
    pane.toggle_manual_raw_override();
    pane.resize_viewport(viewport).unwrap();
    pane.toggle_manual_raw_override();
    pane.resize_viewport(viewport).unwrap();

    // Zero SIGWINCH signals emitted during all intra-pane prompt operations!
    assert_eq!(pane.pty().unwrap().sigwinch_count(), 0);

    // Only an actual outer terminal viewport change emits SIGWINCH
    pane.resize_viewport(Rect::new(0, 0, 100, 30)).unwrap();
    assert_eq!(pane.pty().unwrap().sigwinch_count(), 1);
}
