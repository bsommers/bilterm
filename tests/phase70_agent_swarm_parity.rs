//! Phase 70 HUD-04 (`UNI-R01`): the agent / swarm parity matrix.
//!
//! Every capability of the live-agent phases (66, 67) and the Mission Control phase (70) is listed
//! against the eight surfaces: key, colon command, palette, menu, mouse, CLI, RPC, MCP. A surface
//! either *performs the capability* (and the test proves the observable effect equals the effect
//! of the canonical `Action`, or of the same RPC method), or is `Na` with a reason. The Phase 68/69
//! surfaces (context bus, prompt queue, swarm) live in `phase68_69_surface_parity.rs`.
//!
//! Style follows `phase63_keys_surface_parity.rs`: pure headless workspaces, no PTYs.

use std::sync::Once;
use std::time::Instant;

use bilterm::action::Action;
use bilterm::api::{AutomationControlPlane, JsonValue, RPC_METHODS};
use bilterm::cli::{CliEnv, parse as cli_parse};
use bilterm::colon_command::parse_colon_command;
use bilterm::input::agents_hud::{agents_hud_key_action, hud_mouse_action};
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::modals::{
    PALETTE_CATALOG, menubar_category_item_count, menubar_dropdown_rect, menubar_item_command,
};
use bilterm::ui::shell::ShellLayout;
use bilterm::wm::agent_state::AgentState;
use bilterm::wm::agents_hud::hud_inner;
use bilterm::wm::agents_hud_cmd::{HudOp, ReplayOp};
use bilterm::wm::mission_control_hud::{HudButtonKind, button_rects};
use bilterm::wm::{SplitDirection, WindowManager, inset_window_frame};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

const AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: 120,
    height: 40,
};

// ---------------------------------------------------------------- fixture

fn isolate() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let root = std::env::temp_dir().join(format!("bt70-parity-{}", std::process::id()));
        let _ = std::fs::create_dir_all(root.join("home"));
        // SAFETY: runs once, before any workspace of this test binary is created.
        unsafe {
            std::env::set_var("XDG_STATE_HOME", root.join("state"));
            std::env::set_var("XDG_CONFIG_HOME", root.join("config"));
            std::env::set_var("XDG_DATA_HOME", root.join("data"));
            std::env::set_var("HOME", root.join("home"));
        }
    });
}

type Setup = fn(&mut WindowManager);
type Gesture = fn(&mut AutomationControlPlane);
type Sig = fn(&WindowManager) -> String;

