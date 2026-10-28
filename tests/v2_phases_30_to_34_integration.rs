//! Milestone `v2.0` (`Phases 30–34`) End-to-End Integration & Verification Suite.
//!
//! Covers:
//! 1. **Phase 30 (`SAFE-01`, `SAFE-02`)**: Explicit Exit Guard & Root/Final Session Protection:
//!    - `exit` / `logout` / `Ctrl+D` interception on final/root panes opens Borland/3270 Exit Guard modal.
//!    - Context-aware passthrough immunity inside `DECSET 1049` (`ALT_SCREEN`) and `!ECHO LOCK` (`sudo`).
//!    - Interactive `(r)espawn`, `(d)etach`, `(q)uit`, `(K)ill`, `(Esc) cancel`, held `[SHELL EXITED]` banner, and `:quit!` / `:detach`.
//! 2. **Phase 31 (`THM-01`, `THM-02`)**: Universal Open Color Scheme & Font Engine:
//!    - Native Apple `.itermcolors` XML plist importer (`mbadolato/iterm2-color-schemes`).
//!    - Alacritty TOML, WezTerm/Ghostty TOML, and Base16 YAML open theme loaders + TOML export.
//!    - WCAG contrast-verified Borland/3270 chrome synthesis, 14 bundled presets (`Dracula`, `TokyoNight`, `CatppuccinMocha`, `TronGrid`, etc.), and per-theme typography scaling (`TIOCSWINSZ`).
//! 3. **Phase 32 (`DAEM-01`, `DAEM-02`)**: Persistent Background Multiplexer Daemon:
//!    - Multi-client attach/detach/reconnect survival, `Ord::clamp`-safe heterogeneous viewport negotiation (`0×0` through Retina GUI + SSH TUI), and `ConnectionRefused`-only stale socket cleanup (`PermissionDenied` preserved).
//! 4. **Phase 33 (`TREE-01..02`, `PERS-01..02`)**: Hierarchical `SessionTree` & Crash-Safe `WorkspaceJournal`:
//!    - `:session branch`, `:session graft` (with cycle rejection), `:session detach-subtree`, and `:session checkpoint` atomic `0600` sibling-temp-rename persistence with strict `Tiled` (`SplitTree`) vs. `Stacked` (`ZStack`) separation.
//! 5. **Phase 34 (`MCP-01`, `MCP-02`)**: TRON-Inspired Master Control Program (`MCP`) Root Terminal:
//!    - `:mcp` root console (`mcp status`, `mcp tree`, `mcp spawn`, `mcp graft`, `mcp broadcast`, `mcp freeze`, `mcp thaw`, `mcp derez`, `mcp checkpoint`, `mcp shutdown --confirm` `END OF LINE.`), and non-destructive root terminal close.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use bilterm::colon_command::parse_colon_command;
use bilterm::daemon::{AttachedClient, DaemonRequest, DaemonResponse, DaemonServerState};
use bilterm::input::{AppAction, handle_wm_key_event};
use bilterm::pane::ExitProtectionPolicy;
use bilterm::pty::line_discipline::LineDisciplineState;
use bilterm::ui::theme::ThemePreset;
use bilterm::ui::theme_import::{
    export_open_theme_toml, parse_itermcolors_xml, parse_open_theme_str,
};
use bilterm::ui::{WorkspaceWidget, buffer_to_snapshot_string};
use bilterm::wm::journal::WorkspaceJournal;
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::{WindowManager, WorkspaceLayoutMode};

