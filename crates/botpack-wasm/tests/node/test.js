// Functional + determinism test for botpack_wasm (Node target).
// Must match the Rust/Python reference run bit-for-bit:
//   (0.1000610405107032, 0.19993895948929682, 6.104051070319337e-5)
const wasm = require('./pkg/botpack_wasm.js');

const bits = (x) => {
  const f = new Float64Array(1); f[0] = x;
  return [...new Uint8Array(f.buffer)].map(b => b.toString(16).padStart(2, '0')).join('');
};

let failures = 0;
const check = (name, cond, detail) => {
  if (cond) { console.log(`  ok: ${name}`); }
  else { failures++; console.error(`  FAIL: ${name} ${detail ?? ''}`); }
};

// 1. Construction + clamping (NaN -> 0, out-of-range -> clamp)
const cfg = {
  baseline: { valence: 0.1, arousal: 0.2, dominance: 0.0 },
  decay_rate: 0.5, mood_tau: 60.0, max_step: 0.3,
};
const eng = new wasm.JsAffectEngine(cfg);
check('engine constructed', eng instanceof wasm.JsAffectEngine);

eng.applyStimulus({ valence: 0.5, arousal: 2.0, dominance: NaN });
const clamped = eng.getState();
// Stimulus (0.5, 2.0, NaN) clamps to (0.5, 1.0, 0.0); engine steps from
// baseline (0.1, 0.2, 0.0) by max_step=0.3 per axis: (0.4, 0.5, 0.0).
// NaN->0 semantics visible: dominance stays at baseline (stimulus 0).
check('clamping + step limit + NaN->0',
  clamped.valence === 0.4 && clamped.arousal === 0.5 && clamped.dominance === 0.0,
  JSON.stringify(clamped));

// 2. Invalid dt rejected
let rejected = false;
try { eng.tick(-1.0); } catch (e) { rejected = true; }
check('negative dt rejected', rejected);

// 3. Determinism run — same scenario as Rust/Python
const run = () => {
  const e = new wasm.JsAffectEngine({
    baseline: { valence: 0.1, arousal: 0.2, dominance: 0.0 },
    decay_rate: 0.5, mood_tau: 60.0, max_step: 0.3,
  });
  e.applyStimulus({ valence: 0.9, arousal: -0.3, dominance: 0.6 });
  for (let i = 0; i < 10; i++) e.tick(1.7);
  return e.getState();
};
const a = run(), b = run();
check('two runs bit-identical', bits(a.valence) === bits(b.valence)
  && bits(a.arousal) === bits(b.arousal) && bits(a.dominance) === bits(b.dominance));

const expected = { valence: 0.1000610405107032, arousal: 0.19993895948929682, dominance: 6.104051070319337e-5 };
check('matches Rust/Python reference bits',
  bits(a.valence) === bits(expected.valence)
  && bits(a.arousal) === bits(expected.arousal)
  && bits(a.dominance) === bits(expected.dominance),
  `got (${a.valence}, ${a.arousal}, ${a.dominance})`);

// 4. Elapsed bookkeeping
const e2 = new wasm.JsAffectEngine(cfg);
e2.tick(1.5); e2.tick(2.5);
check('elapsed = 4.0', e2.getElapsed() === 4.0, String(e2.getElapsed()));

if (failures > 0) { console.error(`NODE WASM TEST: ${failures} FAILURES`); process.exit(1); }
console.log('NODE WASM TEST: ALL CHECKS PASSED');
console.log('final state:', JSON.stringify(a));