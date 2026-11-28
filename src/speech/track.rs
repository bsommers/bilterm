//! Phoneme events -> synthesizer parameter segments plus the intonation plan.

use super::detmath;
use super::phoneme::{Class, Ph};
use super::prosody::{Event, takes_vowel_context};
use super::text::Boundary;

/// Instantaneous synthesizer parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Params {
    pub f: [f64; 3],
    /// Voicing, frication and aspiration amplitudes (0..1).
    pub av: f64,
    pub af: f64,
    pub ah: f64,
    /// Frication band centre and bandwidth (Hz).
    pub ff: f64,
    pub fbw: f64,
    /// 1.0 for nasals (widens low formants).
    pub nasal: f64,
}

/// A constant (or linearly gliding) stretch of parameters.
#[derive(Clone, Debug)]
pub struct Segment {
    pub t0: f64,
    pub t1: f64,
    /// Transition time from the previous segment's end values.
    pub blend: f64,
    pub start: Params,
    pub end: Params,
}

/// One phrase (between pauses) for the pitch contour.
#[derive(Clone, Debug)]
pub struct Phrase {
    pub t0: f64,
    pub t1: f64,
    pub kind: Option<Boundary>,
    /// Stressed-vowel centres (ms).
    pub accents: Vec<f64>,
}

pub struct Track {
    pub segs: Vec<Segment>,
    pub phrases: Vec<Phrase>,
    pub total_ms: f64,
}

fn f3(v: [f32; 3]) -> [f64; 3] {
    [f64::from(v[0]), f64::from(v[1]), f64::from(v[2])]
}

/// Builds the track. `blend_ms` is the voice's phoneme transition time.
pub fn build(events: &[Event], blend_ms: f64) -> Track {
    let mut segs: Vec<Segment> = Vec::new();
    let mut phrases: Vec<Phrase> = vec![Phrase { t0: 0.0, t1: 0.0, kind: None, accents: Vec::new() }];
    let mut t = 0.0;
    let mut last_f = f3(Ph::AX.spec().f);
    let add = |segs: &mut Vec<Segment>, t: &mut f64, dur: f64, blend: f64, start: Params, end: Params| {
        if dur <= 0.0 {
            return;
        }
        segs.push(Segment { t0: *t, t1: *t + dur, blend: blend.min(dur), start, end });
        *t += dur;
    };
    for (i, ev) in events.iter().enumerate() {
        let Some(ph) = ev.ph else {
            let p = Params { f: last_f, ..Params::default() };
            add(&mut segs, &mut t, ev.dur_ms, 6.0, p, p);
            if let Some(b) = ev.boundary {
                let cur = phrases.last_mut().expect("phrase exists");
                cur.t1 = t;
                cur.kind = Some(b);
                phrases.push(Phrase { t0: t, t1: t, kind: None, accents: Vec::new() });
            }
            continue;
        };
        let s = ph.spec();
        let next_vowel_f = events[i + 1..]
            .iter()
            .take(4)
            .find_map(|e| e.ph.filter(|p| p.is_vowel()))
            .map_or(f3(Ph::AX.spec().f), |p| f3(p.spec().f));
        let amp = if ev.stress > 0 { 1.0 } else { 0.8 };
        let d = ev.dur_ms;
        let base = Params { f: f3(s.f), ff: f64::from(s.noise.0), fbw: f64::from(s.noise.1), ..Params::default() };
        match s.class {
            Class::Vowel | Class::Diphthong => {
                if ev.stress > 0 {
                    phrases.last_mut().expect("phrase exists").accents.push(t + d * 0.4);
                }
                let a = Params { av: amp, ..base };
                let b = Params { f: f3(s.f_end), av: amp, ..base };
                add(&mut segs, &mut t, d, blend_ms, a, b);
                last_f = b.f;
            }
            Class::Approximant => {
                let p = Params { av: 0.7 * amp, ..base };
                add(&mut segs, &mut t, d, blend_ms, p, p);
                last_f = p.f;
            }
            Class::Nasal => {
                let p = Params { av: 0.5 * amp, nasal: 1.0, ..base };
                add(&mut segs, &mut t, d, blend_ms * 0.8, p, p);
                last_f = p.f;
            }
            Class::Fricative => {
                let p = Params { av: if s.voiced { 0.2 } else { 0.0 }, af: f64::from(s.noise.2), ..base };
                add(&mut segs, &mut t, d, blend_ms.min(14.0), p, p);
            }
            Class::Aspirate => {
                let p = Params { f: next_vowel_f, ah: 0.6, ..base };
                add(&mut segs, &mut t, d, 8.0, p, p);
                last_f = p.f;
            }
            Class::Stop | Class::Affricate => {
                let ctx = if takes_vowel_context(ph) { next_vowel_f } else { f3(s.f) };
                let closure = Params { f: f3(s.f), av: if s.voiced { 0.12 } else { 0.0 }, ..base };
                let (c_frac, burst_ms) = if s.class == Class::Stop { (0.55, 10.0) } else { (0.4, d * 0.6) };
                add(&mut segs, &mut t, d * c_frac, 6.0, closure, closure);
                let gain = if s.class == Class::Stop { 2.0 } else { 1.0 };
                let burst = Params { f: ctx, av: if s.voiced { 0.3 } else { 0.0 }, af: f64::from(s.noise.2) * gain, ..base };
                add(&mut segs, &mut t, burst_ms, 1.0, burst, burst);
                if s.class == Class::Stop && !s.voiced {
                    let asp = Params { f: ctx, ah: 0.5, ..base };
                    add(&mut segs, &mut t, (d * (1.0 - c_frac) - burst_ms).max(8.0), 2.0, asp, asp);
                }
                last_f = ctx;
            }
        }
    }
    if let Some(last) = phrases.last_mut() {
        last.t1 = t;
    }
    phrases.retain(|p| p.t1 > p.t0);
    Track { segs, phrases, total_ms: t }
}

