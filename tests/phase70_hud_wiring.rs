//! Phase 70 wiring tests (HUD-01..03): the Mission Control HUD, the read-only turn replay view and
//! the watchdog, driven through `WindowManager::apply(Action)` on headless workspaces.

use std::sync::Once;
use std::time::Instant;

use bilterm::action::Action;
use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::parse_colon_command;
use bilterm::config::BiltermConfig;
use bilterm::input::agents_hud::{hud_key_action, hud_mouse_action, replay_key_action};
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::shell::ShellLayout;
use bilterm::wm::agent_state::AgentState;
use bilterm::wm::agents_hud::hud_inner;
use bilterm::wm::agents_hud_cmd::{HudOp, ReplayOp};
use bilterm::wm::mission_control_hud::{HudButtonKind, button_rects};
use bilterm::wm::{SplitDirection, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

const AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: 120,
    height: 40,
};

fn isolate() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let root = std::env::temp_dir().join(format!("bt70-wiring-{}", std::process::id()));
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

fn wm_with(panes: usize) -> WindowManager {
    isolate();
    let mut wm = WindowManager::new_headless(AREA.height, AREA.width);
    for _ in 1..panes {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.resize_all(AREA).unwrap();
    wm.update_agent_states(Instant::now());
    wm
}

fn ws(wm: &WindowManager) -> Rect {
    ShellLayout::compute(AREA, wm.config(), wm.pane_count()).workspace
}

fn render_text(wm: &WindowManager) -> String {
    let mut buf = Buffer::empty(AREA);
    WorkspaceWidget::new(wm).render(AREA, &mut buf);
    (0..AREA.height)
        .map(|y| {
            (0..AREA.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn run(wm: &mut WindowManager, cmd: &str) {
    let parsed = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a command: {cmd}"))
        .unwrap_or_else(|e| panic!("{cmd}: {e}"));
    wm.apply(parsed).unwrap();
}

fn set_state(wm: &mut WindowManager, id: usize, st: AgentState) {
    wm.report_agent_state(id, Some(st));
    wm.update_agent_states(Instant::now());
}

fn click(wm: &mut WindowManager, x: u16, y: u16) {
    let ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(wm, ev, AREA).unwrap();
}

fn key(wm: &mut WindowManager, code: KeyCode) {
    handle_wm_key_event(wm, KeyEvent::new(code, KeyModifiers::NONE)).unwrap();
}

/// Button rectangle of `kind` on the tile of `pane`.
fn button_at(wm: &WindowManager, pane: usize, kind: HudButtonKind) -> (u16, u16) {
    let outer = wm.agents_hud_rect(ws(wm)).expect("hud visible");
    let tile = wm
        .agents_hud()
        .hud()
        .layout(hud_inner(outer))
        .into_iter()
        .find(|(id, _)| *id == pane)
        .expect("tile")
        .1;
    let r = button_rects(tile)
        .into_iter()
        .find(|(k, _)| *k == kind)
        .expect("button fits")
        .1;
    (r.x, r.y)
}

// ---------------------------------------------------------------- HUD-01

#[test]
fn hud_opens_toggles_and_renders_a_tile_per_pane() {
    let mut wm = wm_with(3);
    assert!(!render_text(&wm).contains("MISSION CONTROL"));
    run(&mut wm, ":agents hud");
    assert!(wm.agents_hud().active && wm.agents_hud().captures_input());
    assert_eq!(wm.agents_hud().hud().len(), 3);
    let text = render_text(&wm);
    assert!(text.contains("AGENT MISSION CONTROL"), "title");
    assert!(
        text.contains("[Approve]")
            && text.contains("[Prompt]")
            && text.contains("[Diff]")
            && text.contains("[Zoom]")
    );
    run(&mut wm, ":agents hud toggle");
    assert!(!wm.agents_hud().active);
    assert!(!render_text(&wm).contains("MISSION CONTROL"));
    run(&mut wm, ":agents hud toggle");
    assert!(wm.agents_hud().active);
    run(&mut wm, ":agents hud close");
    assert!(!wm.agents_hud().active && !wm.agents_hud().captures_input());
}

#[test]
fn leader_star_and_in_window_keys_lower_to_hud_actions() {
    let mut wm = wm_with(3);
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
    )
    .unwrap();
    key(&mut wm, KeyCode::Char('*'));
    assert!(wm.agents_hud().active, "Leader * opens the HUD");
    let first = wm.agents_hud().hud().selected().unwrap();
    key(&mut wm, KeyCode::Right);
    assert_ne!(
        wm.agents_hud().hud().selected().unwrap(),
        first,
        "Right selects the next tile"
    );
    key(&mut wm, KeyCode::Left);
    assert_eq!(wm.agents_hud().hud().selected().unwrap(), first);
    // A plain letter is swallowed: it never reaches the pane prompt underneath.
    key(&mut wm, KeyCode::Char('x'));
    assert_eq!(wm.active_pane().prompt().text(), "");
    key(&mut wm, KeyCode::Esc);
    assert!(!wm.agents_hud().active, "Esc closes");
    assert_eq!(
        hud_key_action(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
        Some(Action::AgentsHud(HudOp::Press {
            pane: None,
            kind: HudButtonKind::Approve,
            generation: None
        }))
    );
    assert_eq!(
        hud_key_action(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)),
        None
    );
}

#[test]
fn tile_clicks_hit_test_to_the_same_actions_as_colon_forms() {
    let mut wm = wm_with(3);
    run(&mut wm, ":agents hud");
    let (x, y) = button_at(&wm, 2, HudButtonKind::Zoom);
    let ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    };
    let got = hud_mouse_action(&wm, ev, ws(&wm))
        .expect("in window")
        .expect("an action");
    let want = Action::AgentsHud(HudOp::Press {
        pane: Some(2),
        kind: HudButtonKind::Zoom,
        generation: None,
    });
    assert_eq!(got, want);
    assert_eq!(
        parse_colon_command(":agents hud zoom 2").unwrap().unwrap(),
        want
    );
    // Prompt / Diff buttons.
    for (kind, cmd) in [
        (HudButtonKind::Prompt, ":agents hud prompt 3"),
        (HudButtonKind::Diff, ":agents hud diff 3"),
    ] {
        let (x, y) = button_at(&wm, 3, kind);
        let ev = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            hud_mouse_action(&wm, ev, ws(&wm)).unwrap().unwrap(),
            parse_colon_command(cmd).unwrap().unwrap(),
            "{cmd}"
        );
    }
    // A click on a tile body selects it; a click outside the window closes it.
    let outer = wm.agents_hud_rect(ws(&wm)).unwrap();
    let tile = wm
        .agents_hud()
        .hud()
        .layout(hud_inner(outer))
        .into_iter()
        .find(|(id, _)| *id == 3)
        .unwrap()
        .1;
    click(&mut wm, tile.x + 2, tile.y + 1);
    assert_eq!(wm.agents_hud().hud().selected(), Some(3));
    click(&mut wm, 0, AREA.height - 1);
    assert!(!wm.agents_hud().active, "click outside closes");
}

#[test]
fn approve_is_gated_on_blocked_and_the_generation_token() {
    let mut wm = wm_with(2);
    run(&mut wm, ":agents hud");
    // Not blocked: the button is disabled (selects instead) and the op is refused.
    let (x, y) = button_at(&wm, 2, HudButtonKind::Approve);
    let ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    };
    assert_eq!(
        hud_mouse_action(&wm, ev, ws(&wm)).unwrap().unwrap(),
        Action::AgentsHud(HudOp::SelectPane(2))
    );
    let err = wm
        .apply_automation(
            parse_colon_command(":agents hud approve 2")
                .unwrap()
                .unwrap(),
            None,
        )
        .unwrap_err();
    assert!(err.contains("not blocked"), "{err}");

    // Blocked: the click carries the tile's generation token.
    set_state(&mut wm, 2, AgentState::Blocked);
    run(&mut wm, ":agents hud select next");
    run(&mut wm, ":agents hud select prev");
    let got = hud_mouse_action(&wm, ev, ws(&wm)).unwrap().unwrap();
    let Action::AgentsHud(HudOp::Press {
        pane: Some(2),
        kind: HudButtonKind::Approve,
        generation: Some(g),
    }) = got.clone()
    else {
        panic!("expected an Approve press with a token, got {got:?}");
    };
    let before = wm.traffic().len();
    wm.apply_automation(got.clone(), None).unwrap();
    assert!(
        wm.traffic().len() > before,
        "approve is recorded in the traffic bus like agent.approve"
    );

    // The state flips away and back: the old token is stale.
    set_state(&mut wm, 2, AgentState::Working);
    run(&mut wm, ":agents hud select next");
    set_state(&mut wm, 2, AgentState::Blocked);
    run(&mut wm, ":agents hud select prev");
    let stale = Action::AgentsHud(HudOp::Press {
        pane: Some(2),
        kind: HudButtonKind::Approve,
        generation: Some(g),
    });
    let err = wm.apply_automation(stale, None).unwrap_err();
    assert!(err.contains("stale"), "{err}");
}

#[test]
fn prompt_zoom_and_diff_buttons() {
    let mut wm = wm_with(3);
    run(&mut wm, ":agents hud");
    run(&mut wm, ":agents hud diff 2");
    assert!(wm.agents_hud().active, "diff keeps the HUD open");
    assert!(
        wm.active_pane()
            .status_message()
            .unwrap()
            .contains("not part of a swarm")
    );
    run(&mut wm, ":agents hud prompt 2");
    assert!(!wm.agents_hud().active);
    assert_eq!(wm.active_pane_id(), 2, "prompt focuses the pane");
    run(&mut wm, ":agents hud");
    run(&mut wm, ":agents hud zoom 3");
    assert_eq!(wm.active_pane_id(), 3);
    assert_eq!(wm.zoomed_pane(), Some(3));
}

#[test]
fn hud_open_render_select_and_tick_never_resize_a_pty() {
    let mut wm = wm_with(4);
    let before: Vec<(usize, (u16, u16), u64)> = wm
        .ordered_pane_ids()
        .into_iter()
        .map(|id| {
            (
                id,
                wm.pane(id).unwrap().vt().size(),
                wm.pane(id).unwrap().resize_events(),
            )
        })
        .collect();
    run(&mut wm, ":agents hud");
    for cmd in [
        ":agents hud select next",
        ":agents hud page next",
        ":agents hud page prev",
        ":agents hud diff",
        ":agents hud select 2",
    ] {
        run(&mut wm, cmd);
        let _ = render_text(&wm);
    }
    let now = Instant::now();
    for i in 0..5 {
        wm.agents_hud_tick_at(now, 1000 + i);
    }
    key(&mut wm, KeyCode::Down);
    run(&mut wm, ":agents hud close");
    for (id, size, events) in before {
        let p = wm.pane(id).unwrap();
        assert_eq!(
            (p.vt().size(), p.resize_events()),
            (size, events),
            "pane #{id} was resized by the HUD"
        );
    }
    // The counter is live: a real viewport change does bump it.
    let small = Rect::new(0, 0, 60, 20);
    let id = wm.ordered_pane_ids()[0];
    let before = wm.pane(id).unwrap().resize_events();
    wm.resize_all(small).unwrap();
    assert!(wm.pane(id).unwrap().resize_events() > before);
}

#[test]
fn masked_tiles_never_show_pane_text() {
    let mut wm = wm_with(2);
    wm.pane_mut(2)
        .unwrap()
        .process_pty_output(b"hunter2-secret line\r\n[sudo] password for operator: ");
    wm.pane_mut(2).unwrap().line_discipline_mut().echo_enabled = false;
    assert!(wm.pane(2).unwrap().is_password_prompt());
    run(&mut wm, ":agents hud");
    assert!(wm.agents_hud().hud().card(2).unwrap().masked);
    assert!(wm.agents_hud().hud().card(2).unwrap().preview.is_empty());
    let text = render_text(&wm);
    assert!(text.contains("masked"), "masked marker is drawn");
    assert!(!text.contains("hunter2-secret"));
    let mut cp = AutomationControlPlane::new_headless(AREA.height, AREA.width);
    cp.wm.split_focused(SplitDirection::Vertical).unwrap();
    cp.wm
        .pane_mut(2)
        .unwrap()
        .process_pty_output(b"[sudo] password for operator: ");
    cp.wm
        .pane_mut(2)
        .unwrap()
        .line_discipline_mut()
        .echo_enabled = false;
    let snap = cp
        .dispatch_method(
            "agent.hud",
            &JsonValue::obj([("op", JsonValue::String("open".to_string()))]),
        )
        .unwrap();
    let card = snap
        .get("cards")
        .and_then(JsonValue::as_array)
        .unwrap()
        .iter()
        .find(|c| c.get("pane_id").and_then(JsonValue::as_usize) == Some(2))
        .unwrap()
        .clone();
    assert_eq!(card.get("masked").and_then(JsonValue::as_bool), Some(true));
    assert_eq!(
        card.get("preview")
            .and_then(JsonValue::as_array)
            .map(<[JsonValue]>::len),
        Some(0)
    );
}

#[test]
fn tile_telemetry_carries_state_queue_depth_and_a_page_cap() {
    let mut wm = wm_with(3);
    set_state(&mut wm, 2, AgentState::Working);
    wm.prompt_queue_mut().enqueue(2, "next task").unwrap();
    wm.prompt_queue_mut().enqueue(2, "and another").unwrap();
    run(&mut wm, ":agents hud");
    let c = wm.agents_hud().hud().card(2).unwrap();
    assert_eq!((c.state, c.queue_depth), (AgentState::Working, 2));
    assert!(wm.agents_hud().page_size() <= 12, "tile page cap");
}

// ---------------------------------------------------------------- HUD-02

fn run_turn(wm: &mut WindowManager, id: usize, prompt: &str, output: &[u8], t0: u64) {
    wm.agents_hud_note_prompt(id, prompt);
    set_state(wm, id, AgentState::Working);
    wm.agents_hud_tick_at(Instant::now(), t0);
    wm.pane_mut(id).unwrap().process_pty_output(output);
    wm.agents_hud_tick_at(Instant::now(), t0 + 1);
    set_state(wm, id, AgentState::Idle);
    wm.agents_hud_tick_at(Instant::now(), t0 + 2);
}

#[test]
fn replay_log_is_fed_by_state_transitions_and_navigated_read_only() {
    let mut wm = wm_with(2);
    wm.agents_hud_tick_at(Instant::now(), 900); // baseline
    run_turn(
        &mut wm,
        2,
        "write the parser",
        b"parsing...\r\nwrote src/parser.rs\r\n",
        1000,
    );
    run_turn(
        &mut wm,
        2,
        "now the tests",
        b"running tests\r\nall green\r\n",
        2000,
    );
    let log = wm.agents_hud().log(2).expect("log");
    assert_eq!(log.len(), 2);
    assert_eq!(log.turns().next().unwrap().prompt, "write the parser");
    assert!(
        log.turns()
            .next()
            .unwrap()
            .snapshot
            .iter()
            .any(|l| l.contains("parser.rs"))
    );

    let traffic_before = wm.traffic().len();
    run(&mut wm, ":agent replay 2");
    assert_eq!(wm.agents_hud().replay_pane(), Some(2));
    assert!(render_text(&wm).contains("AGENT REPLAY #2"));
    assert_eq!(
        wm.agents_hud().log(2).unwrap().current().unwrap().prompt,
        "now the tests",
        "starts on the newest turn"
    );
    key(&mut wm, KeyCode::Left);
    assert_eq!(
        wm.agents_hud().log(2).unwrap().current().unwrap().prompt,
        "write the parser"
    );
    let text = render_text(&wm);
    assert!(text.contains("write the parser") && text.contains("parser.rs"));
    key(&mut wm, KeyCode::Right);
    assert_eq!(
        wm.agents_hud().log(2).unwrap().current().unwrap().prompt,
        "now the tests"
    );
    key(&mut wm, KeyCode::Down);
    key(&mut wm, KeyCode::Up);
    // Typing is swallowed; nothing reaches the pane or the traffic bus.
    key(&mut wm, KeyCode::Char('x'));
    key(&mut wm, KeyCode::Enter);
    assert_eq!(wm.pane(2).unwrap().prompt().text(), "");
    assert_eq!(wm.traffic().len(), traffic_before, "replay writes nothing");
    key(&mut wm, KeyCode::Esc);
    assert_eq!(wm.agents_hud().replay_pane(), None);
    assert_eq!(
        replay_key_action(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), 10),
        Some(Action::AgentsReplay(ReplayOp::Prev))
    );
    assert!(
        wm.apply_automation(
            parse_colon_command(":agent replay 99").unwrap().unwrap(),
            None
        )
        .is_err()
    );
}

