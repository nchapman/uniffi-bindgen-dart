#!/usr/bin/env bash
set -euo pipefail

cargo test --workspace
./scripts/build_bindings.sh
FIXTURE_LIB="$(./scripts/build_fixture.sh)"

resolve_dart_bin() {
  if [[ -n "${DART_BIN:-}" ]] && [[ -x "${DART_BIN}" ]]; then
    printf '%s\n' "${DART_BIN}"
    return 0
  fi

  if command -v dart >/dev/null 2>&1; then
    command -v dart
    return 0
  fi

  if command -v mise >/dev/null 2>&1; then
    local mise_dart
    mise_dart="$(mise which dart 2>/dev/null || true)"
    if [[ -n "${mise_dart}" ]] && [[ -x "${mise_dart}" ]]; then
      printf '%s\n' "${mise_dart}"
      return 0
    fi
  fi

  return 1
}

if DART_CMD="$(resolve_dart_bin)"; then
  (
    cd binding_tests
    "$DART_CMD" pub get
    "$DART_CMD" analyze
    UBDG_SIMPLE_FNS_LIB="$FIXTURE_LIB" "$DART_CMD" test
  )
else
  echo "dart not found; skipping host binding tests (set DART_BIN to override)"
fi
