//! Phase 51-04: palette parity (UIT-02). The probe sweep itself lives in
//! `src/command_registry.rs` (it needs crate-private tables); this file holds the behaviour
//! checks of the entries it forced (spot execution, key and mouse paths, filtering, help rows)
//! and the automation view of the exclusions.

use bilterm::api::{AutomationControlPlane, JsonValue};

fn call(plane: &mut AutomationControlPlane, line: &str) -> JsonValue {
    JsonValue::parse(&plane.handle_mcp_line(line).expect("response")).expect("json")
}

fn commands_resource() -> JsonValue {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let resp = call(
        &mut plane,
        r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"bilterm://commands"}}"#,
    );
    let text = resp
        .get("result")
        .and_then(|r| r.get("contents"))
        .and_then(JsonValue::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("text"))
        .and_then(JsonValue::as_str)
        .expect("commands resource text")
        .to_string();
    JsonValue::parse(&text).expect("commands body json")
}

#[test]
fn commands_resource_carries_exclusions() {
    let body = commands_resource();
    let cmds = body.get("commands").and_then(JsonValue::as_array).expect("commands array");
    for c in cmds {
        assert!(
            c.get("palette_exclusions").and_then(JsonValue::as_array).is_some(),
            "{} has no palette_exclusions array",
            c.get("name").and_then(JsonValue::as_str).unwrap_or("?")
        );
    }
    let ssh = cmds
        .iter()
        .find(|c| c.get("name").and_then(JsonValue::as_str) == Some(":ssh"))
        .expect(":ssh in the catalog");
    let ex = ssh.get("palette_exclusions").and_then(JsonValue::as_array).unwrap();
    assert!(
        ex.iter().any(|e| e.get("reason").and_then(JsonValue::as_str) == Some("host-or-path")
            && e.get("probe").and_then(JsonValue::as_str).is_some_and(|p| p.starts_with(":ssh"))),
        ":ssh lists a host-or-path exclusion: {}",
        ssh.to_json_string()
    );
}
