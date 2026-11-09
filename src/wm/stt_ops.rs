//! WindowManager STT subsystem integration and event loop (`STT-02`, `STT-03`).

use crate::action::{Action, VoiceOp};
use crate::stt::worker::{SttHandle, SttWorker};
use crate::stt::SttEvent;
use crate::wm::WindowManager;

pub fn ensure_stt_worker(wm: &mut WindowManager) -> Option<&mut SttHandle> {
    if wm.stt_handle.is_none() {
        crate::stt::provider::set_traffic_meter(Some(wm.traffic_meter()));
        let handle = SttWorker::spawn_autonomous(wm.config().stt.clone());
        wm.stt_handle = Some(handle);
    }
    wm.stt_handle.as_mut()
}

pub fn drain_stt_events(wm: &mut WindowManager) {
    let mut events = Vec::new();
    if let Some(handle) = wm.stt_handle.as_mut() {
        while let Some(ev) = handle.try_recv_event() {
            events.push(ev);
        }
    }

    for ev in events {
        match ev {
            SttEvent::Telemetry {
                recording,
                transcribing,
                rms_db,
                elapsed_ms,
                error,
            } => {
                let _ = wm.apply(Action::Voice(VoiceOp::UpdateTelemetry {
                    recording,
                    transcribing,
                    rms_db,
                    elapsed_ms,
                    error,
                }));
            }
            SttEvent::Transcript {
                pane_id,
                text,
                is_final,
            } => {
                let _ = wm.apply(Action::Voice(VoiceOp::ApplyTranscript {
                    pane_id,
                    text,
                    is_final,
                }));
            }
            SttEvent::Error(err) => {
                wm.stt_status.last_error = Some(err.clone());
                wm.stt_status.recording = false;
                wm.stt_status.transcribing = false;
                wm.active_pane_mut()
                    .set_status_message(Some(format!("STT Error: {err}")));
            }
        }
    }

    sync_stt_badges(wm);
}

pub fn sync_stt_badges(wm: &mut WindowManager) {
    let status = &wm.stt_status;
    let badge = if status.recording {
        let vu_char = match status.rms_db {
            db if db > -12 => "█",
            db if db > -24 => "▆",
            db if db > -36 => "▄",
            db if db > -48 => "▂",
            _ => " ",
        };
        let secs = status.elapsed_ms as f64 / 1000.0;
        Some(format!("[🎙 REC {vu_char} {secs:.1}s]"))
    } else if status.transcribing {
        Some("[🎙 STT ...]".to_string())
    } else if status.enabled {
        Some("[🎙 MIC]".to_string())
    } else {
        None
    };

    let target_pane = status.target_pane_id;
    for (&id, pane) in wm.panes_mut() {
        if target_pane.is_none() || target_pane == Some(id) {
            pane.set_stt_badge(badge.clone());
        } else {
            pane.set_stt_badge(None);
        }
    }
}
