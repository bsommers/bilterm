//! WindowManager integration for the retro text-to-speech worker (`TTS-01`).

use crate::speech::SpeechService;
use crate::speech::player::{Player, RecordingPlayer, ShellPlayer};
use crate::wm::WindowManager;
use std::sync::Arc;

/// Creates the worker on first use. `BILTERM_SAY_PLAYER=off` disables audio output (WAV files still work).
pub fn ensure_speech(wm: &mut WindowManager) -> &SpeechService {
    if wm.speech.is_none() {
        let player: Arc<dyn Player> = if std::env::var("BILTERM_SAY_PLAYER").is_ok_and(|v| v == "off") {
            Arc::new(RecordingPlayer { unavailable: Some("audio output disabled (BILTERM_SAY_PLAYER=off)".to_string()), ..Default::default() })
        } else {
            Arc::new(ShellPlayer::new())
        };
        wm.speech = Some(SpeechService::new(player, None));
    }
    wm.speech.as_ref().expect("speech service just created")
}
