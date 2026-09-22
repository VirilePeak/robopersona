//! The runtime loop: deterministic event-scheduled stimulus application.
//!
//! A [`RuntimeLoop`] holds an engine plus a time-ordered stimulus queue.
//! Advancing time (`advance`) applies every stimulus whose timestamp falls
//! within the elapsed window, in `(timestamp, insertion_order)` order, and
//! decays the engine across the window in fixed sub-ticks. Replaying the
//! same event sequence yields a bit-identical state trace.

use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::engine::{AffectConfig, AffectEngine, AffectVector, Seconds};
use crate::{Error, Result};

/// A scheduled stimulus event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StimulusEvent {
    /// Time (seconds since loop start) at which the stimulus fires.
    pub at: f64,
    /// The stimulus vector.
    pub stimulus: AffectVector,
}

/// Deterministic runtime loop over an [`AffectEngine`].
#[derive(Debug, Clone)]
pub struct RuntimeLoop {
    engine: AffectEngine,
    queue: Vec<StimulusEvent>,
    now: f64,
    /// Fixed sub-tick length for decay integration (seconds).
    tick_len: f64,
}

impl RuntimeLoop {
    /// Creates a loop. `tick_len` must be finite and `> 0`.
    pub fn new(config: AffectConfig, tick_len: f64) -> Result<Self> {
        let engine = AffectEngine::new(config)?;
        if !tick_len.is_finite() || tick_len <= 0.0 {
            return Err(Error::InvalidConfig("tick_len must be finite and > 0"));
        }
        Ok(Self {
            engine,
            queue: Vec::new(),
            now: 0.0,
            tick_len,
        })
    }

    /// Schedules a stimulus. Timestamps must be finite and `>= now`
    /// (no rewriting the past — determinism requires a monotone clock).
    pub fn schedule(&mut self, event: StimulusEvent) -> Result<()> {
        if !event.at.is_finite() || event.at < self.now {
            return Err(Error::InvalidConfig(
                "event time must be finite and >= current loop time",
            ));
        }
        self.queue.push(event);
        // Stable sort: (timestamp, insertion order) — deterministic order.
        self.queue.sort_by(|a, b| {
            a.at.partial_cmp(&b.at)
                .unwrap_or(core::cmp::Ordering::Equal)
        });
        Ok(())
    }

    /// Current loop time.
    pub fn now(&self) -> f64 {
        self.now
    }

    /// Current engine state.
    pub fn state(&self) -> AffectVector {
        self.engine.state()
    }

    /// Advances the loop by `duration`, firing all due stimuli.
    ///
    /// Each stimulus fires at its **exact timestamp**: decay is integrated
    /// up to `at`, the stimulus is applied, then decay continues. The fire
    /// point is therefore independent of how the caller chunks `advance`
    /// calls. Between events, decay runs in fixed `tick_len` sub-ticks on
    /// an absolute grid. Bit-identical replay requires the *same* chunking;
    /// different chunkings agree to floating-point epsilon (exp-decay
    /// factorization is not bitwise associative).
    pub fn advance(&mut self, duration: Seconds) {
        let target = self.now + duration.get();
        while self.now < target {
            // Next event time within the window, else window end.
            let next_event = self
                .queue
                .iter()
                .map(|e| e.at)
                .filter(|t| *t > self.now && *t <= target)
                .fold(f64::INFINITY, f64::min);
            let window_end = next_event.min(target);

            // Decay to the next stop point in absolute-grid sub-ticks.
            while self.now < window_end {
                let mut next = (libm::floor(self.now / self.tick_len) + 1.0) * self.tick_len;
                if next > window_end {
                    next = window_end;
                }
                let dt = next - self.now;
                self.engine
                    .tick(Seconds::new(dt).expect("dt > 0 by construction"));
                self.now = next;
            }

            // Fire every event exactly at `next_event` (if any).
            if next_event.is_finite() {
                let due: Vec<AffectVector> = self
                    .queue
                    .iter()
                    .filter(|e| e.at == next_event)
                    .map(|e| e.stimulus)
                    .collect();
                self.queue.retain(|e| e.at != next_event);
                for s in due {
                    self.engine.apply_stimulus(s);
                }
            }
        }
    }
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
    fn rejects_past_events() {
        let mut l = RuntimeLoop::new(config(), 1.0).unwrap();
        l.advance(Seconds::new(5.0).unwrap());
        assert!(
            l.schedule(StimulusEvent {
                at: 2.0,
                stimulus: AffectVector::ZERO,
            })
            .is_err()
        );
        assert!(
            l.schedule(StimulusEvent {
                at: 5.0,
                stimulus: AffectVector::ZERO,
            })
            .is_ok()
        );
    }

