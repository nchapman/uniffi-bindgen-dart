set shell := ["bash", "-cu"]

# Show all available recipes.
default:
  @just --list

# ---------- Quick checks ----------

# Run full CI check (format, lint, test).
check: fmt-check lint test

# Check formatting without changing files.
fmt-check:
  cargo fmt --check

# Run clippy across all targets.
lint:
  cargo clippy --all-targets -- -D warnings

# Run workspace tests (unit + golden).
test *args:
  cargo test --workspace {{ args }}

# ---------- Formatting ----------

# Format all Rust code.
fmt:
  cargo fmt --all

# Fix everything auto-fixable, then check what's left.
fix: fmt lint

# ---------- Golden file analysis ----------

# Analyze golden files with dart analyze.
analyze-golden:
  ./scripts/analyze_golden.sh

# Regenerate all UDL-mode golden files from current generator.
regen-golden:
  #!/usr/bin/env bash
  set -euo pipefail
  for udl in fixtures/*/src/*.udl fixtures/regressions/*/src/*.udl; do
    dir="$(dirname "$(dirname "$udl")")"
    name="$(basename "$udl" .udl)"
    ns="$(echo "$name" | tr '-' '_')"
    cargo run -- generate "$udl" --out-dir /tmp/regen_golden 2>/dev/null
    [ -f "/tmp/regen_golden/${ns}.dart" ] && cp "/tmp/regen_golden/${ns}.dart" "$dir/expected/${ns}.dart"
  done
  echo "Regenerated all UDL-mode golden files."

# ---------- Full integration ----------

# Build fixtures, generate bindings, and run Dart runtime tests.
test-all:
  ./scripts/test_bindings.sh

# Build the workspace.
build:
  cargo build --workspace

# ---------- Code generation ----------

# Generate Dart bindings from a UDL file or compiled library.
generate *args:
  cargo run -- generate {{ args }}
