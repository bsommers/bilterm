use bilterm::input::{handle_wm_key_event, AppAction};
use bilterm::pty::default_shell_for_platform;
use bilterm::tmux::{
    format_send_keys_hex, format_split_window, parse_tmux_control_line, wrap_tmux_dcs_passthrough,
    TmuxControlBridge, TmuxControlEvent, TmuxEnv,
};
use bilterm::ui::cursor::CaretStyle;
use bilterm::ui::glyphs::GlyphPack;
use bilterm::ui::theme::ThemePreset;
use bilterm::wm::WindowManager;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn cross_platform_shell_resolution_linux_macos_and_windows() {
    assert_eq!(
        default_shell_for_platform("linux", Some("/bin/bash"), None),
        "/bin/bash"
    );
    assert_eq!(
        default_shell_for_platform("linux", None, None),
        "/bin/sh"
    );
    assert_eq!(
        default_shell_for_platform("macos", None, None),
        "/bin/zsh"
    );
    assert_eq!(
        default_shell_for_platform("windows", None, Some("C:\\Windows\\System32\\cmd.exe")),
        "C:\\Windows\\System32\\cmd.exe"
    );
    assert_eq!(
        default_shell_for_platform("windows", None, None),
        "powershell.exe"
    );
}

#[test]
fn tmux_control_mode_events_and_dcs_passthrough_end_to_end() {
    let env = TmuxEnv::from_env_str("/tmp/tmux-501/default,9912,2").unwrap();
    assert_eq!(env.status_badge(), "TMUX:$2");

    let osc52 = "\x1b]52;c;dGVzdA==\x07";
    assert_eq!(
        wrap_tmux_dcs_passthrough(osc52),
        "\x1bPtmux;\x1b\x1b]52;c;dGVzdA==\x07\x1b\\"
    );

    assert_eq!(
        parse_tmux_control_line("%window-add @4"),
        TmuxControlEvent::WindowAdd { window_id: 4 }
    );
    assert_eq!(
        parse_tmux_control_line("%session-changed $1 main-work"),
        TmuxControlEvent::SessionChanged {
            session_id: 1,
            name: "main-work".to_string()
        }
    );
    assert_eq!(
        parse_tmux_control_line("%begin 1700000000 1 0"),
        TmuxControlEvent::BeginBlock
    );
    assert_eq!(
        parse_tmux_control_line("%end 1700000000 1 0"),
        TmuxControlEvent::EndBlock
    );
    assert_eq!(
        format_split_window(0, true),
        "split-window -v -t %0\n"
    );
    assert_eq!(
        format_send_keys_hex(2, b"pwd\n"),
        "send-keys -t %2 -H 70 77 64 0a\n"
    );

    let mut wm = WindowManager::new_headless(20, 80);
    let mut bridge = TmuxControlBridge::new();
    bridge.handle_control_line(&mut wm, "%session-changed $0 bilterm-session");
    assert_eq!(
        bridge.active_session_name.as_deref(),
        Some("bilterm-session")
    );
    bridge.handle_control_line(&mut wm, "%output %0 LINE_FROM_TMUX_PANE_0\\015\\012");
    assert!(
        wm.panes
            .get(&1)
            .unwrap()
            .vt
            .screen()
            .contents()
            .contains("LINE_FROM_TMUX_PANE_0")
    );
}

#[test]
fn end_to_end_colon_commands_and_keyboard_chords_across_window_manager() {
    let mut wm = WindowManager::new_headless(24, 80);

    let run_colon = |wm: &mut WindowManager, cmd: &str| {
        wm.active_pane_mut().prompt.set_text(cmd);
        handle_wm_key_event(wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap()
    };

    assert_eq!(run_colon(&mut wm, ":split v"), AppAction::Handled);
    assert_eq!(wm.panes.len(), 2);

    assert_eq!(run_colon(&mut wm, ":theme 3278"), AppAction::Handled);
    assert_eq!(wm.config.theme, ThemePreset::Ibm3278Green);

    assert_eq!(run_colon(&mut wm, ":glyphs nerd-font"), AppAction::Handled);
    assert_eq!(wm.config.glyphs, GlyphPack::NerdFont);

    assert_eq!(run_colon(&mut wm, ":cursor underscore"), AppAction::Handled);
    assert_eq!(wm.config.cursor_style, CaretStyle::Underscore);

    assert_eq!(run_colon(&mut wm, ":crosshair on"), AppAction::Handled);
    assert!(wm.config.crosshair_cursor);

    assert_eq!(run_colon(&mut wm, ":shadows on"), AppAction::Handled);
    assert!(wm.config.drop_shadows);

    assert_eq!(run_colon(&mut wm, ":fkeybar on"), AppAction::Handled);
    assert!(wm.config.show_dos_fkey_bar);

    assert_eq!(run_colon(&mut wm, ":quake"), AppAction::Handled);
    assert!(wm.quick_terminal.visible);

    assert_eq!(run_colon(&mut wm, ":quit"), AppAction::Quit);
}
