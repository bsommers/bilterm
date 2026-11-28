//! JSON-RPC 2.0 handlers for `speech.*` methods (`TTS-01`, `UNI-R01`).
//!
//! `speech.say {text, voice?, speed?, pitch?, wav?}`, `speech.stop`, `speech.status`, `speech.voices`.
//! Every method lowers to `Action::Speech` and runs through `WindowManager::apply`, exactly like `:say`.

use crate::action::{Action, SpeechOp};
use crate::api::json::JsonValue;
use crate::api::rpc::{AutomationControlPlane, SnapshotPolicy};

fn opt_num(p: &JsonValue, key: &str) -> Result<Option<f64>, String> {
    match p.get(key) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(v) => v.as_f64().map(Some).ok_or_else(|| format!("'{key}' must be a number")),
    }
}

fn opt_str<'a>(p: &'a JsonValue, key: &str) -> Result<Option<&'a str>, String> {
    match p.get(key) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(v) => v.as_str().map(Some).ok_or_else(|| format!("'{key}' must be a string")),
    }
}

/// Builds the action for `speech.<verb>`; `None` for an unknown verb.
pub(super) fn speech_action(verb: &str, p: &JsonValue) -> Result<Option<Action>, String> {
    let op = match verb {
        "say" => {
            let text = p.get("text").and_then(JsonValue::as_str).ok_or("missing 'text' parameter")?;
            SpeechOp::say(text, opt_str(p, "voice")?, opt_num(p, "speed")?, opt_num(p, "pitch")?, opt_str(p, "wav")?)?
        }
        "stop" => SpeechOp::Stop,
        "status" => SpeechOp::Status,
        "voices" => SpeechOp::Voices,
        _ => return Ok(None),
    };
    Ok(Some(Action::Speech(op)))
}

impl AutomationControlPlane {
    pub(super) fn rpc_speech(&mut self, method: &str, params: &JsonValue) -> Result<JsonValue, String> {
        let verb = method.strip_prefix("speech.").unwrap_or(method);
        let action = speech_action(verb, params)?.ok_or_else(|| format!("unknown method {method}"))?;
        self.wm.take_command_data();
        self.apply_with_policy(action, SnapshotPolicy::Trusted)?;
        let data = self.wm.take_command_data().ok_or_else(|| format!("no {method} data"))?;
        JsonValue::parse(&data).ok_or_else(|| format!("invalid {method} data"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> JsonValue {
        JsonValue::parse(s).unwrap()
    }

    #[test]
    fn params_lower_to_the_same_actions_as_the_colon_forms() {
        let a = speech_action("say", &p(r#"{"text":"hi there","voice":"tron","speed":1.5,"pitch":90,"wav":"x"}"#)).unwrap().unwrap();
        let b = crate::colon_command::parse_colon_command(":say --voice tron --speed 1.5 --pitch 90 --wav x hi there").unwrap().unwrap();
        assert_eq!(a, b);
        assert_eq!(speech_action("stop", &p("{}")).unwrap(), Some(Action::Speech(SpeechOp::Stop)));
        assert!(speech_action("bogus", &p("{}")).unwrap().is_none());
    }

    #[test]
    fn bad_params_are_errors() {
        assert!(speech_action("say", &p("{}")).is_err());
        assert!(speech_action("say", &p(r#"{"text":"x","speed":"fast"}"#)).is_err());
        assert!(speech_action("say", &p(r#"{"text":"x","speed":10}"#)).is_err());
        assert!(speech_action("say", &p(r#"{"text":"x","wav":"../etc"}"#)).is_err());
        assert!(speech_action("say", &p(r#"{"text":5}"#)).is_err());
    }
}
