#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_FILE="$ROOT_DIR/fixtures/simple-fns/src/simple-fns.udl"
OUT_DIR="$ROOT_DIR/binding_tests/generated"
CONFIG_FILE="$ROOT_DIR/fixtures/simple-fns/uniffi.toml"

mkdir -p "$OUT_DIR"

cargo run -p ubdg_bindgen --bin uniffi-bindgen-dart -- \
  generate "$SOURCE_FILE" \
  --out-dir "$OUT_DIR" \
  --config "$CONFIG_FILE"

echo "Generated bindings in $OUT_DIR"
