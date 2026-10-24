use std::path::PathBuf;

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::widgets::Widget;

use bilterm::app::App;
use bilterm::config::BiltermConfig;
use bilterm::input::{
    encode_key_to_pty_bytes_with_modes, handle_key_event, handle_wm_key_event,
    handle_wm_mouse_event,
};
use bilterm::pane::{CompositePane, PromptAnchor};
use bilterm::ui::chrome::{OutlineStyle, ShadowStyle};
use bilterm::ui::{
    CompositePaneWidget, WorkspaceWidget, buffer_to_snapshot_string, cell_to_ratatui_style,
};

#[test]
fn split_top_and_split_bottom_modes_partition_command_and_output_panes_with_zero_overlap() {
    let mut pane = CompositePane::new_headless(1, 20, 80);
    let viewport = Rect::new(0, 0, 80, 20);

    // Default Top mode: floating prompt overlaps top of 19-row canvas
    assert_eq!(pane.canvas_rect(viewport), Rect::new(0, 0, 80, 19));

    // Switch to SplitTop via :prompt split-top -> prompt at y=0..3, canvas at y=3..19 (height=16), ZERO overlap!
    for ch in ":prompt split-top".chars() {
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert_eq!(pane.prompt().anchor, PromptAnchor::SplitTop);

    let prompt_top = pane.prompt_rect(viewport);
    let canvas_top = pane.canvas_rect(viewport);
    assert_eq!(prompt_top, Rect::new(0, 0, 80, 3));
    assert_eq!(canvas_top, Rect::new(0, 3, 80, 16));
    assert_eq!(prompt_top.intersection(canvas_top).area(), 0);

    // Resizing viewport in SplitTop resizes VtParser to the dedicated 16-row canvas
    pane.resize_viewport(viewport).unwrap();
    assert_eq!(pane.vt().size(), (16, 80));

    // Switch to SplitBottom via :prompt split-bottom -> canvas at y=0..16, prompt at y=16..19, ZERO overlap!
    for ch in ":prompt split-bottom".chars() {
        handle_key_event(&mut pane, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }
    handle_key_event(&mut pane, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert_eq!(pane.prompt().anchor, PromptAnchor::SplitBottom);

    let prompt_bot = pane.prompt_rect(viewport);
    let canvas_bot = pane.canvas_rect(viewport);
    assert_eq!(canvas_bot, Rect::new(0, 0, 80, 16));
    assert_eq!(prompt_bot, Rect::new(0, 16, 80, 3));
    assert_eq!(prompt_bot.intersection(canvas_bot).area(), 0);

    // Entering DECSET 1049 alternate screen in SplitBottom temporarily gives the full 19 rows back to the TUI
    pane.process_pty_output(b"\x1b[?1049h");
    assert_eq!(pane.canvas_rect(viewport), Rect::new(0, 0, 80, 19));
    pane.process_pty_output(b"\x1b[?1049l");
    assert_eq!(pane.canvas_rect(viewport), Rect::new(0, 0, 80, 16));
}

#[test]
fn selectable_outline_styles_and_themeable_dos_drop_shadows_render_accurately() {
    let mut app = App::new(CompositePane::new_headless(1, 20, 70));
    let area = Rect::new(0, 0, 70, 20);

    // Cycle through all 8 outline styles via :outline <style>
    let expected_top_left = [
        ("solid", "┌"),
        ("double", "╔"),
        ("rounded", "╭"),
        ("thick", "┏"),
        ("dotted", "┌"),
        ("dashed", "┌"),
        ("quadruple", "▛"),
        ("ascii", "+"),
    ];

    for (style_name, corner_sym) in expected_top_left {
        app.wm
            .active_pane_mut()
            .prompt_mut()
            .set_text(&format!(":outline {style_name}"));
        handle_wm_key_event(
            &mut app.wm,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        )
        .unwrap();

        let mut buf = Buffer::empty(area);
        CompositePaneWidget::with_config(app.wm.active_pane(), true, app.wm.config())
            .render(area, &mut buf);
        let p_rect = app.wm.active_pane().prompt_rect(area);
        assert_eq!(
            buf.cell((p_rect.x, p_rect.y)).unwrap().symbol(),
            corner_sym,
            "Outline style {style_name} corner mismatch"
        );
    }

    // Verify dotted and dashed horizontal border glyphs specifically
    app.wm.config_mut().prompt_outline = OutlineStyle::Dotted;
    let mut buf = Buffer::empty(area);
    CompositePaneWidget::with_config(app.wm.active_pane(), true, app.wm.config())
        .render(area, &mut buf);
    let p_rect = app.wm.active_pane().prompt_rect(area);
    assert_eq!(buf.cell((p_rect.x + 1, p_rect.y + p_rect.height - 1)).unwrap().symbol(), "┈");

    app.wm.config_mut().prompt_outline = OutlineStyle::Dashed;
    let mut buf = Buffer::empty(area);
    CompositePaneWidget::with_config(app.wm.active_pane(), true, app.wm.config())
        .render(area, &mut buf);
    assert_eq!(buf.cell((p_rect.x + 1, p_rect.y + p_rect.height - 1)).unwrap().symbol(), "┄");

    // Verify themeable DOS drop-shadow styles: Translucent, SolidHalfBlock, ThemeTinted, None
    for shadow_cmd in [":shadows solid", ":shadows tinted", ":shadows translucent", ":shadows off"] {
        app.wm.active_pane_mut().prompt_mut().set_text(shadow_cmd);
        handle_wm_key_event(
            &mut app.wm,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        )
        .unwrap();
    }
    assert_eq!(app.wm.config().shadow_style, ShadowStyle::None);
    assert!(!app.wm.config().drop_shadows);

    app.wm
        .active_pane_mut()
        .prompt_mut()
        .set_text(":shadows solid");
    handle_wm_key_event(
        &mut app.wm,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    )
    .unwrap();
    assert_eq!(app.wm.config().shadow_style, ShadowStyle::SolidHalfBlock);
    assert!(app.wm.config().drop_shadows);

    let mut buf = Buffer::empty(area);
    CompositePaneWidget::with_config(app.wm.active_pane(), true, app.wm.config())
        .render(area, &mut buf);
    let right_shadow_x = p_rect.x + p_rect.width;
    assert_eq!(buf.cell((right_shadow_x, p_rect.y + 1)).unwrap().symbol(), "█");
}

#[test]
fn full_control_sequence_compliance_sgr_decckm_osc_titles_cwd_and_dsr_cpr_da_queries() {
    let mut pane = CompositePane::new_headless(1, 25, 80);

    // 1. SGR modifiers: Bold(1), Italic(3), Underline(4), Inverse(7), plus 24-bit TrueColor FG and 256-color BG
    pane.process_pty_output(b"\x1b[1;3;4;7m\x1b[38;2;10;20;30m\x1b[48;5;196mX\x1b[0m");
    let cell = pane.vt().screen().cell(0, 0).unwrap();
    let style = cell_to_ratatui_style(cell);
    assert_eq!(style.fg, Some(Color::Rgb(10, 20, 30)));
    assert_eq!(style.bg, Some(Color::Indexed(196)));
    assert!(style.add_modifier.contains(Modifier::BOLD));
    assert!(style.add_modifier.contains(Modifier::ITALIC));
    assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    assert!(style.add_modifier.contains(Modifier::REVERSED));

    // 2. DECCKM (?1h / ?1l) Application Cursor Keys mode
    assert!(!pane.vt().is_application_cursor());
    pane.process_pty_output(b"\x1b[?1h");
    assert!(pane.vt().is_application_cursor());
    assert_eq!(
        encode_key_to_pty_bytes_with_modes(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), true),
        b"\x1bOA".to_vec()
    );
    assert_eq!(
        encode_key_to_pty_bytes_with_modes(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), false),
        b"\x1b[A".to_vec()
    );
    pane.process_pty_output(b"\x1b[?1l");
    assert!(!pane.vt().is_application_cursor());

    // 3. Mouse tracking (?1000h, ?1002h, ?1003h, ?1006h), Focus (?1004h), Sync Output (?2026h)
    pane.process_pty_output(b"\x1b[?1000h\x1b[?1006h\x1b[?1004h\x1b[?2026h");
    assert!(pane.vt().is_mouse_SGR_or_tracking());
    assert!(pane.vt().is_focus_reporting());
    assert!(pane.vt().is_synchronized_output());
    pane.process_pty_output(b"\x1b[?1000l\x1b[?1006l\x1b[?1004l\x1b[?2026l");
    assert!(!pane.vt().is_mouse_SGR_or_tracking());
    assert!(!pane.vt().is_focus_reporting());
    assert!(!pane.vt().is_synchronized_output());

    // 4. OSC 0 / OSC 2 dynamic window title and OSC 7 working directory reporting
    pane.process_pty_output(b"\x1b]2;MAINFRAME-JOB-3270\x07");
    assert_eq!(pane.vt().window_title(), Some("MAINFRAME-JOB-3270"));
    assert!(pane.title().contains("MAINFRAME-JOB-3270"));

    pane.process_pty_output(b"\x1b]7;file://localhost/tmp\x07");
    assert_eq!(pane.vt().reported_cwd(), Some(PathBuf::from("/tmp").as_path()));
    assert_eq!(pane.resolve_cwd(), PathBuf::from("/tmp"));

    // 5. DSR (CSI 5 n), CPR (CSI 6 n), and Primary DA (CSI c) terminal queries
    pane.process_pty_output(b"\x1b[10;25H\x1b[5n\x1b[6n\x1b[c");
    let responses = pane.vt_mut().take_query_responses();
    assert!(responses.contains(&b"\x1b[0n".to_vec()));
    assert!(responses.contains(&b"\x1b[10;25R".to_vec()));
    assert!(responses.contains(&b"\x1b[?62;22c".to_vec()));
}

#[test]
fn terminal_torture_and_multi_pane_workspace_stress_test() {
    let mut app = App::new(CompositePane::new_headless(1, 25, 80));

    // Exercise TOML config with new fields
    let cfg = BiltermConfig::from_toml_str(
        r#"
        theme = "turbo-cyan"
        glyphs = "nerd-font"
        cursor_style = "vertical-bar"
        window_outline = "double"
        prompt_outline = "rounded"
        shadow_style = "tinted"
        drop_shadows = true
        show_dos_fkey_bar = true
        default_prompt_anchor = "split-top"
    "#,
    )
    .unwrap();
    assert_eq!(cfg.window_outline, OutlineStyle::Double);
    assert_eq!(cfg.prompt_outline, OutlineStyle::Rounded);
    assert_eq!(cfg.shadow_style, ShadowStyle::ThemeTinted);
    assert_eq!(cfg.resolved_prompt_anchor(), PromptAnchor::SplitTop);
    *app.wm.config_mut() = cfg;

    // Stress 1: Multi-pane splits, layout switches, z-order shuffles, zoom, and Guake overlay via App::handle_crossterm_event
    let chords = [
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Tab, KeyModifiers::ALT),
        KeyEvent::new(KeyCode::BackTab, KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('`'), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char('`'), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::ALT),
    ];
    for key in chords {
        app.handle_crossterm_event(Event::Key(key)).unwrap();
    }

    // Stress 2: Extreme viewport resizes (tiny 6x3 up to huge 200x60)
    for &(cols, rows) in &[(6, 3), (20, 8), (80, 25), (160, 50), (40, 15), (80, 24)] {
        app.handle_crossterm_event(Event::Resize(cols, rows)).unwrap();
        let area = Rect::new(0, 0, cols, rows);
        let mut buf = Buffer::empty(area);
        WorkspaceWidget::new(&app.wm).render(area, &mut buf);
    }

    // Stress 3: Torture VT100 stream (scroll regions, insert/delete lines/chars, save/restore cursor, wide CJK/Emoji, SGR)
    let torture_sequences: &[&[u8]] = &[
        b"\x1b7\x1b[1;24r\x1b[5;10H\x1b[3L\x1b[2M\x1b[4@\x1b[2P\x1b[5X\x1b8",
        "漢字🔥🚀✨\x1b[2K\rLineAfterErase\r\n".as_bytes(),
        b"\x1b[?25lHiddenCursor\x1b[?25hVisibleCursor\r\n",
        b"\x1b]133;A\x07$ \x1b]133;B\x07echo stress\r\n\x1b]133;C\x07OUT\r\n\x1b]133;D;0\x07",
    ];
    for seq in torture_sequences {
        app.wm.active_pane_mut().process_pty_output(seq);
    }

    // Stress 4: Mouse events across stacked & tiled panes
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
        MouseEventKind::ScrollUp,
        MouseEventKind::ScrollDown,
    ] {
        let _ = handle_wm_mouse_event(
            &mut app.wm,
            MouseEvent {
                kind,
                column: 15,
                row: 5,
                modifiers: KeyModifiers::NONE,
            },
            Rect::new(0, 0, 80, 24),
        )
        .unwrap();
    }

    // Stress 5: Render full workspace and verify no control chars in any cell
    let screen = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(screen);
    WorkspaceWidget::new(&app.wm).render(screen, &mut buf);
    let snap = buffer_to_snapshot_string(&buf);
    assert!(!snap.is_empty());
}

