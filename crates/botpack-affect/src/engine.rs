//! The affect engine: PAD state, stimulus application, ODE decay ticks.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::exp::exp_neg;
use crate::{Error, Result};

/// A duration in seconds. Constructed via [`Seconds::new`] — cannot hold
/// negative or non-finite values.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Seconds(f64);

impl Seconds {
    /// Validates and wraps a duration. Must be finite and `>= 0`.
    pub fn new(value: f64) -> Result<Self> {
        if !value.is_finite() || value < 0.0 {
            return Err(Error::InvalidDuration { value });
        }
        Ok(Self(value))
    }

    /// Raw value.
    pub fn get(self) -> f64 {
        self.0
    }
}

impl fmt::Display for Seconds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}s", self.0)
    }
}

/// PAD affect vector, each axis clamped to `[-1, 1]`.
///
/// Construction is total: out-of-range and non-finite inputs are clamped
/// to the nearest representable state (NaN → 0), so an `AffectVector` is
/// always a valid point in the PAD cube. Invalid *config* values are
/// rejected instead — see [`AffectConfig`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AffectVector {
    /// Pleasure: unpleasant (-1) … pleasant (+1).
    pub valence: f64,
    /// Arousal: calm (-1) … excited (+1).
    pub arousal: f64,
    /// Dominance: submissive (-1) … dominant (+1).
    pub dominance: f64,
}

impl AffectVector {
    /// Clamps all axes into `[-1, 1]`, mapping NaN → 0.
    pub fn new(valence: f64, arousal: f64, dominance: f64) -> Self {
        Self {
            valence: clamp_axis(valence),
            arousal: clamp_axis(arousal),
            dominance: clamp_axis(dominance),
        }
    }

    /// The neutral point (0, 0, 0).
    pub const ZERO: Self = Self {
        valence: 0.0,
        arousal: 0.0,
        dominance: 0.0,
    };

    /// Euclidean distance to another vector.
    pub fn distance(self, other: Self) -> f64 {
        let d = self.valence - other.valence;
        let a = self.arousal - other.arousal;
        let o = self.dominance - other.dominance;
        libm::sqrt(d * d + a * a + o * o)
    }
}

fn clamp_axis(v: f64) -> f64 {
    if v.is_nan() { 0.0 } else { v.clamp(-1.0, 1.0) }
}

/// Engine configuration. Validated at construction — invalid states are
/// unrepresentable afterwards.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AffectConfig {
    /// Resting state the vector decays toward.
    pub baseline: AffectVector,
    /// Stimulus decay rate λ in `state = baseline + (s - baseline)·e^{-λt}`.
    /// Must be finite and `> 0`.
    pub decay_rate: f64,
    /// Mood time constant τ (seconds) for slow baseline drift. Must be
    /// finite and `> 0`. (Reserved for baseline adaptation; decay uses
    /// `decay_rate`.)
    pub mood_tau: f64,
    /// Maximum state change per stimulus, in each axis. Must be finite
    /// and in `(0, 2]` (the PAD cube diameter).
    pub max_step: f64,
}

impl AffectConfig {
    /// Validates the configuration.
    pub fn validate(&self) -> Result<()> {
        if !self.decay_rate.is_finite() || self.decay_rate <= 0.0 {
            return Err(Error::InvalidConfig("decay_rate must be finite and > 0"));
        }
        if !self.mood_tau.is_finite() || self.mood_tau <= 0.0 {
            return Err(Error::InvalidConfig("mood_tau must be finite and > 0"));
        }
        if !self.max_step.is_finite() || self.max_step <= 0.0 || self.max_step > 2.0 {
            return Err(Error::InvalidConfig("max_step must be in (0, 2]"));
        }
        Ok(())
    }
}

/// Deterministic affect state machine.
///
/// Invariant: `state` is always inside the PAD cube (guaranteed by
/// [`AffectVector`] construction) and every transition is a pure function
/// of `(state, config, stimulus, dt)` — same inputs, bit-identical output.
#[derive(Debug, Clone)]
pub struct AffectEngine {
    config: AffectConfig,
    state: AffectVector,
    elapsed: f64,
}

impl AffectEngine {
    /// Creates an engine at its baseline state.
    pub fn new(config: AffectConfig) -> Result<Self> {
        config.validate()?;
        let baseline = config.baseline;
        Ok(Self {
            config,
            state: baseline,
            elapsed: 0.0,
        })
    }

    /// Current affect state.
    pub fn state(&self) -> AffectVector {
        self.state
    }

