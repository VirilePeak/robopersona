// In-browser conformance check: runs the reference scenario from
// conformance/affect-reference.json with the released wasm build and compares
// the resulting f64 bit patterns with the fixture. Progressive enhancement:
// without JS or wasm the box stays hidden and the page is unaffected.
import init, { JsAffectEngine } from './wasm/botpack_wasm_opt.js';

const AXES = ['valence', 'arousal', 'dominance'];
const box = document.getElementById('demo');
const out = document.getElementById('demo-result');

// Hex is the u64 bit pattern in digit order (Rust `{:016x}` of to_bits()),
// so DataView is used big-endian.
function fromBits(hex) {
  const view = new DataView(new ArrayBuffer(8));
  view.setBigUint64(0, BigInt('0x' + hex));
  return view.getFloat64(0);
}

function toBits(x) {
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, x);
  return view.getBigUint64(0).toString(16).padStart(16, '0');
}

const vector = (o) => Object.fromEntries(AXES.map((k) => [k, fromBits(o[`${k}_bits`])]));

async function loadFixture() {
  const res = await fetch('conformance/affect-reference.json');
  if (!res.ok) throw new Error(`fixture: HTTP ${res.status}`);
  return res.json();
}

async function main() {
  box.hidden = false;
  try {
    const [fixture] = await Promise.all([loadFixture(), init()]);
    const sc = fixture.scenario;
    const engine = new JsAffectEngine({
      baseline: vector(sc.config.baseline),
      decay_rate: fromBits(sc.config.decay_rate_bits),
      mood_tau: fromBits(sc.config.mood_tau_bits),
      max_step: fromBits(sc.config.max_step_bits),
    });
    engine.applyStimulus(vector(sc.stimulus));
    const dt = fromBits(sc.tick.dt_bits);
    for (let i = 0; i < sc.tick.count; i++) engine.tick(dt);
    const state = engine.getState();
    const got = AXES.map((k) => toBits(state[k]));
    engine.free(); // after the read: never touch engine-derived values post-free
    const want = AXES.map((k) => fixture.expected_state[`${k}_bits`]);
    const ok = got.every((g, i) => g === want[i]);
    const rows = AXES.map((k, i) =>
      `${k.padEnd(10)} ${got[i]}  ${got[i] === want[i] ? '✓' : `✗ expected ${want[i]}`}`);
    rows.push('', ok
      ? '✓ bit-identical to the fixture'
      : '✗ differs from the fixture: please open an issue with your browser version');
    out.textContent = rows.join('\n');
    box.dataset.status = ok ? 'ok' : 'mismatch';
  } catch (err) {
    out.textContent = `could not run the check: ${err}`;
    box.dataset.status = 'error';
  }
}

main();
