//! Retro output stage: ring modulation, echo, peak normalisation, sample-and-hold, bit-crush.

use super::detmath;
use super::voice::{SAMPLE_RATE, Voice};

/// Scales so the peak is `target` (0..1) of full scale. Silent input stays silent.
pub fn normalize_peak(x: &mut [f64], target: f64) {
    let peak = x.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    if peak > 1e-12 {
        let g = target / peak;
        x.iter_mut().for_each(|v| *v *= g);
    }
}

/// Applies the voice's colouring and converts to 16-bit PCM.
pub fn finish(mut x: Vec<f64>, v: &Voice) -> Vec<i16> {
    let fs = f64::from(SAMPLE_RATE);
    if v.ring_mix > 0.0 && v.ring_hz > 0.0 {
        let step = 2.0 * detmath::PI * v.ring_hz / fs;
        let mut phase = 0.0;
        for s in x.iter_mut() {
            let c = detmath::sin(phase);
            phase += step;
            if phase > 2.0 * detmath::PI {
                phase -= 2.0 * detmath::PI;
            }
            *s = *s * (1.0 - v.ring_mix) + *s * c * v.ring_mix;
        }
    }
    if v.echo_ms > 0.0 {
        let d = (v.echo_ms * fs / 1000.0) as usize;
        for n in d..x.len() {
            x[n] += x[n - d] * v.echo_gain;
        }
    }
    normalize_peak(&mut x, 0.85);
    if v.hold > 1 {
        let h = v.hold as usize;
        for n in 0..x.len() {
            x[n] = x[n - n % h];
        }
    }
    let levels = f64::from(1u32 << (v.bits.clamp(4, 16) - 1));
    x.iter()
        .map(|s| {
            let q = if v.bits < 16 { (s * levels).round() / levels } else { *s };
            (q * 32767.0).round().clamp(-32768.0, 32767.0) as i16
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::voice::find_voice;

    #[test]
    fn normalize_hits_target_and_keeps_silence() {
        let mut a = vec![0.1, -0.4, 0.2];
        normalize_peak(&mut a, 0.8);
        assert!((a[1] + 0.8).abs() < 1e-12);
        let mut z = vec![0.0; 4];
        normalize_peak(&mut z, 0.8);
        assert!(z.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn bit_crush_limits_distinct_levels_and_hold_repeats() {
        let x: Vec<f64> = (0..4000).map(|n| detmath::sin(f64::from(n) * 0.05)).collect();
        let v = find_voice("sam").unwrap();
        let out = finish(x, v);
        let mut levels: Vec<i16> = out.clone();
        levels.sort_unstable();
        levels.dedup();
        assert!(levels.len() <= 256, "8-bit voice, got {}", levels.len());
        assert_eq!(out[0], out[1]);
        assert_eq!(out[1], out[2]);
    }
}
