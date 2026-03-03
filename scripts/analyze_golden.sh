#!/usr/bin/env bash
# Runs `dart analyze` on all golden expected files to verify they are valid Dart.
# Golden tests check determinism (byte comparison); this checks correctness.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

# Minimal Dart package structure
cat > "$TMPDIR/pubspec.yaml" <<'YAML'
name: golden_analysis
environment:
  sdk: ^3.1.0
dependencies:
  ffi: ^2.1.4
YAML

mkdir -p "$TMPDIR/lib"

# Stub for ext-types-demo's external package import
mkdir -p "$TMPDIR/lib/src/other_bindings"
cat > "$TMPDIR/lib/src/other_bindings/other_bindings.dart" <<'DART'
// Stub types for ext-types-demo golden analysis.

class RemoteThing {
  Map<String, dynamic> toJson() => {};
  static RemoteThing fromJson(Map<String, dynamic> json) => RemoteThing();
}

class RemoteCounter {}

class RemoteCounterFfiCodec {
  static int lower(RemoteCounter input) => 0;
  static RemoteCounter lift(int handle) => RemoteCounter();
}

class RemoteState {}

class RemoteStateFfiCodec {
  static String encode(RemoteState input) => '';
  static RemoteState decode(String payload) => RemoteState();
}

class RemoteFailureExceptionFfiCodec {
  static Exception decode(Object raw) => Exception();
}
DART

# Make the stub resolvable as package:other_bindings/other_bindings.dart
cat > "$TMPDIR/pubspec_overrides.yaml" <<YAML
dependency_overrides:
  other_bindings:
    path: $TMPDIR/lib/src/other_bindings_pkg
YAML

# Create a proper package for the override
mkdir -p "$TMPDIR/lib/src/other_bindings_pkg/lib"
cat > "$TMPDIR/lib/src/other_bindings_pkg/pubspec.yaml" <<'YAML'
name: other_bindings
environment:
  sdk: ^3.1.0
YAML
cp "$TMPDIR/lib/src/other_bindings/other_bindings.dart" \
   "$TMPDIR/lib/src/other_bindings_pkg/lib/other_bindings.dart"

# Copy all golden files into lib/
find "$REPO_ROOT/fixtures" -path '*/expected/*.dart' -exec cp {} "$TMPDIR/lib/" \;

echo "Analyzing $(ls "$TMPDIR/lib/"*.dart | wc -l | tr -d ' ') golden files..."

cd "$TMPDIR"
dart pub get
dart analyze --fatal-warnings

echo "All golden files pass analysis."
