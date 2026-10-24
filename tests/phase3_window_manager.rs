use bilterm::ui::{buffer_to_snapshot_string, WorkspaceWidget};
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::{WindowManager, WorkspaceLayoutMode};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn snapshot_hv_composite_splits() {
    let mut wm = WindowManager::new_headless(16, 80);
    let area = Rect::new(0, 0, 80, 16);

    wm.panes
        .get_mut(&1)
        .unwrap()
        .process_pty_output(b"LEFT_PANE_OUTPUT\r\n");
    wm.panes
        .get_mut(&1)
        .unwrap()
        .prompt_mut()
        .insert_str("ls -la");

    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.panes
        .get_mut(&p2)
        .unwrap()
        .process_pty_output(b"RIGHT_PANE_OUTPUT\r\n");
    wm.panes
        .get_mut(&p2)
        .unwrap()
        .prompt_mut()
        .insert_str("cargo test");
    wm.resize_all(area).unwrap();

    let backend = TestBackend::new(80, 16);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| {
            frame.render_widget(WorkspaceWidget::new(&wm), frame.area());
        })
        .expect("draw hv split");

    let rendered = buffer_to_snapshot_string(terminal.backend().buffer());
    assert!(rendered.contains("[1] PANE #1 (Z:1/2)"));
    assert!(rendered.contains("[2] PANE #2 (Z:2/2)"));
    assert!(rendered.contains("==> ls -la"));
    assert!(rendered.contains("==> cargo test"));
    insta::assert_snapshot!("hv_composite_splits", rendered);
}

#[test]
fn snapshot_z_order_stacked_cascade_and_deck_shuffle() {
    let mut wm = WindowManager::new_headless(16, 80);
    let area = Rect::new(0, 0, 80, 16);

    let _p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    let _p3 = wm.split_focused(SplitDirection::Horizontal).unwrap();
    wm.layout_mode = WorkspaceLayoutMode::Stacked;

    // Shuffle deck top-to-bottom so Pane #2 becomes topmost active window
    assert_eq!(wm.shuffle_deck_next(), Some(2));
    wm.panes
        .get_mut(&2)
        .unwrap()
        .prompt_mut()
        .insert_str("echo TOP_OF_STACK_PANE_2");
    wm.resize_all(area).unwrap();

    let backend = TestBackend::new(80, 16);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| {
            frame.render_widget(WorkspaceWidget::new(&wm), frame.area());
        })
        .expect("draw stacked cascade");

    let rendered = buffer_to_snapshot_string(terminal.backend().buffer());
    assert!(rendered.contains("[2] PANE #2 (Z:3/3)"));
    assert!(rendered.contains("==> echo TOP_OF_STACK_PANE_2"));
    insta::assert_snapshot!("z_order_stacked_cascade", rendered);
}

#[test]
fn snapshot_guake_quick_terminal_overlay() {
    let mut wm = WindowManager::new_headless(18, 80);
    let area = Rect::new(0, 0, 80, 18);

    wm.panes
        .get_mut(&1)
        .unwrap()
        .process_pty_output(b"UNDER_PANE_1\r\nUNDER_PANE_2\r\nUNDER_PANE_3\r\nUNDER_PANE_4\r\nUNDER_PANE_5\r\nUNDER_PANE_6\r\nUNDER_PANE_7\r\nUNDER_PANE_8\r\nUNDER_PANE_9\r\nUNDER_PANE_10\r\n");

    wm.toggle_quick_terminal();
    wm.quick_terminal
        .pane
        .prompt_mut()
        .insert_str("git status --short");
    wm.resize_all(area).unwrap();

    let backend = TestBackend::new(80, 18);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| {
            frame.render_widget(WorkspaceWidget::new(&wm), frame.area());
        })
        .expect("draw guake overlay");

    let rendered = buffer_to_snapshot_string(terminal.backend().buffer());
    assert!(rendered.contains("[Q] QUAKE QUICK TERM [~]"));
    assert!(rendered.contains("==> git status --short"));
    assert!(rendered.contains("UNDER_PANE_9"));
    insta::assert_snapshot!("guake_quick_terminal_overlay", rendered);
}

#[test]
fn snapshot_wide_char_cjk_occlusion_boundary() {
    let mut wm = WindowManager::new_headless(12, 60);
    let area = Rect::new(0, 0, 60, 12);

    // Fill rows 0..3 with 2-column wide CJK characters ("漢字漢字...") straddling the floating prompt borders
    let wide_row = "漢字".repeat(15);
    let payload = format!("{wide_row}\r\n{wide_row}\r\n{wide_row}\r\n{wide_row}\r\n");
    wm.panes
        .get_mut(&1)
        .unwrap()
        .process_pty_output(payload.as_bytes());
    wm.panes
        .get_mut(&1)
        .unwrap()
        .prompt_mut()
        .insert_str("echo cjk");
    wm.resize_all(area).unwrap();

    let backend = TestBackend::new(60, 12);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| {
            frame.render_widget(WorkspaceWidget::new(&wm), frame.area());
        })
        .expect("draw cjk occlusion");

    let rendered = buffer_to_snapshot_string(terminal.backend().buffer());
    assert!(rendered.contains("==> echo cjk"));
    insta::assert_snapshot!("wide_char_cjk_occlusion_boundary", rendered);
}
