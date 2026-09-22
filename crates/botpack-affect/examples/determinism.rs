//! Cross-runtime determinism check: must match the Python run bit-for-bit.
use botpack_affect::{AffectConfig, AffectEngine, AffectVector, Seconds};

fn main() {
    let mut e = AffectEngine::new(AffectConfig {
        baseline: AffectVector::new(0.1, 0.2, 0.0),
        decay_rate: 0.5,
        mood_tau: 60.0,
        max_step: 0.3,
    })
    .unwrap();
    e.apply_stimulus(AffectVector::new(0.9, -0.3, 0.6));
    for _ in 0..10 {
        e.tick(Seconds::new(1.7).unwrap());
    }
    let s = e.state();
    println!("({:?}, {:?}, {:?})", s.valence, s.arousal, s.dominance);
}
