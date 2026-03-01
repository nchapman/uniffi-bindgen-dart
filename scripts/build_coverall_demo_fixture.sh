#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_MANIFEST="$ROOT_DIR/fixtures/coverall-demo/native-lib/Cargo.toml"
TARGET_DIR="$ROOT_DIR/target/coverall-demo-native"
OUT_DIR="$ROOT_DIR/binding_tests/native"

cargo build --release --manifest-path "$FIXTURE_MANIFEST" --target-dir "$TARGET_DIR"

case "$(uname -s)" in
  Darwin)
    LIB_FILE="libuniffi_coverall_demo.dylib"
    ;;
  Linux)
    LIB_FILE="libuniffi_coverall_demo.so"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    LIB_FILE="uniffi_coverall_demo.dll"
    ;;
  *)
    echo "unsupported platform: $(uname -s)" >&2
    exit 1
    ;;
esac

SRC_LIB="$TARGET_DIR/release/$LIB_FILE"
if [[ ! -f "$SRC_LIB" ]]; then
  echo "expected fixture library at $SRC_LIB" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
cp "$SRC_LIB" "$OUT_DIR/$LIB_FILE"

echo "$OUT_DIR/$LIB_FILE"
