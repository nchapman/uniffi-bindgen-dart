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

if [[ -n "${UBDG_RECORD_ENUM_METHODS_LIB:-}" ]]; then
  cargo run -p ubdg_bindgen --bin uniffi-bindgen-dart -- \
    generate "${UBDG_RECORD_ENUM_METHODS_LIB}" \
    --crate "uniffi_record_enum_methods" \
    --out-dir "$OUT_DIR"
fi

if [[ -n "${UBDG_LIBRARY_MODE_DEMO_LIB:-}" ]]; then
  cargo run -p ubdg_bindgen --bin uniffi-bindgen-dart -- \
    generate "${UBDG_LIBRARY_MODE_DEMO_LIB}" \
    --crate "uniffi_library_mode_demo" \
    --out-dir "$OUT_DIR"
fi

# Generate bindings for additional fixtures (UDL-based)
for fixture in compound-demo coverall-demo keywords-demo; do
  FIXTURE_UDL="$ROOT_DIR/fixtures/$fixture/src/$fixture.udl"
  if [[ -f "$FIXTURE_UDL" ]]; then
    FIXTURE_CONFIG="$ROOT_DIR/fixtures/$fixture/uniffi.toml"
    if [[ -f "$FIXTURE_CONFIG" ]]; then
      cargo run -p ubdg_bindgen --bin uniffi-bindgen-dart -- \
        generate "$FIXTURE_UDL" \
        --out-dir "$OUT_DIR" \
        --config "$FIXTURE_CONFIG"
    else
      cargo run -p ubdg_bindgen --bin uniffi-bindgen-dart -- \
        generate "$FIXTURE_UDL" \
        --out-dir "$OUT_DIR"
    fi
  fi
done

echo "Generated bindings in $OUT_DIR"
