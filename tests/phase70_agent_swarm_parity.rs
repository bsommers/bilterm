//! Phase 70 `HUD-04` / `UNI-R01`: every Phase 68-70 agent capability (context bus, prompt queue,
//! approve/deny, swarm, Mission Control HUD, turn replay) lowers to the SAME `Action` from the
//! colon command, the command palette, the menu model and JSON-RPC, and executing through colon
//! vs RPC yields equivalent state. No default key chord exists for these (leader `A` is the agents
//! list), so the key surface is asserted absent rather than silently skipped.
#![cfg(unix)]

use bilterm::action::Action;
use bilterm::api::{AutomationControlPlane, JsonValue, RPC_METHODS};
use bilterm::colon_command::parse_colon_command;
use bilterm::ui::menu_model::MENUBAR_CATEGORIES;
use bilterm::ui::modals::PALETTE_CATALOG;
use bilterm::wm::WindowManager;
use bilterm::wm::context_queue_ops::{AgentGateOp, AgentQueueOp, ContextOp};
use bilterm::wm::swarm_surface::{HudOp, ReplayOp, SwarmOp};

fn isolate() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let root = std::env::temp_dir().join(format!("bt-p70par-{}", std::process::id()));
        let _ = std::fs::create_dir_all(root.join("home"));
        // SAFETY: runs once before any plane exists in this test binary.
        unsafe {
            std::env::set_var("XDG_STATE_HOME", root.join("state"));
            std::env::set_var("XDG_CONFIG_HOME", root.join("config"));
            std::env::set_var("XDG_DATA_HOME", root.join("data"));
            std::env::set_var("HOME", root.join("home"));
        }
    });
}

fn colon(s: &str) -> Action {
    parse_colon_command(s)
        .unwrap_or_else(|| panic!("{s} did not parse"))
        .unwrap_or_else(|e| panic!("{s}: {e}"))
}

fn p(pairs: &[(&str, JsonValue)]) -> JsonValue {
    JsonValue::obj(pairs.iter().map(|(k, v)| (*k, v.clone())))
}

fn s(v: &str) -> JsonValue {
    JsonValue::String(v.to_string())
}

fn plane() -> AutomationControlPlane {
    isolate();
    AutomationControlPlane::new_headless(24, 80)
}

fn in_palette(prefix: &str) -> bool {
    PALETTE_CATALOG
        .iter()
        .any(|e| e.0.trim_end() == prefix.trim_end())
}

fn in_menu(cmd: &str) -> bool {
    MENUBAR_CATEGORIES
        .iter()
        .flat_map(|c| c.items)
        .any(|i| i.command == cmd)
}

