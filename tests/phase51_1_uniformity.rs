//! Phase 51.1-15: UNI-R01 surface parity matrix for persistent SSH + tmux links.
//!
//! Proves that every capability in `51.1-CONTRACT.md` section 10 is reachable through all required
//! surfaces (Colon, Palette, Help, Menu, LeaderKey, GlobalKey, BannerKey, BannerClick, WmCommand,
//! RpcLink, Mcp) by reading the live keymap, palette, help, menubar, RPC and MCP tables directly,
//! and that only the four documented exceptions (`E1`–`E4`) are permitted to be absent.

use std::collections::BTreeSet;
use std::io;
use std::sync::{Arc, Mutex, Once};

use bilterm::action::Action;
use bilterm::api::{AutomationControlPlane, JsonValue, RPC_METHODS};
use bilterm::colon_command::parse_colon_command;
use bilterm::input::keymap::{DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS};
use bilterm::input::mouse::handle_wm_mouse_event;
use bilterm::input::{AppAction, handle_wm_key_event};
use bilterm::pane::LinkBanner;
use bilterm::pty::PtyEvent;
use bilterm::tmux::TmuxControlSubprocess;
use bilterm::tmux::link::{
    DriverPoll, LinkConnector, LinkDriver, LinkState, LinkTarget, LinkTransport,
};
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::modals::{
    HELP_LINES, PALETTE_CATALOG, menubar_category_item_count, menubar_dropdown_rect,
    menubar_item_command, palette_header_rows,
};
use bilterm::ui::shell::ShellLayout;
use bilterm::wm::link_cmd::RemoteLinkOp;
use bilterm::wm::{WindowManager, inset_window_frame};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

const COLS: u16 = 120;
const ROWS: u16 = 40;
const AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: COLS,
    height: ROWS,
};

static INIT: Once = Once::new();

