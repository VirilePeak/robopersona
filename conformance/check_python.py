#!/usr/bin/env python3
"""Conformance check against conformance/affect-reference.json (SPEC §7).

Normative values are the ``*_bits`` hex strings: each is the exact
``f64::to_bits()`` pattern printed as 16 lowercase hex digits (the integer
value, not little-endian memory order). Inputs are reconstructed from those
bits; decimal renderings in the fixture are never parsed.
"""

import json
import struct
import sys
from pathlib import Path

import botpack

ROOT = Path(__file__).resolve().parent
FIXTURE = ROOT / "affect-reference.json"


def f64_from_bits(hexbits: str) -> float:
    if len(hexbits) != 16 or any(c not in "0123456789abcdef" for c in hexbits):
        raise SystemExit(f"bit pattern must be 16 lowercase hex chars: {hexbits}")
    # Hex is the u64 bit pattern in digit order == big-endian byte order.
    return struct.unpack(">d", bytes.fromhex(hexbits))[0]


def to_bits(value: float) -> str:
    return struct.pack(">d", value).hex()


def main() -> None:
    fx = json.loads(FIXTURE.read_text())
    cfg = fx["scenario"]["config"]
    base = cfg["baseline"]
    stim = fx["scenario"]["stimulus"]
    tick = fx["scenario"]["tick"]
    expected = fx["expected_state"]

    engine = botpack.AffectEngine(
        botpack.AffectConfig(
            botpack.AffectVector(
                f64_from_bits(base["valence_bits"]),
                f64_from_bits(base["arousal_bits"]),
                f64_from_bits(base["dominance_bits"]),
            ),
            f64_from_bits(cfg["decay_rate_bits"]),
            f64_from_bits(cfg["mood_tau_bits"]),
            f64_from_bits(cfg["max_step_bits"]),
        )
    )
    engine.apply_stimulus(
        botpack.AffectVector(
            f64_from_bits(stim["valence_bits"]),
            f64_from_bits(stim["arousal_bits"]),
            f64_from_bits(stim["dominance_bits"]),
        )
    )
    dt = f64_from_bits(tick["dt_bits"])
    for _ in range(tick["count"]):
        engine.tick(dt)

    got = tuple(to_bits(x) for x in engine.state().as_tuple())
    want = (
        expected["valence_bits"],
        expected["arousal_bits"],
        expected["dominance_bits"],
    )
    if got != want:
        print(f"FAIL got {got} want {want}", file=sys.stderr)
        sys.exit(1)
    print("PYTHON CONFORMANCE OK:", got)


if __name__ == "__main__":
    main()
