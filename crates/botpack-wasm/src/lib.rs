//! botpack-wasm — browser/wasm bindings for the deterministic affect engine.
//!
//! Zero-latency execution target: the same bit-identical state machine that
//! runs on ARM64/Jetson runs here, exposed as a JS-friendly class.

#![deny(unsafe_code)]

use botpack_affect::{AffectConfig, AffectEngine, AffectVector, Seconds};
use wasm_bindgen::prelude::*;

/// JS-facing affect engine.
#[wasm_bindgen]
pub struct JsAffectEngine {
    inner: AffectEngine,
}

#[wasm_bindgen]
impl JsAffectEngine {
    /// Creates an engine from a plain JS object
    /// `{ baseline: {valence, arousal, dominance}, decay_rate, mood_tau, max_step }`.
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<JsAffectEngine, JsValue> {
        let config: AffectConfig = serde_wasm_bindgen::from_value(config)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        AffectEngine::new(config)
            .map(|inner| Self { inner })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Current state as `{valence, arousal, dominance}`.
    #[wasm_bindgen(js_name = getState)]
    pub fn state(&self) -> JsValue {
        let s = self.inner.state();
        serde_wasm_bindgen::to_value(&s).expect("AffectVector serializes cleanly")
    }

    /// Applies a stimulus `{valence, arousal, dominance}`.
    #[wasm_bindgen(js_name = applyStimulus)]
    pub fn apply_stimulus(&mut self, stimulus: JsValue) -> Result<(), JsValue> {
        let v: AffectVector = serde_wasm_bindgen::from_value(stimulus)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.inner.apply_stimulus(v);
        Ok(())
    }

    /// Advances time by `dt` seconds (must be finite, >= 0).
    pub fn tick(&mut self, dt: f64) -> Result<(), JsValue> {
        let dt = Seconds::new(dt).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.inner.tick(dt);
        Ok(())
    }

    /// Total simulated seconds.
    #[wasm_bindgen(js_name = getElapsed)]
    pub fn elapsed(&self) -> f64 {
        self.inner.elapsed()
    }
}
