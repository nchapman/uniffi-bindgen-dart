#!/usr/bin/env bash
set -euo pipefail

cargo test --workspace

if command -v dart >/dev/null 2>&1; then
  (
    cd binding_tests
    dart pub get
    dart test
  )
else
  echo "dart not found; skipping host binding tests"
fi