#[test]
fn replay_survives_masked_output() {
    let mut wm = wm_with(2);
    wm.agents_hud_tick_at(Instant::now(), 10);
    set_state(&mut wm, 2, AgentState::Working);
    wm.agents_hud_tick_at(Instant::now(), 11);
    wm.pane_mut(2)
        .unwrap()
        .process_pty_output(b"[sudo] password for operator: ");
    wm.pane_mut(2).unwrap().line_discipline_mut().echo_enabled = false;
    wm.agents_hud_tick_at(Instant::now(), 12);
    set_state(&mut wm, 2, AgentState::Idle);
    wm.agents_hud_tick_at(Instant::now(), 13);
    let t = wm
        .agents_hud()
        .log(2)
        .unwrap()
        .turns()
        .next()
        .expect("turn");
    assert!(
        t.snapshot.iter().all(|l| !l.contains("password")),
        "no-echo prompt text is not recorded: {:?}",
        t.snapshot
    );
}

// ---------------------------------------------------------------- HUD-03

fn watchdog_wm(alert: &[&str]) -> WindowManager {
    let mut wm = wm_with(2);
    let w = &mut wm.config_mut().agents.watchdog;
    w.stall_secs = 5;
    w.loop_window = 10;
    w.loop_repeat = 3;
    w.cooldown_secs = 100;
    w.alert = alert.iter().map(ToString::to_string).collect();
    wm.agents_hud_tick_at(Instant::now(), 1000);
    wm
}