    /// Total simulated time.
    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }

    /// Applies an instantaneous stimulus: moves the state toward the
    /// stimulus point, limited by `max_step` per axis.
    pub fn apply_stimulus(&mut self, stimulus: AffectVector) {
        self.state = AffectVector::new(
            step_axis(self.state.valence, stimulus.valence, self.config.max_step),
            step_axis(self.state.arousal, stimulus.arousal, self.config.max_step),
            step_axis(
                self.state.dominance,
                stimulus.dominance,
                self.config.max_step,
            ),
        );
    }

    /// Advances time by `dt`: exponential decay toward the baseline.
    ///
    /// `state' = baseline + (state - baseline) · exp(-λ·dt)`
    pub fn tick(&mut self, dt: Seconds) {
        let t = dt.get();
        self.elapsed += t;
        let factor = exp_neg(self.config.decay_rate * t);
        self.state = AffectVector::new(
            decay_axis(self.state.valence, self.config.baseline.valence, factor),
            decay_axis(self.state.arousal, self.config.baseline.arousal, factor),
            decay_axis(self.state.dominance, self.config.baseline.dominance, factor),
        );
    }
}

/// Moves `from` toward `to` by at most `max_step`.
fn step_axis(from: f64, to: f64, max_step: f64) -> f64 {
    let delta = to - from;
    if delta.abs() <= max_step {
        to
    } else {
        from + delta.signum() * max_step
    }
}

/// One decay step on a single axis: `s' = base + (s - base)·factor`.
fn decay_axis(state: f64, baseline: f64, factor: f64) -> f64 {
    baseline + (state - baseline) * factor
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> AffectConfig {
        AffectConfig {
            baseline: AffectVector::new(0.1, 0.2, 0.0),
            decay_rate: 0.5,
            mood_tau: 60.0,
            max_step: 0.3,
        }
    }

    #[test]
    fn vector_clamps_everything() {
        let v = AffectVector::new(f64::NAN, 5.0, -7.0);
        assert_eq!(v, AffectVector::new(0.0, 1.0, -1.0));
    }

    #[test]
    fn config_rejects_invalid() {
        let mut c = config();
        c.decay_rate = 0.0;
        assert!(AffectEngine::new(c).is_err());
        let mut c = config();
        c.decay_rate = f64::NAN;
        assert!(AffectEngine::new(c).is_err());
        let mut c = config();
        c.max_step = 3.0;
        assert!(AffectEngine::new(c).is_err());
        let mut c = config();
        c.mood_tau = -1.0;
        assert!(AffectEngine::new(c).is_err());
    }

    #[test]
    fn seconds_rejects_negative_and_nan() {
        assert!(Seconds::new(-0.1).is_err());
        assert!(Seconds::new(f64::NAN).is_err());
        assert!(Seconds::new(f64::INFINITY).is_err());
        assert!(Seconds::new(0.0).is_ok());
    }

    #[test]
    fn stimulus_limited_by_max_step() {
        let mut e = AffectEngine::new(config()).unwrap();
        e.apply_stimulus(AffectVector::new(1.0, 1.0, 1.0));
        // baseline 0.1/0.2/0.0 + 0.3 step each
        let s = e.state();
        assert!((s.valence - 0.4).abs() < 1e-12);
        assert!((s.arousal - 0.5).abs() < 1e-12);
        assert!((s.dominance - 0.3).abs() < 1e-12);
    }

    #[test]
    fn decay_returns_toward_baseline() {
        let mut e = AffectEngine::new(config()).unwrap();
        e.apply_stimulus(AffectVector::new(0.4, 0.5, 0.3));
        let excited = e.state();
        e.tick(Seconds::new(10.0).unwrap());
        let decayed = e.state();
        // Still above baseline on valence, but closer than before.
        assert!(decayed.valence > 0.1);
        assert!(decayed.valence < excited.valence);
        // exp(-0.5*10) = exp(-5) ≈ 0.0067 → nearly back at baseline.
        assert!(decayed.distance(config().baseline) < 0.01);
    }

    #[test]
    fn zero_tick_is_identity() {
        let mut e = AffectEngine::new(config()).unwrap();
        e.apply_stimulus(AffectVector::new(0.4, 0.5, 0.3));
        let before = e.state();
        e.tick(Seconds::new(0.0).unwrap());
        assert_eq!(e.state(), before);
    }

    #[test]
    fn determinism_bit_identical() {
        let run = || {
            let mut e = AffectEngine::new(config()).unwrap();
            e.apply_stimulus(AffectVector::new(0.9, -0.3, 0.6));
            for _ in 0..10 {
                e.tick(Seconds::new(1.7).unwrap());
            }
            e.state()
        };
        let a = run();
        let b = run();
        assert_eq!(a.valence.to_bits(), b.valence.to_bits());
        assert_eq!(a.arousal.to_bits(), b.arousal.to_bits());
        assert_eq!(a.dominance.to_bits(), b.dominance.to_bits());
    }

    #[test]
    fn long_time_converges_exactly_to_baseline() {
        let mut e = AffectEngine::new(config()).unwrap();
        e.apply_stimulus(AffectVector::new(1.0, -1.0, 1.0));
        e.tick(Seconds::new(10_000.0).unwrap());
        let s = e.state();
        assert!((s.valence - 0.1).abs() < 1e-20);
        assert!((s.arousal - 0.2).abs() < 1e-20);
    }

    #[test]
    fn json_roundtrip() {
        let c = config();
        let json = serde_json::to_string(&c).unwrap();
        let back: AffectConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }
}
