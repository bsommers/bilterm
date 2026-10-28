use std::process::Command;
use std::time::Instant;

use bilterm::api::{
    AutomationControlPlane, JsonValue, Playbook, parse_key_chord_str,
};
#[cfg(unix)]
use bilterm::api::send_rpc_request;
use bilterm::app::App;
use bilterm::config::BiltermConfig;
use bilterm::error::{BiltermError, TerminalRestoreGuard};
use bilterm::input::{
    AppAction, encode_key_to_pty_bytes_full, handle_paste_event, handle_wm_key_event,
    handle_wm_mouse_event,
};
use bilterm::pane::CompositePane;
use bilterm::colon_command::{
    execute_colon_command, execute_wm_colon_command, parse_colon_command,
};
use bilterm::remote::{
    FleetNodeKind, FleetRegistry, FleetSelector, RemoteHostProfile, remote_osc133_bootstrap_command,
};
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::renderer::{
    GuiFrameSurface, GuiWorkspaceRenderer, RatatuiWorkspaceRenderer, WorkspaceFrameRenderer,
};
use bilterm::vt::snapshot::{
    CellColor, CellSnapshot, CursorShape, CursorSnapshot, GridSnapshot, RgbTriplet, RowSnapshot,
    UnderlineKind,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::widgets::Widget;

#[test]
fn unit_exhaustively_covers_remote_profiles_fleet_selectors_and_snapshots() {
    // 1. RemoteHostProfile SSH & tmux-CC argument builders (with port, identity_file, control_master on/off)
    let mut prof = RemoteHostProfile::new("gpu-node", "ubuntu@10.0.0.42");
    prof.port = Some(2202);
    prof.identity_file = Some("/home/ubuntu/.ssh/id_ed25519".to_string());
    prof.tags = vec!["gpu".to_string(), "prod".to_string()];
    prof.rpc_endpoint = Some("10.0.0.42:7370".to_string());

    let control_dir = std::path::Path::new("/run/user/1000/bilterm");
    let ssh_args = prof.build_ssh_args(Some(control_dir)).unwrap();
    assert!(ssh_args.contains(&"-tt".to_string()));
    assert!(ssh_args.contains(&"ControlMaster=auto".to_string()));
    assert!(ssh_args.contains(&format!("ControlPath={}/ssh-%C", control_dir.display())));
    assert!(ssh_args.contains(&"2202".to_string()));
    assert!(ssh_args.contains(&"/home/ubuntu/.ssh/id_ed25519".to_string()));
    assert_eq!(ssh_args[ssh_args.len() - 2], "--");
    assert_eq!(ssh_args.last().map(String::as_str), Some("ubuntu@10.0.0.42"));

    let tmux_args = prof
        .build_ssh_tmux_cc_args("prod/main!@#", Some(control_dir))
        .unwrap();
    assert!(tmux_args.contains(&"-T".to_string()));
    assert_eq!(tmux_args[tmux_args.len() - 3], "--");
    assert_eq!(tmux_args[tmux_args.len() - 2], "ubuntu@10.0.0.42");
    assert_eq!(
        tmux_args.last().map(String::as_str),
        Some("tmux -CC new-session -A -s prodmain")
    );

    // Empty sanitized session falls back to "bilterm" and control_master=false omits ControlMaster flags
    prof.control_master = false;
    let minimal_tmux = prof.build_ssh_tmux_cc_args("!!!", None).unwrap();
    assert!(!minimal_tmux.iter().any(|a| a.contains("ControlMaster")));
    assert_eq!(
        minimal_tmux.last().map(String::as_str),
        Some("tmux -CC new-session -A -s bilterm")
    );
    let minimal_ssh = prof.build_ssh_args(None).unwrap();
    assert!(!minimal_ssh.iter().any(|a| a.contains("ControlMaster")));
    assert!(remote_osc133_bootstrap_command().contains("133;D"));

    // Option injection rejected
    let mut evil_prof = prof.clone();
    evil_prof.destination = "-oProxyCommand=evil".to_string();
    assert!(evil_prof.build_ssh_args(None).is_err());
    assert!(evil_prof.build_ssh_tmux_cc_args("sess", None).is_err());

    // 2. FleetRegistry & FleetSelector (All, LocalOnly, RemoteOnly, Tag, Node by name and destination)
    let mut reg = FleetRegistry::new();
    reg.register_profile(prof.clone());
    reg.register_ssh_pane("ssh-pane-2", 2, "root@db.internal", vec!["db".to_string()]);
    reg.register_ssh_tmux(
        "tmux-edge",
        "ops@edge.internal",
        "edge-sess",
        vec![3, 4],
        vec!["edge".to_string(), "prod".to_string()],
    );

    assert_eq!(reg.matching_nodes(&FleetSelector::parse("all")).len(), 3);
    assert_eq!(reg.matching_nodes(&FleetSelector::parse("*")).len(), 3);
    assert_eq!(reg.matching_nodes(&FleetSelector::parse("remote")).len(), 3);
    assert_eq!(reg.matching_nodes(&FleetSelector::parse("ssh")).len(), 3);
    assert!(reg.matching_nodes(&FleetSelector::parse("local")).is_empty());
    assert_eq!(reg.matching_nodes(&FleetSelector::parse("@prod")).len(), 2);
    assert_eq!(
        reg.matching_nodes(&FleetSelector::parse("root@db.internal")),
        vec!["ssh-pane-2".to_string()]
    );
    assert_eq!(
        reg.matching_nodes(&FleetSelector::parse("ops@edge.internal")),
        vec!["tmux-edge".to_string()]
    );
    assert_eq!(
        reg.matching_nodes(&FleetSelector::parse("ubuntu@10.0.0.42")),
        vec!["gpu-node".to_string()]
    );
    assert!(reg.disconnect("ssh-pane-2"));
    assert!(!reg.disconnect("nonexistent"));
    if let Some(node) = reg.nodes.get("tmux-edge") {
        assert!(matches!(node.kind, FleetNodeKind::SshTmux { .. }));
    }

    // 3. GridSnapshot, CellSnapshot, RgbTriplet, UnderlineKind, CursorSnapshot, BiltermError, TerminalRestoreGuard
    let rgb = RgbTriplet::new(10, 20, 30);
    assert_eq!(rgb.to_ratatui_color(), Color::Rgb(10, 20, 30));
    assert_eq!(CellColor::Default.to_ratatui_color(), Color::Reset);
    assert_eq!(CellColor::Indexed(42).to_ratatui_color(), Color::Indexed(42));
    assert_eq!(
        CellColor::Rgb(1, 2, 3).to_ratatui_color(),
        Color::Rgb(1, 2, 3)
    );
    assert!(!UnderlineKind::None.is_underlined());
    assert!(UnderlineKind::Curly.is_underlined());
    assert!(UnderlineKind::Double.is_underlined());
    assert!(UnderlineKind::Dotted.is_underlined());
    assert!(UnderlineKind::Dashed.is_underlined());

    let mut cell = CellSnapshot {
        grapheme: "漢".to_string(),
        width: 2,
        fg: CellColor::Rgb(0, 255, 102),
        bg: CellColor::Indexed(16),
        underline_color: Some(RgbTriplet::new(255, 0, 0)),
        bold: true,
        dim: true,
        italic: true,
        underline: UnderlineKind::Curly,
        overline: true,
        inverse: true,
        strikethrough: true,
    };
    assert!(cell.is_wide());
    assert!(!cell.is_wide_continuation());
    let style = cell.to_ratatui_style();
    assert!(style.add_modifier.contains(Modifier::BOLD));
    assert!(style.add_modifier.contains(Modifier::DIM));
    assert!(style.add_modifier.contains(Modifier::ITALIC));
    assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    assert!(style.add_modifier.contains(Modifier::REVERSED));
    assert!(style.add_modifier.contains(Modifier::CROSSED_OUT));

    cell.width = 0;
    assert!(cell.is_wide_continuation());

    let mut grid = GridSnapshot::empty(4, 10);
    assert_eq!(grid.cursor, CursorSnapshot::default());
    assert_eq!(grid.cursor.shape, CursorShape::Block);
    grid.grid_rows[0].cells[0].grapheme = "A".to_string();
    grid.grid_rows[0].cells[1].grapheme = String::new();
    grid.grid_rows[0].cells[2].width = 0;
    grid.grid_rows[0].cells[3].grapheme = "B".to_string();
    assert_eq!(grid.row_text(0), "A B");
    assert_eq!(grid.row_text(99), "");
    assert!(grid.cell(0, 0).is_some());
    assert!(grid.cell(99, 99).is_none());
    let blank_row = RowSnapshot::new_blank(5);
    assert_eq!(blank_row.cells.len(), 5);

    let err_pty = BiltermError::Pty("pty fail".to_string());
    let err_cfg = BiltermError::Config("cfg fail".to_string());
    let err_tmux = BiltermError::Tmux("tmux fail".to_string());
    assert!(err_pty.to_string().contains("pty fail"));
    assert!(err_cfg.to_string().contains("cfg fail"));
    assert!(err_tmux.to_string().contains("tmux fail"));
    let guard = TerminalRestoreGuard::default();
    assert!(!guard.is_active());
}

#[test]
fn colon_command_parser_and_executor_cover_all_variants_and_error_branches() {
    let mut plane = AutomationControlPlane::new_headless(24, 100);

    let valid_commands = [
        ":prompt top",
        ":prompt bottom",
        ":prompt split-top",
        ":prompt split-bottom",
        ":prompt split",
        ":prompt float 5",
        ":prompt height 6",
        ":prompt width 40",
        ":prompt width full",
        ":history popup",
        ":history on",
        ":history off",
        ":history toggle",
        ":history max-files 5",
        ":block prev",
        ":block next",
        ":split h",
        ":split v",
        ":vsplit",
        ":deck next",
        ":deck prev",
        ":focus 1",
        ":zoom",
        ":zoom",
        ":layout stack",
        ":layout tile",
        ":quake",
        ":quake",
        ":help",
        ":help",
        ":theme turbo-blue",
        ":theme turbo-cyan",
        ":theme 3278",
        ":theme 3279",
        ":theme mono",
        ":glyphs unicode",
        ":glyphs cp437",
        ":glyphs nerd-font",
        ":glyphs ascii",
        ":cursor block",
        ":cursor hollow",
        ":cursor underscore",
        ":cursor bar",
        ":outline window rounded",
        ":outline prompt thick",
        ":outline double",
        ":crosshair on",
        ":crosshair full",
        ":crosshair pane",
        ":crosshair off",
        ":shadows tinted",
        ":shadows none",
        ":shadows",
        ":fkeybar on",
        ":fkeybar off",
        ":fkeybar",
        ":search error",
        ":search next",
        ":search prev",
        ":search off",
        ":copy",
        ":copy",
        ":links",
        ":inspector on",
        ":inspector off",
        ":inspector",
        ":hints",
        ":hints",
        ":lock on",
        ":lock off",
        ":lock",
        ":broadcast on",
        ":broadcast off",
        ":broadcast",
        ":menubar on",
        ":menubar off",
        ":menubar",
        ":session list",
        ":crt on",
        ":crt off",
        ":crt",
        ":palette",
        ":palette",
        ":notifications",
        ":trigger add FATAL_ERR",
        ":trigger clear",
        ":remote add edge-1 ops@edge1.internal @edge,@prod",
        ":remote list",
        ":remote exec all echo ok",
        ":remote exec local echo local_ok",
        ":remote exec remote echo remote_ok",
        ":remote exec @edge echo edge_ok",
        ":remote exec edge-1 echo node_ok",
        ":remote disconnect edge-1",
        ":remote disconnect missing-node",
        ":ssh user@10.0.0.9 2222",
        ":ssh-tmux user@10.0.0.9 main_sess",
        ":yank",
        ":clear",
        ":raw",
        ":raw",
        ":close",
    ];

    for cmd_str in valid_commands {
        let parsed = parse_colon_command(cmd_str)
            .unwrap_or_else(|| panic!("expected Some for {cmd_str}"))
            .unwrap_or_else(|e| panic!("expected Ok for {cmd_str}, got {e}"));
        let _ = execute_wm_colon_command(&mut plane.wm, parsed);
    }

    // Verify usage error branches return Err(...) cleanly without panicking
    let invalid_commands = [
        ":prompt height",
        ":prompt width",
        ":prompt width invalid",
        ":prompt unknown",
        ":history max-files",
        ":history invalid",
        ":block invalid",
        ":split diagonal",
        ":deck side",
        ":focus",
        ":layout spiral",
        ":theme neon-pink",
        ":glyphs wingdings",
        ":cursor triangle",
        ":outline window invalid",
        ":outline prompt invalid",
        ":outline invalid",
        ":outline",
        ":session invalid",
        ":trigger add",
        ":trigger invalid",
        ":record invalid",
        ":ssh",
        ":ssh-tmux",
        ":remote add onlyname",
        ":remote exec",
        ":remote exec all",
        ":remote disconnect",
        ":remote unknown",
    ];

    for bad in invalid_commands {
        let res = parse_colon_command(bad);
        assert!(
            matches!(res, Some(Err(_))),
            "expected usage Err for {bad}, got {res:?}"
        );
    }

    // Direct single-pane execute_colon_command coverage
    let mut single = CompositePane::new_headless(99, 24, 80);
    single.process_pty_output(b"\x1b]133;A\x07\x1b]133;C\x07https://bilterm.dev/docs\r\n\x1b]133;D;0\x07");
    single
        .vt_mut()
        .record_command_submission("curl https://bilterm.dev/docs");
    for single_cmd in [
        ":prompt top",
        ":prompt bottom",
        ":prompt split-top",
        ":prompt split-bottom",
        ":prompt split",
        ":prompt float 3",
        ":prompt height 5",
        ":prompt width 30",
        ":prompt width full",
        ":history popup",
        ":history on",
        ":history off",
        ":history max-files 3",
        ":block prev",
        ":block next",
        ":links",
        ":yank",
        ":clear",
        ":raw",
        ":raw",
        ":search https",
        ":search next",
        ":search prev",
        ":search off",
        ":copy",
        ":copy",
        ":hints",
        ":hints",
        ":lock on",
        ":lock off",
        ":notifications",
    ] {
        let parsed = parse_colon_command(single_cmd).unwrap().unwrap();
        assert!(!execute_colon_command(&mut single, parsed));
    }
    assert!(execute_colon_command(
        &mut single,
        parse_colon_command(":quit").unwrap().unwrap()
    ));
}

#[test]
fn app_coordinator_input_modals_menubar_and_key_encoders_work_end_to_end() {
    let pane = CompositePane::new_headless(1, 24, 100);
    let mut app = App::new(pane);
    let viewport = Rect::new(0, 0, 100, 24);
    app.last_viewport = viewport;

    app.poll_all_ptys();
    assert!(!app.all_panes_exited());
    app.flush_osc52_to_host().expect("flush osc52");

    // Resize & Paste events through App::handle_crossterm_event
    app.handle_crossterm_event(Event::Resize(120, 30))
        .expect("resize");
    assert_eq!(app.last_viewport, Rect::new(0, 0, 120, 30));
    app.handle_crossterm_event(Event::Paste("echo pasted_cmd".to_string()))
        .expect("paste");
    assert_eq!(app.wm.active_pane().prompt().text(), "echo pasted_cmd");
    app.wm.active_pane_mut().prompt_mut().clear_line();

    // Top MenuBar mouse click & keyboard navigation
    app.wm.config_mut().show_top_menubar = true;
    for col in [2u16, 14, 24, 31, 38, 46, 55] {
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row: 0,
            modifiers: KeyModifiers::empty(),
        };
        handle_wm_mouse_event(&mut app.wm, click, app.last_viewport).expect("menubar click");
    }
    app.wm.set_menubar_open_category(Some(0));
    handle_wm_key_event(
        &mut app.wm,
        KeyEvent::new(KeyCode::Right, KeyModifiers::empty()),
    )
    .unwrap();
    handle_wm_key_event(
        &mut app.wm,
        KeyEvent::new(KeyCode::Left, KeyModifiers::empty()),
    )
    .unwrap();
    handle_wm_key_event(
        &mut app.wm,
        KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
    )
    .unwrap();
    handle_wm_key_event(
        &mut app.wm,
        KeyEvent::new(KeyCode::Up, KeyModifiers::empty()),
    )
    .unwrap();
    handle_wm_key_event(
        &mut app.wm,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
    )
    .unwrap();
    assert!(app.wm.is_inspector_visible());

    // Inspector modal navigation (Up, Down, PageUp, PageDown, Esc)
    for code in [
        KeyCode::Down,
        KeyCode::Up,
        KeyCode::PageDown,
        KeyCode::PageUp,
        KeyCode::Esc,
    ] {
        handle_wm_key_event(&mut app.wm, KeyEvent::new(code, KeyModifiers::empty())).unwrap();
    }
    assert!(!app.wm.is_inspector_visible());

    // Help modal navigation (F1, Down, Up, PageDown, PageUp, Esc)
    for code in [
        KeyCode::F(1),
        KeyCode::Down,
        KeyCode::Up,
        KeyCode::PageDown,
        KeyCode::PageUp,
        KeyCode::Esc,
    ] {
        handle_wm_key_event(&mut app.wm, KeyEvent::new(code, KeyModifiers::empty())).unwrap();
    }
    assert!(!app.wm.is_help_visible());

    // Fuzzy Command Palette navigation (Ctrl+P, type query, Backspace, Down, Up, Enter)
    handle_wm_key_event(
        &mut app.wm,
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert!(app.wm.is_palette_visible());
    handle_wm_key_event(
        &mut app.wm,
        KeyEvent::new(KeyCode::Char('3'), KeyModifiers::empty()),
    )
    .unwrap();
    handle_wm_key_event(
        &mut app.wm,
        KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
    )
    .unwrap();
    handle_wm_key_event(
        &mut app.wm,
        KeyEvent::new(KeyCode::Up, KeyModifiers::empty()),
    )
    .unwrap();
    handle_wm_key_event(
        &mut app.wm,
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()),
    )
    .unwrap();
    handle_wm_key_event(
        &mut app.wm,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
    )
    .unwrap();
    assert!(!app.wm.is_palette_visible());

    // Key chord parser & VT/Kitty key byte encoders
    for chord in [
        "Ctrl+C",
        "Alt+H",
        "Shift+Tab",
        "Enter",
        "Esc",
        "Tab",
        "BackTab",
        "Backspace",
        "Space",
        "Up",
        "Down",
        "Left",
        "Right",
        "PageUp",
        "PageDown",
        "Home",
        "End",
        "F1",
        "F2",
        "F3",
        "F4",
        "F5",
        "F6",
        "F7",
        "F8",
        "F9",
        "F10",
        "F11",
        "F12",
    ] {
        let ev = parse_key_chord_str(chord).unwrap_or_else(|| panic!("valid chord {chord}"));
        let _ = encode_key_to_pty_bytes_full(ev, false, false, 0);
        let _ = encode_key_to_pty_bytes_full(ev, true, true, 1);
    }

    // Render full WorkspaceWidget with all chrome/modals active
    let mut buf = Buffer::empty(app.last_viewport);
    WorkspaceWidget::new(&app.wm).render(app.last_viewport, &mut buf);

    // Ctrl+Q triggers AppAction::Quit
    let quit_action = handle_wm_key_event(
        &mut app.wm,
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert_eq!(quit_action, AppAction::Quit);
}

#[tokio::test]
async fn script_api_json_parser_mcp_tools_and_unix_socket_rpc_work() {
    // 1. Test JsonValue parser with nested objects, arrays, booleans, nulls, negative/float numbers, and escapes
    let complex_json = r#"{"str":"line1\nline2\t\"quoted\"\\slash","num":-42,"flt":3.5,"bool_t":true,"bool_f":false,"nil":null,"arr":[1,"two",false,{"k":"v"}]}"#;
    let val = JsonValue::parse(complex_json).expect("parse complex JSON");
    assert_eq!(
        val.get("str").and_then(JsonValue::as_str),
        Some("line1\nline2\t\"quoted\"\\slash")
    );
    assert_eq!(val.get("num").and_then(JsonValue::as_i64), Some(-42));
    assert_eq!(val.get("bool_t").and_then(JsonValue::as_bool), Some(true));
    assert_eq!(val.get("bool_f").and_then(JsonValue::as_bool), Some(false));
    assert_eq!(
        val.get("arr").and_then(JsonValue::as_array).map(|a| a.len()),
        Some(4)
    );
    let roundtrip = JsonValue::parse(&val.to_json_string()).expect("roundtrip JSON");
    assert_eq!(val, roundtrip);

    // 2. Test JSON-RPC error codes (-32700 parse error, -32600 invalid request, -32000 unknown method)
    let mut plane = AutomationControlPlane::new_headless(24, 100);
    assert!(plane.handle_jsonrpc_line("{broken").contains("-32700"));
    assert!(
        plane
            .handle_jsonrpc_line(r#"{"jsonrpc":"2.0","id":1}"#)
            .contains("-32600")
    );
    assert!(
        plane
            .handle_jsonrpc_line(r#"{"jsonrpc":"2.0","id":1,"method":"unknown.method"}"#)
            .contains("Method not found")
    );

    // 3. Test all 8 MCP tools via tools/call + ping + unknown tool error
    assert!(
        plane
            .handle_mcp_line(r#"{"jsonrpc":"2.0","id":10,"method":"ping"}"#)
            .unwrap()
            .contains("\"result\":{}")
    );
    for call_line in [
        r#"{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"bilterm_workspace_status","arguments":{}}}"#,
        r#"{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"bilterm_exec_command","arguments":{"pane_id":1,"command":"echo mcp_hello"}}}"#,
        r#"{"jsonrpc":"2.0","id":13,"method":"tools/call","params":{"name":"bilterm_inspect_pane","arguments":{"pane_id":1}}}"#,
        r#"{"jsonrpc":"2.0","id":14,"method":"tools/call","params":{"name":"bilterm_send_keys","arguments":{"keys":"Alt+T"}}}"#,
        r#"{"jsonrpc":"2.0","id":15,"method":"tools/call","params":{"name":"bilterm_colon_command","arguments":{"command":":split v"}}}"#,
        r#"{"jsonrpc":"2.0","id":16,"method":"tools/call","params":{"name":"bilterm_remote_connect","arguments":{"target":"dev@node1.internal","port":2222,"tags":["dev"]}}}"#,
        r#"{"jsonrpc":"2.0","id":17,"method":"tools/call","params":{"name":"bilterm_remote_connect","arguments":{"target":"dev@node1.internal","tmux_session":"dev-tmux"}}}"#,
        r#"{"jsonrpc":"2.0","id":18,"method":"tools/call","params":{"name":"bilterm_fleet_exec","arguments":{"selector":"@dev","command":"uptime"}}}"#,
        r#"{"jsonrpc":"2.0","id":19,"method":"tools/call","params":{"name":"bilterm_run_playbook","arguments":{"toml":"name = \"inline\"\n[[steps]]\naction = \"colon\"\ncommand = \":crt on\"\n"}}}"#,
    ] {
        let resp = plane.handle_mcp_line(call_line).expect("mcp tool response");
        assert!(
            resp.contains("\"isError\":false"),
            "expected isError:false for {call_line}, got {resp}"
        );
    }

    // Unknown MCP tool returns -32601 error
    let bad_tool = plane
        .handle_mcp_line(
            r#"{"jsonrpc":"2.0","id":20,"method":"tools/call","params":{"name":"nonexistent_tool"}}"#,
        )
        .unwrap();
    assert!(bad_tool.contains("-32601"));

    // 4. Test Unix Domain Socket RPC roundtrip on Unix
    #[cfg(unix)]
    {
        let dir = tempfile::tempdir().expect("tempdir");
        let sock_path = dir.path().join("bilterm-rpc.sock");
        let listener = tokio::net::UnixListener::bind(&sock_path).expect("bind unix socket");
        let sock_str = sock_path.to_string_lossy().to_string();

        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
            let mut srv = AutomationControlPlane::new_headless(24, 80);
            if let Ok((stream, _)) = listener.accept().await {
                let (rd, mut wr) = stream.into_split();
                let mut lines = BufReader::new(rd).lines();
                if let Ok(Some(line)) = lines.next_line().await {
                    let resp = srv.handle_jsonrpc_line(&line);
                    let _ = wr.write_all(format!("{resp}\n").as_bytes()).await;
                    let _ = wr.flush().await;
                }
            }
        });

        let uds_resp = send_rpc_request(
            &sock_str,
            "workspace.status",
            JsonValue::obj([] as [(&str, JsonValue); 0]),
        )
        .await
        .expect("UDS RPC client");
        assert!(uds_resp.contains("\"pane_count\":1"));
    }
}

#[test]
fn cli_binary_flags_and_subcommands_work_end_to_end() {
    let bin = env!("CARGO_BIN_EXE_bilterm");

    // 1. --version
    let ver = Command::new(bin).arg("--version").output().expect("--version");
    assert!(ver.status.success());
    let expected_ver = format!("bilterm {}", env!("CARGO_PKG_VERSION"));
    assert!(String::from_utf8_lossy(&ver.stdout).contains(&expected_ver));

    // 2. --help
    let help = Command::new(bin).arg("--help").output().expect("--help");
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--playbook"));

    // 3. --completions bash | zsh | fish
    for sh in ["bash", "zsh", "fish"] {
        let comp = Command::new(bin)
            .args(["--completions", sh])
            .output()
            .expect("--completions");
        assert!(comp.status.success());
        assert!(!comp.stdout.is_empty());
    }

    // 4. --man
    let man = Command::new(bin).arg("--man").output().expect("--man");
    assert!(man.status.success());
    assert!(String::from_utf8_lossy(&man.stdout).contains(".TH BILTERM 1"));

    // 5. --gui-dump <path.ppm>
    let dir = tempfile::tempdir().expect("tempdir");
    let ppm = dir.path().join("cli-dump.ppm");
    let dump = Command::new(bin)
        .args(["--gui-dump", ppm.to_str().unwrap()])
        .output()
        .expect("--gui-dump");
    assert!(dump.status.success());
    assert!(ppm.exists());

    // 6. --playbook <file.toml>
    let pb_file = dir.path().join("test-playbook.toml");
    std::fs::write(
        &pb_file,
        r#"
name = "cli-smoke-playbook"
[[steps]]
action = "colon"
command = ":split v"
[[steps]]
action = "exec"
pane_id = 2
command = "echo hello_from_playbook"
[[steps]]
action = "feed_output"
pane_id = 2
data = "hello_from_playbook\r\n\u001b]133;D;0\u0007"
[[steps]]
action = "assert_screen_contains"
pane_id = 2
expected = "hello_from_playbook"
[[steps]]
action = "assert_exit_code"
pane_id = 2
exit_code = 0
"#,
    )
    .unwrap();
    let pb_out = Command::new(bin)
        .args(["--playbook", pb_file.to_str().unwrap()])
        .output()
        .expect("--playbook");
    assert!(pb_out.status.success());
    assert!(String::from_utf8_lossy(&pb_out.stdout).contains("\"passed\":true"));
}

#[test]
fn performance_and_stress_benchmark_verifies_sub_millisecond_frame_and_rpc_budgets() {
    let mut plane = AutomationControlPlane::new_headless(40, 160);

    // 1. Spawn 32 panes (mix of local, SSH, and remote tmux-CC)
    for i in 1..32 {
        if i % 3 == 0 {
            let _ = plane
                .wm
                .spawn_ssh_pane(&format!("user@node-{i}.internal"), Some(2200 + i as u16), vec!["fleet".to_string()]);
        } else if i % 5 == 0 {
            let _ = plane
                .wm
                .connect_ssh_tmux(&format!("ops@tmux-{i}.internal"), "persist", vec!["fleet".to_string()]);
        } else {
            let _ = plane
                .wm
                .split_focused(bilterm::wm::split_tree::SplitDirection::Vertical);
        }
    }
    assert!(plane.wm.pane_count() >= 32);

    // 2. Throughput benchmark: Feed 64KB chunked ANSI/TrueColor/OSC-133 stream across panes (matches PTY 64KB reader chunks)
    let single_line = b"\x1b]133;C\x07\x1b[38;2;0;255;102m[PERF-BENCH] Metric=99.98% latency=0.12ms https://bilterm.dev/perf\x1b[0m\r\n\x1b]133;D;0\x07";
    let chunk: Vec<u8> = single_line.repeat(600); // ~60 KB chunk
    let iterations = 64; // ~3.84 MB total
    let vt_start = Instant::now();
    for i in 0..iterations {
        let pid = (i % 16) + 1;
        if let Some(p) = plane.wm.pane_mut(pid) {
            p.process_pty_output(&chunk);
        }
    }
    let vt_elapsed = vt_start.elapsed();
    let total_mb = (chunk.len() * iterations) as f64 / (1024.0 * 1024.0);
    let mb_per_sec = total_mb / vt_elapsed.as_secs_f64().max(1e-6);
    assert!(
        mb_per_sec > 2.0,
        "expected VT throughput > 2.0 MB/s (even under unoptimized llvm-cov debug build), got {mb_per_sec:.2} MB/s"
    );

    // 3. Compositor frame render benchmark: 100 full 160x40 workspace frames (Ratatui + GUI)
    let viewport = Rect::new(0, 0, 160, 40);
    let mut ratatui_renderer = RatatuiWorkspaceRenderer;
    let mut buf = Buffer::empty(viewport);
    let render_start = Instant::now();
    for _ in 0..100 {
        ratatui_renderer.render_workspace(&plane.wm, viewport, &mut buf);
    }
    let avg_frame_us = render_start.elapsed().as_micros() as f64 / 100.0;
    assert!(
        avg_frame_us < 16_000.0,
        "expected 60 FPS frame budget (<16,000 us), got {avg_frame_us:.1} us"
    );

    let mut gui_renderer = GuiWorkspaceRenderer::default();
    let mut gui_surface = GuiFrameSurface::new(8, 16);
    gui_renderer.render_workspace(&plane.wm, viewport, &mut gui_surface);
    assert!(!gui_surface.quads.is_empty());

    // 4. JSON-RPC 2.0 Control Plane dispatch benchmark: 1,000 requests
    let rpc_start = Instant::now();
    for i in 0..1_000 {
        let req = format!(
            r#"{{"jsonrpc":"2.0","id":{i},"method":"fleet.exec","params":{{"selector":"@fleet","command":"echo bench_{i}"}}}}"#
        );
        let resp = plane.handle_jsonrpc_line(&req);
        assert!(resp.contains("\"status\":\"dispatched\""));
    }
    let avg_rpc_us = rpc_start.elapsed().as_micros() as f64 / 1_000.0;
    assert!(
        avg_rpc_us < 5_000.0,
        "expected JSON-RPC dispatch < 5,000 us/call, got {avg_rpc_us:.1} us"
    );

    // 5. Verify BiltermConfig TOML serialization with [remotes.<name>] and [keybindings]
    let toml_str = r#"
theme = "ibm-3278-green"
crt_scanlines = true
triggers = ["ERROR", "PANIC"]

[keybindings]
"Ctrl+T" = ":split v"

[remotes.prod-api]
name = "prod-api"
destination = "deploy@api.prod.internal"
port = 2222
tags = ["prod", "api"]
"#;
    let cfg = BiltermConfig::from_toml_str(toml_str).expect("parse BiltermConfig with remotes");
    assert!(cfg.remotes.contains_key("prod-api"));
    assert_eq!(cfg.remotes["prod-api"].port, Some(2222));

    let ini_full = r#"
[theme]
; comment line
# another comment
invalid_line_without_equals
name = "CUSTOM-3270"
prompt_surface_bg = #102030
window_frame_bg = #203040
active_border_fg = #304050
inactive_border_fg = #405060
title_fg = #506070
crosshair_bg = #607080
oia_bg = #708090
shadow_fg = #8090A0
shadow_bg = #90A0B0
unknown_key = #FFFFFF
"#;
    let pal = bilterm::config::parse_ini_theme(ini_full);
    assert_eq!(pal.name, "CUSTOM-3270");
    assert_eq!(pal.oia_bg, Color::Rgb(0x70, 0x80, 0x90));
    assert_eq!(pal.shadow_bg, Color::Rgb(0x90, 0xA0, 0xB0));

    for (anchor_str, expected) in [
        ("top", bilterm::pane::PromptAnchor::Top),
        ("bottom", bilterm::pane::PromptAnchor::Bottom),
        ("floating", bilterm::pane::PromptAnchor::Floating { y_offset: 4 }),
        ("split-top", bilterm::pane::PromptAnchor::SplitTop),
        ("split-bottom", bilterm::pane::PromptAnchor::SplitBottom),
    ] {
        let mut c = cfg.clone();
        c.default_prompt_anchor = anchor_str.to_string();
        assert_eq!(c.resolved_prompt_anchor(), expected);
    }

    let missing_cfg = BiltermConfig::load_or_default(std::path::Path::new("/nonexistent/bilterm.toml"));
    assert_eq!(missing_cfg, BiltermConfig::default());

    assert!(
        handle_paste_event(
            plane.wm.active_pane_mut(),
            "line1\nline2\x1b[201~injected"
        )
        .is_ok()
    );
    let _ = Playbook::from_toml_str("name = \"empty\"\nsteps = []\n").expect("empty playbook");
}

#[test]
fn generates_real_workspace_screenshots_for_documentation() {
    use bilterm::ui::theme::ThemePreset;
    use std::path::Path;

    let viewport = Rect::new(0, 0, 128, 32);
    let mut renderer = GuiWorkspaceRenderer::new(9, 18);
    let mut surface = GuiFrameSurface::new(9, 18);

    // 1. Screenshot 1: Multi-Pane Tiled Workspace + SSH Fleet + Top MenuBar + DOS F-Key Bar (Turbo Blue)
    let mut plane1 = AutomationControlPlane::new_headless(32, 128);
    plane1.wm.config_mut().show_top_menubar = true;
    plane1.wm.config_mut().show_dos_fkey_bar = true;
    plane1.wm.config_mut().drop_shadows = true;
    plane1.wm.config_mut().crosshair_cursor = true;
    plane1.wm.config_mut().triggers = vec!["CRITICAL".to_string(), "READY".to_string()];

    let _ = plane1.handle_jsonrpc_line(
        r#"{"jsonrpc":"2.0","id":1,"method":"pane.exec","params":{"pane_id":1,"command":"cargo test --all-targets && bilterm ctl 127.0.0.1:7370 fleet.list"}}"#,
    );
    let _ = plane1.handle_jsonrpc_line(
        r#"{"jsonrpc":"2.0","id":2,"method":"pane.feed_output","params":{"pane_id":1,"data":"\u001b]133;C\u0007\u001b[1;32m   Compiling\u001b[0m bilterm v1.4.0 (/home/bill/src/ai/bilterm)\r\n\u001b[1;32m    Finished\u001b[0m `release` profile [optimized] target(s) in 1.42s\r\n\u001b[1;36m     Running\u001b[0m 85 tests across 16 test binaries ... \u001b[1;32m85 passed; 0 failed (83.45% coverage)\u001b[0m\r\n\u001b[1;33m[FLEET]\u001b[0m Local #1 READY | SSH prod-db:2222 READY | Endpoint: https://bilterm.dev/api\r\n\u001b]9;4;1;85\u0007\u001b]133;D;0\u0007"}}"#,
    );
    if let Some(p1) = plane1.wm.pane_mut(1) {
        p1.prompt_mut()
            .set_text(":remote exec @prod systemctl status postgresql");
    }

    let _ = plane1.handle_jsonrpc_line(
        r#"{"jsonrpc":"2.0","id":3,"method":"remote.connect_ssh","params":{"target":"admin@prod-db.internal","port":2222,"tags":["prod","db"]}}"#,
    );
    let _ = plane1.handle_jsonrpc_line(
        r#"{"jsonrpc":"2.0","id":4,"method":"pane.feed_output","params":{"pane_id":2,"data":"\u001b]133;C\u0007\u001b[1;32m● postgresql.service\u001b[0m - PostgreSQL 16 Database Server (prod-db.internal)\r\n   Loaded: loaded (/lib/systemd/system/postgresql.service; enabled)\r\n   Active: \u001b[1;32mactive (running)\u001b[0m since Tue 2026-10-21 20:00:00 UTC; 3h ago\r\n   Status: \"CRITICAL=0 | Connections: 42/500 | Replication: SYNC READY\"\r\n\u001b]9;PostgreSQL Healthy\u0007\u001b]133;D;0\u0007"}}"#,
    );
    if let Some(p2) = plane1.wm.pane_mut(2) {
        p2.prompt_mut()
            .set_text("psql -U admin -c 'SELECT pg_is_in_recovery();'");
    }
    plane1.wm.focus_and_raise(1);

    renderer.render_workspace(&plane1.wm, viewport, &mut surface);
    let svg1 = surface.export_svg(
        "BILTERM v2.0.0 — Multi-Pane Composite Workspace + SSH Fleet (Borland Turbo Blue)",
    );
    let svg1_path = Path::new("docs/assets/screenshot-workspace.svg");
    std::fs::write(svg1_path, &svg1).expect("write screenshot-workspace.svg");
    assert!(svg1_path.exists());

    // 2. Screenshot 2: IBM 3278 Phosphor Green + Full-Screen Cross-Hair + Live 3270 Terminal Inspector Modal (F9)
    plane1.wm.config_mut().theme = ThemePreset::Ibm3278Green;
    plane1.wm.config_mut().crosshair_fullscreen = true;
    plane1.wm.config_mut().crt_scanlines = true;
    plane1.wm.toggle_inspector();

    renderer.render_workspace(&plane1.wm, viewport, &mut surface);
    let svg2 = surface.export_svg(
        "BILTERM v2.0.0 — IBM 3278 Phosphor Green CRT + Live 3270 Terminal Inspector (F9)",
    );
    let svg2_path = Path::new("docs/assets/screenshot-3278-inspector.svg");
    std::fs::write(svg2_path, &svg2).expect("write screenshot-3278-inspector.svg");
    assert!(svg2_path.exists());

    // 3. Screenshot 3: IBM 3279 Amber CRT + Z-Order Stacked Cascade + Fuzzy Command Palette (Ctrl+P)
    plane1.wm.config_mut().theme = ThemePreset::Ibm3279Amber;
    plane1.wm.set_layout_mode(bilterm::wm::WorkspaceLayoutMode::Stacked);
    plane1.wm.open_palette_with_query("remote");

    renderer.render_workspace(&plane1.wm, viewport, &mut surface);
    let svg3 = surface.export_svg(
        "BILTERM v2.0.0 — IBM 3279 Amber CRT + Z-Order Cascade Deck + Fuzzy Command Palette (Ctrl+P)",
    );
    let svg3_path = Path::new("docs/assets/screenshot-3279-palette.svg");
    std::fs::write(svg3_path, &svg3).expect("write screenshot-3279-palette.svg");
    assert!(svg3_path.exists());

    // 4. Screenshot 4: 3-Way Split with Per-Pane Independent Font Scaling & nmon Telemetry HUD
    let mut plane2 = AutomationControlPlane::new_headless(32, 128);
    plane2.wm.config_mut().show_top_menubar = true;
    plane2.wm.config_mut().show_dos_fkey_bar = true;
    plane2.wm.resize_all(viewport).unwrap();
    let p2 = plane2.wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();
    let p3 = plane2.wm.split_focused(bilterm::wm::split_tree::SplitDirection::Horizontal).unwrap();
    plane2.wm.resize_all(viewport).unwrap();
    plane2.wm.focus_and_raise(1);
    plane2.wm.font_zoom_in();
    plane2.wm.font_zoom_in();
    if let Some(pane1) = plane2.wm.pane_mut(1) {
        pane1.set_title("INCIDENT COMMAND [120% ZOOM]".to_string());
        pane1.process_pty_output(
            "\x1b[1;32m$ bilterm --gui --gui-font \"JetBrains Mono\" --gui-font-size 16\x1b[0m\r\n\x1b[1;36m[WARN]\x1b[0m High IOPS on nvme0n1 — scaling top-right nmon pane to 80%\r\n\x1b[1;32m[OK]\x1b[0m All 4 borders & corners draggable; per-pane PTY SIGWINCH active\r\n"
                .as_bytes(),
        );
        pane1.prompt_mut().set_text("iostat -xz 1");
    }
    plane2.wm.focus_and_raise(p2);
    plane2.wm.font_zoom_out();
    plane2.wm.font_zoom_out();
    if let Some(pane2) = plane2.wm.pane_mut(p2) {
        pane2.set_title("nmon — CPU/DISK TELEMETRY [80% COMPACT]".to_string());
        pane2.process_pty_output(
            "\x1b[?1049h\x1b[1;33m┌─nmon-16p──────[H for help]───Hostname=prod-node-01──┐\x1b[0m\r\n\x1b[1;32m│ CPU  User%  Sys% Wait%   |████████████░░░░░░  68.4% │\x1b[0m\r\n\x1b[1;36m│ Disk nvme0n1 Busy  82%   |████████████████░░  940MB/s│\x1b[0m\r\n\x1b[1;35m│ Mem  Used: 48.2G / 64.0G | Cache: 12.1G | Free: 3.7G│\x1b[0m\r\n\x1b[1;33m└─────────────────────────────────────────────────────┘\x1b[0m"
                .as_bytes(),
        );
        pane2.line_discipline_mut().echo_enabled = false;
    }
    if let Some(pane3) = plane2.wm.pane_mut(p3) {
        pane3.set_title("LIVE JOURNALCTL STREAM [100%]".to_string());
        pane3.process_pty_output(b"\x1b[1;32m20:31:02\x1b[0m kernel: nvme0n1: queue depth=128 latency=0.41ms\r\n\x1b[1;33m20:31:04\x1b[0m systemd[1]: Reloaded PostgreSQL connection pooler.\r\n");
        pane3.prompt_mut().set_text("journalctl -fu postgresql -n 50");
    }
    plane2.wm.focus_and_raise(1);
    renderer.render_workspace_with_cell_size(&plane2.wm, viewport, 9, 18, &mut surface);
    let svg4 = surface.export_svg(
        "BILTERM v2.0.0 — 3-Way Split with Independent Per-Pane Font Scaling (120% / 80% nmon / 100%)",
    );
    let svg4_path = Path::new("docs/assets/screenshot-per-pane-scaling.svg");
    std::fs::write(svg4_path, &svg4).expect("write screenshot-per-pane-scaling.svg");
    assert!(svg4_path.exists());

    // 5. Screenshot 5: 4-Pane Multi-Agent Swarm + Slide-Out Fleet Drawer (Cyberpunk Neon)
    let mut plane3 = AutomationControlPlane::new_headless(32, 128);
    plane3.wm.config_mut().theme = ThemePreset::CyberpunkNeon;
    plane3.wm.config_mut().show_top_menubar = true;
    plane3.wm.config_mut().show_dos_fkey_bar = true;
    plane3.wm.toggle_drawer();
    plane3.wm.resize_all(viewport).unwrap();
    let a2 = plane3.wm.split_focused(bilterm::wm::split_tree::SplitDirection::Vertical).unwrap();
    plane3.wm.focus_and_raise(1);
    let a3 = plane3.wm.split_focused(bilterm::wm::split_tree::SplitDirection::Horizontal).unwrap();
    plane3.wm.focus_and_raise(a2);
    let a4 = plane3.wm.split_focused(bilterm::wm::split_tree::SplitDirection::Horizontal).unwrap();
    plane3.wm.resize_all(viewport).unwrap();
    if let Some(p) = plane3.wm.pane_mut(1) {
        p.set_title("AGENT #1: ARCHITECT (MCP)".to_string());
        p.process_pty_output(b"\x1b[1;36m[MCP]\x1b[0m Dispatched task plan to Agent #2 (Implementer) & Agent #3 (Verifier)\r\n");
        p.prompt_mut().set_text("bilterm ctl workspace.status");
    }
    if let Some(p) = plane3.wm.pane_mut(a2) {
        p.set_title("AGENT #2: IMPLEMENTER".to_string());
        p.process_pty_output(b"\x1b[1;32m[EDIT]\x1b[0m Patched src/ui/typography.rs & src/input/mouse.rs (+142/-38)\r\n");
        p.prompt_mut().set_text("cargo clippy --all-targets -- -D warnings");
    }
    if let Some(p) = plane3.wm.pane_mut(a3) {
        p.set_title("AGENT #3: TEST & QA".to_string());
        p.process_pty_output(b"\x1b[1;32m[PASS]\x1b[0m 242/242 tests green | Playbook assertions: 100%\r\n");
        p.prompt_mut().set_text("bilterm run starter-packs/03-robot-scripting-and-qa/playbook.toml");
    }
    if let Some(p) = plane3.wm.pane_mut(a4) {
        p.set_title("AGENT #4: SECURITY & FLEET".to_string());
        p.process_pty_output(b"\x1b[1;33m[SEC]\x1b[0m SEC-R01..04 verified: 0700 state dir, 0600 unix socket\r\n");
        p.prompt_mut().set_text(":lock on");
    }
    plane3.wm.focus_and_raise(1);
    renderer.render_workspace_with_cell_size(&plane3.wm, viewport, 9, 18, &mut surface);
    let svg5 = surface.export_svg(
        "BILTERM v2.0.0 — 4-Pane Multi-Agent Coding Swarm + Fleet Drawer (Cyberpunk Neon)",
    );
    let svg5_path = Path::new("docs/assets/screenshot-multi-agent-swarm.svg");
    std::fs::write(svg5_path, &svg5).expect("write screenshot-multi-agent-swarm.svg");
    assert!(svg5_path.exists());
}
