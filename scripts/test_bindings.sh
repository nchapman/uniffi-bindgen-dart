#!/usr/bin/env bash
set -euo pipefail

cargo test --workspace
./scripts/build_bindings.sh
FIXTURE_LIB="$(./scripts/build_fixture.sh)"

if command -v dart >/dev/null 2>&1; then
  (
    cd binding_tests
    dart pub get
    dart analyze
    UBDG_SIMPLE_FNS_LIB="$FIXTURE_LIB" dart test
  )
else
  echo "dart not found; skipping host binding tests"
fi
