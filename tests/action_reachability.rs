//! Reachability of the Phase 66/67 control surface: every RPC method is dispatchable, and every
//! mutating live-control RPC lowers to an existing `Action` applied through
//! `WindowManager::apply` (observable on the window manager) rather than a parallel code path.
#![cfg(unix)]

use bilterm::api::{AutomationControlPlane, JsonValue, RPC_METHODS};

fn isolate() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let root = std::env::temp_dir().join(format!("bt-reach-{}", std::process::id()));
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

fn params(entries: &[(&str, &str)]) -> JsonValue {
    JsonValue::obj(entries.iter().map(|(k, v)| (k.to_string(), JsonValue::String(v.to_string()))).collect::<Vec<_>>())
}

#[test]
fn every_new_rpc_method_is_registered_and_dispatchable() {
    isolate();
    let new = [
        "agent.prompt", "agent.approve", "agent.last_turn", "agent.wait",
        "pane.list", "pane.split", "pane.focus", "pane.zoom", "pane.send",
        "workspace.list", "workspace.select", "workspace.new", "workspace.rename",
    ];
    let mut cp = AutomationControlPlane::new_headless(24, 80);
    for m in new {
        assert!(RPC_METHODS.contains(&m), "{m} missing from RPC_METHODS");
        assert_eq!(RPC_METHODS.iter().filter(|x| **x == m).count(), 1, "{m} listed twice");
        if let Err(e) = cp.dispatch_method(m, &params(&[])) {
            assert!(!e.starts_with("Method not found"), "{m} is not dispatched");
        }
    }
}

#[test]
fn live_control_rpc_lowers_to_actions_visible_on_the_window_manager() {
    isolate();
    let mut cp = AutomationControlPlane::new_headless(24, 80);
    // Action::SplitHorizontal / SplitVertical
    cp.dispatch_method("pane.split", &params(&[("direction", "h")])).unwrap();
    cp.dispatch_method("pane.split", &params(&[("direction", "v")])).unwrap();
    assert_eq!(cp.wm.pane_count(), 3);
    // Action::FocusPane
    cp.dispatch_method("pane.focus", &params(&[("selector", "w1:p1")])).unwrap();
    assert_eq!(cp.wm.active_pane_id(), 1);
    // Action::ZoomToggle
    cp.dispatch_method("pane.zoom", &params(&[("selector", "p2")])).unwrap();
    assert_eq!(cp.wm.zoomed_pane(), Some(2));
    // Action::SessionManage(Rename)
    cp.dispatch_method("workspace.rename", &params(&[("name", "reach-ws")])).unwrap();
    assert_eq!(cp.wm.config().history_session, "reach-ws");
    // The same behaviours remain reachable through the colon surface (`wm.command`).
    let r = cp.dispatch_method("wm.command", &params(&[("command", ":split v")])).unwrap();
    assert_eq!(r.get("pane_count").and_then(JsonValue::as_usize), Some(4));
}
