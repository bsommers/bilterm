//! Phase 51-07: keys and mouse of the AGENTS list window (UIT-03) and the demo leader chords
//! (UIT-01). Every gesture lowers to `Action::AgentsList(..)` through `WindowManager::apply`
//! (UNI-R01); the pure mouse planner lets these tests assert exactly which Action each gesture
//! produces without depending on the state machine of 51-05.

use std::sync::Mutex;

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::input::agents_list_keys::{AGENTS_LIST_BINDINGS, agents_list_action};
use bilterm::input::keymap::{default_global_action, default_leader_action};
use bilterm::input::{
    DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS, DEFAULT_PANE_BINDINGS, handle_wm_key_event,
};
use bilterm::wm::agents_cmd::AgentsOp;
use bilterm::wm::agents_list_cmd::AgentsListOp;
use bilterm::wm::{SplitDirection, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

static ENV_LOCK: Mutex<()> = Mutex::new(());
const CTRL_ALT: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::ALT);
const AREA: Rect = Rect { x: 0, y: 0, width: 120, height: 40 };

/// Isolates config/state directories for tests that drive real key dispatch.
fn with_temp_xdg<T>(f: impl FnOnce() -> T) -> T {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!(
        "bilterm-agents-list-input-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Serialized by ENV_LOCK.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("XDG_STATE_HOME", &dir);
        std::env::set_var("XDG_DATA_HOME", &dir);
    }
    let out = f();
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// A headless 120x40 manager with `panes` panes (list closed).
fn fixture(panes: usize) -> WindowManager {
    let mut wm = WindowManager::new_headless(AREA.height, AREA.width);
    for _ in 1..panes {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.resize_all(AREA).unwrap();
    wm
}

fn press(wm: &mut WindowManager, code: KeyCode, mods: KeyModifiers) {
    handle_wm_key_event(wm, KeyEvent::new(code, mods)).unwrap();
}

fn prompts(wm: &WindowManager) -> Vec<String> {
    wm.panes().values().map(|p| p.prompt().text()).collect()
}

fn list_action(op: AgentsListOp) -> Action {
    Action::AgentsList(op)
}

// ---------------------------------------------------------------- keys

#[test]
fn key_global_chord_toggles_list() {
    with_temp_xdg(|| {
        let mut wm = fixture(2);
        for code in [KeyCode::Char('n'), KeyCode::Char('N')] {
            assert_eq!(
                default_global_action(KeyEvent::new(code, CTRL_ALT), false),
                Some(list_action(AgentsListOp::Toggle))
            );
        }
        assert_eq!(
            default_global_action(KeyEvent::new(KeyCode::Char('N'), CTRL_ALT | KeyModifiers::SHIFT), false),
            Some(list_action(AgentsListOp::Toggle)),
            "the Shift form resolves too"
        );
        press(&mut wm, KeyCode::Char('n'), CTRL_ALT);
        assert!(wm.agents_list().active && wm.agents_list().focused, "chord opens the list");
        press(&mut wm, KeyCode::Char('n'), CTRL_ALT);
        assert!(!wm.agents_list().active, "chord closes it again while focused");
        press(&mut wm, KeyCode::Char('N'), CTRL_ALT | KeyModifiers::SHIFT);
        assert!(wm.agents_list().active, "Ctrl+Alt+Shift+N opens it");
        press(&mut wm, KeyCode::Char('N'), CTRL_ALT | KeyModifiers::SHIFT);
        assert!(!wm.agents_list().active);
    });
}

#[test]
fn key_leader_chords() {
    with_temp_xdg(|| {
        let mut wm = fixture(2);
        press(&mut wm, KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(wm.is_leader_pending());
        press(&mut wm, KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert!(!wm.is_leader_pending());
        assert!(wm.agents_list().active, "leader A opens the list");
        // Focused list: the leader key is not consumed, so the chord still toggles it closed.
        press(&mut wm, KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(wm.is_leader_pending(), "the focused list never eats the leader key");
        press(&mut wm, KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert!(!wm.agents_list().active, "leader A closes it again");

        let colon = |s: &str| parse_colon_command(s).unwrap().unwrap();
        let leader = |c: char| default_leader_action(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        assert_eq!(leader('A'), Some(list_action(AgentsListOp::Toggle)));
        assert_eq!(leader('y'), Some(Action::Agents(AgentsOp::Demo)));
        assert_eq!(leader('Y'), Some(Action::Agents(AgentsOp::DemoStop)));
        assert_eq!(leader('y'), Some(colon(":agents demo")));
        assert_eq!(leader('Y'), Some(colon(":agents demo stop")));
        assert_eq!(leader('A'), Some(colon(":agents list toggle")));
    });
}

#[test]
fn key_chord_works_in_raw_passthrough() {
    with_temp_xdg(|| {
        let mut wm = fixture(1);
        wm.active_pane_mut().toggle_manual_raw_override();
        assert_eq!(
            wm.active_pane().effective_routing_mode(),
            bilterm::pty::line_discipline::InputRoutingMode::RawPtyPassthrough,
            "fixture must be in raw passthrough"
        );
        let before = prompts(&wm);
        press(&mut wm, KeyCode::Char('n'), CTRL_ALT);
        assert!(wm.agents_list().active, "Ctrl+Alt+N reaches the window manager over a raw pane");
        assert_eq!(prompts(&wm), before, "nothing was typed into the pane");
        press(&mut wm, KeyCode::Char('n'), CTRL_ALT);
        assert!(!wm.agents_list().active);
    });
}

#[test]
fn key_table_maps_every_row() {
    let key = |mods, code| agents_list_action(KeyEvent::new(code, mods));
    let n = KeyModifiers::NONE;
    let op = AgentsListOp::clone;
    let expect: Vec<(KeyModifiers, KeyCode, AgentsListOp)> = vec![
        (n, KeyCode::Up, AgentsListOp::Step(-1)),
        (n, KeyCode::Char('k'), AgentsListOp::Step(-1)),
        (n, KeyCode::Down, AgentsListOp::Step(1)),
        (n, KeyCode::Char('j'), AgentsListOp::Step(1)),
        (n, KeyCode::PageUp, AgentsListOp::Page(-1)),
        (n, KeyCode::PageDown, AgentsListOp::Page(1)),
        (n, KeyCode::Home, AgentsListOp::First),
        (n, KeyCode::End, AgentsListOp::Last),
        (n, KeyCode::Enter, AgentsListOp::Jump(None)),
        (n, KeyCode::Char('z'), AgentsListOp::Zoom(None)),
        (n, KeyCode::Char('f'), AgentsListOp::Filter(None)),
        (n, KeyCode::Char('*'), AgentsListOp::Pin(None)),
        (n, KeyCode::Char('['), AgentsListOp::ScaleStep(-10)),
        (n, KeyCode::Char(']'), AgentsListOp::ScaleStep(10)),
        (KeyModifiers::SHIFT, KeyCode::Left, AgentsListOp::Move { dx: -1, dy: 0 }),
        (KeyModifiers::SHIFT, KeyCode::Right, AgentsListOp::Move { dx: 1, dy: 0 }),
        (KeyModifiers::SHIFT, KeyCode::Up, AgentsListOp::Move { dx: 0, dy: -1 }),
        (KeyModifiers::SHIFT, KeyCode::Down, AgentsListOp::Move { dx: 0, dy: 1 }),
        (KeyModifiers::ALT, KeyCode::Left, AgentsListOp::Resize { dw: -1, dh: 0 }),
        (KeyModifiers::ALT, KeyCode::Right, AgentsListOp::Resize { dw: 1, dh: 0 }),
        (KeyModifiers::ALT, KeyCode::Up, AgentsListOp::Resize { dw: 0, dh: -1 }),
        (KeyModifiers::ALT, KeyCode::Down, AgentsListOp::Resize { dw: 0, dh: 1 }),
        (n, KeyCode::Char('d'), AgentsListOp::Dock(None)),
        (n, KeyCode::Esc, AgentsListOp::Dismiss),
    ];
    for (mods, code, want) in &expect {
        assert_eq!(key(*mods, *code), Some(list_action(op(want))), "row {mods:?} {code:?}");
    }
    assert_eq!(AGENTS_LIST_BINDINGS.len(), expect.len(), "the table has exactly the documented rows");
    assert_eq!(key(n, KeyCode::Char('x')), None);
    assert_eq!(key(KeyModifiers::CONTROL, KeyCode::Char('d')), None, "modifiers are exact");
}

#[test]
fn key_focused_swallows_and_leader_passes() {
    with_temp_xdg(|| {
        let mut wm = fixture(2);
        wm.open_agents_list();
        assert!(wm.agents_list().captures_input());
        let before = prompts(&wm);
        press(&mut wm, KeyCode::Char('x'), KeyModifiers::NONE);
        press(&mut wm, KeyCode::Char('q'), KeyModifiers::NONE);
        press(&mut wm, KeyCode::F(12), KeyModifiers::NONE);
        assert_eq!(prompts(&wm), before, "unknown keys are swallowed, nothing reaches the pane");
        assert!(wm.agents_list().active);
        press(&mut wm, KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(wm.is_leader_pending(), "the leader key is not consumed");
        press(&mut wm, KeyCode::Esc, KeyModifiers::NONE);
        assert!(!wm.is_leader_pending());

        // Not focused (blurred): the window intercepts nothing and typing reaches the pane.
        wm.agents_list_mut().blur();
        assert!(wm.agents_list().active && !wm.agents_list().captures_input());
        press(&mut wm, KeyCode::Char('x'), KeyModifiers::NONE);
        press(&mut wm, KeyCode::Char('z'), KeyModifiers::NONE);
        assert_ne!(prompts(&wm), before, "typing reaches the pane when the list is not focused");
        assert!(wm.agents_list().active, "Esc/keys of the window did not run");
    });
}

#[test]
fn key_esc_dismiss_semantics_plan() {
    with_temp_xdg(|| {
        assert_eq!(
            agents_list_action(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(list_action(AgentsListOp::Dismiss))
        );
        // The toggle chord is handled before the table, so the focused window closes like Ctrl+Alt+A.
        let mut wm = fixture(2);
        wm.open_agents_list();
        press(&mut wm, KeyCode::Char('n'), CTRL_ALT);
        assert!(!wm.agents_list().active, "Ctrl+Alt+N while focused toggles (closes)");
        wm.open_agents_list();
        press(&mut wm, KeyCode::Esc, KeyModifiers::NONE);
        assert!(!wm.agents_list().active, "Esc dismisses an unpinned window");
    });
}

#[test]
fn key_drift_no_duplicates_no_shadow() {
    for (i, (m, c, _)) in AGENTS_LIST_BINDINGS.iter().enumerate() {
        for (m2, c2, _) in &AGENTS_LIST_BINDINGS[i + 1..] {
            assert!(!(m == m2 && c == c2), "duplicate row {m:?} {c:?}");
        }
    }
    assert!(AGENTS_LIST_BINDINGS.iter().all(|(_, _, a)| matches!(a, Action::AgentsList(_))));
    // Every row has a colon twin that parses back to the same Action.
    for (_, _, a) in AGENTS_LIST_BINDINGS {
        let Action::AgentsList(op) = a else { unreachable!() };
        let back = parse_colon_command(&op.colon_form()).expect("colon").expect("parses");
        assert_eq!(&back, a, "{} does not round-trip", op.colon_form());
    }
    // The new global rows: exactly Ctrl+Alt+n / Ctrl+Alt+N, no other global row shares the chord.
    let toggles: Vec<_> = DEFAULT_GLOBAL_BINDINGS
        .iter()
        .filter(|(_, _, a)| *a == list_action(AgentsListOp::Toggle))
        .collect();
    assert_eq!(toggles.len(), 2, "Ctrl+Alt+n and Ctrl+Alt+N");
    for (m, c, _) in &toggles {
        assert_eq!(*m, CTRL_ALT);
        let same = DEFAULT_GLOBAL_BINDINGS.iter().filter(|(gm, gc, _)| gm == m && gc == c).count();
        assert_eq!(same, 1, "global chord {m:?} {c:?} is unique");
        assert!(!DEFAULT_PANE_BINDINGS.iter().any(|(pm, pc, _)| pm == m && pc == c), "no pane collision");
    }
    // Leader rows are read live: A, y, Y each appear exactly once and `?` stays HelpToggle.
    for (ch, want) in [
        ('A', list_action(AgentsListOp::Toggle)),
        ('y', Action::Agents(AgentsOp::Demo)),
        ('Y', Action::Agents(AgentsOp::DemoStop)),
        ('?', Action::HelpToggle),
    ] {
        let rows: Vec<_> = DEFAULT_LEADER_BINDINGS.iter().filter(|(c, _)| *c == KeyCode::Char(ch)).collect();
        assert_eq!(rows.len(), 1, "leader {ch} appears once");
        assert_eq!(rows[0].1, want, "leader {ch}");
    }
}

#[test]
fn key_input_calls_only_allow_listed_wm_methods() {
    const ALLOW: &[&str] = &[
        "apply",
        "agents_list",
        "agents_list_mut",
        "agents_list_hit",
        "agents_list_rect",
        "agents_list_layout",
        "panel_space",
        "config",
        "is_leader_pending",
        "agents_window",
        "modal",
    ];
    for (name, src) in [
        ("agents_list_keys.rs", include_str!("../src/input/agents_list_keys.rs")),
            ] {
        let mut rest = src;
        let mut seen = 0;
        while let Some(at) = rest.find("wm.") {
            let after = &rest[at + 3..];
            let ident: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            let is_call = after[ident.len()..].starts_with('(');
            if is_call {
                seen += 1;
                assert!(ALLOW.contains(&ident.as_str()), "{name} calls non-allow-listed wm.{ident}(..)");
            }
            rest = &after[ident.len()..];
        }
        assert!(seen > 0, "{name} should drive the manager through wm.apply");
    }
}