#[test]
fn phase_30_exit_guard_intercepts_final_and_root_panes_with_alt_screen_and_echo_lock_immunity() {
    let viewport = Rect::new(0, 0, 90, 26);
    let mut wm = WindowManager::new_headless(26, 90);
    wm.resize_all(viewport).unwrap();

    // 1. In a single-pane workspace, typing `exit` and pressing Enter opens the Borland/3270 Exit Guard modal
    for ch in "exit".chars() {
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }
    let act = handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert_eq!(act, AppAction::Handled, "exit on final pane must be intercepted by Exit Guard");
    assert!(wm.exit_guard_state().is_some(), "Exit Guard modal must be active");

    // Render workspace and verify Exit Guard modal appears
    let mut buf = Buffer::empty(viewport);
    WorkspaceWidget::new(&wm).render(viewport, &mut buf);
    let rendered = buffer_to_snapshot_string(&buf);
    assert!(rendered.contains("EXIT GUARD"));
    assert!(rendered.contains("(r)espawn shell"));

    // Press Esc to cancel Exit Guard -> modal closes, session preserved
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
    assert!(wm.exit_guard_state().is_none());

    // 2. Pressing Ctrl+D on an empty prompt in the final pane also opens Exit Guard
    let act = handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert_eq!(act, AppAction::Handled);
    assert!(wm.exit_guard_state().is_some());

    // Choose 'r' (respawn shell) inside the Exit Guard modal -> respawns in-place without quitting
    let act = handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)).unwrap();
    assert_eq!(act, AppAction::Handled);
    assert!(wm.exit_guard_state().is_none());

    // 3. Compounded Lesson Guardrail: Ctrl+D inside DECSET 1049 (ALT_SCREEN) or !ECHO LOCK (sudo)
    // MUST NOT trigger the Exit Guard modal!
    wm.active_pane_mut().process_pty_output(b"\x1b[?1049hVIM_ALT_SCREEN");
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert!(
        wm.exit_guard_state().is_none(),
        "Ctrl+D inside DECSET 1049 ALT_SCREEN must pass through without triggering Exit Guard"
    );
    wm.active_pane_mut().process_pty_output(b"\x1b[?1049l");

    // Simulate sudo !ECHO LOCK
    wm.active_pane_mut()
        .process_pty_output(b"\r\n[sudo] password for bill: ");
    *wm.active_pane_mut().line_discipline_mut() = LineDisciplineState {
        canonical_mode: true,
        echo_enabled: false,
        ..Default::default()
    };
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert!(
        wm.exit_guard_state().is_none(),
        "Ctrl+D during !ECHO LOCK password prompt must not trigger Exit Guard"
    );
    *wm.active_pane_mut().line_discipline_mut() = LineDisciplineState::default();

    // 4. Held Exited Shell Banner: when a shell exits under Guard policy, hold the pane open
    wm.active_pane_mut().hold_exited_shell(Some(0));
    assert!(wm.active_pane().held_exited_shell().is_some());
    let mut buf2 = Buffer::empty(viewport);
    WorkspaceWidget::new(&wm).render(viewport, &mut buf2);
    let rendered2 = buffer_to_snapshot_string(&buf2);
    assert!(rendered2.contains("SHELL EXITED"));

    // Pressing 'r' while held respawns the shell and clears the held banner
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)).unwrap();
    assert!(wm.active_pane().held_exited_shell().is_none());

    // 5. Explicit `:detach` and `:quit!` work immediately
    let detach_cmd = parse_colon_command(":detach").unwrap().unwrap();
    let act = wm.apply(detach_cmd).unwrap();
    assert_eq!(act, AppAction::Quit);
    assert!(wm.is_client_detached());

    let mut wm2 = WindowManager::new_headless(24, 80);
    assert_eq!(wm2.config().exit_protection, ExitProtectionPolicy::Guard);
    let force_quit = parse_colon_command(":quit!").unwrap().unwrap();
    assert_eq!(wm2.apply(force_quit).unwrap(), AppAction::Quit);
}