/// (canonical colon form, expected action, RPC method, RPC params).
type Row = (
    &'static str,
    Action,
    &'static str,
    Vec<(&'static str, JsonValue)>,
);

fn matrix() -> Vec<Row> {
    vec![
        (
            ":context set k v",
            Action::Context(ContextOp::Set {
                key: "k".into(),
                value: "v".into(),
            }),
            "context.set",
            vec![("key", s("k")), ("value", s("v"))],
        ),
        (
            ":context get k",
            Action::Context(ContextOp::Get { key: "k".into() }),
            "context.get",
            vec![("key", s("k"))],
        ),
        (
            ":context list",
            Action::Context(ContextOp::List { prefix: None }),
            "context.list",
            vec![],
        ),
        (
            ":agent queue list",
            Action::AgentQueue(AgentQueueOp::List),
            "agent.queue",
            vec![("op", s("list"))],
        ),
        (
            ":agent queue add hi",
            Action::AgentQueue(AgentQueueOp::Add("hi".into())),
            "agent.queue",
            vec![("op", s("add")), ("text", s("hi"))],
        ),
        (
            ":agent queue drop 1",
            Action::AgentQueue(AgentQueueOp::Drop(1)),
            "agent.queue",
            vec![("op", s("drop")), ("index", JsonValue::Number(1))],
        ),
        (
            ":agent queue clear",
            Action::AgentQueue(AgentQueueOp::Clear),
            "agent.queue",
            vec![("op", s("clear"))],
        ),
        (
            ":agent approve",
            Action::AgentGate(AgentGateOp::Approve),
            "agent.queue",
            vec![("op", s("approve"))],
        ),
        (
            ":agent deny",
            Action::AgentGate(AgentGateOp::Deny),
            "agent.queue",
            vec![("op", s("deny"))],
        ),
        (
            ":swarm status",
            Action::Swarm(SwarmOp::Status),
            "swarm.status",
            vec![],
        ),
        (
            ":agents hud toggle",
            Action::AgentsHud(HudOp::Toggle),
            "hud.toggle",
            vec![],
        ),
        (
            ":agent replay show",
            Action::AgentReplay(ReplayOp::Show),
            "agent.replay",
            vec![],
        ),
    ]
}

#[test]
fn colon_parse_yields_expected_action_and_canonical_form_roundtrips() {
    for (text, want, _, _) in matrix() {
        let got = colon(text);
        assert_eq!(got, want, "{text}");
        assert_eq!(
            colon(&got.colon_form_or_summary()),
            want,
            "colon_form roundtrip for {text}"
        );
    }
}

#[test]
fn every_capability_is_in_palette_and_registry() {
    for pal in [
        ":context list",
        ":context set",
        ":context get",
        ":agent queue list",
        ":agent queue add",
        ":agent queue clear",
        ":agent queue drop",
        ":agent approve",
        ":agent deny",
        ":swarm status",
        ":agents hud",
        ":agent replay",
    ] {
        assert!(in_palette(pal), "palette missing {pal}");
    }
    for head in [":swarm", ":context", ":agent"] {
        assert!(
            bilterm::command_registry::lookup(head).is_some(),
            "registry missing {head}"
        );
    }
}

#[test]
fn menu_items_lower_to_the_same_actions() {
    for (cmd, want) in [
        (
            ":context list",
            Action::Context(ContextOp::List { prefix: None }),
        ),
        (":agent queue list", Action::AgentQueue(AgentQueueOp::List)),
        (":agents hud toggle", Action::AgentsHud(HudOp::Toggle)),
        (":swarm status", Action::Swarm(SwarmOp::Status)),
        (":agent replay show", Action::AgentReplay(ReplayOp::Show)),
    ] {
        assert!(in_menu(cmd), "menu missing {cmd}");
        assert_eq!(colon(cmd), want, "{cmd}");
    }
}

#[test]
fn every_rpc_method_is_registered_and_dispatched() {
    let mut cp = plane();
    for (_, _, method, params) in matrix() {
        assert!(RPC_METHODS.contains(&method), "{method} not in RPC_METHODS");
        if let Err(e) = cp.dispatch_method(method, &p(&params)) {
            assert!(
                !e.starts_with("Method not found"),
                "{method} not dispatched: {e}"
            );
        }
    }
    for m in ["swarm.load", "swarm.clear"] {
        assert!(RPC_METHODS.contains(&m), "{m} not in RPC_METHODS");
    }
}

#[test]
fn no_default_key_chord_claims_these_actions() {
    use bilterm::input::keymap::default_global_action;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    // Leader `A` (agents list) must not have been repurposed by the HUD.
    let a = default_global_action(
        KeyEvent::new(
            KeyCode::Char('A'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        ),
        false,
    );
    assert!(!matches!(
        a,
        Some(Action::AgentsHud(_) | Action::Swarm(_) | Action::AgentReplay(_))
    ));
}

fn json_field(v: &JsonValue, k: &str) -> String {
    v.get(k).map(JsonValue::to_json_string).unwrap_or_default()
}

#[test]
fn context_colon_and_rpc_produce_equivalent_state() {
    isolate();
    let mut wm = WindowManager::new_headless(24, 80);
    wm.apply(colon(":context set a.b hello")).unwrap();
    wm.apply(colon(":context get a.b")).unwrap();
    let colon_data = JsonValue::parse(&wm.take_command_data().unwrap()).unwrap();

    let mut cp = plane();
    cp.dispatch_method(
        "context.set",
        &p(&[("key", s("a.b")), ("value", s("hello"))]),
    )
    .unwrap();
    let rpc_data = cp
        .dispatch_method("context.get", &p(&[("key", s("a.b"))]))
        .unwrap();
    for k in ["key", "value", "revision"] {
        assert_eq!(
            json_field(&colon_data, k),
            json_field(&rpc_data, k),
            "context.get {k}"
        );
    }
    assert_eq!(
        wm.context_bus().list(None).len(),
        cp.wm.context_bus().list(None).len()
    );
}

#[test]
fn queue_colon_and_rpc_produce_equivalent_state() {
    isolate();
    let mut wm = WindowManager::new_headless(24, 80);
    let mut cp = plane();
    for t in ["one", "two", "three"] {
        wm.apply(colon(&format!(":agent queue add {t}"))).unwrap();
        cp.dispatch_method("agent.queue", &p(&[("op", s("add")), ("text", s(t))]))
            .unwrap();
    }
    wm.apply(colon(":agent queue drop 2")).unwrap();
    cp.dispatch_method(
        "agent.queue",
        &p(&[("op", s("drop")), ("index", JsonValue::Number(2))]),
    )
    .unwrap();
    let (wid, cid) = (wm.active_pane_id(), cp.wm.active_pane_id());
    assert_eq!(
        wm.prompt_queue().items(wid),
        cp.wm.prompt_queue().items(cid)
    );
    assert_eq!(wm.prompt_queue().items(wid), vec!["one", "three"]);
    wm.apply(colon(":agent queue clear")).unwrap();
    cp.dispatch_method("agent.queue", &p(&[("op", s("clear"))]))
        .unwrap();
    assert_eq!(wm.prompt_queue().len(wid), 0);
    assert_eq!(cp.wm.prompt_queue().len(cid), 0);
}

#[test]
fn approve_deny_on_unblocked_pane_fail_identically() {
    isolate();
    let mut wm = WindowManager::new_headless(24, 80);
    let mut cp = plane();
    for (c, op) in [(":agent approve", "approve"), (":agent deny", "deny")] {
        assert!(wm.apply_automation(colon(c), None).is_err(), "{c}");
        assert!(
            cp.dispatch_method("agent.queue", &p(&[("op", s(op))]))
                .is_err(),
            "{op}"
        );
    }
}

#[test]
fn swarm_colon_and_rpc_produce_equivalent_state() {
    isolate();
    let plan = std::env::temp_dir().join(format!("bt-p70-plan-{}.toml", std::process::id()));
    std::fs::write(
        &plan,
        "[swarm]\nmax_parallel = 2\n\n[[node]]\nid = \"a\"\nprompt = \"do a\"\n\n[[node]]\nid = \"b\"\nprompt = \"do b\"\ndepends_on = [\"a\"]\n",
    )
    .unwrap();
    let path = plan.to_string_lossy().to_string();

    let mut wm = WindowManager::new_headless(24, 80);
    assert!(wm.swarm_surface().swarm.is_none());
    wm.apply(colon(&format!(":swarm load {path}"))).unwrap();
    wm.apply(colon(":swarm status")).unwrap();
    let colon_data = JsonValue::parse(&wm.take_command_data().unwrap()).unwrap();

    let mut cp = plane();
    cp.dispatch_method("swarm.load", &p(&[("path", s(&path))]))
        .unwrap();
    let rpc_data = cp.dispatch_method("swarm.status", &p(&[])).unwrap();
    for k in ["loaded", "nodes", "waves", "finished"] {
        assert_eq!(
            json_field(&colon_data, k),
            json_field(&rpc_data, k),
            "swarm.status {k}"
        );
    }
    assert_eq!(json_field(&rpc_data, "loaded"), "true");
    assert_eq!(
        rpc_data
            .get("waves")
            .and_then(JsonValue::as_array)
            .map(<[_]>::len),
        Some(2)
    );

    wm.apply(colon(":swarm clear")).unwrap();
    cp.dispatch_method("swarm.clear", &p(&[])).unwrap();
    assert!(wm.swarm_surface().swarm.is_none() && cp.wm.swarm_surface().swarm.is_none());
    assert!(
        cp.dispatch_method("swarm.load", &p(&[("path", s("/nonexistent/plan.toml"))]))
            .is_err()
    );
    let _ = std::fs::remove_file(plan);
}

#[test]
fn hud_colon_and_rpc_produce_equivalent_state() {
    isolate();
    let mut wm = WindowManager::new_headless(24, 80);
    let mut cp = plane();
    assert!(!wm.swarm_surface().hud_open);
    wm.apply(colon(":agents hud")).unwrap();
    cp.dispatch_method("hud.toggle", &p(&[])).unwrap();
    assert!(wm.swarm_surface().hud_open && cp.wm.swarm_surface().hud_open);
    assert_eq!(
        wm.swarm_surface().hud.len(),
        cp.wm.swarm_surface().hud.len()
    );
    assert!(!wm.swarm_surface().hud.is_empty(), "one tile per pane");
    wm.apply(colon(":agents hud close")).unwrap();
    cp.dispatch_method("hud.toggle", &p(&[("op", s("close"))]))
        .unwrap();
    assert!(!wm.swarm_surface().hud_open && !cp.wm.swarm_surface().hud_open);
    assert!(
        cp.dispatch_method("hud.toggle", &p(&[("op", s("bogus"))]))
            .is_err()
    );
}

#[test]
fn replay_colon_and_rpc_produce_equivalent_state() {
    isolate();
    let mut wm = WindowManager::new_headless(24, 80);
    let mut cp = plane();
    wm.apply(colon(":agent replay prev")).unwrap();
    let colon_data = JsonValue::parse(&wm.take_command_data().unwrap()).unwrap();
    let rpc_data = cp
        .dispatch_method("agent.replay", &p(&[("op", s("prev"))]))
        .unwrap();
    for k in ["turns", "moved", "turn_id", "prompt"] {
        assert_eq!(
            json_field(&colon_data, k),
            json_field(&rpc_data, k),
            "agent.replay {k}"
        );
    }
    assert_eq!(
        json_field(&rpc_data, "turns"),
        "0",
        "no turns recorded on a fresh pane"
    );
    assert!(
        cp.dispatch_method("agent.replay", &p(&[("op", s("bogus"))]))
            .is_err()
    );
}
