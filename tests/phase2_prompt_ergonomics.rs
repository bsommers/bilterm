use bilterm::pane::CompositePane;
use bilterm::ui::{buffer_to_snapshot_string, CompositePaneWidget};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn snapshot_multiline_syntax_highlighted_prompt() {
    let mut pane = CompositePane::new_headless(1, 10, 60);
    pane.process_pty_output(b"$ echo ready\r\nready\r\nUNDER_LINE_1\r\nUNDER_LINE_2\r\nUNDER_LINE_3\r\nUNDER_LINE_4\r\n");
    pane.prompt_mut()
        .insert_str("git status --short |\ngrep \"modified\" $USER");

    let backend = TestBackend::new(60, 10);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| {
            let area = frame.area();
            frame.render_widget(CompositePaneWidget::new(&pane, true), area);
        })
        .expect("draw multiline prompt");

    let rendered = buffer_to_snapshot_string(terminal.backend().buffer());
    assert!(rendered.contains("==> git status --short |"));
    assert!(rendered.contains("... grep \"modified\" $USER"));
    assert!(rendered.contains("[▲][▼]"));
    insta::assert_snapshot!("multiline_syntax_highlighted_prompt", rendered);
}

#[test]
fn snapshot_fuzzy_history_search_prompt() {
    let mut pane = CompositePane::new_headless(1, 10, 60);
    pane.prompt_mut().history = vec![
        "cargo build --release".to_string(),
        "git log --oneline -n 5".to_string(),
        "cargo test --all-targets".to_string(),
    ];
    pane.prompt_mut().open_fuzzy_search();
    pane.prompt_mut().push_fuzzy_query_char('c');
    pane.prompt_mut().push_fuzzy_query_char('t');

    let backend = TestBackend::new(60, 10);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| {
            let area = frame.area();
            frame.render_widget(CompositePaneWidget::new(&pane, true), area);
        })
        .expect("draw fuzzy history prompt");

    let rendered = buffer_to_snapshot_string(terminal.backend().buffer());
    assert!(rendered.contains("(reverse-i-search)`ct':"));
    assert!(rendered.contains("> cargo test --all-targets"));
    insta::assert_snapshot!("fuzzy_history_search_prompt", rendered);
}

#[test]
fn snapshot_osc133_command_block_jump_oia() {
    let mut pane = CompositePane::new_headless(1, 10, 60);
    pane.vt_mut().record_command_submission("echo BLOCK_ONE");
    for i in 0..6 {
        pane.process_pty_output(format!("BLOCK_ONE_LINE_{i}\r\n").as_bytes());
    }
    pane.vt_mut().record_command_submission("echo BLOCK_TWO");
    for i in 0..6 {
        pane.process_pty_output(format!("BLOCK_TWO_LINE_{i}\r\n").as_bytes());
    }

    // Jump back to BLOCK_TWO then BLOCK_ONE
    pane.vt_mut().jump_to_prev_command_block();
    pane.vt_mut().jump_to_prev_command_block();

    let backend = TestBackend::new(60, 10);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| {
            let area = frame.area();
            frame.render_widget(CompositePaneWidget::new(&pane, true), area);
        })
        .expect("draw block jump oia");

    let rendered = buffer_to_snapshot_string(terminal.backend().buffer());
    assert!(rendered.contains("BLK 1/2"));
    insta::assert_snapshot!("osc133_command_block_jump_oia", rendered);
}
