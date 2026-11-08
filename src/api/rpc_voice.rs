//! JSON-RPC 2.0 handlers for `voice.*` methods (`STT-01..03`, `UNI-R01`).

use crate::action::{Action, VoiceOp};
use crate::api::json::JsonValue;
use crate::api::rpc::AutomationControlPlane;
use crate::stt::{SttBackendKind, SttUiStatus};

fn voice_status_json(status: &SttUiStatus) -> JsonValue {
    JsonValue::obj([
        ("enabled", JsonValue::Bool(status.enabled)),
        ("recording", JsonValue::Bool(status.recording)),
        ("transcribing", JsonValue::Bool(status.transcribing)),
        (
            "target_pane_id",
            status
                .target_pane_id
                .map_or(JsonValue::Null, |id| JsonValue::Number(id as i64)),
        ),
        ("rms_db", JsonValue::Number(status.rms_db as i64)),
        ("elapsed_ms", JsonValue::Number(status.elapsed_ms as i64)),
        (
            "last_transcript",
            status
                .last_transcript
                .as_ref()
                .map_or(JsonValue::Null, |t| JsonValue::String(t.clone())),
        ),
        (
            "last_error",
            status
                .last_error
                .as_ref()
                .map_or(JsonValue::Null, |e| JsonValue::String(e.clone())),
        ),
        (
            "active_provider",
            JsonValue::String(status.active_provider.as_str().to_string()),
        ),
    ])
}

impl AutomationControlPlane {
    pub(super) fn rpc_voice_toggle(&mut self, _params: &JsonValue) -> Result<JsonValue, String> {
        let _ = self
            .wm
            .apply(Action::Voice(VoiceOp::Toggle))
            .map_err(|e| e.to_string())?;
        Ok(voice_status_json(self.wm.stt_status()))
    }

    pub(super) fn rpc_voice_start(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let pane_id = params.get("pane_id").and_then(JsonValue::as_usize);
        let _ = self
            .wm
            .apply(Action::Voice(VoiceOp::Start { pane_id }))
            .map_err(|e| e.to_string())?;
        Ok(voice_status_json(self.wm.stt_status()))
    }

    pub(super) fn rpc_voice_stop(&mut self, _params: &JsonValue) -> Result<JsonValue, String> {
        let _ = self
            .wm
            .apply(Action::Voice(VoiceOp::Stop))
            .map_err(|e| e.to_string())?;
        Ok(voice_status_json(self.wm.stt_status()))
    }

    pub(super) fn rpc_voice_cancel(&mut self, _params: &JsonValue) -> Result<JsonValue, String> {
        let _ = self
            .wm
            .apply(Action::Voice(VoiceOp::Cancel))
            .map_err(|e| e.to_string())?;
        Ok(voice_status_json(self.wm.stt_status()))
    }

    pub(super) fn rpc_voice_status(&mut self, _params: &JsonValue) -> Result<JsonValue, String> {
        let _ = self
            .wm
            .apply(Action::Voice(VoiceOp::Status))
            .map_err(|e| e.to_string())?;
        Ok(voice_status_json(self.wm.stt_status()))
    }

    pub(super) fn rpc_voice_provider(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let prov_str = params
            .get("provider")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "missing 'provider' parameter".to_string())?;
        let kind = match prov_str.to_ascii_lowercase().as_str() {
            "openai" | "openai_compatible" | "whisper" | "speaches" => {
                SttBackendKind::OpenAiCompatible
            }
            "ollama" => SttBackendKind::Ollama,
            "cli" | "local_cli" => SttBackendKind::LocalCli,
            "mock" => SttBackendKind::Mock,
            other => return Err(format!("unknown provider '{other}'")),
        };
        let _ = self
            .wm
            .apply(Action::Voice(VoiceOp::SetProvider { provider: kind }))
            .map_err(|e| e.to_string())?;
        Ok(voice_status_json(self.wm.stt_status()))
    }

    pub(super) fn rpc_voice_inject_wav(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let pane_id = params.get("pane_id").and_then(JsonValue::as_usize);
        let wav_bytes = if let Some(b64) = params.get("wav_base64").and_then(JsonValue::as_str) {
            crate::vt::codec::base64_decode(b64.trim())
        } else if let Some(hex_str) = params.get("wav_hex").and_then(JsonValue::as_str) {
            (0..hex_str.len())
                .step_by(2)
                .map(|i| {
                    let end = (i + 2).min(hex_str.len());
                    u8::from_str_radix(&hex_str[i..end], 16)
                })
                .collect::<Result<Vec<u8>, _>>()
                .map_err(|e| format!("invalid wav_hex: {e}"))?
        } else {
            return Err("missing 'wav_base64' or 'wav_hex' parameter".to_string());
        };

        let _ = self
            .wm
            .apply(Action::Voice(VoiceOp::InjectWav {
                pane_id,
                wav_bytes,
            }))
            .map_err(|e| e.to_string())?;
        Ok(voice_status_json(self.wm.stt_status()))
    }
}
