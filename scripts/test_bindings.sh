#!/usr/bin/env bash
set -euo pipefail

cargo test --workspace
FIXTURE_LIB="$(./scripts/build_fixture.sh)"
RECORD_ENUM_METHODS_LIB="$(./scripts/build_record_enum_methods_fixture.sh)"
COMPOUND_DEMO_LIB="$(./scripts/build_compound_demo_fixture.sh)"
COVERALL_DEMO_LIB="$(./scripts/build_coverall_demo_fixture.sh)"
KEYWORDS_DEMO_LIB="$(./scripts/build_keywords_demo_fixture.sh)"
LIBRARY_MODE_DEMO_LIB="$(./scripts/build_library_mode_demo_fixture.sh)"
UBDG_RECORD_ENUM_METHODS_LIB="$RECORD_ENUM_METHODS_LIB" \
  UBDG_LIBRARY_MODE_DEMO_LIB="$LIBRARY_MODE_DEMO_LIB" \
  ./scripts/build_bindings.sh

# Re-run workspace tests with library paths set for library-mode golden tests
UBDG_RECORD_ENUM_METHODS_LIB="$RECORD_ENUM_METHODS_LIB" \
  UBDG_LIBRARY_MODE_DEMO_LIB="$LIBRARY_MODE_DEMO_LIB" \
  cargo test --workspace

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
    UBDG_SIMPLE_FNS_LIB="$FIXTURE_LIB" \
      UBDG_RECORD_ENUM_METHODS_LIB="$RECORD_ENUM_METHODS_LIB" \
      UBDG_COMPOUND_DEMO_LIB="$COMPOUND_DEMO_LIB" \
      UBDG_COVERALL_DEMO_LIB="$COVERALL_DEMO_LIB" \
      UBDG_KEYWORDS_DEMO_LIB="$KEYWORDS_DEMO_LIB" \
      UBDG_LIBRARY_MODE_DEMO_LIB="$LIBRARY_MODE_DEMO_LIB" \
      "$DART_CMD" test
  )
else
  echo "dart not found; skipping host binding tests (set DART_BIN to override)"
fi