#[test]
fn stall_alert_surfaces_per_sink_and_respects_cooldown() {
    let mut wm = watchdog_wm(&["event"]);
    set_state(&mut wm, 2, AgentState::Working);
    wm.agents_hud_tick_at(Instant::now(), 1001);
    wm.agents_hud_tick_at(Instant::now(), 1004);
    assert_eq!(wm.agents_hud().alerts().count(), 0, "not stalled yet");
    wm.agents_hud_tick_at(Instant::now(), 1010);
    let a: Vec<_> = wm.agents_hud().alerts().collect();
    assert_eq!((a.len(), a[0].pane, a[0].kind), (1, 2, "stall"));
    assert!(
        wm.active_pane().status_message().is_none(),
        "event-only config shows nothing in the OIA"
    );
    assert_eq!(wm.take_watchdog_events().len(), 1);
    wm.agents_hud_tick_at(Instant::now(), 1050);
    assert_eq!(wm.agents_hud().alerts().count(), 1, "cooldown dedupes");
    wm.agents_hud_tick_at(Instant::now(), 1200);
    assert_eq!(
        wm.agents_hud().alerts().count(),
        2,
        "alerts again after the cooldown"
    );
    // The watchdog never kills or writes: the pane is untouched.
    assert!(!wm.pane(2).unwrap().has_exited());
}

