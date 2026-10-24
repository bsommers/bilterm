use bilterm::config::BiltermConfig;
use bilterm::input::{handle_wm_key_event, AppAction};
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer, WorkspaceFrameRenderer};
use bilterm::ui::theme::ThemePreset;
use bilterm::wm::WindowManager;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

#[test]
fn v060_command_palette_custom_keybindings_and_crt_scanlines_work() {
    let mut wm = WindowManager::new_headless(24, 80);

    // 1. Custom [keybindings] in config.toml dispatches :theme 3279 on F7
    let cfg = BiltermConfig::from_toml_str(
        r#"
        crt_scanlines = true
        triggers = ["FATAL_ERR"]
        [keybindings]
        "F7" = ":theme 3279"
        "#,
    )
    .expect("valid v0.6.0 config.toml");
    wm.config = cfg;
    assert!(wm.config.crt_scanlines);

    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.config.theme, ThemePreset::Ibm3279Amber);

    // 2. Open Fuzzy Command Palette via Ctrl+P, filter for "broadcast", and press Enter
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert!(wm.palette_visible);

    for ch in "bcast".chars() {
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
        )
        .unwrap();
    }
    let filtered = wm.filtered_palette_items();
    assert!(!filtered.is_empty());
    assert_eq!(filtered[0].0, ":broadcast");

    // Render workspace with Command Palette open
    let viewport = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(viewport);
    WorkspaceWidget::new(&wm).render(viewport, &mut buf);
    let snap = bilterm::ui::buffer_to_snapshot_string(&buf);
    assert!(snap.contains("COMMAND PALETTE"));

    // Press Enter to execute :broadcast from the Command Palette
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert!(!wm.palette_visible);
    assert!(wm.broadcast_input);

    // 3. Verify GPU rasterizer applies CRT scanlines
    let mut gpu_renderer = GuiWorkspaceRenderer::new(8, 16);
    let mut gpu_surface = GuiFrameSurface::new(8, 16);
    gpu_renderer.render_workspace(&wm, viewport, &mut gpu_surface);
    assert!(gpu_surface.crt_scanlines);
    let (w, h, rgba) = gpu_surface.rasterize_rgba();
    assert_eq!((w, h), (640, 384));
    assert!(!rgba.is_empty());
}

#[test]
fn v070_osc9_osc777_notifications_triggers_and_asciicast_v2_recorder_work() {
    let mut wm = WindowManager::new_headless(20, 80);

    // 1. Emit OSC 9 notification and OSC 777 notification
    wm.active_pane_mut().process_pty_output(
        b"\x1b]9;Build succeeded\x07\x1b]777;notify;CI-GATE;All 82 tests green\x07FATAL_ERR in module X\r\n",
    );
    assert_eq!(wm.active_pane().vt().notifications().len(), 2);
    assert!(wm.active_pane().vt().unread_bell());
    assert_eq!(wm.active_pane().vt().notifications()[1].title, "CI-GATE");
    assert_eq!(
        wm.active_pane().vt().notifications()[1].body,
        "All 82 tests green"
    );

    // 2. Add output highlight trigger via :trigger add FATAL_ERR
    wm.active_pane_mut()
        .prompt_mut()
        .set_text(":trigger add FATAL_ERR");
    assert_eq!(
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap(),
        AppAction::Handled
    );
    assert_eq!(wm.config.triggers, vec!["FATAL_ERR".to_string()]);

    // Render workspace and verify 🔔 notification badge appears in OIA bar
    let viewport = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(viewport);
    WorkspaceWidget::new(&wm).render(viewport, &mut buf);
    let snap = bilterm::ui::buffer_to_snapshot_string(&buf);
    assert!(snap.contains("CI-GATE: All 82 tests green"));

    // Clear notifications via :notifications clear
    wm.active_pane_mut()
        .prompt_mut()
        .set_text(":notifications clear");
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert!(wm.active_pane().vt().notifications().is_empty());
    assert!(!wm.active_pane().vt().unread_bell());

    // 3. Asciicast v2 (.cast) recording roundtrip via :record start <path> and :record stop
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let cast_path = temp_dir.path().join("demo.cast");
    wm.active_pane_mut()
        .prompt_mut()
        .set_text(&format!(":record start {}", cast_path.display()));
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert!(wm.active_pane().vt().is_recording());

    wm.active_pane_mut()
        .process_pty_output(b"Hello Asciinema v2!\r\n");

    wm.active_pane_mut().prompt_mut().set_text(":record stop");
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert!(!wm.active_pane().vt().is_recording());

    let cast_contents = std::fs::read_to_string(&cast_path).expect("cast file written");
    let mut lines = cast_contents.lines();
    let header = lines.next().expect("asciicast v2 header");
    assert!(header.contains("\"version\":2"));
    let event_line = lines.next().expect("asciicast v2 event");
    assert!(event_line.contains("\"o\""));
    assert!(event_line.contains("Hello Asciinema v2!"));
}
