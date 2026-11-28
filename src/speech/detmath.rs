//! Deterministic math for the speech synthesizer.
//!
//! `f64::sin`/`exp` call the platform libm and may differ in the last ulp between Linux, macOS and
//! Windows. Golden-hash tests of the synthesized PCM must be byte-identical everywhere, so the
//! synthesizer uses only `+ - * /` (IEEE-754 exact, no fused multiply-add is emitted by rustc) via
//! the small polynomial implementations below.

pub const PI: f64 = std::f64::consts::PI;
const TWO_PI: f64 = 2.0 * PI;
const LN2: f64 = std::f64::consts::LN_2;

/// Sine of `x` radians, absolute error < 1e-7.
pub fn sin(x: f64) -> f64 {
    let mut r = x - TWO_PI * (x / TWO_PI + 0.5).floor();
    if r > PI / 2.0 {
        r = PI - r;
    } else if r < -PI / 2.0 {
        r = -PI - r;
    }
    let r2 = r * r;
    // Horner form of the Taylor series through x^13.
    let mut acc = -1.0 / 6_227_020_800.0;
    acc = acc * r2 + 1.0 / 39_916_800.0;
    acc = acc * r2 - 1.0 / 362_880.0;
    acc = acc * r2 + 1.0 / 5_040.0;
    acc = acc * r2 - 1.0 / 120.0;
    acc = acc * r2 + 1.0 / 6.0;
    acc = acc * r2 - 1.0;
    // sin(r) = r * (1 - r2/6 + r2^2/120 ...) ; the Horner chain above computes -(series)/... so negate
    // consistently: acc now equals -(1 - r2/6 + ...) with the sign folded in.
    -acc * r
}

/// Cosine of `x` radians.
pub fn cos(x: f64) -> f64 {
    sin(x + PI / 2.0)
}

/// `e^x`, relative error < 1e-9 for |x| < 40; clamps beyond.
pub fn exp(x: f64) -> f64 {
    let x = x.clamp(-40.0, 40.0);
    let n = (x / LN2 + if x >= 0.0 { 0.5 } else { -0.5 }).trunc();
    let r = x - n * LN2;
    let mut term = 1.0;
    let mut sum = 1.0;
    for k in 1..=12 {
        term = term * r / f64::from(k);
        sum += term;
    }
    let mut k = n as i32;
    while k > 0 {
        sum *= 2.0;
        k -= 1;
    }
    while k < 0 {
        sum *= 0.5;
        k += 1;
    }
    sum
}

/// Frequency ratio for `st` semitones (`2^(st/12)`).
pub fn semitones(st: f64) -> f64 {
    exp(st * LN2 / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_matches_libm_within_tolerance() {
        let mut x = -20.0;
        while x < 20.0 {
            assert!((sin(x) - x.sin()).abs() < 1e-6, "sin({x})");
            assert!((cos(x) - x.cos()).abs() < 1e-6, "cos({x})");
            x += 0.173;
        }
    }

    #[test]
    fn exp_matches_libm_within_tolerance() {
        let mut x = -12.0;
        while x < 12.0 {
            let rel = (exp(x) - x.exp()).abs() / x.exp();
            assert!(rel < 1e-8, "exp({x}) rel {rel}");
            x += 0.37;
        }
        assert!((semitones(12.0) - 2.0).abs() < 1e-8);
    }
}