#[test]
fn loop_alert_and_oia_toast_sinks() {
    for (sink, expect_toast) in [("oia", false), ("toast", true)] {
        let mut wm = watchdog_wm(&[sink]);
        set_state(&mut wm, 2, AgentState::Working);
        for i in 0..4u64 {
            wm.pane_mut(2)
                .unwrap()
                .process_pty_output(b"retrying step 7\r\n");
            wm.agents_hud_tick_at(Instant::now(), 1001 + i);
        }
        let a: Vec<_> = wm.agents_hud().alerts().collect();
        assert!(
            a.iter().any(|r| r.kind == "loop" && r.pane == 2),
            "{sink}: {a:?}"
        );
        let msg = wm
            .active_pane()
            .status_message()
            .unwrap_or_default()
            .to_string();
        assert!(msg.contains("WATCHDOG #2 loop"), "{sink}: {msg}");
        assert!(
            wm.take_watchdog_events().is_empty(),
            "{sink}: no event sink configured"
        );
        let _ = expect_toast;
    }
}

#[test]
fn disabled_watchdog_is_silent() {
    let mut wm = wm_with(2);
    wm.config_mut().agents.watchdog.enabled = false;
    wm.config_mut().agents.watchdog.stall_secs = 1;
    wm.agents_hud_tick_at(Instant::now(), 1000);
    set_state(&mut wm, 2, AgentState::Working);
    for i in 0..6u64 {
        wm.pane_mut(2).unwrap().process_pty_output(b"same line\r\n");
        wm.agents_hud_tick_at(Instant::now(), 1001 + i);
    }
    wm.agents_hud_tick_at(Instant::now(), 5000);
    assert_eq!(wm.agents_hud().alerts().count(), 0);
    assert!(wm.take_watchdog_events().is_empty());
    run(&mut wm, ":agents watchdog");
    let data: JsonValue = JsonValue::parse(&wm.take_command_data().expect("status data")).unwrap();
    assert_eq!(
        data.get("enabled").and_then(JsonValue::as_bool),
        Some(false)
    );
}

