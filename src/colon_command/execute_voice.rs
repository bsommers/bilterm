//! Execution logic for `:voice` and `:stt` colon commands and `Action::Voice` (`STT-01..03`).

use crate::action::VoiceOp;
use crate::wm::WindowManager;

pub fn execute_voice_command(wm: &mut WindowManager, op: &VoiceOp) {
    match op {
        VoiceOp::Toggle => {
            if wm.stt_status().recording {
                execute_voice_command(wm, &VoiceOp::Stop);
            } else {
                execute_voice_command(wm, &VoiceOp::Start { pane_id: None });
            }
        }
        VoiceOp::Start { pane_id } => {
            let target_id = pane_id.unwrap_or_else(|| wm.active_pane_id());
            wm.stt_status_mut().recording = true;
            wm.stt_status_mut().transcribing = false;
            wm.stt_status_mut().target_pane_id = Some(target_id);
            wm.stt_status_mut().elapsed_ms = 0;
            wm.stt_status_mut().last_error = None;
            if let Some(worker) = wm.ensure_stt_worker() {
                worker.start(Some(target_id));
            }
            if let Some(pane) = wm.pane_mut(target_id) {
                pane.set_status_message(Some("Voice recording started [🎙 REC]...".to_string()));
            }
            wm.sync_stt_badges();
        }
        VoiceOp::Stop => {
            wm.stt_status_mut().recording = false;
            wm.stt_status_mut().transcribing = true;
            if let Some(worker) = wm.ensure_stt_worker() {
                worker.stop();
            }
            let target_id = wm.stt_status().target_pane_id.unwrap_or_else(|| wm.active_pane_id());
            if let Some(pane) = wm.pane_mut(target_id) {
                pane.set_status_message(Some("Transcribing voice audio [🎙 STT]...".to_string()));
            }
            wm.sync_stt_badges();
        }
        VoiceOp::Cancel => {
            wm.stt_status_mut().recording = false;
            wm.stt_status_mut().transcribing = false;
            if let Some(worker) = wm.ensure_stt_worker() {
                worker.cancel();
            }
            let target_id = wm.stt_status().target_pane_id.unwrap_or_else(|| wm.active_pane_id());
            if let Some(pane) = wm.pane_mut(target_id) {
                pane.set_status_message(Some("Voice recording cancelled".to_string()));
            }
            wm.sync_stt_badges();
        }
        VoiceOp::Status => {
            let status = wm.stt_status().clone();
            let prov = status.active_provider.as_str();
            let state_str = if status.recording {
                "RECORDING"
            } else if status.transcribing {
                "TRANSCRIBING"
            } else if status.enabled {
                "READY"
            } else {
                "IDLE"
            };
            let msg = format!(
                "STT [{state_str}]: provider={prov}, last_transcript={:?}",
                status.last_transcript.as_deref().unwrap_or("none")
            );
            let json = crate::api::json::JsonValue::obj([
                ("enabled", crate::api::json::JsonValue::Bool(status.enabled)),
                ("recording", crate::api::json::JsonValue::Bool(status.recording)),
                ("transcribing", crate::api::json::JsonValue::Bool(status.transcribing)),
                (
                    "target_pane_id",
                    status
                        .target_pane_id
                        .map_or(crate::api::json::JsonValue::Null, |id| {
                            crate::api::json::JsonValue::Number(id as i64)
                        }),
                ),
                ("rms_db", crate::api::json::JsonValue::Number(status.rms_db as i64)),
                ("elapsed_ms", crate::api::json::JsonValue::Number(status.elapsed_ms as i64)),
                (
                    "last_transcript",
                    status
                        .last_transcript
                        .as_ref()
                        .map_or(crate::api::json::JsonValue::Null, |t| {
                            crate::api::json::JsonValue::String(t.clone())
                        }),
                ),
                (
                    "active_provider",
                    crate::api::json::JsonValue::String(status.active_provider.as_str().to_string()),
                ),
            ])
            .to_json_string();
            wm.set_command_data(json);
            wm.active_pane_mut().set_status_message(Some(msg));
        }
        VoiceOp::SetProvider { provider } => {
            let kind = *provider;
            wm.config_mut().stt.provider = kind.as_str().to_string();
            wm.stt_status_mut().active_provider = kind;
            if let Some(worker) = wm.ensure_stt_worker() {
                worker.set_provider(kind);
            }
            wm.active_pane_mut().set_status_message(Some(format!(
                "STT provider switched to {}",
                kind.as_str()
            )));
        }
        VoiceOp::ApplyTranscript {
            pane_id,
            text,
            is_final,
        } => {
            if *is_final {
                let target_id = pane_id.unwrap_or_else(|| wm.active_pane_id());
                let auto_submit = wm.config().stt.auto_submit;
                if let Some(pane) = wm.pane_mut(target_id) {
                    pane.insert_dictated_text(text);
                    pane.set_status_message(Some(format!("Dictated: \"{text}\"")));
                    if auto_submit {
                        let submitted = pane.prompt_mut().take_submission();
                        let _ = pane.submit_command(&submitted);
                    }
                }
                wm.stt_status_mut().last_transcript = Some(text.clone());
                wm.stt_status_mut().transcribing = false;
                wm.sync_stt_badges();
            }
        }
        VoiceOp::UpdateTelemetry {
            recording,
            transcribing,
            rms_db,
            elapsed_ms,
            error,
        } => {
            let status = wm.stt_status_mut();
            status.recording = *recording;
            status.transcribing = *transcribing;
            status.rms_db = *rms_db;
            status.elapsed_ms = *elapsed_ms;
            if let Some(err) = error {
                status.last_error = Some(err.clone());
            }
            wm.sync_stt_badges();
        }
        VoiceOp::InjectWav { pane_id, wav_bytes } => {
            let target_id = pane_id.unwrap_or_else(|| wm.active_pane_id());
            wm.stt_status_mut().transcribing = true;
            wm.stt_status_mut().target_pane_id = Some(target_id);
            if let Some(worker) = wm.ensure_stt_worker() {
                worker.inject_wav(Some(target_id), wav_bytes.clone());
            }
            wm.sync_stt_badges();
        }
    }
}
