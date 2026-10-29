use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::input::{handle_paste_event, handle_wm_key_event};
use bilterm::mcp_console::{McpRowAction, infer_row_action};
use bilterm::pane::PromptMode;
use bilterm::pane::fuzzy_history::FuzzyHistorySearchState;
use bilterm::pty::line_discipline::InputRoutingMode;
use bilterm::wm::WindowManager;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;

const SAMPLE_ITERMCOLORS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Foreground Color</key>
    <dict>
        <key>Red Component</key><real>0.9</real>
        <key>Green Component</key><real>0.95</real>
        <key>Blue Component</key><real>1.0</real>
    </dict>
    <key>Background Color</key>
    <dict>
        <key>Red Component</key><real>0.05</real>
        <key>Green Component</key><real>0.08</real>
        <key>Blue Component</key><real>0.15</real>
    </dict>
    <key>Cursor Color</key>
    <dict>
        <key>Red Component</key><real>1.0</real>
        <key>Green Component</key><real>0.5</real>
        <key>Blue Component</key><real>0.0</real>
    </dict>
</dict>
</plist>
"#;

#[test]
fn palette_and_fuzzy_history_never_panic_on_iterm2colors_or_uppercase_unicode_queries() {
    let mut wm = WindowManager::new_headless(24, 80);

    for query in [
        "iterm2colors",
        "iTerm2Colors",
        "ITERM2COLORS",
        "itermcolors",
        ".itermcolors",
        "Spotlight ×",
        "Ctrl+X —",
    ] {
        if wm.palette_state().is_some() {
            wm.toggle_palette();
        }
        wm.toggle_palette();
        for ch in query.chars() {
            handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .unwrap();
            let _ = wm.filtered_palette_items();
        }
    }

    if wm.palette_state().is_some() {
        wm.toggle_palette();
    }
    wm.toggle_palette();
    for ch in "iterm2colors".chars() {
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }
    let items = wm.filtered_palette_items();
    assert!(
        items.iter().any(|(cmd, _, _)| *cmd == ":itermcolors"),
        "expected :itermcolors in filtered palette items for 'iterm2colors', got {items:?}"
    );

    let history = vec![
        "git status — check".to_string(),
        ":theme import Solarized Dark.itermcolors".to_string(),
        "iterm2colors × preview".to_string(),
    ];
    let mut fuzzy = FuzzyHistorySearchState {
        active: true,
        query: "iTerm2Colors".to_string(),
        ..Default::default()
    };
    fuzzy.recompute(&history);
    assert!(!fuzzy.matches.is_empty());
}

#[test]
fn view_and_set_itermcolors_via_colon_commands_and_interactive_popup() {
    let dir = tempfile::tempdir().expect("tempdir");
    let scheme_path = dir.path().join("Solarized Dark.itermcolors");
    std::fs::write(&scheme_path, SAMPLE_ITERMCOLORS_XML).expect("write .itermcolors");

    let mut wm = WindowManager::new_headless(24, 80);

    // 1. :iterm2colors / :itermcolors opens the theme catalog popup without crashing
    let cmd = parse_colon_command(":iterm2colors").unwrap().unwrap();
    assert!(!execute_wm_colon_command(&mut wm, cmd));
    assert!(wm.mcp_console().active);

    // 2. :theme import <dir> lists discovered .itermcolors in the popup and applies the scheme
    let import_dir_cmd = parse_colon_command(&format!(":theme import {}", dir.path().display()))
        .unwrap()
        .unwrap();
    assert!(!execute_wm_colon_command(&mut wm, import_dir_cmd));
    assert!(wm.config().custom_palette_override.is_some());
    assert!(
        wm.mcp_console()
            .view_lines
            .iter()
            .any(|l| l.contains("Solarized Dark.itermcolors")),
        "expected discovered .itermcolors row in popup"
    );

    // 3. Interactive row action in popup resolves `import <path with spaces>`
    let import_line = wm
        .mcp_console()
        .view_lines
        .iter()
        .find(|l| l.contains("Solarized Dark.itermcolors"))
        .expect("import row")
        .clone();
    assert_eq!(
        infer_row_action(&import_line),
        McpRowAction::ApplyTheme(format!("import {}", scheme_path.display()))
    );

    // 4. Direct `:theme <path.itermcolors>` and `:iterm2colors <path.itermcolors>` apply cleanly
    wm.config_mut().custom_palette_override = None;
    let direct_cmd = parse_colon_command(&format!(":iterm2colors {}", scheme_path.display()))
        .unwrap()
        .unwrap();
    assert!(!execute_wm_colon_command(&mut wm, direct_cmd));
    let pal = wm
        .config()
        .custom_palette_override
        .as_ref()
        .expect("custom palette applied");
    assert_eq!(pal.window_frame_bg, Color::Rgb(13, 20, 38));
}

