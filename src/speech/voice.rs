//! Voice presets and the user-facing synthesis options with their hard bounds.

/// Output sample rate (matches the STT WAV helpers, so the shared encoder applies).
pub const SAMPLE_RATE: u32 = 16_000;

/// Hard bounds applied to every surface (colon command, RPC, MCP, CLI).
pub const MAX_TEXT_CHARS: usize = 2_000;
pub const MAX_AUDIO_SECONDS: u32 = 30;
pub const MIN_SPEED: f64 = 0.5;
pub const MAX_SPEED: f64 = 3.0;
pub const MIN_PITCH_HZ: f64 = 50.0;
pub const MAX_PITCH_HZ: f64 = 300.0;

/// Static description of one retro voice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Voice {
    pub name: &'static str,
    pub about: &'static str,
    /// Base fundamental frequency (Hz).
    pub base_f0: f64,
    /// Scale on the intonation contour: 0 is a perfect monotone.
    pub pitch_range: f64,
    /// 0 = smooth glottal pulse, 1 = harsh sawtooth buzz.
    pub buzz: f64,
    /// Formant frequency scale (< 1 deeper, > 1 smaller vocal tract).
    pub formant_scale: f64,
    /// Formant bandwidth scale (< 1 ringing, > 1 muffled).
    pub bw_scale: f64,
    /// Transition time between phonemes in ms (small = choppy chip speech).
    pub blend_ms: f64,
    /// Sample-and-hold factor emulating a low DAC rate (1 = off).
    pub hold: u32,
    /// Output bit depth emulating an 8-bit DAC (16 = off).
    pub bits: u32,
    /// Ring-modulator carrier (Hz) and mix, the Tron "MCP" effect (mix 0 = off).
    pub ring_hz: f64,
    pub ring_mix: f64,
    /// Short feedback echo (ms, gain); 0 = off.
    pub echo_ms: f64,
    pub echo_gain: f64,
}

pub const VOICES: &[Voice] = &[
    Voice { name: "robot", about: "flat buzzing robot, light bit-crush", base_f0: 105.0, pitch_range: 0.2, buzz: 0.7, formant_scale: 1.0, bw_scale: 0.85, blend_ms: 26.0, hold: 2, bits: 12, ring_hz: 0.0, ring_mix: 0.0, echo_ms: 0.0, echo_gain: 0.0 },
    Voice { name: "tron", about: "deep ring-modulated computer voice with a short echo", base_f0: 88.0, pitch_range: 0.1, buzz: 0.9, formant_scale: 0.92, bw_scale: 0.9, blend_ms: 30.0, hold: 1, bits: 12, ring_hz: 46.0, ring_mix: 0.5, echo_ms: 23.0, echo_gain: 0.35 },
    Voice { name: "votrax", about: "choppy 1980s arcade speech chip", base_f0: 128.0, pitch_range: 0.35, buzz: 0.55, formant_scale: 1.05, bw_scale: 1.0, blend_ms: 12.0, hold: 2, bits: 8, ring_hz: 0.0, ring_mix: 0.0, echo_ms: 0.0, echo_gain: 0.0 },
    Voice { name: "sam", about: "8-bit home-computer monotone with a low sample rate", base_f0: 100.0, pitch_range: 0.15, buzz: 0.85, formant_scale: 0.97, bw_scale: 0.95, blend_ms: 18.0, hold: 3, bits: 8, ring_hz: 0.0, ring_mix: 0.0, echo_ms: 0.0, echo_gain: 0.0 },
    Voice { name: "dectalk", about: "smoother 1980s hardware synthesizer with lively pitch", base_f0: 122.0, pitch_range: 1.0, buzz: 0.35, formant_scale: 1.0, bw_scale: 1.1, blend_ms: 42.0, hold: 1, bits: 16, ring_hz: 0.0, ring_mix: 0.0, echo_ms: 0.0, echo_gain: 0.0 },
];

/// Default voice name.
pub const DEFAULT_VOICE: &str = "robot";

/// Finds a voice by case-insensitive name.
pub fn find_voice(name: &str) -> Option<&'static Voice> {
    VOICES.iter().find(|v| v.name.eq_ignore_ascii_case(name))
}

/// Per-request synthesis options (already validated).
#[derive(Clone, Debug, PartialEq)]
pub struct SayOptions {
    pub voice: &'static Voice,
    pub speed: f64,
    /// Base pitch override in Hz.
    pub pitch_hz: Option<f64>,
}

impl Default for SayOptions {
    fn default() -> Self {
        Self { voice: find_voice(DEFAULT_VOICE).unwrap_or(&VOICES[0]), speed: 1.0, pitch_hz: None }
    }
}

impl SayOptions {
    /// Builds validated options from optional string/number inputs. Out-of-range numbers are rejected,
    /// not clamped, so the caller learns that the request was not honoured.
    pub fn build(voice: Option<&str>, speed: Option<f64>, pitch_hz: Option<f64>) -> Result<Self, String> {
        let mut o = Self::default();
        if let Some(name) = voice {
            o.voice = find_voice(name).ok_or_else(|| format!("unknown voice '{name}' (voices: {})", voice_names()))?;
        }
        if let Some(s) = speed {
            if !(MIN_SPEED..=MAX_SPEED).contains(&s) {
                return Err(format!("speed {s} out of range ({MIN_SPEED}..{MAX_SPEED})"));
            }
            o.speed = s;
        }
        if let Some(p) = pitch_hz {
            if !(MIN_PITCH_HZ..=MAX_PITCH_HZ).contains(&p) {
                return Err(format!("pitch {p} Hz out of range ({MIN_PITCH_HZ}..{MAX_PITCH_HZ})"));
            }
            o.pitch_hz = Some(p);
        }
        Ok(o)
    }

    pub fn base_f0(&self) -> f64 {
        self.pitch_hz.unwrap_or(self.voice.base_f0)
    }
}

/// Comma separated voice names.
pub fn voice_names() -> String {
    VOICES.iter().map(|v| v.name).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_exist_and_are_sane() {
        for n in ["robot", "tron", "votrax", "sam", "dectalk"] {
            let v = find_voice(n).unwrap();
            assert!(v.base_f0 >= MIN_PITCH_HZ && v.base_f0 <= MAX_PITCH_HZ);
            assert!(v.hold >= 1 && v.bits >= 4 && v.bits <= 16);
        }
        assert!(find_voice("TRON").is_some());
        assert!(find_voice("hal").is_none());
    }

    #[test]
    fn option_bounds_are_enforced() {
        assert!(SayOptions::build(None, Some(0.1), None).is_err());
        assert!(SayOptions::build(None, Some(9.0), None).is_err());
        assert!(SayOptions::build(None, None, Some(10.0)).is_err());
        assert!(SayOptions::build(None, Some(f64::NAN), None).is_err());
        assert!(SayOptions::build(Some("nope"), None, None).unwrap_err().contains("robot"));
        let o = SayOptions::build(Some("tron"), Some(1.5), Some(90.0)).unwrap();
        assert_eq!((o.voice.name, o.speed, o.base_f0()), ("tron", 1.5, 90.0));
    }
}