/// Pitch offset in semitones at time `t` (before the voice's range scale).
pub fn pitch_semitones(phrases: &[Phrase], t: f64) -> f64 {
    let Some(p) = phrases.iter().find(|p| t >= p.t0 && t < p.t1).or_else(|| phrases.last()) else {
        return 0.0;
    };
    let len = (p.t1 - p.t0).max(1.0);
    let x = ((t - p.t0) / len).clamp(0.0, 1.0);
    let (start, end) = match p.kind {
        Some(Boundary::Question) => (-0.5, 0.0),
        Some(Boundary::Exclaim) => (2.0, -4.0),
        Some(Boundary::Statement) => (1.0, -3.5),
        _ => (1.0, -1.0),
    };
    let mut st = start + (end - start) * x;
    if p.kind == Some(Boundary::Question) {
        let tail = ((t - (p.t1 - 380.0)) / 380.0).clamp(0.0, 1.0);
        st += 5.5 * tail * tail;
    }
    for &a in &p.accents {
        let d = (t - a).abs();
        if d < 90.0 {
            st += 2.5 * (1.0 - d / 90.0);
        }
    }
    st
}

/// Fundamental frequency at `t`.
pub fn f0_at(phrases: &[Phrase], t: f64, base_f0: f64, range: f64) -> f64 {
    base_f0 * detmath::semitones(pitch_semitones(phrases, t) * range)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::{prosody::build_events, text::normalize};

    fn track(s: &str) -> Track {
        build(&build_events(&normalize(s, 200), 1.0, 1e9).0, 25.0)
    }

    #[test]
    fn segments_are_contiguous_and_cover_the_total() {
        let tr = track("this is a test. is it?");
        assert!(tr.segs.len() > 20);
        for w in tr.segs.windows(2) {
            assert!((w[0].t1 - w[1].t0).abs() < 1e-9);
        }
        assert!((tr.segs.last().unwrap().t1 - tr.total_ms).abs() < 1e-9);
        assert_eq!(tr.phrases.len(), 2);
    }

    #[test]
    fn question_rises_and_statement_falls() {
        let q = track("really?");
        let s = track("really.");
        let end_q = pitch_semitones(&q.phrases, q.total_ms - 10.0);
        let end_s = pitch_semitones(&s.phrases, s.total_ms - 10.0);
        assert!(end_q > 2.0, "{end_q}");
        assert!(end_s < -1.0, "{end_s}");
    }

    #[test]
    fn monotone_range_zero_is_flat() {
        let tr = track("flat tone");
        let a = f0_at(&tr.phrases, 10.0, 100.0, 0.0);
        let b = f0_at(&tr.phrases, 300.0, 100.0, 0.0);
        assert!((a - 100.0).abs() < 1e-6 && (b - 100.0).abs() < 1e-6);
    }
}
