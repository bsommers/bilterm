//! Retro text-to-speech: letter-to-sound rules -> phonemes -> Klatt-style formant synthesis.
//!
//! Pure algorithmic and offline (no models, no network, no voice data): think SAM, Votrax,
//! DECtalk and the 1982 Tron computer voices. Pipeline: [`text`] normalisation -> [`rules`]
//! (NRL-style letter-to-sound) -> [`prosody`] (stress, durations, pauses) -> [`track`] (formant
//! parameter segments + intonation) -> [`klatt`] (source-filter synthesis) -> [`fx`] (voice
//! colouring, 16-bit PCM). [`render::render_text`] runs it all and is a pure function of its inputs;
//! [`detmath`] keeps the PCM bit-identical across platforms.
//!
//! Playback and queueing live in [`service`] (worker thread, bounded queue, cancellable) and
//! [`player`] (shell-out to the platform player). The `:say` colon command, `speech.*` RPC methods,
//! `bilterm_say` MCP tool and `bilterm say` CLI all lower to [`SpeechOp`] / [`cli`].

pub mod cli;
pub mod detmath;
pub mod fx;
pub mod klatt;
pub mod phoneme;
pub mod player;
pub mod prosody;
pub mod render;
pub mod rules;
mod rules_data;
pub mod service;
pub mod text;
pub mod track;
pub mod voice;

pub use render::{Rendered, render_text};
pub use service::{QUEUE_DEPTH, SpeechService, SpeechState, SpeechStatus};
pub use voice::{SayOptions, VOICES, Voice};

use serde::{Deserialize, Serialize};

/// Operations dispatched through `Action::Speech(SpeechOp)` (`ARCH-R01`).
///
/// Numbers are integers so `Action` stays `Eq`: `speed_pct` is percent (100 = normal), `pitch_hz` whole Hz.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechOp {
    /// Speak `text` (or, with `wav`, write `<runtime>/speech/<wav>.wav` instead of playing).
    Say {
        text: String,
        voice: Option<String>,
        speed_pct: Option<u32>,
        pitch_hz: Option<u32>,
        wav: Option<String>,
    },
    /// Cancel the current utterance and the queue.
    Stop,
    /// Report state, queue depth and the last error.
    Status,
    /// List the voice presets.
    Voices,
}

impl SpeechOp {
    /// Builds a validated `Say` from user-facing values (`speed` as a multiplier, `pitch` in Hz).
    pub fn say(
        text: &str,
        voice: Option<&str>,
        speed: Option<f64>,
        pitch: Option<f64>,
        wav: Option<&str>,
    ) -> Result<Self, String> {
        render::check_text(text)?;
        SayOptions::build(voice, speed, pitch)?;
        if let Some(w) = wav {
            crate::secure_fs::validate_safe_name(w).map_err(|e| format!("invalid wav name: {e}"))?;
        }
        Ok(Self::Say {
            text: text.to_string(),
            voice: voice.map(str::to_ascii_lowercase),
            speed_pct: speed.map(|s| (s * 100.0).round() as u32),
            pitch_hz: pitch.map(|p| p.round() as u32),
            wav: wav.map(str::to_string),
        })
    }

    /// Resolves the options of a `Say` (re-validating, since the op may come from RPC).
    pub fn options(&self) -> Result<SayOptions, String> {
        match self {
            Self::Say { voice, speed_pct, pitch_hz, .. } => SayOptions::build(
                voice.as_deref(),
                speed_pct.map(|s| f64::from(s) / 100.0),
                pitch_hz.map(f64::from),
            ),
            _ => Ok(SayOptions::default()),
        }
    }
}
