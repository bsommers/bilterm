//! `agent.prompt`, the mail nudge and `:agent guide` share one `!ECHO LOCK` rule: ECHO-off is a
//! password prompt only when the program has not declared itself a TUI; a password line on the
//! canvas is always refused.
#![cfg(unix)]

use bilterm::api::{AutomationControlPlane, JsonValue};

fn obj(e: &[(&str, JsonValue)]) -> JsonValue {
    JsonValue::obj(e.iter().map(|(k, v)| (k.to_string(), v.clone())).collect::<Vec<_>>())
}
fn js(s: &str) -> JsonValue {
    JsonValue::String(s.to_string())
}

#[derive(Clone, Copy)]
enum Kind {
    RawTui,
    EchoOffShell,
    CanvasPassword,
}

fn plane(kind: Kind) -> AutomationControlPlane {
    let mut cp = AutomationControlPlane::new_headless(24, 80);
    cp.dispatch_method("pane.split", &obj(&[("direction", js("v"))])).unwrap();
    cp.wm.agent_mail_mut().register_name(1, "alice").unwrap();
    cp.wm.agent_mail_mut().register_name(2, "bob").unwrap();
    let pane = cp.wm.pane_mut(2).unwrap();
    match kind {
        Kind::RawTui => {
            pane.process_pty_output(b"\x1b[?2004h\x1b[>1u");
            pane.line_discipline_mut().echo_enabled = false;
        }
        Kind::EchoOffShell => pane.line_discipline_mut().echo_enabled = false,
        Kind::CanvasPassword => {
            pane.process_pty_output(b"[sudo] password for bill:");
            pane.line_discipline_mut().echo_enabled = false;
        }
    }
    cp
}

fn enable_mail(cp: &mut AutomationControlPlane) {
    let _ = cp.wm.agent_mail_apply(&bilterm::wm::agent_mail::AgentMailOp::Enable(Some(true)));
}

fn prompt(cp: &mut AutomationControlPlane) -> Result<JsonValue, String> {
    cp.dispatch_method("agent.prompt", &obj(&[("selector", js("p2")), ("text", js("hi"))]))
}

fn guide(cp: &mut AutomationControlPlane) -> Result<JsonValue, String> {
    cp.dispatch_method("agent.guide", &obj(&[("op", js("inject")), ("selector", js("p2"))]))
}

fn nudge(cp: &mut AutomationControlPlane) -> String {
    enable_mail(cp);
    let r = cp
        .dispatch_method("agent.send", &obj(&[("from", js("alice")), ("to", js("bob")), ("body", js("ping")), ("delivery", js("nudge"))]))
        .unwrap();
    r.get("status").and_then(JsonValue::as_str).unwrap_or("").to_string()
}

#[test]
fn raw_tui_pane_is_accepted_by_every_caller() {
    let mut cp = plane(Kind::RawTui);
    assert!(!cp.wm.pane(2).unwrap().is_injection_password_locked());
    prompt(&mut cp).expect("agent.prompt accepts a raw TUI");
    guide(&mut cp).expect("agent.guide accepts a raw TUI");
    assert_eq!(nudge(&mut cp), "nudged");
}

#[test]
fn plain_echo_off_pane_is_refused_by_every_caller() {
    let mut cp = plane(Kind::EchoOffShell);
    assert!(cp.wm.pane(2).unwrap().is_injection_password_locked());
    assert!(prompt(&mut cp).unwrap_err().contains("ECHO LOCK"));
    assert!(guide(&mut cp).unwrap_err().contains("ECHO LOCK"));
    assert!(nudge(&mut cp).contains("suppressed"));
}

#[test]
fn canvas_password_line_is_refused_by_every_caller() {
    let mut cp = plane(Kind::CanvasPassword);
    assert!(cp.wm.pane(2).unwrap().is_injection_password_locked());
    assert!(prompt(&mut cp).unwrap_err().contains("ECHO LOCK"));
    assert!(guide(&mut cp).unwrap_err().contains("ECHO LOCK"));
    assert!(nudge(&mut cp).contains("suppressed"));
}

#[test]
fn canvas_password_line_is_refused_even_on_a_declared_tui() {
    let mut cp = plane(Kind::CanvasPassword);
    cp.wm.pane_mut(2).unwrap().process_pty_output(b"\x1b[?2004h");
    cp.wm.pane_mut(2).unwrap().line_discipline_mut().echo_enabled = false;
    assert!(prompt(&mut cp).unwrap_err().contains("ECHO LOCK"));
}
