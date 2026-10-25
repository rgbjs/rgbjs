#!/bin/sh
# Builds wasm/v011 and wasm/v012 from rust/. Needs cargo with the
# wasm32-unknown-unknown target and wasm-bindgen-cli matching each Cargo.lock.
# CARGO overrides the cargo command (e.g. a sandboxed wrapper).
set -eu
cd "$(dirname "$0")/.."
CARGO=${CARGO:-cargo}
TARGET_DIR=${CARGO_TARGET_DIR:-target}
for line in v011 v012; do
  $CARGO build --release --locked --target wasm32-unknown-unknown --manifest-path "rust/$line/Cargo.toml"
  rm -rf "wasm/$line"
  wasm-bindgen --target web --no-typescript --out-dir "wasm/$line" \
    "$TARGET_DIR/wasm32-unknown-unknown/release/rgbjs_$line.wasm"
done
ls -l wasm/*/*.wasm