#[test]
fn watchdog_config_parses_validates_and_round_trips() {
    let cfg = BiltermConfig::from_toml_str(
        "[agents.claude]\nblocked = [\"approve?\"]\n[agents.watchdog]\nenabled = true\nstall_secs = 30\nloop_window = 12\nloop_repeat = 3\nalert = [\"oia\", \"event\"]\ncooldown = 45\n",
    )
    .unwrap();
    let w = &cfg.agents.watchdog;
    assert_eq!(
        (w.stall_secs, w.loop_window, w.loop_repeat, w.cooldown_secs),
        (30, 12, 3, 45)
    );
    assert_eq!(w.alert, ["oia", "event"]);
    assert!(w.validate().is_ok());
    assert_eq!(
        cfg.agents["claude"].blocked,
        ["approve?"],
        "profiles still parse beside [agents.watchdog]"
    );
    assert!(!cfg.agents.contains_key("watchdog"));
    let back = BiltermConfig::from_toml_str(&toml::to_string(&cfg).unwrap()).unwrap();
    assert_eq!(back.agents, cfg.agents);
    // Defaults: enabled, all three sinks, nothing serialised.
    let d = BiltermConfig::default();
    assert!(d.agents.watchdog.enabled && d.agents.watchdog.alert.len() == 3);
    assert!(!toml::to_string(&d).unwrap().contains("watchdog"));
    for bad in [
        "stall_secs = 0",
        "loop_window = 1",
        "loop_repeat = 99",
        "alert = [\"pager\"]",
        "cooldown = 999999",
    ] {
        let c = BiltermConfig::from_toml_str(&format!("[agents.watchdog]\n{bad}\n")).unwrap();
        assert!(
            c.agents.watchdog.validate().is_err(),
            "{bad} must be rejected"
        );
    }
}

