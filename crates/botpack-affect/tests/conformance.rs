//! Conformance test against `conformance/affect-reference.json` (SPEC §7).
//!
//! The fixture's `*_bits` fields are the ONLY normative values: every input
//! is reconstructed via `f64::from_bits`, every output is compared via
//! `f64::to_bits`. Decimal renderings in the fixture are non-normative and
//! deliberately never parsed.

use std::fs;

use botpack_affect::{AffectConfig, AffectEngine, AffectVector, Seconds};

fn bits(hex: &str) -> f64 {
    assert_eq!(hex.len(), 16, "bit pattern must be 16 hex chars: {hex}");
    f64::from_bits(u64::from_str_radix(hex, 16).expect("valid hex"))
}

#[derive(serde::Deserialize)]
struct Fixture {
    scenario: Scenario,
    expected_state: ExpectedState,
}

#[derive(serde::Deserialize)]
struct Scenario {
    config: Config,
    stimulus: Stimulus,
    tick: Tick,
}

#[derive(serde::Deserialize)]
struct Config {
    baseline: Baseline,
    #[serde(rename = "decay_rate_bits")]
    decay_rate: String,
    #[serde(rename = "mood_tau_bits")]
    mood_tau: String,
    #[serde(rename = "max_step_bits")]
    max_step: String,
}

#[derive(serde::Deserialize)]
struct Baseline {
    valence_bits: String,
    arousal_bits: String,
    dominance_bits: String,
}

#[derive(serde::Deserialize)]
struct Stimulus {
    valence_bits: String,
    arousal_bits: String,
    dominance_bits: String,
}

#[derive(serde::Deserialize)]
struct Tick {
    #[serde(rename = "dt_bits")]
    dt: String,
    count: u32,
}

#[derive(serde::Deserialize)]
struct ExpectedState {
    valence_bits: String,
    arousal_bits: String,
    dominance_bits: String,
}

#[test]
fn conformance_affect_reference() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/affect-reference.json"
    );
    let raw = fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let fx: Fixture = serde_json::from_str(&raw).expect("valid fixture JSON");

    let config = AffectConfig {
        baseline: AffectVector::new(
            bits(&fx.scenario.config.baseline.valence_bits),
            bits(&fx.scenario.config.baseline.arousal_bits),
            bits(&fx.scenario.config.baseline.dominance_bits),
        ),
        decay_rate: bits(&fx.scenario.config.decay_rate),
        mood_tau: bits(&fx.scenario.config.mood_tau),
        max_step: bits(&fx.scenario.config.max_step),
    };
    let dt = Seconds::new(bits(&fx.scenario.tick.dt)).unwrap();

    let mut e = AffectEngine::new(config).unwrap();
    e.apply_stimulus(AffectVector::new(
        bits(&fx.scenario.stimulus.valence_bits),
        bits(&fx.scenario.stimulus.arousal_bits),
        bits(&fx.scenario.stimulus.dominance_bits),
    ));
    for _ in 0..fx.scenario.tick.count {
        e.tick(dt);
    }
    let s = e.state();

    assert_eq!(
        format!("{:016x}", s.valence.to_bits()),
        fx.expected_state.valence_bits
    );
    assert_eq!(
        format!("{:016x}", s.arousal.to_bits()),
        fx.expected_state.arousal_bits
    );
    assert_eq!(
        format!("{:016x}", s.dominance.to_bits()),
        fx.expected_state.dominance_bits
    );
}
