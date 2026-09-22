#!/bin/sh
# Builds the optimized wasm artifact for browser distribution.
# Usage: ./scripts/build-wasm.sh [out-dir]   (default: dist/wasm)
set -e
cd "$(dirname "$0")/.."
OUT="${1:-dist/wasm}"

cargo build -p botpack-wasm --target wasm32-unknown-unknown --release
wasm-opt -Oz --enable-bulk-memory \
    target/wasm32-unknown-unknown/release/botpack_wasm.wasm \
    -o /tmp/botpack_wasm_opt.wasm
mkdir -p "$OUT"
wasm-bindgen --target web --out-dir "$OUT" /tmp/botpack_wasm_opt.wasm

echo "wasm artifact in $OUT:"
ls -la "$OUT" | awk 'NR>1 {print "  " $NF " " $5 "B"}'