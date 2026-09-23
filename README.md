# RoboPersona — the `.botpack` standard

An open, cross-platform standard for embodied AI behavior packages.
A `.botpack` file bundles a persona manifest, payload files, and a
SHA-256 integrity index into one verifiable archive — with a
**deterministic affect engine** whose outputs are bit-identical across
Rust, Python, and WebAssembly.

## Why

Existing formats are either infrastructure (ROS2 packages) or LLM-only
(character cards) — none carry verified state, safety bounds, and
reproducibility. `.botpack` does:

- **Determinism contract:** identical operation sequences produce
  byte-identical IEEE-754 doubles on every platform. Verified:
  Rust (ARM64) ≡ Python (pyo3) ≡ JavaScript (wasm-bindgen/Node).
- **Safety by construction:** invalid states are unrepresentable —
  validated configs, clamped PAD vectors, rejected negative durations.
- **Integrity:** every payload entry is digest-verified on read;
  corruption aborts with a precise error.

## Quickstart (CLI)

```console
$ cargo install --path crates/botpack-cli   # after publish: cargo install botpack
$ botpack pack ./my-persona -o my-persona.botpack
$ botpack verify my-persona.botpack
OK atlas-caretaker v0.1.0 (format 0.1.0) — 2 payload entries
$ botpack unpack my-persona.botpack -o ./extracted
$ botpack show my-persona.botpack
```

## Crates

| Crate | Purpose |
|---|---|
| `botpack-core` | Manifest schema, validation, JSON round-trip |
| `botpack-archive` | `.botpack` container: tar+zstd, SHA-256 index |
| `botpack-affect` | `no_std` deterministic affect engine (PAD model, ODE decay) |
| `botpack-wasm` | wasm-bindgen bindings for the affect engine |
| `botpack-python` | pyo3 bindings (`botpack` module) |
| `botpack` | `botpack` binary: pack / unpack / verify / show (crate dir: `crates/botpack-cli`) |

## Determinism example

The same scenario — baseline `(0.1, 0.2, 0.0)`, λ=0.5, one stimulus,
ten 1.7 s ticks — produces identical bits in all three runtimes.
Normative values are the `*_bits` fields in
[`conformance/affect-reference.json`](conformance/affect-reference.json)
(`f64::to_bits()` as 16 lowercase hex digits). The decimals below are a
non-normative rendering only — `1.7` is not exactly representable:

```text
3fb99d99b098e231  3fc997998e19f54f  3f10005bfd225a03
(0.1000610405107032, 0.19993895948929682, 6.104051070319337e-5)
```

This is achieved by a fixed-budget `exp(−x)` implementation (range
reduction + 13 Taylor terms + exponent-field scaling) and pure-Rust
`sqrt`/`round`/`floor` — never platform libm.

## Specification

See [SPEC.md](SPEC.md) — the normative format document. Every rule in it
is enforced by and tested against this reference implementation.

## Status

Crate 0.1.1 — format spec still 0.1.0 (draft), reference implementation,
34 tests green, clippy-clean, wasm32 + Python + Node verified against
`conformance/affect-reference.json`. Crates are
not published to crates.io yet.

## License

MIT OR Apache-2.0