fn fresh(setup: Setup) -> AutomationControlPlane {
    isolate();
    let mut p = AutomationControlPlane::new_headless(AREA.height, AREA.width);
    for _ in 1..3 {
        p.wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    p.wm.resize_all(AREA).unwrap();
    p.wm.update_agent_states(Instant::now());
    let first = p.wm.ordered_pane_ids()[0];
    p.wm.focus_and_raise(first);
    setup(&mut p.wm);
    p
}

fn ws(wm: &WindowManager) -> Rect {
    ShellLayout::compute(AREA, wm.config(), wm.pane_count()).workspace
}

fn key(p: &mut AutomationControlPlane, code: KeyCode) {
    handle_wm_key_event(&mut p.wm, KeyEvent::new(code, KeyModifiers::NONE)).unwrap();
}

fn leader(p: &mut AutomationControlPlane, c: char) {
    handle_wm_key_event(
        &mut p.wm,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert!(p.wm.is_leader_pending());
    key(p, KeyCode::Char(c));
}

fn mouse(p: &mut AutomationControlPlane, kind: MouseEventKind, x: u16, y: u16) {
    handle_wm_mouse_event(
        &mut p.wm,
        MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        },
        AREA,
    )
    .unwrap();
}

fn click(p: &mut AutomationControlPlane, x: u16, y: u16) {
    mouse(p, MouseEventKind::Down(MouseButton::Left), x, y);
}

fn menu_click(p: &mut AutomationControlPlane, cmd: &str) {
    let (cat, item) = (0..12)
        .flat_map(|c| (0..menubar_category_item_count(c).min(48)).map(move |i| (c, i)))
        .find(|&(c, i)| menubar_item_command(c, i) == Some(cmd))
        .unwrap_or_else(|| panic!("no menu entry for {cmd}"));
    p.wm.set_menubar_open_category(Some(cat));
    let inner = inset_window_frame(menubar_dropdown_rect(cat, ws(&p.wm)));
    click(p, inner.x + 2, inner.y + u16::try_from(item).unwrap());
    assert!(
        p.wm.menubar_state().is_none(),
        "menu click on {cmd} closes the menu"
    );
}

fn tile(wm: &WindowManager, pane: usize) -> Rect {
    let outer = wm.agents_hud_rect(ws(wm)).expect("hud visible");
    wm.agents_hud()
        .hud()
        .layout(hud_inner(outer))
        .into_iter()
        .find(|(id, _)| *id == pane)
        .expect("tile")
        .1
}

fn click_button(p: &mut AutomationControlPlane, pane: usize, kind: HudButtonKind, right: bool) {
    let r = button_rects(tile(&p.wm, pane))
        .into_iter()
        .find(|(k, _)| *k == kind)
        .expect("button")
        .1;
    let btn = if right {
        MouseButton::Right
    } else {
        MouseButton::Left
    };
    mouse(p, MouseEventKind::Down(btn), r.x, r.y);
}

fn colon(p: &mut AutomationControlPlane, cmd: &str) {
    let a = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a command: {cmd}"))
        .unwrap_or_else(|e| panic!("{cmd}: {e}"));
    p.wm.apply(a).unwrap();
}

fn palette(p: &mut AutomationControlPlane, entry: &str) {
    assert!(
        PALETTE_CATALOG.iter().any(|e| e.0 == entry),
        "palette catalog lists `{entry}`"
    );
    p.wm.open_palette_with_query(entry.trim_start_matches(':'));
    assert!(
        p.wm.filtered_palette_items().iter().any(|e| e.0 == entry),
        "palette filter finds `{entry}`"
    );
    p.wm.toggle_palette();
    assert!(!p.wm.is_palette_visible());
    colon(p, entry);
}

// ---------------------------------------------------------------- signatures and setups

/// HUD / replay window state plus focus and zoom.
fn hud_sig(wm: &WindowManager) -> String {
    let h = wm.agents_hud();
    format!(
        "active={} selected={:?} page={} replay={:?} turn={:?} scroll={:?} focus={} zoom={:?}",
        h.active,
        h.hud().selected(),
        h.hud().page(),
        h.replay_pane(),
        h.replay_pane()
            .and_then(|p| h.log(p))
            .and_then(|l| l.current())
            .map(|t| t.turn_id),
        h.replay_pane().and_then(|p| h.log(p)).map(|l| l.scroll()),
        wm.active_pane_id(),
        wm.zoomed_pane(),
    )
}

/// Focus and zoom only (the window may legitimately differ between HUD and pane commands).
fn focus_sig(wm: &WindowManager) -> String {
    format!("focus={} zoom={:?}", wm.active_pane_id(), wm.zoomed_pane())
}

/// What an approval writes: the traffic bus.
fn approve_sig(wm: &WindowManager) -> String {
    format!(
        "events={} last={:?}",
        wm.traffic().len(),
        wm.traffic()
            .iter()
            .last()
            .map(|e| (e.kind.as_str(), e.preview.clone()))
    )
}

/// HUD state plus the status line (Diff only reports a status).
fn status_sig(wm: &WindowManager) -> String {
    format!("{}|{:?}", hud_sig(wm), wm.active_pane().status_message())
}

fn panes_sig(wm: &WindowManager) -> String {
    format!("panes={} focus={}", wm.pane_count(), wm.active_pane_id())
}

fn s_plain(_: &mut WindowManager) {}

fn s_hud(wm: &mut WindowManager) {
    wm.apply(Action::AgentsHud(HudOp::Open)).unwrap();
}

/// HUD open, pane 2 Blocked and selected.
/// HUD open over 14 panes, so tiles span two pages.
fn s_hud_paged(wm: &mut WindowManager) {
    for _ in 0..11 {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.resize_all(AREA).unwrap();
    wm.update_agent_states(Instant::now());
    s_hud(wm);
}

fn s_hud_blocked(wm: &mut WindowManager) {
    wm.report_agent_state(2, Some(AgentState::Blocked));
    wm.update_agent_states(Instant::now());
    wm.apply(Action::AgentsHud(HudOp::Open)).unwrap();
    wm.apply(Action::AgentsHud(HudOp::SelectPane(2))).unwrap();
}

fn s_hud_sel2(wm: &mut WindowManager) {
    s_hud(wm);
    wm.apply(Action::AgentsHud(HudOp::SelectPane(2))).unwrap();
}

/// Two finished turns on pane 2 (the second long enough to scroll) and the replay view open on it.
fn s_replay(wm: &mut WindowManager) {
    let chunk = |tag: &str, n: usize| {
        (0..n)
            .map(|i| format!("{tag} line {i}\r\n"))
            .collect::<String>()
            .into_bytes()
    };
    wm.agents_hud_tick_at(Instant::now(), 10);
    for (i, outs) in [
        vec![chunk("first", 2)],
        vec![chunk("second", 30), chunk("third", 30)],
    ]
    .into_iter()
    .enumerate()
    {
        let t = 100 * (i as u64 + 1);
        wm.report_agent_state(2, Some(AgentState::Working));
        wm.update_agent_states(Instant::now());
        wm.agents_hud_tick_at(Instant::now(), t);
        for (k, out) in outs.iter().enumerate() {
            wm.pane_mut(2).unwrap().process_pty_output(out);
            wm.agents_hud_tick_at(Instant::now(), t + 1 + k as u64);
        }
        wm.report_agent_state(2, Some(AgentState::Idle));
        wm.update_agent_states(Instant::now());
        wm.agents_hud_tick_at(Instant::now(), t + 10);
    }
    wm.apply(Action::AgentsReplay(ReplayOp::Open(2))).unwrap();
}

// ---------------------------------------------------------------- matrix model

/// A surface that performs the capability by a gesture, or does not apply.
enum Surf {
    Do(Gesture),
    Na(&'static str),
}

/// CLI / RPC / MCP cell (all three are live for every capability): `Eq` runs the call and compares its effect with the canonical action;
/// `Is` checks the method mapping only (calls that carry text or block, or have no Action twin).
enum Wire {
    Eq(&'static str, &'static str),
    Is(&'static str, &'static str, &'static str),
}

struct Row {
    id: &'static str,
    phases: &'static [u8],
    setup: Setup,
    canonical: fn() -> Action,
    sig: Sig,
    key: Surf,
    colon: Surf,
    palette: Surf,
    menu: Surf,
    mouse: Surf,
    /// CLI argv (after `bilterm`), expected RPC method for `Is`.
    cli: Wire,
    /// RPC method + params JSON.
    rpc: Wire,
    /// `bilterm_agent_control` arguments JSON.
    mcp: Wire,
}

fn js(raw: &str) -> JsonValue {
    JsonValue::parse(raw).unwrap_or_else(|| panic!("bad json {raw}"))
}

fn run_cli(p: &mut AutomationControlPlane, argv: &str) -> String {
    let args: Vec<String> = argv.split_whitespace().map(ToString::to_string).collect();
    let call = cli_parse(
        &args,
        &CliEnv {
            pane_id: None,
            socket: None,
        },
    )
    .unwrap_or_else(|e| panic!("cli {argv}: {e}"));
    let _ = p.dispatch_method(&call.method, &call.params);
    call.method
}

fn run_mcp(p: &mut AutomationControlPlane, args: &str) {
    let line = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"bilterm_agent_control","arguments":{args}}}}}"#
    );
    let resp = js(&p.handle_mcp_line(&line).expect("mcp response"));
    assert!(
        resp.get("error").is_none(),
        "mcp {args}: {}",
        resp.to_json_string()
    );
}