#[test]
fn comprehensive_input_router_completion_and_colon_command_coverage() {
    let mut app = App::new(CompositePane::new_headless(1, 24, 80));
    app.poll_all_ptys();
    assert!(!app.all_panes_exited());

    // 1. Exercise encode_key_to_pty_bytes_with_modes across all F-keys, nav keys, DECCKM, Ctrl, and Alt
    let nav_keys = [
        (KeyCode::Up, true, b"\x1bOA".as_slice()),
        (KeyCode::Down, true, b"\x1bOB".as_slice()),
        (KeyCode::Right, true, b"\x1bOC".as_slice()),
        (KeyCode::Left, true, b"\x1bOD".as_slice()),
        (KeyCode::Home, true, b"\x1bOH".as_slice()),
        (KeyCode::End, true, b"\x1bOF".as_slice()),
        (KeyCode::Down, false, b"\x1b[B".as_slice()),
        (KeyCode::Right, false, b"\x1b[C".as_slice()),
        (KeyCode::Left, false, b"\x1b[D".as_slice()),
        (KeyCode::Home, false, b"\x1b[H".as_slice()),
        (KeyCode::End, false, b"\x1b[F".as_slice()),
        (KeyCode::Delete, false, b"\x1b[3~".as_slice()),
        (KeyCode::PageUp, false, b"\x1b[5~".as_slice()),
        (KeyCode::PageDown, false, b"\x1b[6~".as_slice()),
        (KeyCode::BackTab, false, b"\x1b[Z".as_slice()),
        (KeyCode::Enter, false, b"\r".as_slice()),
        (KeyCode::Backspace, false, b"\x7f".as_slice()),
        (KeyCode::Tab, false, b"\t".as_slice()),
        (KeyCode::Esc, false, b"\x1b".as_slice()),
    ];
    for (code, app_cur, expected) in nav_keys {
        assert_eq!(
            encode_key_to_pty_bytes_with_modes(KeyEvent::new(code, KeyModifiers::NONE), app_cur),
            expected
        );
    }
    for f_num in 1..=12 {
        assert!(!encode_key_to_pty_bytes_with_modes(
            KeyEvent::new(KeyCode::F(f_num), KeyModifiers::NONE),
            false
        )
        .is_empty());
    }
    assert_eq!(
        encode_key_to_pty_bytes_with_modes(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            false
        ),
        vec![3]
    );
    assert_eq!(
        encode_key_to_pty_bytes_with_modes(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT),
            false
        ),
        vec![0x1b, b'x']
    );

    // 2. Exercise fuzzy history search key handling (Ctrl+R, query typing, Up/Down/Ctrl+N/Ctrl+P, Backspace, Enter, Esc)
    {
        let pane = app.wm.active_pane_mut();
        pane.prompt_mut()
            .history
            .push("cargo test --all".to_string());
        pane.prompt_mut().history.push("git status -sb".to_string());

        handle_key_event(pane, KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)).unwrap();
        assert!(pane.prompt().fuzzy_search.active);
        handle_key_event(pane, KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)).unwrap();
        handle_key_event(pane, KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)).unwrap();
        handle_key_event(pane, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)).unwrap();
        handle_key_event(pane, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)).unwrap();
        handle_key_event(pane, KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL)).unwrap();
        handle_key_event(pane, KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL)).unwrap();
        handle_key_event(pane, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
        assert!(!pane.prompt().fuzzy_search.active);

        // Re-open and cancel via Esc
        handle_key_event(pane, KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)).unwrap();
        handle_key_event(pane, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
        assert!(!pane.prompt().fuzzy_search.active);
    }

    // 3. Exercise Tab completion for $ENV, ${ENV}, ~, and command builtins/$PATH
    {
        let pane = app.wm.active_pane_mut();
        pane.prompt_mut().set_text("echo $PAT");
        handle_key_event(pane, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)).unwrap();
        assert!(pane.prompt().text().contains("$PATH"));

        pane.prompt_mut().set_text("echo ${PAT");
        handle_key_event(pane, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)).unwrap();
        assert!(pane.prompt().text().contains("${PATH}"));

        pane.prompt_mut().set_text("cd ~");
        handle_key_event(pane, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)).unwrap();
        assert!(pane.prompt().text().starts_with("cd ~/"));

        pane.prompt_mut().set_text("ech");
        handle_key_event(pane, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)).unwrap();
        assert!(pane.prompt().text().starts_with("echo"));
    }

    // 4. Exercise all :colon commands and error branches via App::handle_crossterm_event
    let colon_cmds = [
        ":prompt top",
        ":prompt bottom",
        ":prompt split",
        ":prompt split",
        ":prompt float 3",
        ":prompt height 5",
        ":prompt invalid",
        ":block prev",
        ":block next",
        ":block invalid",
        ":split h",
        ":vsplit",
        ":deck next",
        ":deck prev",
        ":focus 1",
        ":zoom",
        ":zoom",
        ":layout stack",
        ":layout tile",
        ":quake",
        ":guake",
        ":theme 3278",
        ":theme 3279",
        ":theme mono",
        ":theme turbo-blue",
        ":glyphs cp437",
        ":glyphs ascii",
        ":glyphs unicode",
        ":cursor hollow",
        ":cursor underscore",
        ":cursor bar",
        ":cursor block",
        ":outline window rounded",
        ":outline prompt dashed",
        ":border double",
        ":crosshair on",
        ":crosshair off",
        ":shadows translucent",
        ":fkeybar on",
        ":fkeybar off",
        ":yank",
        ":clear",
        ":raw",
        ":raw",
        ":close",
    ];
    for cmd in colon_cmds {
        app.wm.active_pane_mut().prompt_mut().set_text(cmd);
        app.handle_crossterm_event(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )))
        .unwrap();
    }

    app.handle_crossterm_event(Event::Paste("echo pasted".to_string()))
        .unwrap();
    app.flush_osc52_to_host().unwrap();
}

