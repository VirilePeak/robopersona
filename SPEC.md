# The `.botpack` Package Format — Specification

**Version:** 0.1.0 (DRAFT — tracks the Rust reference implementation in `crates/`)
**Status:** Implemented. Every normative statement below is enforced by and
tested against the reference implementation.

The key words MUST, MUST NOT, SHALL, and MAY are to be interpreted as
described in RFC 2119.

---

## 1. Overview

A `.botpack` file is a behavior package for embodied AI personas. It bundles:

- a **manifest** (identity + persona description),
- arbitrary **payload files** (prompts, configs, assets),
- a **checksum index** (integrity).

The container is a **zstd-compressed tar archive**. All JSON in this spec is
UTF-8, pretty or compact — readers MUST accept both.

## 2. Container layout

Entry order is normative:

| Order | Entry | Content |
|---|---|---|
| 1 (MUST be first) | `manifest.json` | Manifest object (§3) |
| middle | payload entries | Arbitrary files, paths per §5 |
| last (MUST be last) | `checksums.json` | Checksum index (§4) |

Readers MUST reject archives where the first entry is not `manifest.json`
or the last entry is not `checksums.json`. Writers MUST emit them in this
order.

## 3. Manifest schema (`manifest.json`)

```json
{
  "format_version": "0.1.0",
  "name": "atlas-caretaker",
  "version": "0.1.0",
  "persona": {
    "display_name": "Atlas Caretaker",
    "description": "Warehouse patrol persona.",
    "system_prompt": "You are a careful warehouse robot.",
    "languages": ["en"]
  }
}
```

### 3.1 Fields

- `format_version` — semver string. Readers MUST reject a **different major
  version**. Minor/patch differences within the same major MUST be accepted.
- `name` — package name, rules in §3.2.
- `version` — semver string of the package itself.
- `persona.display_name` — non-empty after trimming; readers MUST reject
  blank values.
- `persona.description`, `persona.system_prompt` — optional strings,
  default `""`.
- `persona.languages` — optional array of ISO 639-1 codes, default `[]`.

### 3.2 Package name rules

A valid name:

1. is 2–63 bytes long,
2. contains only `a–z`, `0–9`, `-`,
3. starts with a lowercase letter,
4. does not end with `-`,
5. contains no consecutive `-`.

### 3.3 Forward compatibility

Readers MUST ignore unknown fields (top-level and nested). Writers MUST NOT
fail on round-tripping manifests that carry unknown fields.

## 4. Integrity (`checksums.json`)

```json
{
  "files": {
    "prompts/greeting.txt": "2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae"
  }
}
```

- Keys: payload entry paths. Values: lowercase hex SHA-256 of the entry's
  **uncompressed** content.
- Readers MUST recompute every digest and MUST abort on any mismatch.
- Readers MUST reject an archive where the checksum index and the payload
  entry set do not match **exactly** (missing or extra digests are errors).
- Writers MUST include every payload entry exactly once.

## 5. Path safety

Payload entry paths MUST:

1. be relative (no leading `/`),
2. contain no `..` components,
3. be valid UTF-8,
4. not collide with the reserved names `manifest.json` / `checksums.json`.

Readers MUST reject violations. (Duplicate structural entries inside the
payload region are likewise rejected.)

## 6. Affect engine (normative mathematics)

The affect state is a PAD vector `(v, a, d)` — valence, arousal, dominance —
each axis clamped to `[-1, 1]`. Non-finite axis inputs are mapped: `NaN → 0`,
`±∞ → ±1`.

### 6.1 Configuration

| Field | Constraint |
|---|---|
| `baseline` | PAD vector (resting state) |
| `decay_rate` λ | finite, `> 0` |
| `mood_tau` τ | finite, `> 0` (reserved for baseline adaptation) |
| `max_step` | finite, in `(0, 2]` |

Invalid configurations MUST be rejected at construction; a constructed
engine can never hold an invalid state.

### 6.2 Stimulus application

A stimulus moves each axis toward the stimulus point by at most `max_step`:

```text
s' = s + clamp(|target − s|, 0, max_step) · sign(target − s)
```

### 6.3 Decay

Time advancement by `dt ≥ 0` (negative or non-finite `dt` MUST be rejected):

```text
s' = baseline + (s − baseline) · exp(−λ·dt)
```

`dt = 0` MUST be an exact identity.

## 7. Determinism contract

1. **Cross-runtime bit-identity:** For identical operation sequences
   (same config, same stimuli, same tick durations), conforming
   implementations MUST produce bit-identical IEEE-754 doubles on all
   platforms. This is achieved by a fixed-budget `exp(−x)` implementation
   (range reduction + 13 Taylor terms + exponent-field scaling) and
   pure-Rust `sqrt`/`round`/`floor` (libm), never platform libm.
   *Verified:* Rust (ARM64 macOS) ≡ Python (pyo3 extension) ≡ JavaScript
   (wasm-bindgen, Node) — byte-equal float64s.
2. **Replay:** Identical event sequences replayed with identical `advance`
   chunking are bit-identical.
3. **Chunking tolerance:** Different `advance` chunkings of the same event
   schedule agree to `1e-12` absolute tolerance (exponential factorization
   is not bitwise associative). Implementations MUST document this bound
   rather than claim unconditional bit-identity.

## 8. Runtime loop

A conforming runtime loop:

1. fires each scheduled stimulus at its **exact timestamp** (decay up to
   `at`, apply stimulus, decay onward) — the fire point MUST NOT depend on
   how the caller chunks time advancement;
2. integrates decay in fixed sub-ticks on an **absolute grid**
   (`k·tick_len` measured from loop start);
3. enforces a **monotone clock**: events scheduled in the past (earlier
   than current loop time) MUST be rejected;
4. applies simultaneous events (equal timestamps) in stable insertion
   order.

## 9. Versioning policy

- The format major version gates breaking changes; readers reject foreign
  majors instead of guessing.
- Unknown-field tolerance (§3.3) is the only forward-compatibility mechanism
  within a major version; new fields MUST be optional with defaults.

---

## Reference implementations

| Target | Crate / Artifact | Conformance |
|---|---|---|
| Rust (ARM64/embedded) | `botpack-core`, `botpack-archive`, `botpack-affect` | 31 tests |
| WebAssembly | `botpack-wasm` → `botpack_wasm.wasm` | Node functional test |
| Python | `botpack-python` → `botpack` module | Functional test + bit-identity |