//! Bodies of `:keys check` and `:keys explain` (`KEY-02`, Phase 63-03).
//!
//! `:keys list|profile` live in `input::keyprofile_ops`; every `Action::Keys` surface (colon,
//! palette, menu, `keys.*` RPC, `bilterm_keys` MCP) reaches these through `WindowManager::apply`,
//! so they all report the same JSON through `command_data`.

use crate::api::JsonValue;
use crate::input::chord::KeyChord;
use crate::input::keyconflict::{self, Explanation, Finding};
use crate::input::keyconflict_data::Severity;
use crate::wm::WindowManager;

fn finding_json(f: &Finding) -> JsonValue {
    JsonValue::obj([
        ("chord", JsonValue::String(f.chord.clone())),
        ("bound_as", JsonValue::String(f.bound_as.clone())),
        ("action", f.action.as_ref().map_or(JsonValue::Null, |a| JsonValue::String(a.colon_form_or_summary()))),
        ("severity", JsonValue::String(f.severity.as_str().to_string())),
        ("class", JsonValue::String(f.class.as_str().to_string())),
        ("consumer", JsonValue::String(f.consumer.clone())),
        ("note", JsonValue::String(f.note.clone())),
        ("source", JsonValue::String(f.source.to_string())),
    ])
}

/// `{profile, env, counts, findings[]}` for `:keys check`.
pub fn findings_json(profile: &str, os: &str, findings: &[Finding]) -> JsonValue {
    let count = |s: Severity| JsonValue::Number(findings.iter().filter(|f| f.severity == s).count() as i64);
    JsonValue::obj([
        ("profile", JsonValue::String(profile.to_string())),
        ("os", JsonValue::String(os.to_string())),
        ("block", count(Severity::Block)),
        ("warn", count(Severity::Warn)),
        ("info", count(Severity::Info)),
        ("findings", JsonValue::Array(findings.iter().map(finding_json).collect())),
    ])
}

/// `:keys check`: runs the conflict engine over the active keymap and the user's bindings.
pub(crate) fn check_op(wm: &mut WindowManager) -> String {
    let km = wm.keymap();
    let env = km.env();
    let findings = keyconflict::check_config(&km, &env, wm.config());
    wm.set_command_data(findings_json(km.profile().as_str(), env.os.name(), &findings).to_json_string());
    for f in keyconflict::user_findings(&findings) {
        tracing::warn!("key conflict: {} ({}) vs {}: {}", f.chord, f.class.as_str(), f.consumer, f.note);
    }
    keyconflict::summary_line(km.profile().as_str(), &findings)
}

fn explanation_json(x: &Explanation) -> JsonValue {
    JsonValue::obj([
        ("chord", JsonValue::String(x.chord.clone())),
        ("bound", JsonValue::Bool(x.action.is_some() || x.origin == "user")),
        ("action", x.action.as_ref().map_or(JsonValue::Null, |a| JsonValue::String(a.colon_form_or_summary()))),
        ("origin", JsonValue::String(x.origin.to_string())),
        ("findings", JsonValue::Array(x.findings.iter().map(finding_json).collect())),
    ])
}

/// `:keys explain <chord>`.
pub(crate) fn explain_op(wm: &mut WindowManager, raw: &str) -> String {
    if let Err(e) = KeyChord::parse(raw) {
        wm.record_automation_error(e.clone());
        return e;
    }
    let km = wm.keymap();
    let env = km.env();
    let x = keyconflict::explain(raw, &km, &env, wm.config());
    wm.set_command_data(explanation_json(&x).to_json_string());
    x.text(km.profile().as_str())
}