#[test]
fn paste_bindings_and_bracketed_multiline_paste_in_raw_pty() {
    let mut wm = WindowManager::new_headless(24, 80);
    // Switch active pane to RegularTerminal (INPUT: RAW PTY)
    wm.active_pane_mut()
        .set_prompt_mode(PromptMode::RegularTerminal);
    assert_eq!(
        wm.active_pane().effective_routing_mode(),
        InputRoutingMode::RawPtyPassthrough
    );

    // Seed fallback yank_buffer so tests are deterministic even on headless CI without X11/Wayland
    wm.active_pane_mut().prompt_mut().yank_buffer = "echo from_clipboard".to_string();

    // Fix 1a: Ctrl+Shift+V in INPUT: RAW PTY injects clipboard into PTY
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(
            KeyCode::Char('V'),
            KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
        ),
    )
    .unwrap();
    assert!(!wm.active_pane().last_pty_write().is_empty());

    // Fix 1b: Shift+Insert in INPUT: RAW PTY injects clipboard into PTY
    wm.active_pane_mut().write_to_pty(b"").unwrap();
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Insert, KeyModifiers::SHIFT),
    )
    .unwrap();
    assert!(!wm.active_pane().last_pty_write().is_empty());

    // Fix 1c: Leader Ctrl+X p and Ctrl+X v in INPUT: RAW PTY inject clipboard into PTY
    wm.active_pane_mut().write_to_pty(b"").unwrap();
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
    )
    .unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE)).unwrap();
    assert!(!wm.active_pane().last_pty_write().is_empty());

    wm.active_pane_mut().write_to_pty(b"").unwrap();
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
    )
    .unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE)).unwrap();
    assert!(!wm.active_pane().last_pty_write().is_empty());

    // Fix 1d: `:paste` command injects clipboard into PTY
    wm.active_pane_mut().write_to_pty(b"").unwrap();
    let paste_cmd = parse_colon_command(":paste").unwrap().unwrap();
    assert!(!execute_wm_colon_command(&mut wm, paste_cmd));
    assert!(!wm.active_pane().last_pty_write().is_empty());

    // Fix 2: Compound DECSET `\x1b[?1049;2004h` enables bracketed paste (?2004h) in RAW PTY,
    // and multi-line paste bypasses the confirmation modal while wrapping in `\x1b[200~...\x1b[201~`.
    wm.active_pane_mut()
        .process_pty_output(b"\x1b[?1049;2004h");
    assert!(wm.active_pane().vt().is_alternate_screen());
    assert!(wm.active_pane().vt().is_bracketed_paste());

    handle_paste_event(wm.active_pane_mut(), "line1\nline2\nline3").unwrap();
    assert!(
        wm.active_pane().pending_unsafe_paste().is_none(),
        "multi-line paste must bypass confirmation modal when ?2004h is enabled"
    );
    assert_eq!(
        wm.active_pane().last_pty_write(),
        b"\x1b[200~line1\nline2\nline3\x1b[201~"
    );
}
