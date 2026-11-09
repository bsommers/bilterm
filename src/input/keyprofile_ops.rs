//! Bodies of `Action::Keys(KeysOp)` (`:keys list|check|explain|profile`, Phase 63).
//!
//! `Check` and `Explain` delegate to `colon_command::execute_keys` (the `keyconflict` engine, 63-03);
//! the surface stays this one function.

use crate::action::{Action, KeysOp};
use crate::api::JsonValue;
use crate::config::keys::KeyProfileName;
use crate::wm::WindowManager;

use super::chord::KeyChord;
use super::keyenv::primary_mods;

/// Colon form when the action has one, else its name.
fn action_label(a: &Action) -> String {
    a.colon_form_or_summary()
}

/// Runs `op` against `wm`, stores structured JSON via `set_command_data`, returns the status line.
pub fn run_keys_op(wm: &mut WindowManager, op: &KeysOp) -> String {
    let km = wm.keymap();
    let env = km.env();
    match op {
        KeysOp::List => {
            let mut rows: Vec<(String, String, &'static str)> = Vec::new();
            for c in super::keyprofile_tables::PRIMARY_BINDINGS.iter().filter(|_| km.primary_enabled()) {
                rows.push((KeyChord::primary(c.0).display(&env), action_label(&c.1), "primary"));
            }
            for (m, c, a) in km.global_rows() {
                rows.push((KeyChord::new(m, c).display(&env), action_label(a), "global"));
            }
            for (m, c, a) in km.pane_rows() {
                rows.push((KeyChord::new(m, c).display(&env), action_label(a), "pane"));
            }
            for (code, a) in km.leader_rows() {
                rows.push((format!("Leader {}", super::keyprofile::leader_key_label(code)), action_label(&a), "leader"));
            }
            let json = JsonValue::obj([
                ("profile", JsonValue::String(km.profile().as_str().to_string())),
                (
                    "rows",
                    JsonValue::Array(
                        rows.iter()
                            .map(|(c, a, t)| {
                                JsonValue::obj([
                                    ("chord", JsonValue::String(c.clone())),
                                    ("action", JsonValue::String(a.clone())),
                                    ("table", JsonValue::String((*t).to_string())),
                                ])
                            })
                            .collect(),
                    ),
                ),
            ]);
            wm.set_command_data(json.to_json_string());
            format!("Keys [{}]: {} bindings (Primary {})", km.profile().as_str(), rows.len(), primary_label(wm))
        }
        KeysOp::Check => crate::colon_command::execute_keys::check_op(wm),
        KeysOp::Explain(raw) => crate::colon_command::execute_keys::explain_op(wm, raw),
        KeysOp::Profile(None) => {
            set_profile_data(wm);
            format!("Key profile: {} (configured {})", km.profile().as_str(), wm.config().keys.profile.as_str())
        }
        KeysOp::Profile(Some(name)) => match KeyProfileName::parse(name) {
            Some(p) => {
                wm.set_key_profile(p);
                set_profile_data(wm);
                let now = wm.keymap().profile();
                format!("Key profile set to {} (this session; :config save to persist)", now.as_str())
            }
            None => {
                let valid: Vec<&str> = KeyProfileName::ALL.iter().map(|n| n.as_str()).collect();
                let e = format!("unknown key profile '{name}' (valid: {})", valid.join(", "));
                wm.record_automation_error(e.clone());
                e
            }
        },
    }
}

/// Stores `{profile, configured, available}` for `:keys profile` and the `keys.profile` RPC.
fn set_profile_data(wm: &mut WindowManager) {
    let json = JsonValue::obj([
        ("profile", JsonValue::String(wm.keymap().profile().as_str().to_string())),
        ("configured", JsonValue::String(wm.config().keys.profile.as_str().to_string())),
        ("available", JsonValue::Array(KeyProfileName::ALL.iter().map(|n| JsonValue::String(n.as_str().to_string())).collect())),
    ]);
    wm.set_command_data(json.to_json_string());
}

fn primary_label(wm: &WindowManager) -> &'static str {
    let km = wm.keymap();
    match primary_mods(&km.env()) {
        Some(m) if km.primary_enabled() && m.contains(crossterm::event::KeyModifiers::SUPER) => "Cmd",
        Some(_) if km.primary_enabled() => "Ctrl+Shift",
        _ => "unavailable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_switch_and_unknown_name() {
        let mut wm = WindowManager::new_headless(24, 80);
        assert!(run_keys_op(&mut wm, &KeysOp::Profile(Some("mac".into()))).contains("mac"));
        assert_eq!(wm.keymap().profile(), KeyProfileName::Mac);
        let err = run_keys_op(&mut wm, &KeysOp::Profile(Some("bogus".into())));
        assert!(err.contains("unknown key profile") && err.contains("portable"), "{err}");
        assert_eq!(wm.keymap().profile(), KeyProfileName::Mac);
    }
}