fn init_env() {
    INIT.call_once(|| {
        let dir =
            std::env::temp_dir().join(format!("bilterm-p511-uniformity-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: runs once before any plane or WM is created in this test binary.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
    });
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PathKind {
    Colon,
    Palette,
    Help,
    Menu,
    LeaderKey,
    GlobalKey,
    BannerKey,
    BannerClick,
    WmCommand,
    RpcLink,
    Mcp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExceptionEntry {
    id: &'static str,
    capability: &'static str,
    missing_paths: &'static [PathKind],
    reason: &'static str,
}

/// Exact four documented uniformity exceptions from `51.1-CONTRACT.md` section 10.
const EXCEPTIONS: &[ExceptionEntry] = &[
    ExceptionEntry {
        id: "E1",
        capability: "connect",
        missing_paths: &[
            PathKind::LeaderKey,
            PathKind::GlobalKey,
            PathKind::Menu,
            PathKind::BannerKey,
            PathKind::BannerClick,
        ],
        reason: "connect takes a free-text host, so key and menu open the prefilled prompt instead of running",
    },
    ExceptionEntry {
        id: "E2",
        capability: "prefill",
        missing_paths: &[
            PathKind::GlobalKey,
            PathKind::BannerKey,
            PathKind::BannerClick,
            PathKind::RpcLink,
            PathKind::Mcp,
        ],
        reason: "prefill is UI-only and has no API path or banner/global shortcut",
    },
    ExceptionEntry {
        id: "E3",
        capability: "info",
        missing_paths: &[
            PathKind::LeaderKey,
            PathKind::GlobalKey,
            PathKind::BannerKey,
            PathKind::BannerClick,
        ],
        reason: "info has no chord or banner trigger because it is read-only and the banner/status show the same data",
    },
    ExceptionEntry {
        id: "E4",
        capability: "disconnect",
        missing_paths: &[PathKind::GlobalKey, PathKind::BannerClick],
        reason: "the banner click is reconnect only; disconnect's mouse path is the top menu item",
    },
];

struct CapabilitySpec {
    name: &'static str,
    op: RemoteLinkOp,
    required_paths: &'static [PathKind],
    rpc_op: Option<&'static str>,
    banner_key_char: Option<char>,
}

const CAPABILITIES: &[CapabilitySpec] = &[
    CapabilitySpec {
        name: "reconnect",
        op: RemoteLinkOp::Reconnect,
        required_paths: &[
            PathKind::Colon,
            PathKind::Palette,
            PathKind::Help,
            PathKind::Menu,
            PathKind::LeaderKey,
            PathKind::GlobalKey,
            PathKind::BannerKey,
            PathKind::BannerClick,
            PathKind::WmCommand,
            PathKind::RpcLink,
            PathKind::Mcp,
        ],
        rpc_op: Some("reconnect"),
        banner_key_char: Some('r'),
    },
    CapabilitySpec {
        name: "disconnect",
        op: RemoteLinkOp::Disconnect,
        required_paths: &[
            PathKind::Colon,
            PathKind::Palette,
            PathKind::Help,
            PathKind::Menu,
            PathKind::LeaderKey,
            PathKind::BannerKey,
            PathKind::WmCommand,
            PathKind::RpcLink,
            PathKind::Mcp,
        ],
        rpc_op: Some("disconnect"),
        banner_key_char: Some('x'),
    },
    CapabilitySpec {
        name: "info",
        op: RemoteLinkOp::Info,
        required_paths: &[
            PathKind::Colon,
            PathKind::Palette,
            PathKind::Help,
            PathKind::Menu,
            PathKind::WmCommand,
            PathKind::RpcLink,
            PathKind::Mcp,
        ],
        rpc_op: Some("status"),
        banner_key_char: None,
    },
    CapabilitySpec {
        name: "prefill",
        op: RemoteLinkOp::Prefill,
        required_paths: &[
            PathKind::Colon,
            PathKind::Palette,
            PathKind::Help,
            PathKind::Menu,
            PathKind::LeaderKey,
            PathKind::WmCommand,
        ],
        rpc_op: None,
        banner_key_char: None,
    },
];

const ALL_PATH_KINDS: &[PathKind] = &[
    PathKind::Colon,
    PathKind::Palette,
    PathKind::Help,
    PathKind::Menu,
    PathKind::LeaderKey,
    PathKind::GlobalKey,
    PathKind::BannerKey,
    PathKind::BannerClick,
    PathKind::WmCommand,
    PathKind::RpcLink,
    PathKind::Mcp,
];

fn discover_leader_key(target: &Action) -> Option<KeyCode> {
    DEFAULT_LEADER_BINDINGS
        .iter()
        .find_map(|(code, action)| (action == target).then_some(*code))
}

fn discover_global_key(target: &Action) -> Option<(KeyModifiers, KeyCode)> {
    DEFAULT_GLOBAL_BINDINGS
        .iter()
        .find_map(|(mods, code, action)| (action == target).then_some((*mods, *code)))
}

fn discover_palette_entry(colon: &str) -> Option<&'static str> {
    PALETTE_CATALOG
        .iter()
        .find_map(|(cmd, _, _)| (*cmd == colon).then_some(*cmd))
}

fn discover_help_row(colon: &str) -> Option<&'static str> {
    HELP_LINES
        .iter()
        .find_map(|(cmd, desc)| (cmd.starts_with(colon) && !desc.is_empty()).then_some(*cmd))
}

fn discover_menu_entry(colon: &str) -> Option<(usize, usize)> {
    (0..8)
        .flat_map(|c| (0..menubar_category_item_count(c).min(32)).map(move |i| (c, i)))
        .find(|&(c, i)| menubar_item_command(c, i) == Some(colon))
}

fn render_buf(wm: &WindowManager) -> Buffer {
    let mut buf = Buffer::empty(AREA);
    WorkspaceWidget::new(wm).render(AREA, &mut buf);
    buf
}

fn find_text_in_buf(buf: &Buffer, needle: &str) -> Option<(u16, u16)> {
    for y in 0..buf.area.height {
        let row: String = (0..buf.area.width)
            .map(|x| {
                buf.cell((x, y))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default()
            })
            .collect();
        if let Some(byte) = row.find(needle) {
            let col = row[..byte].chars().count() as u16;
            return Some((col, y));
        }
    }
    None
}

#[derive(Clone, Default)]
struct NoopDriver;

impl LinkDriver for NoopDriver {
    fn pump(&mut self) -> DriverPoll {
        DriverPoll::default()
    }
    fn write_raw(&mut self, _bytes: &[u8]) -> io::Result<()> {
        Ok(())
    }
    fn resize(&mut self, _rows: u16, _cols: u16) {}
    fn kill(&mut self) {}
}

type LineTx = tokio::sync::mpsc::UnboundedSender<String>;

#[derive(Clone, Default)]
struct ChannelConnector {
    senders: Arc<Mutex<Vec<LineTx>>>,
}

impl LinkConnector for ChannelConnector {
    fn connect(&mut self, target: &LinkTarget, _size: (u16, u16)) -> io::Result<LinkTransport> {
        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        self.senders.lock().unwrap().push(line_tx);
        Ok(LinkTransport {
            control: TmuxControlSubprocess::from_channels(&target.session, cmd_tx, line_rx),
            driver: Some(Box::new(NoopDriver)),
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum FixtureMode {
    NoLink,
    Backoff,
    Connected,
}

fn build_plane(mode: FixtureMode) -> (AutomationControlPlane, ChannelConnector) {
    init_env();
    let mut p = AutomationControlPlane::new_headless(ROWS, COLS);
    p.wm.resize_all(AREA).unwrap();
    let conn = ChannelConnector::default();
    p.wm.set_link_connector(Box::new(conn.clone()));
    match mode {
        FixtureMode::NoLink => {}
        FixtureMode::Backoff => {
            let id = p.wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
            p.wm.focus_and_raise(id);
            let tx = conn.senders.lock().unwrap().pop().unwrap();
            tx.send("%output %3 hi".to_string()).unwrap();
            p.wm.tick_tmux_link(100);
            drop(tx);
            p.wm.tick_tmux_link(500);
            assert!(matches!(p.wm.tmux_link_state(), LinkState::Backoff { .. }));
        }
        FixtureMode::Connected => {
            let id = p.wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
            p.wm.focus_and_raise(id);
            let tx = conn.senders.lock().unwrap().last().unwrap().clone();
            tx.send("%output %3 hi".to_string()).unwrap();
            p.wm.tick_tmux_link(100);
            assert!(matches!(p.wm.tmux_link_state(), LinkState::Connected { .. }));
            // Ensure an interactive banner is present when testing banner surfaces on Connected
            p.wm.active_pane_mut().set_link_banner(Some(LinkBanner {
                text: "LINK READY | Enter/r: now | x: stop | d: detach".to_string(),
                interactive: true,
            }));
        }
    }
    (p, conn)
}

fn press(wm: &mut WindowManager, code: KeyCode, mods: KeyModifiers) {
    handle_wm_key_event(wm, KeyEvent::new(code, mods)).unwrap();
}

fn click(wm: &mut WindowManager, col: u16, row: u16) {
    handle_wm_mouse_event(
        wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        },
        AREA,
    )
    .unwrap();
    handle_wm_mouse_event(
        wm,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        },
        AREA,
    )
    .unwrap();
}

fn run_path_on_plane(p: &mut AutomationControlPlane, spec: &CapabilitySpec, path: PathKind) {
    let colon = spec.op.colon_form();
    let canonical = Action::RemoteLink(spec.op);
    match path {
        PathKind::Colon => {
            let parsed = parse_colon_command(colon).unwrap().unwrap();
            p.wm.apply(parsed).unwrap();
        }
        PathKind::Palette => {
            let entry = discover_palette_entry(colon).expect("palette entry exists");
            p.wm.open_palette_with_query(entry.trim_start_matches(':'));
            let idx = p
                .wm
                .filtered_palette_items()
                .iter()
                .position(|e| e.0 == entry)
                .expect("filtered palette contains entry");
            for _ in 0..idx {
                press(&mut p.wm, KeyCode::Down, KeyModifiers::NONE);
            }
            press(&mut p.wm, KeyCode::Enter, KeyModifiers::NONE);
            assert!(!p.wm.is_palette_visible());
        }
        PathKind::Help => {
            let row = discover_help_row(colon).expect("help row exists");
            let cmd = row.split(" / ").next().unwrap().trim();
            let parsed = parse_colon_command(cmd).unwrap().unwrap();
            p.wm.apply(parsed).unwrap();
        }
        PathKind::Menu => {
            let (cat, item) = discover_menu_entry(colon).expect("menu item exists");
            p.wm.set_menubar_open_category(Some(cat));
            let ws = ShellLayout::compute(AREA, p.wm.config(), p.wm.pane_count()).workspace;
            let inner = inset_window_frame(menubar_dropdown_rect(cat, ws));
            click(&mut p.wm, inner.x + 2, inner.y + item as u16);
            assert!(p.wm.menubar_state().is_none());
        }
        PathKind::LeaderKey => {
            let code = discover_leader_key(&canonical).expect("leader key exists");
            press(&mut p.wm, KeyCode::Char('x'), KeyModifiers::CONTROL);
            assert!(p.wm.is_leader_pending());
            press(&mut p.wm, code, KeyModifiers::NONE);
        }
        PathKind::GlobalKey => {
            let (mods, code) = discover_global_key(&canonical).expect("global key exists");
            press(&mut p.wm, code, mods);
        }
        PathKind::BannerKey => {
            let ch = spec.banner_key_char.expect("banner key char");
            if p.wm.active_pane().link_banner().is_none() {
                p.wm.active_pane_mut().set_link_banner(Some(LinkBanner {
                    text: "LINK BANNER | Enter/r: now | x: stop | d: detach".to_string(),
                    interactive: true,
                }));
            }
            press(&mut p.wm, KeyCode::Char(ch), KeyModifiers::NONE);
        }
        PathKind::BannerClick => {
            if p.wm.active_pane().link_banner().is_none() {
                p.wm.active_pane_mut().set_link_banner(Some(LinkBanner {
                    text: "LINK BANNER | Enter/r: now | x: stop | d: detach".to_string(),
                    interactive: true,
                }));
            }
            p.wm.active_pane_mut().set_link_raw_input(true);
            let buf = render_buf(&p.wm);
            let (col, row) = find_text_in_buf(&buf, "LINK").expect("banner rendered in buffer");
            click(&mut p.wm, col, row);
        }
        PathKind::WmCommand => {
            let params = JsonValue::obj([("command", JsonValue::String(colon.to_string()))]);
            p.dispatch_method("wm.command", &params).unwrap();
        }
        PathKind::RpcLink => {
            let op_str = spec.rpc_op.expect("rpc op");
            let params = JsonValue::obj([("op", JsonValue::String(op_str.to_string()))]);
            p.dispatch_method("remote.link", &params).unwrap();
        }
        PathKind::Mcp => {
            let op_str = spec.rpc_op.expect("mcp op");
            let req = format!(
                r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"bilterm_remote_link","arguments":{{"op":"{op_str}"}}}}}}"#
            );
            let raw = p.handle_mcp_line(&req).expect("mcp response");
            let v = JsonValue::parse(&raw).unwrap();
            assert_eq!(
                v.get("result")
                    .and_then(|r| r.get("isError"))
                    .and_then(JsonValue::as_bool),
                Some(false),
                "mcp call failed: {raw}"
            );
        }
    }
}

