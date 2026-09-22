//! botpack-affect — deterministic affect engine for robot personas.
//!
//! Pure mathematics, no I/O, `no_std`-compatible. The engine holds a
//! PAD-style affect vector (valence, arousal, dominance) in `[-1, 1]` and
//! evolves it by first-order exponential decay toward a baseline:
//!
//! ```text
//! state(t + dt) = baseline + (state(t) - baseline) * exp(-λ·dt)
//! ```
//!
//! # Determinism
//! Cross-platform bit-identical results (x86-64, ARM64, wasm32) require
//! avoiding platform `exp()` implementations. [`exp_neg`] is a fixed-budget
//! range-reduction + Taylor implementation using only IEEE-754 basic ops
//! (`+ - * /`, `round`, bit manipulation) — all deterministic in Rust
//! (no FMA contraction, no fast-math).
//!
//! # Example
//! ```
//! use botpack_affect::{AffectConfig, AffectEngine, AffectVector, Seconds};
//!
//! let config = AffectConfig {
//!     baseline: AffectVector::new(0.1, 0.2, 0.0),
//!     decay_rate: 0.5,
//!     mood_tau: 60.0,
//!     max_step: 0.3,
//! };
//! let mut engine = AffectEngine::new(config).unwrap();
//! engine.apply_stimulus(AffectVector::new(0.5, 0.0, 0.0));
//! assert!(engine.state().valence > 0.1);
//! engine.tick(Seconds::new(10.0).unwrap());
//! // decayed back toward baseline, never below it in this direction
//! assert!(engine.state().valence > 0.1);
//! ```

#![cfg_attr(not(test), no_std)]
#![deny(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod engine;
pub mod exp;
pub mod runtime;

pub use engine::{AffectConfig, AffectEngine, AffectVector, Seconds};
pub use runtime::{RuntimeLoop, StimulusEvent};
use thiserror::Error;

/// All errors produced by botpack-affect.
#[derive(Debug, Error)]
pub enum Error {
    /// A configuration value violates its domain (NaN, sign, range).
    #[error("invalid config: {0}")]
    InvalidConfig(&'static str),

    /// A duration is negative or non-finite.
    #[error("invalid duration: {value} (must be finite and >= 0)")]
    InvalidDuration {
        /// The rejected value.
        value: f64,
    },
}

/// Convenient result alias.
pub type Result<T> = core::result::Result<T, Error>;
