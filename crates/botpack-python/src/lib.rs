//! Python bindings for the deterministic affect engine.
//!
//! Exposed as the `botpack` module: `AffectEngine`, `AffectVector`,
//! `AffectConfig` — same bit-identical state machine as the wasm target.

#![deny(unsafe_code)]

use botpack_affect::{AffectConfig, AffectEngine, AffectVector, Seconds};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// PAD affect vector, each axis clamped to [-1, 1] (NaN → 0).
#[pyclass(name = "AffectVector")]
#[derive(Clone)]
struct PyAffectVector {
    inner: AffectVector,
}

#[pymethods]
impl PyAffectVector {
    #[new]
    fn new(valence: f64, arousal: f64, dominance: f64) -> Self {
        Self {
            inner: AffectVector::new(valence, arousal, dominance),
        }
    }

    /// (valence, arousal, dominance) tuple.
    fn as_tuple(&self) -> (f64, f64, f64) {
        (self.inner.valence, self.inner.arousal, self.inner.dominance)
    }

    fn __repr__(&self) -> String {
        let (v, a, d) = self.as_tuple();
        format!("AffectVector(valence={v}, arousal={a}, dominance={d})")
    }
}

/// Engine configuration; validated on construction.
#[pyclass(name = "AffectConfig")]
#[derive(Clone)]
struct PyAffectConfig {
    inner: AffectConfig,
}

#[pymethods]
impl PyAffectConfig {
    #[new]
    #[pyo3(signature = (baseline, decay_rate, mood_tau, max_step))]
    fn new(
        baseline: &PyAffectVector,
        decay_rate: f64,
        mood_tau: f64,
        max_step: f64,
    ) -> PyResult<Self> {
        let config = AffectConfig {
            baseline: baseline.inner,
            decay_rate,
            mood_tau,
            max_step,
        };
        config
            .validate()
            .map(|()| Self { inner: config })
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }
}

/// Deterministic affect state machine.
#[pyclass(name = "AffectEngine")]
struct PyAffectEngine {
    inner: AffectEngine,
}

#[pymethods]
impl PyAffectEngine {
    #[new]
    fn new(config: PyAffectConfig) -> Self {
        Self {
            inner: AffectEngine::new(config.inner).expect("config pre-validated"),
        }
    }

    /// Current state as AffectVector.
    fn state(&self) -> PyAffectVector {
        PyAffectVector {
            inner: self.inner.state(),
        }
    }

    /// Applies an instantaneous stimulus (limited by max_step per axis).
    fn apply_stimulus(&mut self, stimulus: &PyAffectVector) {
        self.inner.apply_stimulus(stimulus.inner);
    }

    /// Advances time by dt seconds (finite, >= 0).
    fn tick(&mut self, dt: f64) -> PyResult<()> {
        let dt = Seconds::new(dt).map_err(|e| PyValueError::new_err(e.to_string()))?;
        self.inner.tick(dt);
        Ok(())
    }

    /// Total simulated seconds.
    fn elapsed(&self) -> f64 {
        self.inner.elapsed()
    }
}

/// Python module entry point.
#[pymodule]
fn botpack(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAffectVector>()?;
    m.add_class::<PyAffectConfig>()?;
    m.add_class::<PyAffectEngine>()?;
    Ok(())
}
