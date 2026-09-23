// Functional + determinism test for botpack_wasm (Node target).
// The cross-runtime oracle is conformance/affect-reference.json: *_bits
// fields are the f64::to_bits() patterns (16 lowercase hex digits, integer
// value / big-endian digit order). Decimal renderings are not parsed.
const fs = require('fs');
const path = require('path');
const wasm = require('./pkg/botpack_wasm.js');
const fx = JSON.parse(fs.readFileSync(
  path.join(__dirname, '../../../../conformance/affect-reference.json'), 'utf8'));

// Hex is the u64 bit pattern in digit order, matching Rust `{:016x}` of to_bits().
function fromBits(hex) {
  if (!/^[0-9a-f]{16}$/.test(hex)) throw new Error('bad bit pattern: ' + hex);
  const buf = new ArrayBuffer(8);
  const view = new DataView(buf);
  view.setUint32(0, parseInt(hex.slice(0, 8), 16), false);
  view.setUint32(4, parseInt(hex.slice(8, 16), 16), false);
  return view.getFloat64(0, false);
}
function toBits(x) {
  const buf = new ArrayBuffer(8);
  const view = new DataView(buf);
  view.setFloat64(0, x, false);
  return view.getUint32(0, false).toString(16).padStart(8, '0')
    + view.getUint32(4, false).toString(16).padStart(8, '0');
}

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

// 3. Determinism run — inputs reconstructed from fixture bit patterns.
const sc = fx.scenario;
const run = () => {
  const e = new wasm.JsAffectEngine({
    baseline: {
      valence: fromBits(sc.config.baseline.valence_bits),
      arousal: fromBits(sc.config.baseline.arousal_bits),
      dominance: fromBits(sc.config.baseline.dominance_bits),
    },
    decay_rate: fromBits(sc.config.decay_rate_bits),
    mood_tau: fromBits(sc.config.mood_tau_bits),
    max_step: fromBits(sc.config.max_step_bits),
  });
  e.applyStimulus({
    valence: fromBits(sc.stimulus.valence_bits),
    arousal: fromBits(sc.stimulus.arousal_bits),
    dominance: fromBits(sc.stimulus.dominance_bits),
  });
  const dt = fromBits(sc.tick.dt_bits);
  for (let i = 0; i < sc.tick.count; i++) e.tick(dt);
  return e.getState();
};
const a = run(), b = run();
check('two runs bit-identical', toBits(a.valence) === toBits(b.valence)
  && toBits(a.arousal) === toBits(b.arousal) && toBits(a.dominance) === toBits(b.dominance));

const exp = fx.expected_state;
check('matches conformance fixture bits',
  toBits(a.valence) === exp.valence_bits
  && toBits(a.arousal) === exp.arousal_bits
  && toBits(a.dominance) === exp.dominance_bits,
  `got ${toBits(a.valence)} ${toBits(a.arousal)} ${toBits(a.dominance)}`);

// 4. Elapsed bookkeeping
const e2 = new wasm.JsAffectEngine(cfg);
e2.tick(1.5); e2.tick(2.5);
check('elapsed = 4.0', e2.getElapsed() === 4.0, String(e2.getElapsed()));

if (failures > 0) { console.error(`NODE WASM TEST: ${failures} FAILURES`); process.exit(1); }
console.log('NODE WASM TEST: ALL CHECKS PASSED');
console.log('final state:', JSON.stringify(a));