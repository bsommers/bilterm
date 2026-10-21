use bilterm::ui::cursor::CaretStyle;
use bilterm::ui::glyphs::GlyphPack;
use bilterm::ui::theme::ThemePreset;
use bilterm::ui::{buffer_to_snapshot_string, WorkspaceWidget};
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::{WindowManager, WorkspaceLayoutMode};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Terminal;

#[test]
fn snapshot_borland_turbo_vision_shadows_and_fkey_bar() {
    let mut wm = WindowManager::new_headless(16, 80);
    let area = Rect::new(0, 0, 80, 16);

    wm.config.drop_shadows = true;
    wm.config.show_dos_fkey_bar = true;
    wm.layout_mode = WorkspaceLayoutMode::Stacked;

    wm.panes
        .get_mut(&1)
        .unwrap()
        .process_pty_output(b"BACKGROUND_WINDOW_TEXT_VISIBLE_UNDER_SHADOW_0123456789\r\n");
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.panes
        .get_mut(&p2)
        .unwrap()
        .prompt
        .insert_str("echo BORLAND_TURBO_VISION");
    wm.resize_all(area).unwrap();

    let backend = TestBackend::new(80, 16);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| {
            frame.render_widget(WorkspaceWidget::new(&wm), frame.area());
        })
        .expect("draw borland shadows and fkey bar");

    let rendered = buffer_to_snapshot_string(terminal.backend().buffer());
    assert!(rendered.contains("F1 Help"));
    assert!(rendered.contains("F5 Zoom"));
    assert!(rendered.contains("[TURBO-BLUE]"));
    assert!(rendered.contains("[↕][▪]"));
    assert!(rendered.contains("◢"));
    insta::assert_snapshot!("borland_turbo_vision_shadows_and_fkey_bar", rendered);
}

#[test]
fn snapshot_ibm_3279_amber_cp437_crosshair_hollow_caret() {
    let mut wm = WindowManager::new_headless(12, 80);
    let area = Rect::new(0, 0, 80, 12);

    wm.config.theme = ThemePreset::Ibm3279Amber;
    wm.config.glyphs = GlyphPack::Cp437;
    wm.config.cursor_style = CaretStyle::BlinkingHollowBlock;
    wm.config.crosshair_cursor = true;
    wm.config.show_dos_fkey_bar = true;

    wm.panes
        .get_mut(&1)
        .unwrap()
        .process_pty_output(b"MAINFRAME READY\r\nTSO LOGON\r\n");
    wm.panes.get_mut(&1).unwrap().prompt.insert_str("submit jcl");
    wm.resize_all(area).unwrap();

    let backend = TestBackend::new(80, 12);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| {
            frame.render_widget(WorkspaceWidget::new(&wm), frame.area());
        })
        .expect("draw 3279 amber crosshair");

    let buf = terminal.backend().buffer();
    // Verify the vertical cross-hair tint is present below the prompt at the cursor column (x = 1 + 1 + 4 + 10 = 16)
    assert_eq!(
        buf.cell((16, 6)).unwrap().style().bg,
        Some(Color::Rgb(51, 25, 0))
    );

    let rendered = buffer_to_snapshot_string(buf);
    assert!(rendered.contains("♦=> submit jcl▯"));
    assert!(rendered.contains("[IBM-3279-AMB]"));
    insta::assert_snapshot!("ibm_3279_amber_cp437_crosshair_hollow_caret", rendered);
}
