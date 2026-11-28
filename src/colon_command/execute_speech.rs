//! Execution of `Action::Speech` (`:say`, `speech.*` RPC, `bilterm_speech` MCP) (`TTS-01`).
//!
//! Never blocks: synthesis and playback run on the speech worker thread. Problems the caller must
//! see (bad option, queue full, no audio player) are recorded as automation errors (RPC/MCP get an
//! `Err`) and always shown on the status line. A structured JSON result is left in `command_data`.

use crate::action::SpeechOp;
use crate::api::json::JsonValue;
use crate::speech::SpeechStatus;
use crate::speech::cli::voices_text;
use crate::speech::voice::{MAX_AUDIO_SECONDS, MAX_TEXT_CHARS, VOICES};
use crate::wm::WindowManager;

fn status_json(s: &SpeechStatus) -> JsonValue {
    let opt = |v: &Option<String>| v.as_ref().map_or(JsonValue::Null, |t| JsonValue::String(t.clone()));
    JsonValue::obj([
        ("state", JsonValue::String(s.state.as_str().to_string())),
        ("pending", JsonValue::Number(s.pending as i64)),
        ("spoken", JsonValue::Number(s.spoken as i64)),
        ("player", JsonValue::String(s.player.clone())),
        ("last_error", opt(&s.last_error)),
        ("last_wav", opt(&s.last_wav)),
        ("last_duration_ms", JsonValue::Number(s.last_duration_ms as i64)),
        ("last_truncated", JsonValue::Bool(s.last_truncated)),
    ])
}

fn ok(wm: &mut WindowManager, msg: String, mut fields: Vec<(&str, JsonValue)>) {
    fields.push(("message", JsonValue::String(msg.clone())));
    wm.set_command_data(JsonValue::obj(fields).to_json_string());
    wm.active_pane_mut().set_status_message(Some(msg));
}

fn fail(wm: &mut WindowManager, msg: String) {
    wm.record_automation_error_msg(msg.clone());
    wm.active_pane_mut().set_status_message(Some(msg));
}

pub fn execute_speech_command(wm: &mut WindowManager, op: &SpeechOp) {
    match op {
        SpeechOp::Say { text, wav, .. } => {
            let opts = match op.options() {
                Ok(o) => o,
                Err(e) => return fail(wm, format!("say: {e}")),
            };
            let voice = opts.voice.name;
            match wm.speech_service().submit(text, opts, wav.clone()) {
                Ok(pos) => {
                    let msg = match wav {
                        Some(w) => format!("say: rendering {w}.wav ({voice})"),
                        None if pos > 1 => format!("say: queued #{pos} ({voice})"),
                        None => format!("say: speaking ({voice}); limits {MAX_TEXT_CHARS} chars, {MAX_AUDIO_SECONDS}s"),
                    };
                    let fields = vec![("voice", JsonValue::String(voice.to_string())), ("queued", JsonValue::Number(pos as i64))];
                    ok(wm, msg, fields);
                }
                Err(e) => fail(wm, format!("say: {e}")),
            }
        }
        SpeechOp::Stop => {
            let n = wm.speech_service().stop();
            ok(wm, format!("say: stopped ({n} utterance(s) cancelled)"), vec![("cancelled", JsonValue::Number(n as i64))]);
        }
        SpeechOp::Status => {
            let s = wm.speech_service().status();
            let mut msg = format!("say: {} | pending {} | spoken {} | player {}", s.state.as_str(), s.pending, s.spoken, s.player);
            if let Some(w) = &s.last_wav {
                msg.push_str(&format!(" | last wav {w}"));
            }
            if let Some(e) = &s.last_error {
                msg.push_str(&format!(" | last error: {e}"));
            }
            ok(wm, msg, vec![("status", status_json(&s))]);
        }
        SpeechOp::Voices => {
            let list = VOICES
                .iter()
                .map(|v| JsonValue::obj([("name", JsonValue::String(v.name.to_string())), ("about", JsonValue::String(v.about.to_string())), ("base_pitch_hz", JsonValue::Number(v.base_f0 as i64))]))
                .collect();
            let msg = format!("say voices: {}", voices_text().replace('\n', " | "));
            ok(wm, msg, vec![("voices", JsonValue::Array(list))]);
        }
    }
}