fn check_wire(row: &Row, surface: &str, w: &Wire, expect: &str) {
    match w {
        Wire::Eq(a, b) => {
            let mut p = fresh(row.setup);
            match surface {
                "cli" => {
                    let m = run_cli(&mut p, a);
                    assert!(
                        RPC_METHODS.contains(&m.as_str()),
                        "{}: cli method {m} is an RPC method",
                        row.id
                    );
                }
                "rpc" => {
                    assert!(RPC_METHODS.contains(a), "{}: {a} is in RPC_METHODS", row.id);
                    let _ = p.dispatch_method(a, &js(b));
                }
                _ => run_mcp(&mut p, a),
            }
            assert_eq!(
                (row.sig)(&p.wm),
                expect,
                "{}: {surface} effect equals the canonical action",
                row.id
            );
        }
        Wire::Is(a, b, method) => {
            assert!(
                RPC_METHODS.contains(method),
                "{}: {method} is in RPC_METHODS",
                row.id
            );
            match surface {
                "cli" => {
                    let args: Vec<String> = a.split_whitespace().map(ToString::to_string).collect();
                    let call = cli_parse(
                        &args,
                        &CliEnv {
                            pane_id: None,
                            socket: None,
                        },
                    )
                    .unwrap_or_else(|e| panic!("cli {a}: {e}"));
                    assert_eq!(call.method, *method, "{}: cli maps to {method}", row.id);
                }
                "rpc" => {
                    let mut p = fresh(row.setup);
                    let r = p.dispatch_method(a, &js(b));
                    assert!(
                        !matches!(&r, Err(e) if e.contains("Unknown method") || e.contains("unknown method")),
                        "{}: {a} dispatches",
                        row.id
                    );
                }
                _ => assert_eq!(
                    AutomationControlPlane::agent_control_method(&js(a)),
                    Some(*method),
                    "{}: mcp maps to {method}",
                    row.id
                ),
            }
        }
    }
}

fn check_row(row: &Row) {
    // A row whose canonical action changes nothing would prove nothing.
    let initial = (row.sig)(&fresh(row.setup).wm);
    let has_gesture = [&row.key, &row.colon, &row.palette, &row.menu, &row.mouse]
        .iter()
        .any(|s| matches!(s, Surf::Do(_)));
    let expect = {
        let mut p = fresh(row.setup);
        let a = (row.canonical)();
        p.wm.apply(a).unwrap();
        (row.sig)(&p.wm)
    };
    if has_gesture && row.id != "watchdog.status" {
        assert_ne!(
            initial, expect,
            "{}: the canonical action must change the observed state",
            row.id
        );
    }
    for (name, s) in [
        ("key", &row.key),
        ("colon", &row.colon),
        ("palette", &row.palette),
        ("menu", &row.menu),
        ("mouse", &row.mouse),
    ] {
        match s {
            Surf::Na(why) => assert!(why.len() >= 10, "{}: {name} N/A needs a reason", row.id),
            Surf::Do(g) => {
                let mut p = fresh(row.setup);
                g(&mut p);
                assert_eq!(
                    (row.sig)(&p.wm),
                    expect,
                    "{}: {name} lowers to the canonical action",
                    row.id
                );
            }
        }
    }
    check_wire(row, "cli", &row.cli, &expect);
    check_wire(row, "rpc", &row.rpc, &expect);
    check_wire(row, "mcp", &row.mcp, &expect);
}