// ---------------------------------------------------------------- RPC / MCP

fn plane(panes: usize) -> AutomationControlPlane {
    isolate();
    let mut cp = AutomationControlPlane::new_headless(AREA.height, AREA.width);
    for _ in 1..panes {
        cp.wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    cp
}

fn p(entries: &[(&str, JsonValue)]) -> JsonValue {
    JsonValue::obj(entries.iter().map(|(k, v)| (*k, v.clone())))
}

fn st(v: &str) -> JsonValue {
    JsonValue::String(v.to_string())
}

#[test]
fn rpc_hud_replay_watchdog_and_events() {
    let mut cp = plane(3);
    let snap = cp.dispatch_method("agent.hud", &p(&[])).unwrap();
    assert_eq!(
        snap.get("tile_count").and_then(JsonValue::as_usize),
        Some(3)
    );
    assert_eq!(snap.get("active").and_then(JsonValue::as_bool), Some(false));
    cp.dispatch_method("agent.hud", &p(&[("op", st("open"))]))
        .unwrap();
    assert!(
        cp.wm.agents_hud().active,
        "agent.hud op=open lowers to the same Action"
    );
    assert!(
        cp.dispatch_method(
            "agent.hud",
            &p(&[("op", st("approve")), ("pane_id", JsonValue::Number(2))])
        )
        .is_err()
    );
    assert!(
        cp.dispatch_method("agent.hud", &p(&[("op", st("nope"))]))
            .is_err()
    );

    cp.wm.config_mut().agents.watchdog.stall_secs = 2;
    cp.wm.agents_hud_tick_at(Instant::now(), 100);
    cp.wm.report_agent_state(2, Some(AgentState::Working));
    cp.wm.update_agent_states(Instant::now());
    cp.wm.agents_hud_tick_at(Instant::now(), 101);
    cp.wm.pane_mut(2).unwrap().process_pty_output(b"hello\r\n");
    cp.wm.agents_hud_tick_at(Instant::now(), 102);
    cp.wm.report_agent_state(2, Some(AgentState::Idle));
    cp.wm.update_agent_states(Instant::now());
    cp.wm.agents_hud_tick_at(Instant::now(), 103);
    let list = cp
        .dispatch_method("agent.replay", &p(&[("pane_id", JsonValue::Number(2))]))
        .unwrap();
    assert_eq!(
        list.get("turn_count").and_then(JsonValue::as_usize),
        Some(1)
    );
    let id = list.get("turns").and_then(JsonValue::as_array).unwrap()[0]
        .get("turn_id")
        .and_then(JsonValue::as_i64)
        .unwrap();
    let one = cp
        .dispatch_method(
            "agent.replay",
            &p(&[
                ("pane_id", JsonValue::Number(2)),
                ("turn_id", JsonValue::Number(id)),
            ]),
        )
        .unwrap();
    assert!(
        one.get("output")
            .and_then(JsonValue::as_array)
            .unwrap()
            .iter()
            .any(|l| l.as_str() == Some("hello"))
    );
    assert!(
        cp.dispatch_method(
            "agent.replay",
            &p(&[
                ("pane_id", JsonValue::Number(2)),
                ("turn_id", JsonValue::Number(999))
            ])
        )
        .is_err()
    );
    cp.dispatch_method(
        "agent.replay",
        &p(&[("pane_id", JsonValue::Number(2)), ("op", st("open"))]),
    )
    .unwrap();
    assert_eq!(cp.wm.agents_hud().replay_pane(), Some(2));

    cp.wm.report_agent_state(3, Some(AgentState::Working));
    cp.wm.update_agent_states(Instant::now());
    cp.wm.agents_hud_tick_at(Instant::now(), 200);
    cp.wm.agents_hud_tick_at(Instant::now(), 210);
    let w = cp.dispatch_method("agent.watchdog", &p(&[])).unwrap();
    assert_eq!(w.get("enabled").and_then(JsonValue::as_bool), Some(true));
    assert!(
        w.get("alerts")
            .and_then(JsonValue::as_array)
            .is_some_and(|a| a
                .iter()
                .any(|x| x.get("kind").and_then(JsonValue::as_str) == Some("stall")))
    );
    let ev = cp.dispatch_method("events.poll", &p(&[])).unwrap();
    assert!(
        ev.get("events")
            .and_then(JsonValue::as_array)
            .is_some_and(|e| e
                .iter()
                .any(|x| x.get("type").and_then(JsonValue::as_str) == Some("watchdog_alert")))
    );
}

#[test]
fn mcp_agent_control_tool_maps_to_the_rpc_methods() {
    let mut cp = plane(2);
    let call = |cp: &mut AutomationControlPlane, args: &str| {
        let line = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"bilterm_agent_control","arguments":{args}}}}}"#
        );
        JsonValue::parse(&cp.handle_mcp_line(&line).unwrap()).unwrap()
    };
    let r = call(&mut cp, r#"{"action":"hud"}"#);
    let text = r
        .get("result")
        .and_then(|x| x.get("content"))
        .and_then(JsonValue::as_array)
        .unwrap()[0]
        .get("text")
        .and_then(JsonValue::as_str)
        .unwrap()
        .to_string();
    let via_mcp = JsonValue::parse(&text).unwrap();
    let direct = cp.dispatch_method("agent.hud", &p(&[])).unwrap();
    assert_eq!(via_mcp.get("tile_count"), direct.get("tile_count"));
    let bad = call(&mut cp, r#"{"action":"zzz"}"#);
    let errored = bad.get("error").is_some()
        || bad
            .get("result")
            .and_then(|x| x.get("isError"))
            .and_then(JsonValue::as_bool)
            == Some(true);
    assert!(errored, "{}", bad.to_json_string());
}
