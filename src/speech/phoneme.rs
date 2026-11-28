//! Phoneme inventory (ARPAbet subset) and its formant targets.
//!
//! Vowel and sonorant formant values follow the classic Klatt (1980) / Peterson-Barney tables;
//! consonants carry a frication band or a stop burst spectrum. All numbers are plain data.

/// One phoneme (ARPAbet-style symbol).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[rustfmt::skip]
pub enum Ph {
    IY, IH, EH, AE, AA, AH, AO, UH, UW, ER, AX, EY, AY, OY, AW, OW,
    P, B, T, D, K, G, F, V, TH, DH, S, Z, SH, ZH, HH, CH, JH,
    M, N, NG, L, R, W, Y,
}

/// Articulation class; decides how the synthesizer builds the parameter track.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Vowel,
    Diphthong,
    Stop,
    Affricate,
    Fricative,
    Aspirate,
    Nasal,
    Approximant,
}

/// Static description of one phoneme.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub class: Class,
    /// F1, F2, F3 targets in Hz (start of the phoneme for diphthongs).
    pub f: [f32; 3],
    /// F1, F2, F3 at the end (diphthongs); equal to `f` otherwise.
    pub f_end: [f32; 3],
    /// Nominal duration in milliseconds at speed 1.0.
    pub dur_ms: f32,
    pub voiced: bool,
    /// Frication / burst noise centre (Hz), bandwidth (Hz), amplitude (0..1).
    pub noise: (f32, f32, f32),
}

const SYMBOLS: &[(&str, Ph)] = &[
    ("IY", Ph::IY), ("IH", Ph::IH), ("EH", Ph::EH), ("AE", Ph::AE), ("AA", Ph::AA), ("AH", Ph::AH),
    ("AO", Ph::AO), ("UH", Ph::UH), ("UW", Ph::UW), ("ER", Ph::ER), ("AX", Ph::AX), ("EY", Ph::EY),
    ("AY", Ph::AY), ("OY", Ph::OY), ("AW", Ph::AW), ("OW", Ph::OW), ("P", Ph::P), ("B", Ph::B),
    ("T", Ph::T), ("D", Ph::D), ("K", Ph::K), ("G", Ph::G), ("F", Ph::F), ("V", Ph::V),
    ("TH", Ph::TH), ("DH", Ph::DH), ("S", Ph::S), ("Z", Ph::Z), ("SH", Ph::SH), ("ZH", Ph::ZH),
    ("HH", Ph::HH), ("CH", Ph::CH), ("JH", Ph::JH), ("M", Ph::M), ("N", Ph::N), ("NG", Ph::NG),
    ("L", Ph::L), ("R", Ph::R), ("W", Ph::W), ("Y", Ph::Y),
];

impl Ph {
    /// Parses an ARPAbet symbol such as `"AE"`.
    pub fn from_symbol(s: &str) -> Option<Ph> {
        SYMBOLS.iter().find(|(n, _)| *n == s).map(|(_, p)| *p)
    }