    #[test]
    fn stimuli_fire_in_order() {
        let mut l = RuntimeLoop::new(config(), 1.0).unwrap();
        // max_step = 0.3: two stimuli in the same window stack to +0.6.
        l.schedule(StimulusEvent {
            at: 1.5,
            stimulus: AffectVector::new(1.0, 0.0, 0.0),
        })
        .unwrap();
        l.schedule(StimulusEvent {
            at: 1.2,
            stimulus: AffectVector::new(1.0, 0.0, 0.0),
        })
        .unwrap();
        l.advance(Seconds::new(2.0).unwrap());
        // Both fired (order irrelevant for the sum), decay ran 2 ticks.
        let v = l.state().valence;
        assert!(
            v > 0.1,
            "stimuli must have lifted valence above baseline: {v}"
        );
        assert!(l.queue.is_empty());
    }

    #[test]
    fn chunking_independent_within_epsilon() {
        // Same events, different caller chunking → same state up to FP
        // epsilon (bitwise identity holds only for identical chunking).
        let run = |chunks: &[f64]| {
            let mut l = RuntimeLoop::new(config(), 0.5).unwrap();
            l.schedule(StimulusEvent {
                at: 1.3,
                stimulus: AffectVector::new(1.0, 0.5, -0.2),
            })
            .unwrap();
            l.schedule(StimulusEvent {
                at: 2.7,
                stimulus: AffectVector::new(-0.8, 0.0, 0.9),
            })
            .unwrap();
            for c in chunks {
                l.advance(Seconds::new(*c).unwrap());
            }
            l.state()
        };
        let a = run(&[4.0]);
        let b = run(&[1.0, 1.0, 1.0, 1.0]);
        let c = run(&[0.4, 2.2, 1.4]);
        for (x, y) in [
            (a.valence, b.valence),
            (a.arousal, b.arousal),
            (a.dominance, b.dominance),
            (a.valence, c.valence),
            (a.arousal, c.arousal),
            (a.dominance, c.dominance),
        ] {
            assert!((x - y).abs() < 1e-12, "{x} vs {y}");
        }
    }

    #[test]
    fn identical_chunking_bit_identical() {
        let run = || {
            let mut l = RuntimeLoop::new(config(), 0.5).unwrap();
            l.schedule(StimulusEvent {
                at: 1.3,
                stimulus: AffectVector::new(1.0, 0.5, -0.2),
            })
            .unwrap();
            for c in [1.0, 3.0] {
                l.advance(Seconds::new(c).unwrap());
            }
            l.state()
        };
        let a = run();
        let b = run();
        assert_eq!(a.valence.to_bits(), b.valence.to_bits());
        assert_eq!(a.arousal.to_bits(), b.arousal.to_bits());
        assert_eq!(a.dominance.to_bits(), b.dominance.to_bits());
    }

    #[test]
    fn json_roundtrip_events() {
        let e = StimulusEvent {
            at: 1.5,
            stimulus: AffectVector::new(0.1, 0.2, 0.3),
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: StimulusEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }
}