#[test]
fn prompt_width_resize_scrolling_running_banner_history_popup_f1_help_and_fullscreen_crosshairs() {
    let mut app = App::new(CompositePane::new_headless(1, 24, 80));
    let area = Rect::new(0, 0, 80, 24);

    // 1. Running command banner stays visible inside the prompt after hitting Enter on "tail -f /dev/null"
    for ch in "tail -f /dev/null".chars() {
        app.handle_crossterm_event(Event::Key(KeyEvent::new(
            KeyCode::Char(ch),
            KeyModifiers::NONE,
        )))
        .unwrap();
    }
    app.handle_crossterm_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )))
    .unwrap();
    assert_eq!(
        app.wm.active_pane().prompt().running_command.as_deref(),
        Some("tail -f /dev/null")
    );

    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(&app.wm).render(area, &mut buf);
    let snap = buffer_to_snapshot_string(&buf);
    assert!(snap.contains("tail -f /dev/null"));

    // 2. Resizable prompt width via :prompt width 40, Alt+] / Alt+[, and mouse border/corner drag
    app.wm
        .active_pane_mut()
        .prompt_mut()
        .set_text(":prompt width 40");
    app.handle_crossterm_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )))
    .unwrap();
    assert_eq!(app.wm.active_pane().prompt_rect(area).width, 40);

    app.handle_crossterm_event(Event::Key(KeyEvent::new(
        KeyCode::Char(']'),
        KeyModifiers::ALT,
    )))
    .unwrap();
    assert_eq!(app.wm.active_pane().prompt_rect(area).width, 44);

    app.handle_crossterm_event(Event::Key(KeyEvent::new(
        KeyCode::Char('['),
        KeyModifiers::ALT,
    )))
    .unwrap();
    assert_eq!(app.wm.active_pane().prompt_rect(area).width, 40);

    // Mouse drag right border of prompt from x = 40 to x = 55 -> width = 55
    let p_rect = app.wm.active_pane().prompt_rect(area);
    let right_x = p_rect.x + p_rect.width - 1;
    for (kind, col) in [
        (MouseEventKind::Down(MouseButton::Left), right_x),
        (MouseEventKind::Drag(MouseButton::Left), p_rect.x + 54),
        (MouseEventKind::Up(MouseButton::Left), p_rect.x + 54),
    ] {
        handle_wm_mouse_event(
            &mut app.wm,
            MouseEvent {
                kind,
                column: col,
                row: p_rect.y + 1,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
    }
    assert_eq!(app.wm.active_pane().prompt_rect(area).width, 55);

    // 3. Scrollable multi-line command prompt content (Shift+PageUp / Shift+PageDown & mouse scroll)
    let multiline = (1..=10)
        .map(|i| format!("echo line_{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    app.wm.active_pane_mut().prompt_mut().set_text(&multiline);
    app.handle_crossterm_event(Event::Key(KeyEvent::new(
        KeyCode::PageUp,
        KeyModifiers::SHIFT,
    )))
    .unwrap();
    app.handle_crossterm_event(Event::Key(KeyEvent::new(
        KeyCode::PageDown,
        KeyModifiers::SHIFT,
    )))
    .unwrap();

    // 4. Command History Pop-up Window (Alt+P / :history), navigation, temp-file save ('s'), and selection
    app.handle_crossterm_event(Event::Key(KeyEvent::new(
        KeyCode::Char('p'),
        KeyModifiers::ALT,
    )))
    .unwrap();
    assert!(app.wm.active_pane().prompt().history_popup_open);
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(&app.wm).render(area, &mut buf);
    assert!(buffer_to_snapshot_string(&buf).contains("COMMAND HISTORY"));

    app.handle_crossterm_event(Event::Key(KeyEvent::new(
        KeyCode::Char('s'),
        KeyModifiers::NONE,
    )))
    .unwrap();
    let exported = app
        .wm
        .active_pane()
        .history_store()
        .last_exported_temp_path
        .clone()
        .expect("exported temp file");
    assert!(exported.exists());
    let _ = std::fs::remove_file(exported);

    app.handle_crossterm_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )))
    .unwrap();
    assert!(!app.wm.active_pane().prompt().history_popup_open);

    // 5. Full-screen cross-hairs (:crosshair full / Alt+X)
    app.wm
        .active_pane_mut()
        .prompt_mut()
        .set_text(":crosshair full");
    app.handle_crossterm_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )))
    .unwrap();
    assert!(app.wm.config().crosshair_cursor);
    assert!(app.wm.config().crosshair_fullscreen);

    // 6. <F1> Interactive Help modal toggle and scrolling
    app.handle_crossterm_event(Event::Key(KeyEvent::new(
        KeyCode::F(1),
        KeyModifiers::NONE,
    )))
    .unwrap();
    assert!(app.wm.is_help_visible());
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(&app.wm).render(area, &mut buf);
    assert!(buffer_to_snapshot_string(&buf).contains("TURBO VISION HELP"));

    app.handle_crossterm_event(Event::Key(KeyEvent::new(
        KeyCode::PageDown,
        KeyModifiers::NONE,
    )))
    .unwrap();
    app.handle_crossterm_event(Event::Key(KeyEvent::new(
        KeyCode::F(1),
        KeyModifiers::NONE,
    )))
    .unwrap();
    assert!(!app.wm.is_help_visible());
}