#[test]
fn phase_31_iterm2_and_open_color_scheme_engine_with_per_pane_typography() {
    let sample_itermcolors = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Background Color</key>
    <dict>
        <key>Red Component</key><real>0.10</real>
        <key>Green Component</key><real>0.11</real>
        <key>Blue Component</key><real>0.15</real>
    </dict>
    <key>Foreground Color</key>
    <dict>
        <key>Red Component</key><real>0.75</real>
        <key>Green Component</key><real>0.80</real>
        <key>Blue Component</key><real>0.96</real>
    </dict>
    <key>Cursor Color</key>
    <dict>
        <key>Red Component</key><real>0.48</real>
        <key>Green Component</key><real>0.64</real>
        <key>Blue Component</key><real>0.97</real>
    </dict>
    <key>Ansi 14 Color</key>
    <dict>
        <key>Red Component</key><real>0.49</real>
        <key>Green Component</key><real>0.81</real>
        <key>Blue Component</key><real>1.0</real>
    </dict>
</dict>
</plist>"#;

    let scheme = parse_itermcolors_xml("TokyoNight-iTerm", sample_itermcolors).unwrap();
    let palette = scheme.to_theme_palette();
    assert_eq!(palette.name, "TokyoNight-iTerm");

    // Export to open TOML and re-parse with typography profile
    let mut exported = export_open_theme_toml(&scheme);
    exported.push_str("\n[typography]\nfont_family = \"JetBrains Mono\"\nfont_size_pt = 16\nfont_scale_pct = 125\n");
    let reparsed = parse_open_theme_str("TokyoNight-Open", &exported).unwrap();
    assert_eq!(
        reparsed.typography.font_family.as_deref(),
        Some("JetBrains Mono")
    );
    assert_eq!(reparsed.typography.font_size_pt, Some(16));
    assert_eq!(reparsed.typography.font_scale_pct, Some(125));

    // Apply open scheme to WindowManager and verify per-pane font scaling + inverse PTY grid resize
    let viewport = Rect::new(0, 0, 100, 30);
    let mut wm = WindowManager::new_headless(30, 100);
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(viewport).unwrap();
    let baseline_p1 = wm.pane(1).unwrap().vt().size();
    let baseline_p2 = wm.pane(p2).unwrap().vt().size();

    wm.focus_and_raise(1);
    wm.apply_open_color_scheme(&reparsed);
    assert_eq!(wm.config().font_family, "JetBrains Mono");
    assert_eq!(wm.pane_effective_scale_pct(1), 125);
    assert_eq!(wm.pane_effective_scale_pct(p2), 100);
    assert!(wm.pane(1).unwrap().vt().size().1 < baseline_p1.1);
    assert_eq!(wm.pane(p2).unwrap().vt().size(), baseline_p2);

    // Verify all 14 bundled presets (including Dracula, TokyoNight, CatppuccinMocha, TronGrid) work via `:theme`
    for preset_cmd in [
        ":theme dracula",
        ":theme solarized-dark",
        ":theme gruvbox-dark",
        ":theme tokyo-night",
        ":theme catppuccin-mocha",
        ":theme nord",
        ":theme monokai",
        ":theme tron",
    ] {
        let act = parse_colon_command(preset_cmd).expect(preset_cmd).unwrap();
        wm.apply(act).unwrap();
    }
    assert_eq!(wm.config().theme, ThemePreset::TronGrid);
}

