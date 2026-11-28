//! Source-filter (Klatt-style) formant synthesizer core.
//!
//! A buzz source (glottal pulse blended with a sawtooth) plus aspiration noise feeds a cascade of
//! five two-pole resonators (F1..F3 follow the phoneme track, F4/F5 are fixed). Frication is white
//! noise through a separate band resonator added in parallel. Control parameters update every
//! [`FRAME`] samples; everything is plain `f64` arithmetic from [`super::detmath`], so output is
//! bit-identical on every platform.

use super::detmath;
use super::track::{self, Params, Track};
use super::voice::{SAMPLE_RATE, SayOptions};

/// Samples per control frame (4 ms at 16 kHz).
pub const FRAME: usize = 64;

#[derive(Clone, Copy, Default)]
struct Resonator {
    a: f64,
    b: f64,
    c: f64,
    y1: f64,
    y2: f64,
}

impl Resonator {
    fn set(&mut self, freq: f64, bw: f64) {
        let fs = f64::from(SAMPLE_RATE);
        let freq = freq.clamp(60.0, fs * 0.45);
        let bw = bw.clamp(20.0, 4000.0);
        let r = detmath::exp(-detmath::PI * bw / fs);
        self.c = -r * r;
        self.b = 2.0 * r * detmath::cos(2.0 * detmath::PI * freq / fs);
        self.a = 1.0 - self.b - self.c;
    }

    fn run(&mut self, x: f64) -> f64 {
        let y = self.a * x + self.b * self.y1 + self.c * self.y2;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// Deterministic white noise in [-1, 1).
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(self.0 >> 8) / 8_388_608.0 - 1.0
    }
}

fn lerp(a: &Params, b: &Params, t: f64) -> Params {
    let l = |x: f64, y: f64| x + (y - x) * t;
    Params {
        f: [l(a.f[0], b.f[0]), l(a.f[1], b.f[1]), l(a.f[2], b.f[2])],
        av: l(a.av, b.av),
        af: l(a.af, b.af),
        ah: l(a.ah, b.ah),
        ff: if a.fbw == 0.0 { b.ff } else if b.fbw == 0.0 { a.ff } else { l(a.ff, b.ff) },
        fbw: if a.fbw == 0.0 { b.fbw } else if b.fbw == 0.0 { a.fbw } else { l(a.fbw, b.fbw) },
        nasal: l(a.nasal, b.nasal),
    }
}

/// Parameters at time `t` (ms) for segment `k` given the previous segment's end values.
fn params_at(tr: &Track, k: usize, t: f64) -> Params {
    let seg = &tr.segs[k];
    let prev = if k == 0 { Params::default() } else { tr.segs[k - 1].end };
    let local = t - seg.t0;
    if local < seg.blend && seg.blend > 0.0 {
        let a = local / seg.blend;
        let target = lerp(&seg.start, &seg.end, 0.0);
        lerp(&prev, &target, a)
    } else if seg.end != seg.start {
        let span = (seg.t1 - seg.t0 - seg.blend).max(1.0);
        lerp(&seg.start, &seg.end, ((local - seg.blend) / span).clamp(0.0, 1.0))
    } else {
        seg.start
    }
}

/// Renders the track to floating point samples (unnormalised). Stops at `max_samples`.
pub fn render(tr: &Track, opts: &SayOptions, max_samples: usize) -> Vec<f64> {
    let v = opts.voice;
    let fs = f64::from(SAMPLE_RATE);
    let total = (((tr.total_ms / 1000.0) * fs) as usize + FRAME).min(max_samples);
    let mut out = Vec::with_capacity(total);
    let (mut r1, mut r2, mut r3) = (Resonator::default(), Resonator::default(), Resonator::default());
    let (mut r4, mut r5, mut rn) = (Resonator::default(), Resonator::default(), Resonator::default());
    r4.set(3300.0 * v.formant_scale, 250.0);
    r5.set(3750.0 * v.formant_scale, 200.0);
    let mut noise = Noise(0x1234_5678);
    let mut phase = 0.0f64;
    let mut k = 0usize;
    let mut prev_noise = 0.0;
    let mut dc_prev_in = 0.0;
    let mut dc_prev_out = 0.0;
    let oq = 0.6;
    while out.len() < total && !tr.segs.is_empty() {
        let t = out.len() as f64 / fs * 1000.0 + FRAME as f64 * 0.5 / fs * 1000.0;
        while k + 1 < tr.segs.len() && t >= tr.segs[k].t1 {
            k += 1;
        }
        let p = params_at(tr, k, t.min(tr.segs[k].t1));
        let f0 = track::f0_at(&tr.phrases, t, opts.base_f0(), v.pitch_range).clamp(40.0, 400.0);
        let (bwa, bwb) = (v.bw_scale * (1.0 + 1.4 * p.nasal), v.bw_scale);
        r1.set(p.f[0] * v.formant_scale, 70.0 * bwa);
        r2.set(p.f[1] * v.formant_scale, 100.0 * (bwb + 0.4 * p.nasal));
        r3.set(p.f[2] * v.formant_scale, 150.0 * bwb);
        if p.fbw > 0.0 {
            rn.set(p.ff, p.fbw);
        }
        let n = FRAME.min(total - out.len());
        for _ in 0..n {
            phase += f0 / fs;
            if phase >= 1.0 {
                phase -= 1.0;
            }
            let pulse = if phase < oq { detmath::sin(2.0 * detmath::PI * phase / oq) } else { 0.0 };
            let saw = 1.0 - 2.0 * phase;
            let glottal = (1.0 - v.buzz) * pulse + v.buzz * saw * 0.6;
            let src = p.av * glottal + p.ah * noise.next() * 0.03;
            let y = r5.run(r4.run(r3.run(r2.run(r1.run(src)))));
            let nz = noise.next();
            // First difference tilts the noise upward so frication has no low-frequency rumble.
            let fr = if p.af > 0.0 { rn.run(nz - prev_noise) * p.af * 0.07 } else { 0.0 };
            prev_noise = nz;
            let s = y * 0.6 + fr;
            // One-pole DC blocker keeps the saw's offset out of the output.
            let o = s - dc_prev_in + 0.995 * dc_prev_out;
            dc_prev_in = s;
            dc_prev_out = o;
            out.push(o);
        }
    }
    out
}