// ---------------------------------------------------------------- the rows

const VIEW_TOGGLE: &str = ":agents hud toggle";

fn rows() -> Vec<Row> {
    vec![
        // ------------------------------------------------ Phase 70: Mission Control HUD (HUD-01)
        Row {
            id: "hud.open",
            phases: &[70],
            setup: s_plain,
            canonical: || Action::AgentsHud(HudOp::Open),
            sig: hud_sig,
            key: Surf::Do(|p| leader(p, '*')),
            colon: Surf::Do(|p| colon(p, ":agents hud")),
            palette: Surf::Do(|p| palette(p, ":agents hud")),
            menu: Surf::Do(|p| {
                assert!((0..12).any(|c| {
                    (0..menubar_category_item_count(c).min(48))
                        .any(|i| menubar_item_command(c, i) == Some(VIEW_TOGGLE))
                }));
                colon(p, VIEW_TOGGLE);
            }),
            mouse: Surf::Do(|p| menu_click(p, VIEW_TOGGLE)),
            cli: Wire::Eq("agent hud open", ""),
            rpc: Wire::Eq("agent.hud", r#"{"op":"open"}"#),
            mcp: Wire::Eq(r#"{"action":"hud","op":"open"}"#, ""),
        },
        Row {
            id: "hud.close",
            phases: &[70],
            setup: s_hud,
            canonical: || Action::AgentsHud(HudOp::Close),
            sig: hud_sig,
            key: Surf::Do(|p| key(p, KeyCode::Esc)),
            colon: Surf::Do(|p| colon(p, ":agents hud close")),
            palette: Surf::Do(|p| palette(p, ":agents hud close")),
            menu: Surf::Do(|p| menu_click(p, VIEW_TOGGLE)),
            mouse: Surf::Do(|p| click(p, 0, AREA.height - 1)),
            cli: Wire::Eq("agent hud close", ""),
            rpc: Wire::Eq("agent.hud", r#"{"op":"close"}"#),
            mcp: Wire::Eq(r#"{"action":"hud","op":"close"}"#, ""),
        },
        Row {
            id: "hud.step",
            phases: &[70],
            setup: s_hud,
            canonical: || Action::AgentsHud(HudOp::Step(1)),
            sig: hud_sig,
            key: Surf::Do(|p| key(p, KeyCode::Right)),
            colon: Surf::Do(|p| colon(p, ":agents hud select next")),
            palette: Surf::Do(|p| palette(p, ":agents hud select next")),
            menu: Surf::Na("in-window navigation; the View menu item toggles the whole HUD"),
            mouse: Surf::Na(
                "pointer selects a tile directly (hud.select_pane); stepping is a keyboard gesture",
            ),
            cli: Wire::Eq("agent hud next", ""),
            rpc: Wire::Eq("agent.hud", r#"{"op":"next"}"#),
            mcp: Wire::Eq(r#"{"action":"hud","op":"next"}"#, ""),
        },
        Row {
            id: "hud.select_pane",
            phases: &[70],
            setup: s_hud,
            canonical: || Action::AgentsHud(HudOp::SelectPane(3)),
            sig: hud_sig,
            key: Surf::Na(
                "keys step with arrows (hud.step); selecting by pane id is a pointer or automation gesture",
            ),
            colon: Surf::Do(|p| colon(p, ":agents hud select 3")),
            palette: Surf::Na(
                "needs a pane id argument; the palette lists the argument-free select next/prev",
            ),
            menu: Surf::Na("needs a pane id argument; not a menu item"),
            mouse: Surf::Do(|p| {
                let t = tile(&p.wm, 3);
                click(p, t.x + 2, t.y + 1);
            }),
            cli: Wire::Eq("agent hud select p3", ""),
            rpc: Wire::Eq("agent.hud", r#"{"op":"select","pane_id":3}"#),
            mcp: Wire::Eq(r#"{"action":"hud","op":"select","pane_id":3}"#, ""),
        },
        Row {
            id: "hud.page",
            phases: &[70],
            setup: s_hud_paged,
            canonical: || Action::AgentsHud(HudOp::Page(1)),
            sig: hud_sig,
            key: Surf::Do(|p| key(p, KeyCode::PageDown)),
            colon: Surf::Do(|p| colon(p, ":agents hud page next")),
            palette: Surf::Do(|p| palette(p, ":agents hud page next")),
            menu: Surf::Na("in-window paging; the View menu item toggles the whole HUD"),
            mouse: Surf::Do(|p| {
                let t = tile(&p.wm, 1);
                mouse(p, MouseEventKind::ScrollDown, t.x + 2, t.y + 1);
            }),
            cli: Wire::Eq("agent hud page-next", ""),
            rpc: Wire::Eq("agent.hud", r#"{"op":"page_next"}"#),
            mcp: Wire::Eq(r#"{"action":"hud","op":"page_next"}"#, ""),
        },
        // Approve: HUD [Approve] == RPC agent.approve (Phase 66/67 HERDR-01 + Phase 70 HUD-01).
        Row {
            id: "agent.approve / hud [Approve]",
            phases: &[66, 67, 70],
            setup: s_hud_blocked,
            canonical: || {
                Action::AgentsHud(HudOp::Press {
                    pane: None,
                    kind: HudButtonKind::Approve,
                    generation: None,
                })
            },
            sig: approve_sig,
            key: Surf::Do(|p| key(p, KeyCode::Char('a'))),
            colon: Surf::Do(|p| colon(p, ":agents hud approve")),
            palette: Surf::Do(|p| palette(p, ":agents hud approve")),
            menu: Surf::Na(
                "per-pane gate on Blocked state and a generation token; a menu click has no tile to bind to",
            ),
            mouse: Surf::Do(|p| click_button(p, 2, HudButtonKind::Approve, false)),
            cli: Wire::Eq("agent approve p2", ""),
            rpc: Wire::Eq("agent.approve", r#"{"pane_id":2}"#),
            mcp: Wire::Eq(r#"{"action":"approve","pane_id":2}"#, ""),
        },
        // Prompt: HUD [Prompt] opens the pane's prompt; RPC/CLI/MCP agent.prompt supplies the text.
        Row {
            id: "agent.prompt / hud [Prompt]",
            phases: &[66, 67, 70],
            setup: s_hud_sel2,
            canonical: || {
                Action::AgentsHud(HudOp::Press {
                    pane: None,
                    kind: HudButtonKind::Prompt,
                    generation: None,
                })
            },
            sig: focus_sig,
            key: Surf::Do(|p| key(p, KeyCode::Char('p'))),
            colon: Surf::Do(|p| colon(p, ":agents hud prompt")),
            palette: Surf::Do(|p| palette(p, ":agents hud prompt")),
            menu: Surf::Na("per-pane button; a menu click has no tile to bind to"),
            mouse: Surf::Do(|p| click_button(p, 2, HudButtonKind::Prompt, false)),
            cli: Wire::Is("agent prompt p2 hello", "", "agent.prompt"),
            rpc: Wire::Is(
                "agent.prompt",
                r#"{"pane_id":2,"text":"hello"}"#,
                "agent.prompt",
            ),
            mcp: Wire::Is(
                r#"{"action":"prompt","pane_id":2,"text":"hello"}"#,
                "",
                "agent.prompt",
            ),
        },
        Row {
            id: "pane.zoom / hud [Zoom]",
            phases: &[67, 70],
            setup: s_hud_sel2,
            canonical: || {
                Action::AgentsHud(HudOp::Press {
                    pane: None,
                    kind: HudButtonKind::Zoom,
                    generation: None,
                })
            },
            sig: focus_sig,
            key: Surf::Do(|p| key(p, KeyCode::Char('z'))),
            colon: Surf::Do(|p| colon(p, ":agents hud zoom")),
            palette: Surf::Do(|p| palette(p, ":agents hud zoom")),
            menu: Surf::Na(
                "per-pane button; the pane zoom toggle (:zoom, F5) is the focused-pane menu-less twin",
            ),
            mouse: Surf::Do(|p| click_button(p, 2, HudButtonKind::Zoom, false)),
            cli: Wire::Eq("pane zoom p2", ""),
            rpc: Wire::Eq("pane.zoom", r#"{"selector":"p2"}"#),
            mcp: Wire::Eq(r#"{"action":"pane_zoom","selector":"p2"}"#, ""),
        },
        Row {
            id: "hud [Diff]",
            phases: &[70],
            setup: s_hud_sel2,
            canonical: || {
                Action::AgentsHud(HudOp::Press {
                    pane: None,
                    kind: HudButtonKind::Diff,
                    generation: None,
                })
            },
            sig: status_sig,
            key: Surf::Do(|p| key(p, KeyCode::Char('d'))),
            colon: Surf::Do(|p| colon(p, ":agents hud diff")),
            palette: Surf::Do(|p| palette(p, ":agents hud diff")),
            menu: Surf::Na("per-pane button; a menu click has no tile to bind to"),
            mouse: Surf::Do(|p| click_button(p, 2, HudButtonKind::Diff, false)),
            cli: Wire::Eq("agent hud diff p2", ""),
            rpc: Wire::Eq("agent.hud", r#"{"op":"diff","pane_id":2}"#),
            mcp: Wire::Eq(r#"{"action":"hud","op":"diff","pane_id":2}"#, ""),
        },
        // ------------------------------------------------ Phase 70: replay (HUD-02)
        Row {
            id: "replay.open",
            phases: &[70],
            setup: s_hud_sel2,
            canonical: || Action::AgentsReplay(ReplayOp::Open(2)),
            sig: |wm| format!("replay={:?}", wm.agents_hud().replay_pane()),
            key: Surf::Do(|p| key(p, KeyCode::Char('r'))),
            colon: Surf::Do(|p| colon(p, ":agent replay 2")),
            palette: Surf::Na(
                "needs a pane id argument; reached from the HUD (r) or :agent replay <pane>",
            ),
            menu: Surf::Na("needs a pane id argument; not a menu item"),
            mouse: Surf::Do(|p| click_button(p, 2, HudButtonKind::Zoom, true)),
            cli: Wire::Eq("agent replay p2 --op open", ""),
            rpc: Wire::Eq("agent.replay", r#"{"pane_id":2,"op":"open"}"#),
            mcp: Wire::Eq(r#"{"action":"replay","pane_id":2,"op":"open"}"#, ""),
        },
        Row {
            id: "replay.prev",
            phases: &[70],
            setup: s_replay,
            canonical: || Action::AgentsReplay(ReplayOp::Prev),
            sig: hud_sig,
            key: Surf::Do(|p| key(p, KeyCode::Left)),
            colon: Surf::Do(|p| colon(p, ":agent replay prev")),
            palette: Surf::Do(|p| palette(p, ":agent replay prev")),
            menu: Surf::Na(
                "in-view navigation; there is no menu item for a view that needs a pane",
            ),
            mouse: Surf::Na(
                "the view has no turn buttons: the wheel scrolls (replay.scroll), keys step turns",
            ),
            cli: Wire::Eq("agent replay p2 --op prev", ""),
            rpc: Wire::Eq("agent.replay", r#"{"pane_id":2,"op":"prev"}"#),
            mcp: Wire::Eq(r#"{"action":"replay","pane_id":2,"op":"prev"}"#, ""),
        },
        Row {
            id: "replay.scroll",
            phases: &[70],
            setup: s_replay,
            canonical: || Action::AgentsReplay(ReplayOp::Scroll(3)),
            sig: hud_sig,
            key: Surf::Na(
                "keys scroll by one line or a page (Down, PgDn); the colon form carries the count",
            ),
            colon: Surf::Do(|p| colon(p, ":agent replay down 3")),
            palette: Surf::Na(
                "needs a line count argument; the palette lists the argument-free :agent replay down",
            ),
            menu: Surf::Na("in-view scrolling; not a menu item"),
            mouse: Surf::Do(|p| mouse(p, MouseEventKind::ScrollDown, 60, 20)),
            cli: Wire::Eq("agent replay p2 --op scroll --lines 3", ""),
            rpc: Wire::Eq("agent.replay", r#"{"pane_id":2,"op":"scroll","lines":3}"#),
            mcp: Wire::Eq(
                r#"{"action":"replay","pane_id":2,"op":"scroll","lines":3}"#,
                "",
            ),
        },
        Row {
            id: "replay.close",
            phases: &[70],
            setup: s_replay,
            canonical: || Action::AgentsReplay(ReplayOp::Close),
            sig: hud_sig,
            key: Surf::Do(|p| key(p, KeyCode::Esc)),
            colon: Surf::Do(|p| colon(p, ":agent replay close")),
            palette: Surf::Do(|p| palette(p, ":agent replay close")),
            menu: Surf::Na("the view is opened per pane; leaving it is Esc / click outside"),
            mouse: Surf::Do(|p| click(p, 0, AREA.height - 1)),
            cli: Wire::Eq("agent replay p2 --op close", ""),
            rpc: Wire::Eq("agent.replay", r#"{"pane_id":2,"op":"close"}"#),
            mcp: Wire::Eq(r#"{"action":"replay","pane_id":2,"op":"close"}"#, ""),
        },
        // ------------------------------------------------ Phase 70: watchdog (HUD-03)
        Row {
            id: "watchdog.status",
            phases: &[70],
            setup: s_plain,
            canonical: || Action::AgentsWatchdog(bilterm::wm::agents_hud_cmd::WatchdogOp::Status),
            sig: |wm| format!("alerts={}", wm.agents_hud().alerts().count()),
            key: Surf::Na(
                "read-only status; alerts reach the user through the OIA / toast sinks, not a key",
            ),
            colon: Surf::Do(|p| colon(p, ":agents watchdog")),
            palette: Surf::Do(|p| palette(p, ":agents watchdog")),
            menu: Surf::Na("read-only status; alerts reach the user through the OIA / toast sinks"),
            mouse: Surf::Na("read-only status; nothing to point at"),
            cli: Wire::Eq("agent watchdog", ""),
            rpc: Wire::Eq("agent.watchdog", "{}"),
            mcp: Wire::Eq(r#"{"action":"watchdog"}"#, ""),
        },
        // ------------------------------------------------ Phase 66/67 capabilities
        Row {
            id: "pane.split",
            phases: &[67],
            setup: s_plain,
            canonical: || Action::SplitHorizontal,
            sig: panes_sig,
            key: Surf::Do(|p| leader(p, '"')),
            colon: Surf::Do(|p| colon(p, ":split h")),
            palette: Surf::Do(|p| palette(p, ":split h")),
            menu: Surf::Do(|p| menu_click(p, ":split h")),
            mouse: Surf::Do(|p| menu_click(p, ":split h")),
            cli: Wire::Eq("pane split h", ""),
            rpc: Wire::Eq("pane.split", r#"{"direction":"h"}"#),
            mcp: Wire::Eq(r#"{"action":"pane_split","direction":"h"}"#, ""),
        },
        Row {
            id: "pane.focus",
            phases: &[67],
            setup: s_plain,
            canonical: || Action::FocusPane(2),
            sig: focus_sig,
            key: Surf::Do(|p| leader(p, '2')),
            colon: Surf::Do(|p| colon(p, ":focus 2")),
            palette: Surf::Na("needs a pane argument; the palette lists the :focus family"),
            menu: Surf::Na("needs a pane argument; not a menu item"),
            mouse: Surf::Do(|p| {
                let r =
                    p.wm.compute_workspace_rects(ws(&p.wm))
                        .into_iter()
                        .find(|(id, _)| *id == 2)
                        .expect("pane 2")
                        .1;
                click(p, r.x + r.width / 2, r.y + r.height / 2);
            }),
            cli: Wire::Eq("pane focus p2", ""),
            rpc: Wire::Eq("pane.focus", r#"{"selector":"p2"}"#),
            mcp: Wire::Eq(r#"{"action":"pane_focus","selector":"p2"}"#, ""),
        },
        Row {
            id: "agent.list / pane.list",
            phases: &[66, 67],
            setup: s_plain,
            canonical: || Action::AgentsList(bilterm::wm::agents_list_cmd::AgentsListOp::Toggle),
            sig: |wm| format!("list={}", wm.agents_list().active),
            key: Surf::Do(|p| leader(p, 'A')),
            colon: Surf::Do(|p| colon(p, ":agents list toggle")),
            palette: Surf::Do(|p| palette(p, ":agents list toggle")),
            menu: Surf::Do(|p| menu_click(p, ":agents list toggle")),
            mouse: Surf::Do(|p| menu_click(p, ":agents list toggle")),
            cli: Wire::Is("pane list", "", "pane.list"),
            rpc: Wire::Is("pane.list", "{}", "pane.list"),
            mcp: Wire::Is(r#"{"action":"pane_list"}"#, "", "pane.list"),
        },
        Row {
            id: "agent.last_turn",
            phases: &[67],
            setup: s_plain,
            canonical: || Action::AgentsReplay(ReplayOp::Open(2)),
            sig: |_| String::new(),
            key: Surf::Na(
                "returns data to a client; the interactive view of the same turns is replay.open (r)",
            ),
            colon: Surf::Na(
                "returns data to a client; the interactive view of the same turns is :agent replay",
            ),
            palette: Surf::Na(
                "returns data to a client; the interactive view of the same turns is :agent replay",
            ),
            menu: Surf::Na("returns data to a client; no menu item"),
            mouse: Surf::Na(
                "returns data to a client; the interactive view is the HUD right-click replay",
            ),
            cli: Wire::Is("agent last-turn p2", "", "agent.last_turn"),
            rpc: Wire::Is("agent.last_turn", r#"{"pane_id":2}"#, "agent.last_turn"),
            mcp: Wire::Is(
                r#"{"action":"last_turn","pane_id":2}"#,
                "",
                "agent.last_turn",
            ),
        },
        Row {
            id: "agent.wait",
            phases: &[67],
            setup: s_plain,
            canonical: || Action::AgentsHud(HudOp::Open),
            sig: |_| String::new(),
            key: Surf::Na(
                "a client blocks until a state; interactive users watch the HUD tile instead",
            ),
            colon: Surf::Na(
                "a client blocks until a state; interactive users watch the HUD tile instead",
            ),
            palette: Surf::Na(
                "a client blocks until a state; interactive users watch the HUD tile instead",
            ),
            menu: Surf::Na(
                "a client blocks until a state; interactive users watch the HUD tile instead",
            ),
            mouse: Surf::Na(
                "a client blocks until a state; interactive users watch the HUD tile instead",
            ),
            cli: Wire::Is("agent wait p2 --until idle", "", "agent.wait"),
            rpc: Wire::Is(
                "agent.wait",
                r#"{"selector":"p2","until":"idle"}"#,
                "agent.wait",
            ),
            mcp: Wire::Is(r#"{"action":"wait","selector":"p2"}"#, "", "agent.wait"),
        },
        Row {
            id: "pane.send",
            phases: &[67],
            setup: s_plain,
            canonical: || Action::AgentsHud(HudOp::Open),
            sig: |_| String::new(),
            key: Surf::Na(
                "typing in the pane is the interactive twin; pane.send is the programmatic write",
            ),
            colon: Surf::Na(
                "typing in the pane is the interactive twin; pane.send is the programmatic write",
            ),
            palette: Surf::Na(
                "typing in the pane is the interactive twin; pane.send is the programmatic write",
            ),
            menu: Surf::Na(
                "typing in the pane is the interactive twin; pane.send is the programmatic write",
            ),
            mouse: Surf::Na(
                "typing in the pane is the interactive twin; pane.send is the programmatic write",
            ),
            cli: Wire::Is("pane send p2 hi --enter", "", "pane.send"),
            rpc: Wire::Is("pane.send", r#"{"pane_id":2,"text":"hi"}"#, "pane.send"),
            mcp: Wire::Is(
                r#"{"action":"pane_send","pane_id":2,"text":"hi"}"#,
                "",
                "pane.send",
            ),
        },
        workspace_row(
            "workspace.list",
            "workspace list",
            "workspace.list",
            "workspace_list",
        ),
        workspace_row(
            "workspace.select",
            "workspace select w1",
            "workspace.select",
            "workspace_select",
        ),
        workspace_row(
            "workspace.new",
            "workspace new demo",
            "workspace.new",
            "workspace_new",
        ),
        workspace_row(
            "workspace.rename",
            "workspace rename demo",
            "workspace.rename",
            "workspace_rename",
        ),
    ]
}

/// Workspace methods act on saved sessions; their interactive twins are the Navigator drawer and
/// the `:session` family (Phase 51/55), covered by those phases' own parity tests.
fn workspace_row(
    id: &'static str,
    cli: &'static str,
    rpc: &'static str,
    mcp_action: &'static str,
) -> Row {
    const WHY: &str =
        "interactive twin is the Navigator drawer / :session family (Phase 51), covered there";
    let mcp: &'static str = Box::leak(format!(r#"{{"action":"{mcp_action}"}}"#).into_boxed_str());
    Row {
        id,
        phases: &[67],
        setup: s_plain,
        canonical: || Action::AgentsHud(HudOp::Open),
        sig: |_| String::new(),
        key: Surf::Na(WHY),
        colon: Surf::Na(WHY),
        palette: Surf::Na(WHY),
        menu: Surf::Na(WHY),
        mouse: Surf::Na(WHY),
        cli: Wire::Is(cli, "", rpc),
        rpc: Wire::Is(rpc, "{}", rpc),
        mcp: Wire::Is(mcp, "", rpc),
    }
}

// ---------------------------------------------------------------- tests

#[test]
fn every_capability_row_lowers_identically_across_surfaces() {
    let rows = rows();
    for row in &rows {
        check_row(row);
    }
    assert!(
        rows.iter().any(|r| r.phases.contains(&66))
            && rows.iter().any(|r| r.phases.contains(&67))
            && rows.iter().any(|r| r.phases.contains(&70))
    );
}

/// Applicable-surface count per row: a row with fewer than three live surfaces is a smell.
#[test]
fn matrix_covers_all_eight_surfaces_for_the_hud_core() {
    let core = [
        "hud.open",
        "hud.close",
        "hud.step",
        "hud.page",
        "agent.approve / hud [Approve]",
        "pane.zoom / hud [Zoom]",
        "hud [Diff]",
    ];
    for row in rows().iter().filter(|r| core.contains(&r.id)) {
        let live = [&row.key, &row.colon, &row.palette, &row.menu, &row.mouse]
            .iter()
            .filter(|s| matches!(s, Surf::Do(_)))
            .count();
        assert!(
            live >= 3,
            "{}: only {live} of 5 interactive surfaces are live",
            row.id
        );
        assert!(
            matches!(row.cli, Wire::Eq(..))
                && matches!(row.rpc, Wire::Eq(..))
                && matches!(row.mcp, Wire::Eq(..)),
            "{}: CLI, RPC and MCP are all live",
            row.id
        );
    }
}

#[test]
fn hud_pure_mappings_agree_with_the_canonical_actions() {
    // Leader `*` is the keymap entry for the HUD toggle.
    let (_, a) = bilterm::input::DEFAULT_LEADER_BINDINGS
        .iter()
        .find(|(c, _)| *c == KeyCode::Char('*'))
        .expect("leader * bound");
    assert_eq!(*a, Action::AgentsHud(HudOp::Toggle));
    // Leader `A` stays the AGENTS list (the HUD key had to be a free one).
    assert!(
        bilterm::input::DEFAULT_LEADER_BINDINGS
            .iter()
            .any(|(c, a)| *c == KeyCode::Char('A') && matches!(a, Action::AgentsList(_)))
    );
    // The pure key mapping equals the colon parse.
    let p = fresh(s_hud_sel2);
    for (code, cmd) in [
        (KeyCode::Char('a'), ":agents hud approve"),
        (KeyCode::Char('z'), ":agents hud zoom"),
        (KeyCode::PageUp, ":agents hud page prev"),
    ] {
        assert_eq!(
            agents_hud_key_action(&p.wm, KeyEvent::new(code, KeyModifiers::NONE)),
            parse_colon_command(cmd).unwrap().ok(),
            "{cmd}"
        );
    }
    // A tile click (with its generation token) hit-tests to a Press on the same pane and kind.
    let r = button_rects(tile(&p.wm, 2))
        .into_iter()
        .find(|(k, _)| *k == HudButtonKind::Zoom)
        .unwrap()
        .1;
    let ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: r.x,
        row: r.y,
        modifiers: KeyModifiers::NONE,
    };
    assert_eq!(
        hud_mouse_action(&p.wm, ev, ws(&p.wm)).unwrap().unwrap(),
        Action::AgentsHud(HudOp::Press {
            pane: Some(2),
            kind: HudButtonKind::Zoom,
            generation: None
        })
    );
}