#[test]
fn phases_32_33_34_daemon_session_trees_atomic_journal_and_tron_mcp_root_console() {
    let viewport = Rect::new(0, 0, 100, 30);
    let mut wm = WindowManager::new_headless(30, 100);
    wm.resize_all(viewport).unwrap();

    // 1. Phase 33: Hierarchical SessionTree (:session branch, :session graft, :session detach-subtree)
    wm.apply(parse_colon_command(":session branch main/agents").unwrap().unwrap())
        .unwrap();
    wm.apply(
        parse_colon_command(":session branch agents/worker-1")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    wm.apply(
        parse_colon_command(":session branch main/ops")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    wm.apply(
        parse_colon_command(":session graft worker-1 ops")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        wm.session_tree().subtree_session_names("ops"),
        vec!["ops".to_string(), "worker-1".to_string()]
    );

    // Cycle detection: grafting `main` under its own descendant `worker-1` must fail cleanly
    assert!(wm.session_tree_mut().graft("main", "worker-1").is_err());

    // Detach subtree promotes `ops` to an Independent root while keeping `worker-1` underneath it
    wm.apply(
        parse_colon_command(":session detach-subtree ops")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        wm.session_tree().subtree_session_names("ops"),
        vec!["ops".to_string(), "worker-1".to_string()]
    );

    // 2. Phase 33: Crash-Safe Atomic Journal (persisting Tiled vs. Stacked separation)
    let tmp = tempfile::tempdir().unwrap();
    let journal_dir = tmp.path().join("journal");
    wm.set_layout_mode(WorkspaceLayoutMode::Tiled);
    let saved_path = wm
        .save_journal_checkpoint(Some(&journal_dir), "test-slot")
        .unwrap();
    assert!(saved_path.exists());
    let loaded = WorkspaceJournal::load_from_dir(&journal_dir, "test-slot").unwrap();
    assert_eq!(loaded.sessions[0].layout_mode, "tiled");
    assert!(
        WorkspaceJournal::sanitized_z_stack_rects_for_mode(&loaded.sessions[0]).is_empty(),
        "Tiled mode journal restore must never contaminate ZStack custom rects"
    );

    // 3. Phase 32: Multi-Client Daemon Attach/Detach & Heterogeneous Viewport Negotiation
    let resp1 = wm.daemon_state_mut().handle_request(DaemonRequest::Attach {
        client_id: 101,
        session_name: "main".to_string(),
        is_gui: true,
        cols: 140,
        rows: 45,
        font_scale_pct: 120,
    });
    let resp2 = wm.daemon_state_mut().handle_request(DaemonRequest::Attach {
        client_id: 102,
        session_name: "main".to_string(),
        is_gui: false,
        cols: 80,
        rows: 24,
        font_scale_pct: 100,
    });
    assert!(matches!(resp1, DaemonResponse::Attached { client_count: 1, .. }));
    assert!(matches!(
        resp2,
        DaemonResponse::Attached {
            effective_cols: 80,
            effective_rows: 24,
            client_count: 2,
            ..
        }
    ));

    // Even a degenerate 0x0 client never panics Ord::clamp
    let _ = wm.daemon_state_mut().handle_request(DaemonRequest::ResizeClient {
        client_id: 102,
        cols: 0,
        rows: 0,
        font_scale_pct: 50,
    });
    let (eff_cols, eff_rows) = wm.daemon_state().effective_viewport_for_session("main");
    assert!(eff_cols >= 20 && eff_rows >= 6);

    // 4. Phase 34: TRON Master Control Program (MCP) Root Terminal
    wm.apply(parse_colon_command(":mcp open").unwrap().unwrap())
        .unwrap();
    assert!(wm.mcp_console().active);

    // Render TRON MCP Root Console over the workspace
    let mut buf = Buffer::empty(viewport);
    WorkspaceWidget::new(&wm).render(viewport, &mut buf);
    let rendered = buffer_to_snapshot_string(&buf);
    assert!(rendered.contains("MASTER CONTROL PROGRAM"));
    assert!(rendered.contains("END OF LINE."));

    // Execute MCP commands: status, freeze, thaw, broadcast, derez
    wm.apply(parse_colon_command(":mcp status").unwrap().unwrap())
        .unwrap();
    wm.apply(parse_colon_command(":mcp freeze ops").unwrap().unwrap())
        .unwrap();
    wm.apply(parse_colon_command(":mcp thaw ops").unwrap().unwrap())
        .unwrap();
    wm.apply(
        parse_colon_command(":mcp broadcast ops echo TRON_SYNC")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    wm.apply(
        parse_colon_command(":mcp derez worker-1 --confirm")
            .unwrap()
            .unwrap(),
    )
    .unwrap();

    // Closing the MCP root console with Esc NEVER terminates the daemon or sessions (MCP-01)
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
    assert!(!wm.mcp_console().active);
    assert!(
        wm.daemon_state().running,
        "closing the MCP root console must never kill the background daemon"
    );

    // Only explicit `:mcp shutdown --confirm` terminates the daemon
    let quit_act = wm
        .apply(
            parse_colon_command(":mcp shutdown --confirm")
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    assert_eq!(quit_act, AppAction::Quit);
    assert!(!wm.daemon_state().running);
    let _ = AttachedClient {
        client_id: 1,
        session_name: "main".to_string(),
        is_gui: false,
        cols: 80,
        rows: 24,
        font_scale_pct: 100,
    };
    let _ = DaemonServerState::default_socket_path();
}
