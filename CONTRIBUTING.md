# Contributing to uniffi-bindgen-dart

Thank you for your interest in contributing! This document covers the essentials.

## Prerequisites

- **Rust** (stable toolchain) with `clippy` and `rustfmt` components
- **Dart SDK** (for running runtime binding tests)

## Development Workflow

```bash
# Clone and build
git clone https://github.com/nchapman/uniffi-bindgen-dart.git
cd uniffi-bindgen-dart
cargo build --workspace

# Run the full CI gate (format + lint + tests)
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace

# Run runtime binding tests (requires Dart SDK)
./scripts/test_bindings.sh
```

## Project Structure

| Directory | Purpose |
|-----------|---------|
| `crates/ubdg_bindgen/` | Code generator (CLI + Dart output) |
| `crates/ubdg_runtime/` | Runtime support library |
| `crates/ubdg_testing/` | Shared test helpers |
| `fixtures/` | UDL fixtures with expected golden output |
| `integration/` | Dart runtime smoke tests |
| `scripts/` | Build and test automation |

## Adding a New UDL Feature

1. Create a fixture in `fixtures/{name}/src/{name}.udl` (kebab-case filename)
2. Generate the golden output:
   ```bash
   cargo run -- generate fixtures/{name}/src/{name}.udl --out-dir /tmp/gen
   cp /tmp/gen/{namespace}.dart fixtures/{name}/expected/{namespace}.dart
   ```
3. Add a golden test to `crates/ubdg_bindgen/tests/golden_generated.rs`
4. Run the full test suite to verify

## Golden Test Pattern

Golden tests compare generated Dart output byte-for-byte against expected files. When modifying the generator:

1. Make your code changes in `crates/ubdg_bindgen/src/dart/mod.rs`
2. Regenerate affected golden files (see step 2 above)
3. Review the diff to ensure changes are intentional
4. Commit both the code change and updated golden files together

## Code Style

- Run `cargo fmt` before committing
- All code must pass `cargo clippy --all-targets -- -D warnings`
- Follow existing patterns in the codebase
- Commit messages use imperative mood ("Add feature" not "Added feature")

## Reporting Issues

Open an issue at [github.com/nchapman/uniffi-bindgen-dart/issues](https://github.com/nchapman/uniffi-bindgen-dart/issues) with:
- UniFFI version and Rust toolchain version
- Minimal UDL that reproduces the problem
- Expected vs actual generated output
