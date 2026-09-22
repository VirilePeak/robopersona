//! Deterministic `exp(-x)` for `x >= 0`.
//!
//! Fixed computation budget: one `round`, one multiply/subtract pair,
//! 13 Taylor terms, one exponent-field scaling. No data-dependent loop
//! bounds, no platform libm. Error vs. `f64::exp` < 1e-13 relative on
//! the clamped domain `[0, 64]` (beyond which the result is ≈ 0 anyway).

use core::f64::consts::LN_2;

/// Upper clamp for the argument. `exp(-64) ≈ 1.6e-28`; anything smaller is
/// numerically zero for affect decay purposes.
const MAX_X: f64 = 64.0;

/// Taylor term count. Remainder after 13 terms at |r| ≤ ln(2)/2:
/// r^14 / 14! ≈ 4.4e-18 — far below f64 resolution.
const TERMS: u32 = 13;

/// Computes `exp(-x)` for `x >= 0`, deterministically.
///
/// Negative inputs return `1.0` (treated as "no decay"); callers should
/// reject them upstream. NaN propagates as NaN.
pub fn exp_neg(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x <= 0.0 {
        return 1.0;
    }
    let x = if x > MAX_X { MAX_X } else { x };

    // Range reduction: x = n·ln2 + r with |r| <= ln2/2.
    let n = libm::round(x / LN_2);
    let r = x - n * LN_2;

    // Taylor series of e^{-r}.
    let mut term = 1.0_f64;
    let mut sum = 1.0_f64;
    for k in 1..=TERMS {
        term *= -r / f64::from(k);
        sum += term;
    }

    scale_pow2(sum, -(n as i32))
}

/// Multiplies `x` by `2^exp` via exponent-field arithmetic.
///
/// Safe (no `unsafe`, no subnormal edge) because callers keep
/// `x` in `[0.7, 1.5]` and `exp` in `[-93, 0]`: the biased exponent stays
/// within `[929, 1024]`, so no borrow/carry crosses the mantissa boundary.
fn scale_pow2(x: f64, exp: i32) -> f64 {
    debug_assert!((-93..=0).contains(&exp), "scale_pow2 domain violated");
    let bits = if exp >= 0 {
        x.to_bits() + ((exp as u64) << 52)
    } else {
        x.to_bits() - (((-exp) as u64) << 52)
    };
    f64::from_bits(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_boundaries() {
        assert_eq!(exp_neg(0.0), 1.0);
        assert_eq!(exp_neg(-5.0), 1.0); // negative input contract
        assert!(exp_neg(f64::NAN).is_nan());
        assert_eq!(exp_neg(MAX_X * 10.0), exp_neg(MAX_X));
    }

    #[test]
    fn accuracy_against_std() {
        // Spot-check the full domain against the platform exp (test-only).
        let mut x = 0.0_f64;
        while x <= 64.0 {
            let expected = (-x).exp();
            let got = exp_neg(x);
            let rel_err = if expected == 0.0 {
                got.abs()
            } else {
                ((got - expected) / expected).abs()
            };
            assert!(
                rel_err < 1e-13,
                "x={x}: got {got}, want {expected}, rel {rel_err}"
            );
            x += 0.125;
        }
    }

    #[test]
    fn monotone_decreasing() {
        let mut prev = 1.0;
        let mut x = 0.0;
        while x <= 64.0 {
            let v = exp_neg(x);
            assert!(v <= prev + f64::EPSILON, "not monotone at x={x}");
            assert!((0.0..=1.0).contains(&v), "out of range at x={x}: {v}");
            prev = v;
            x += 0.5;
        }
    }

    #[test]
    fn bit_identical_across_calls() {
        let a = exp_neg(0.377);
        let b = exp_neg(0.377);
        assert_eq!(a.to_bits(), b.to_bits());
    }
}