    /// ARPAbet symbol.
    pub fn symbol(self) -> &'static str {
        SYMBOLS.iter().find(|(_, p)| *p == self).map_or("?", |(n, _)| n)
    }

    pub fn is_vowel(self) -> bool {
        matches!(self.spec().class, Class::Vowel | Class::Diphthong)
    }

    /// Static spec of this phoneme.
    pub fn spec(self) -> Spec {
        use Class::*;
        let v = |f: [f32; 3], dur: f32| Spec { class: Vowel, f, f_end: f, dur_ms: dur, voiced: true, noise: (0.0, 0.0, 0.0) };
        let d = |f: [f32; 3], e: [f32; 3], dur: f32| Spec { class: Diphthong, f, f_end: e, dur_ms: dur, voiced: true, noise: (0.0, 0.0, 0.0) };
        let c = |class, f: [f32; 3], dur: f32, voiced: bool, noise: (f32, f32, f32)| Spec { class, f, f_end: f, dur_ms: dur, voiced, noise };
        match self {
            Ph::IY => v([290.0, 2200.0, 2950.0], 120.0),
            Ph::IH => v([400.0, 1900.0, 2570.0], 85.0),
            Ph::EH => v([530.0, 1700.0, 2500.0], 100.0),
            Ph::AE => v([660.0, 1700.0, 2400.0], 130.0),
            Ph::AA => v([730.0, 1100.0, 2440.0], 130.0),
            Ph::AH => v([620.0, 1220.0, 2550.0], 85.0),
            Ph::AO => v([570.0, 850.0, 2410.0], 130.0),
            Ph::UH => v([440.0, 1020.0, 2240.0], 85.0),
            Ph::UW => v([320.0, 950.0, 2200.0], 130.0),
            Ph::ER => v([470.0, 1270.0, 1540.0], 120.0),
            Ph::AX => v([500.0, 1500.0, 2500.0], 60.0),
            Ph::EY => d([530.0, 1700.0, 2500.0], [310.0, 2100.0, 2900.0], 150.0),
            Ph::AY => d([730.0, 1100.0, 2440.0], [400.0, 1900.0, 2570.0], 170.0),
            Ph::OY => d([570.0, 850.0, 2410.0], [330.0, 2100.0, 2800.0], 170.0),
            Ph::AW => d([730.0, 1100.0, 2440.0], [440.0, 1000.0, 2240.0], 170.0),
            Ph::OW => d([500.0, 900.0, 2300.0], [350.0, 850.0, 2200.0], 150.0),
            Ph::P => c(Stop, [400.0, 800.0, 2400.0], 60.0, false, (900.0, 1500.0, 0.25)),
            Ph::B => c(Stop, [300.0, 800.0, 2400.0], 55.0, true, (900.0, 1500.0, 0.2)),
            Ph::T => c(Stop, [400.0, 1800.0, 2700.0], 60.0, false, (4200.0, 2000.0, 0.35)),
            Ph::D => c(Stop, [300.0, 1800.0, 2700.0], 50.0, true, (4000.0, 2000.0, 0.25)),
            Ph::K => c(Stop, [400.0, 1800.0, 2400.0], 65.0, false, (2000.0, 1200.0, 0.35)),
            Ph::G => c(Stop, [300.0, 1700.0, 2400.0], 55.0, true, (1900.0, 1200.0, 0.25)),
            Ph::F => c(Fricative, [400.0, 1300.0, 2500.0], 95.0, false, (5200.0, 3500.0, 0.12)),
            Ph::V => c(Fricative, [300.0, 1300.0, 2500.0], 65.0, true, (5200.0, 3500.0, 0.10)),
            Ph::TH => c(Fricative, [400.0, 1400.0, 2500.0], 90.0, false, (5000.0, 3500.0, 0.10)),
            Ph::DH => c(Fricative, [300.0, 1400.0, 2500.0], 50.0, true, (5000.0, 3500.0, 0.08)),
            Ph::S => c(Fricative, [400.0, 1700.0, 2600.0], 110.0, false, (5600.0, 1500.0, 0.55)),
            Ph::Z => c(Fricative, [300.0, 1700.0, 2600.0], 80.0, true, (5600.0, 1500.0, 0.40)),
            Ph::SH => c(Fricative, [400.0, 1700.0, 2400.0], 115.0, false, (2900.0, 1300.0, 0.55)),
            Ph::ZH => c(Fricative, [300.0, 1700.0, 2400.0], 85.0, true, (2900.0, 1300.0, 0.40)),
            Ph::HH => c(Aspirate, [500.0, 1500.0, 2500.0], 70.0, false, (0.0, 0.0, 0.0)),
            Ph::CH => c(Affricate, [400.0, 1700.0, 2400.0], 120.0, false, (3000.0, 1400.0, 0.5)),
            Ph::JH => c(Affricate, [300.0, 1700.0, 2400.0], 100.0, true, (3000.0, 1400.0, 0.35)),
            Ph::M => c(Nasal, [250.0, 1100.0, 2400.0], 75.0, true, (0.0, 0.0, 0.0)),
            Ph::N => c(Nasal, [250.0, 1600.0, 2600.0], 70.0, true, (0.0, 0.0, 0.0)),
            Ph::NG => c(Nasal, [250.0, 2000.0, 2700.0], 85.0, true, (0.0, 0.0, 0.0)),
            Ph::L => c(Approximant, [360.0, 1200.0, 2700.0], 70.0, true, (0.0, 0.0, 0.0)),
            Ph::R => c(Approximant, [330.0, 1060.0, 1400.0], 75.0, true, (0.0, 0.0, 0.0)),
            Ph::W => c(Approximant, [290.0, 650.0, 2150.0], 70.0, true, (0.0, 0.0, 0.0)),
            Ph::Y => c(Approximant, [260.0, 2100.0, 3020.0], 65.0, true, (0.0, 0.0, 0.0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_round_trip() {
        for (name, ph) in SYMBOLS {
            assert_eq!(Ph::from_symbol(name), Some(*ph));
            assert_eq!(ph.symbol(), *name);
        }
        assert_eq!(Ph::from_symbol("QQ"), None);
        assert_eq!(SYMBOLS.len(), 40);
    }

    #[test]
    fn specs_are_sane() {
        for (_, ph) in SYMBOLS {
            let s = ph.spec();
            assert!(s.dur_ms >= 40.0 && s.dur_ms <= 200.0, "{ph:?}");
            assert!(s.f[0] < s.f[1] && s.f[1] < s.f[2], "{ph:?} formants ascend");
            assert!(s.f_end[0] < s.f_end[1] && s.f_end[1] < s.f_end[2]);
        }
        assert!(Ph::AE.is_vowel() && !Ph::S.is_vowel());
    }
}