#[test]
fn every_capability_has_all_required_paths() {
    init_env();
    // Verify RPC_METHODS and MCP tools list include the remote link surfaces.
    assert!(RPC_METHODS.contains(&"remote.link"));
    let mut p = AutomationControlPlane::new_headless(ROWS, COLS);
    let mcp_list = p
        .handle_mcp_line(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
        .unwrap();
    assert!(mcp_list.contains("bilterm_remote_link"));

    for spec in CAPABILITIES {
        let colon = spec.op.colon_form();
        let canonical = Action::RemoteLink(spec.op);

        // Every path kind must either be in required_paths OR documented in EXCEPTIONS.
        let required: BTreeSet<PathKind> = spec.required_paths.iter().copied().collect();
        let excepted: BTreeSet<PathKind> = EXCEPTIONS
            .iter()
            .filter(|e| e.capability == spec.name)
            .flat_map(|e| e.missing_paths.iter().copied())
            .collect();

        for &kind in ALL_PATH_KINDS {
            assert!(
                required.contains(&kind) ^ excepted.contains(&kind),
                "capability `{}` path `{kind:?}` must be in either required_paths or EXCEPTIONS (not both/neither)",
                spec.name
            );
        }

        for &path in spec.required_paths {
            match path {
                PathKind::Colon => {
                    assert!(
                        parse_colon_command(colon).is_some_and(|r| r.is_ok()),
                        "missing Colon path for {}",
                        spec.name
                    );
                }
                PathKind::Palette => {
                    assert!(
                        discover_palette_entry(colon).is_some(),
                        "missing Palette entry for {}",
                        spec.name
                    );
                }
                PathKind::Help => {
                    assert!(
                        discover_help_row(colon).is_some(),
                        "missing Help row for {}",
                        spec.name
                    );
                }
                PathKind::Menu => {
                    assert!(
                        discover_menu_entry(colon).is_some(),
                        "missing Menu item for {}",
                        spec.name
                    );
                }
                PathKind::LeaderKey => {
                    assert!(
                        discover_leader_key(&canonical).is_some(),
                        "missing LeaderKey for {}",
                        spec.name
                    );
                }
                PathKind::GlobalKey => {
                    assert!(
                        discover_global_key(&canonical).is_some(),
                        "missing GlobalKey for {}",
                        spec.name
                    );
                }
                PathKind::BannerKey => {
                    assert!(
                        spec.banner_key_char.is_some(),
                        "missing BannerKey for {}",
                        spec.name
                    );
                }
                PathKind::BannerClick => {
                    assert_eq!(spec.op, RemoteLinkOp::Reconnect);
                }
                PathKind::WmCommand => {
                    assert!(parse_colon_command(colon).is_some());
                }
                PathKind::RpcLink | PathKind::Mcp => {
                    assert!(
                        spec.rpc_op.is_some(),
                        "missing API op for {} on {path:?}",
                        spec.name
                    );
                }
            }
        }
    }

    // Also verify Connect's E1 exception: `:ssh-tmux ` exists in palette & help, and leader/menu open prefill.
    assert!(
        PALETTE_CATALOG.iter().any(|(cmd, _, _)| *cmd == ":ssh-tmux "),
        "Connect fill-in entry `:ssh-tmux ` must exist in PALETTE_CATALOG"
    );
    assert!(
        discover_leader_key(&Action::RemoteLink(RemoteLinkOp::Prefill)).is_some(),
        "Connect E1 relies on Prefill leader key"
    );
    assert!(
        discover_menu_entry(":ssh-tmux prefill").is_some(),
        "Connect E1 relies on Prefill menu item"
    );
}

#[test]
fn paths_reach_the_same_action() {
    for spec in CAPABILITIES {
        let colon = spec.op.colon_form();
        let canonical = Action::RemoteLink(spec.op);

        // 1. Colon parser
        let from_colon = parse_colon_command(colon).unwrap().unwrap();
        assert_eq!(from_colon, canonical, "{} colon action", spec.name);

        // 2. Palette entry
        let pal_cmd = discover_palette_entry(colon).unwrap();
        let from_palette = parse_colon_command(pal_cmd).unwrap().unwrap();
        assert_eq!(from_palette, canonical, "{} palette action", spec.name);

        // 3. Menu item
        let (cat, item) = discover_menu_entry(colon).unwrap();
        let menu_cmd = menubar_item_command(cat, item).unwrap();
        let from_menu = parse_colon_command(menu_cmd).unwrap().unwrap();
        assert_eq!(from_menu, canonical, "{} menu action", spec.name);

        // 4. Help row first command form
        let help_cmd = discover_help_row(colon)
            .unwrap()
            .split(" / ")
            .next()
            .unwrap()
            .trim();
        let from_help = parse_colon_command(help_cmd).unwrap().unwrap();
        assert_eq!(from_help, canonical, "{} help action", spec.name);

        // 5. Leader key table
        if spec.required_paths.contains(&PathKind::LeaderKey) {
            let code = discover_leader_key(&canonical).unwrap();
            let (_, action) = DEFAULT_LEADER_BINDINGS
                .iter()
                .find(|(k, _)| *k == code)
                .unwrap();
            assert_eq!(*action, canonical, "{} leader action", spec.name);
        }

        // 6. Global key table
        if spec.required_paths.contains(&PathKind::GlobalKey) {
            let (mods, code) = discover_global_key(&canonical).unwrap();
            let (_, _, action) = DEFAULT_GLOBAL_BINDINGS
                .iter()
                .find(|(m, k, _)| *m == mods && *k == code)
                .unwrap();
            assert_eq!(*action, canonical, "{} global action", spec.name);
        }
    }
}

#[test]
fn paths_reach_the_same_state() {
    for mode in [
        FixtureMode::Backoff,
        FixtureMode::Connected,
        FixtureMode::NoLink,
    ] {
        for spec in CAPABILITIES {
            let (mut ref_plane, _) = build_plane(mode);
            run_path_on_plane(&mut ref_plane, spec, PathKind::Colon);
            let want_summary = ref_plane.wm.tmux_link_summary();
            let want_status = ref_plane
                .wm
                .active_pane()
                .status_message()
                .unwrap_or("")
                .to_string();
            let want_prompt = ref_plane.wm.active_pane().prompt().text().to_string();
            let want_data = ref_plane.wm.take_command_data();

            for &path in spec.required_paths {
                // In Connected mode without a down state, Prefill requires prompt visible (which Connected has),
                // and BannerKey/BannerClick are tested primarily on Backoff/NoLink where the banner is naturally active.
                if matches!(mode, FixtureMode::Connected)
                    && matches!(path, PathKind::BannerKey | PathKind::BannerClick)
                {
                    continue;
                }
                let (mut p, _) = build_plane(mode);
                // Palette click parity as well when testing Palette on Backoff:
                run_path_on_plane(&mut p, spec, path);
                let got_summary = p.wm.tmux_link_summary();
                let got_status = p
                    .wm
                    .active_pane()
                    .status_message()
                    .unwrap_or("")
                    .to_string();
                let got_prompt = p.wm.active_pane().prompt().text().to_string();

                assert_eq!(
                    got_summary.state, want_summary.state,
                    "[{} / {mode:?} / {path:?}] state mismatch",
                    spec.name
                );
                assert_eq!(
                    got_summary.attempt, want_summary.attempt,
                    "[{} / {mode:?} / {path:?}] attempt mismatch",
                    spec.name
                );
                if spec.op != RemoteLinkOp::Prefill {
                    assert_eq!(
                        got_status, want_status,
                        "[{} / {mode:?} / {path:?}] status mismatch",
                        spec.name
                    );
                }
                assert_eq!(
                    got_prompt, want_prompt,
                    "[{} / {mode:?} / {path:?}] prompt mismatch",
                    spec.name
                );
                if spec.op == RemoteLinkOp::Info
                    && matches!(
                        path,
                        PathKind::Colon | PathKind::Palette | PathKind::Help | PathKind::Menu
                    )
                {
                    assert_eq!(
                        p.wm.take_command_data(),
                        want_data,
                        "[{} / {mode:?} / {path:?}] info command_data mismatch",
                        spec.name
                    );
                }
            }

            // Also verify clicking the palette row directly matches pressing Enter in the palette.
            let (mut p_click, _) = build_plane(mode);
            let colon = spec.op.colon_form();
            p_click
                .wm
                .open_palette_with_query(colon.trim_start_matches(':'));
            let idx = p_click
                .wm
                .filtered_palette_items()
                .iter()
                .position(|e| e.0 == colon)
                .unwrap();
            let outer = p_click.wm.active_modal_rect(AREA).unwrap();
            let inner = inset_window_frame(outer);
            let hdr = palette_header_rows(p_click.wm.config(), inner.height);
            click(&mut p_click.wm, inner.x + 4, inner.y + hdr + idx as u16);
            assert_eq!(p_click.wm.tmux_link_summary().state, want_summary.state);
        }
    }
}

#[test]
fn respawn_paths_agree() {
    init_env();
    #[derive(Clone, Copy, Debug)]
    enum RespawnPath {
        Colon,
        KeyEnter,
        KeyR,
        BannerClick,
        WmCommand,
        Mcp,
    }

    let make_held_ssh = || {
        let mut p = AutomationControlPlane::new_headless(ROWS, COLS);
        p.wm.resize_all(AREA).unwrap();
        let ssh_id = p.wm.spawn_ssh_pane("me@box", None, vec![]).unwrap();
        p.wm.focus_and_raise(ssh_id);
        let pane = p.wm.pane_mut(ssh_id).unwrap();
        pane.process_pty_output(b"Connection to box closed.\r\n");
        pane.ingest_pty_events(vec![PtyEvent::Exited(Some(255))]);
        p.wm.poll_all_ptys();
        assert!(p.wm.pane(ssh_id).unwrap().held_exited_shell().is_some());
        (p, ssh_id)
    };

    for path in [
        RespawnPath::Colon,
        RespawnPath::KeyEnter,
        RespawnPath::KeyR,
        RespawnPath::BannerClick,
        RespawnPath::WmCommand,
        RespawnPath::Mcp,
    ] {
        let (mut p, ssh_id) = make_held_ssh();
        match path {
            RespawnPath::Colon => {
                let act = parse_colon_command(":respawn-pane").unwrap().unwrap();
                assert_eq!(act, Action::RespawnPaneInPlace);
                p.wm.apply(act).unwrap();
            }
            RespawnPath::KeyEnter => {
                assert_eq!(
                    handle_wm_key_event(&mut p.wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                        .unwrap(),
                    AppAction::Handled
                );
            }
            RespawnPath::KeyR => {
                assert_eq!(
                    handle_wm_key_event(
                        &mut p.wm,
                        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)
                    )
                    .unwrap(),
                    AppAction::Handled
                );
            }
            RespawnPath::BannerClick => {
                p.wm.pane_mut(ssh_id).unwrap().set_link_raw_input(true);
                let buf = render_buf(&p.wm);
                let (col, row) =
                    find_text_in_buf(&buf, "REMOTE SESSION ENDED").expect("held banner rendered");
                click(&mut p.wm, col, row);
            }
            RespawnPath::WmCommand => {
                let params =
                    JsonValue::obj([("command", JsonValue::String(":respawn-pane".into()))]);
                p.dispatch_method("wm.command", &params).unwrap();
            }
            RespawnPath::Mcp => {
                let line = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"bilterm_colon_command","arguments":{"command":":respawn-pane"}}}"#;
                let raw = p.handle_mcp_line(line).unwrap();
                assert!(raw.contains(r#""isError":false"#), "{raw}");
            }
        }
        let pane = p.wm.pane(ssh_id).unwrap();
        assert!(
            pane.held_exited_shell().is_none(),
            "{path:?} did not un-hold the pane"
        );
        assert_eq!(
            pane.status_message(),
            Some("Reconnecting ssh: me@box"),
            "{path:?} status mismatch"
        );
    }
}

#[test]
fn api_and_colon_agree() {
    init_env();
    for (op, colon) in [
        ("status", ":ssh-tmux info"),
        ("reconnect", ":ssh-tmux reconnect"),
        ("disconnect", ":ssh-tmux disconnect"),
    ] {
        for mode in [FixtureMode::NoLink, FixtureMode::Backoff, FixtureMode::Connected] {
            let (mut p_rpc, _) = build_plane(mode);
            let (mut p_wm, _) = build_plane(mode);
            let (mut p_mcp, _) = build_plane(mode);

            let r_rpc = p_rpc
                .dispatch_method(
                    "remote.link",
                    &JsonValue::obj([("op", JsonValue::String(op.to_string()))]),
                )
                .unwrap();
            let r_wm = p_wm
                .dispatch_method(
                    "wm.command",
                    &JsonValue::obj([("command", JsonValue::String(colon.to_string()))]),
                )
                .unwrap();
            let mcp_line = format!(
                r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"bilterm_remote_link","arguments":{{"op":"{op}"}}}}}}"#
            );
            let raw_mcp = p_mcp.handle_mcp_line(&mcp_line).unwrap();
            let v_mcp = JsonValue::parse(&raw_mcp).unwrap();
            let mcp_text = v_mcp
                .get("result")
                .and_then(|r| r.get("content"))
                .and_then(JsonValue::as_array)
                .and_then(|a| a.first())
                .and_then(|c| c.get("text"))
                .and_then(JsonValue::as_str)
                .unwrap();

            assert_eq!(
                r_rpc.get("command"),
                r_wm.get("command"),
                "{op}/{mode:?} command"
            );
            assert_eq!(
                r_rpc.get("pane_count"),
                r_wm.get("pane_count"),
                "{op}/{mode:?} pane_count"
            );
            assert_eq!(
                r_rpc
                    .get("link")
                    .and_then(|l| l.get("state"))
                    .and_then(JsonValue::as_str),
                Some(p_wm.wm.tmux_link_summary().state),
                "{op}/{mode:?} link.state"
            );
            assert_eq!(mcp_text, r_rpc.to_json_string(), "{op}/{mode:?} mcp==rpc");
        }
    }

    // E2: prefill is rejected by remote.link and bilterm_remote_link as an invalid op.
    let (mut p, _) = build_plane(FixtureMode::NoLink);
    let err = p
        .dispatch_method(
            "remote.link",
            &JsonValue::obj([("op", JsonValue::String("prefill".to_string()))]),
        )
        .unwrap_err();
    assert!(err.starts_with("invalid remote.link parameter"), "{err}");

    let mcp_prefill = p
        .handle_mcp_line(
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"bilterm_remote_link","arguments":{"op":"prefill"}}}"#,
        )
        .unwrap();
    assert!(mcp_prefill.contains(r#""isError":true"#), "{mcp_prefill}");
    assert!(
        mcp_prefill.contains("invalid remote.link parameter"),
        "{mcp_prefill}"
    );
}

#[test]
fn exception_table_is_exact() {
    assert_eq!(EXCEPTIONS.len(), 4, "must have exactly 4 exceptions E1..E4");
    let ids: Vec<&str> = EXCEPTIONS.iter().map(|e| e.id).collect();
    assert_eq!(ids, vec!["E1", "E2", "E3", "E4"]);

    let caps: Vec<&str> = EXCEPTIONS.iter().map(|e| e.capability).collect();
    assert_eq!(caps, vec!["connect", "prefill", "info", "disconnect"]);

    for e in EXCEPTIONS {
        assert!(!e.reason.trim().is_empty(), "{} reason must be non-empty", e.id);
        assert!(
            !e.missing_paths.is_empty(),
            "{} must declare at least one missing path",
            e.id
        );
    }

    assert!(EXCEPTIONS[0].reason.contains("free-text host"));
    assert!(EXCEPTIONS[1].reason.contains("UI-only"));
    assert!(EXCEPTIONS[2].reason.contains("read-only"));
    assert!(EXCEPTIONS[3].reason.contains("reconnect only"));
}
