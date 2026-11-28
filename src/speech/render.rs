//! Text -> 16-bit PCM pipeline (pure, deterministic, no I/O).

use super::phoneme::Ph;
use super::text::{Item, normalize};
use super::voice::{MAX_AUDIO_SECONDS, MAX_TEXT_CHARS, SAMPLE_RATE, SayOptions};
use super::{fx, klatt, prosody, rules, track};

/// Synthesized audio.
#[derive(Clone, Debug, PartialEq)]
pub struct Rendered {
    pub samples: Vec<i16>,
    /// True when the text did not fit the audio-length cap and was cut off.
    pub truncated: bool,
}

impl Rendered {
    pub fn duration_ms(&self) -> u64 {
        self.samples.len() as u64 * 1000 / u64::from(SAMPLE_RATE)
    }

    /// RIFF/WAVE bytes (16 kHz, 16-bit, mono).
    pub fn to_wav(&self) -> Vec<u8> {
        crate::stt::wav::encode_wav_s16le_mono(&self.samples)
    }
}

/// Validates `text` against the input bounds shared by every surface.
pub fn check_text(text: &str) -> Result<(), String> {
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(format!("text too long (max {MAX_TEXT_CHARS} characters)"));
    }
    if text.trim().is_empty() {
        return Err("nothing to say (empty text)".to_string());
    }
    Ok(())
}

/// Renders `text` with `opts`. Rejects over-long or unspeakable input.
pub fn render_text(text: &str, opts: &SayOptions) -> Result<Rendered, String> {
    check_text(text)?;
    let items = normalize(text, MAX_TEXT_CHARS);
    let max_ms = f64::from(MAX_AUDIO_SECONDS) * 1000.0;
    let (events, truncated) = prosody::build_events(&items, opts.speed, max_ms);
    if events.is_empty() {
        return Err("nothing speakable in the text".to_string());
    }
    let tr = track::build(&events, opts.voice.blend_ms);
    let max_samples = (MAX_AUDIO_SECONDS * SAMPLE_RATE) as usize;
    let raw = klatt::render(&tr, opts, max_samples);
    Ok(Rendered { samples: fx::finish(raw, opts.voice), truncated })
}

/// Phoneme string for `text` (space separated ARPAbet, `|` between words); for tests and debugging.
pub fn phonemes(text: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    for item in normalize(text, MAX_TEXT_CHARS) {
        if let Item::Word(w) = item {
            let p: Vec<&str> = rules::word_to_phonemes(&w).iter().map(|p: &Ph| p.symbol()).collect();
            words.push(p.join(" "));
        }
    }
    words.join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn fnv1a(samples: &[i16]) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for s in samples {
            for b in s.to_le_bytes() {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        }
        h
    }

    fn opts(v: &str) -> SayOptions {
        SayOptions::build(Some(v), None, None).unwrap()
    }

    #[test]
    fn phonemes_of_a_sentence() {
        assert_eq!(phonemes("Hello world"), "HH AX L OW | W ER L D");
    }

    #[test]
    fn output_is_non_silent_bounded_and_deterministic() {
        for v in ["robot", "tron", "votrax", "sam", "dectalk"] {
            let a = render_text("Greetings, program. Shall we play a game?", &opts(v)).unwrap();
            let b = render_text("Greetings, program. Shall we play a game?", &opts(v)).unwrap();
            assert_eq!(a, b, "{v} deterministic");
            let peak = a.samples.iter().map(|s| i32::from(s.unsigned_abs())).max().unwrap();
            assert!((20_000..=32_767).contains(&peak), "{v} peak {peak}");
            let nonzero = a.samples.iter().filter(|s| **s != 0).count();
            assert!(nonzero * 5 > a.samples.len(), "{v} mostly non-silent");
            assert!(a.duration_ms() > 1_500 && a.duration_ms() < 8_000, "{v} {}", a.duration_ms());
            assert!(!a.truncated);
        }
    }

    #[test]
    fn voices_sound_different_and_speed_changes_length() {
        let t = "testing one two three";
        let r = render_text(t, &opts("robot")).unwrap();
        let tr = render_text(t, &opts("tron")).unwrap();
        assert_ne!(fnv1a(&r.samples), fnv1a(&tr.samples));
        let fast = render_text(t, &SayOptions::build(None, Some(2.0), None).unwrap()).unwrap();
        assert!(fast.samples.len() * 3 < r.samples.len() * 2);
        let high = render_text(t, &SayOptions::build(None, None, Some(200.0)).unwrap()).unwrap();
        assert_ne!(fnv1a(&high.samples), fnv1a(&r.samples));
    }

    #[test]
    fn audio_length_is_capped() {
        let text = "supercalifragilistic ".repeat(95);
        let r = render_text(&text, &opts("robot")).unwrap();
        assert!(r.truncated);
        assert!(r.samples.len() <= (MAX_AUDIO_SECONDS * SAMPLE_RATE) as usize);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(render_text("", &opts("robot")).is_err());
        assert!(render_text("   ", &opts("robot")).is_err());
        assert!(render_text("?!...", &opts("robot")).is_err());
        assert!(render_text(&"x".repeat(MAX_TEXT_CHARS + 1), &opts("robot")).is_err());
        assert!(render_text("日本語 \u{1b}[0m", &opts("robot")).is_ok(), "controls/unicode are just separators");
    }

    #[test]
    fn wav_container_is_valid() {
        let r = render_text("ok", &opts("robot")).unwrap();
        let w = r.to_wav();
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        assert_eq!(w.len(), 44 + r.samples.len() * 2);
    }

    /// Golden hashes pin the exact PCM so any DSP change is deliberate. `detmath` keeps these
    /// identical across Linux, macOS and Windows.
    #[test]
    fn golden_pcm_hashes() {
        let got: Vec<(String, u64)> = ["robot", "tron", "votrax", "sam", "dectalk"]
            .iter()
            .map(|v| ((*v).to_string(), fnv1a(&render_text("Hello world. Bilterm is online.", &opts(v)).unwrap().samples)))
            .collect();
        eprintln!("GOLDEN {got:?}"); // copy these into GOLDEN after an intentional DSP change
        let expected: [(&str, u64); 5] = GOLDEN;
        for ((n, h), (en, eh)) in got.iter().zip(expected.iter()) {
            assert_eq!(n, en);
            assert_eq!(h, eh, "golden hash for {n} changed (update GOLDEN only for an intentional DSP change)");
        }
    }

    const GOLDEN: [(&str, u64); 5] = [
        ("robot", 10_835_218_583_541_662_645),
        ("tron", 7_219_737_491_268_751_379),
        ("votrax", 11_149_074_772_158_883_505),
        ("sam", 1_794_456_482_056_174_871),
        ("dectalk", 10_624_260_771_126_587_427),
    ];
}

