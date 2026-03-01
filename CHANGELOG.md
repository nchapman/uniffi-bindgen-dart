# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Full UDL-to-Dart code generation via `dart:ffi` and `DynamicLibrary`
- CLI entrypoint: `cargo run -- generate <udl> --out-dir <dir>`
- Configurable bindings via `uniffi.toml` (`[bindings.dart]` section)
- Complete primitive type support: bool, integers, floats, string, bytes, void
- Nullable, optional, sequence, and map container types
- Record and enum model types with JSON string codec bridges
- Data-carrying (tagged) enums with runtime deserialization
- Typed exception mapping for UDL `[Throws]` errors
- Object lifecycle bindings: constructors, methods, disposable pointers
- Async support via Rust futures with full return-type coverage
- Callback interfaces: sync, async, throwing, with object and model arguments
- Trait interface support with object return lifting
- External type imports with cross-package `external_packages` config
- Custom type aliases and container alias coverage
- Rename/exclude config for generated Dart API names
- Docstring emission from UDL `///` comments
- Dart reserved-word escaping in generated signatures
- `Eq` trait mapping to Dart `operator ==`
- `Ord` trait mapping to Dart `Comparable.compareTo`
- `Display` and `Hash` trait helpers on objects
- Record and enum method bindings via metadata
- Library-mode metadata parsing for advanced binding generation
- `configureDefaultBindings(libraryPath:)` for consumer-side library loading
- `ffibuffer` runtime fallback for unsupported UniFFI ABI functions
- Timestamp and Duration runtime conversions
- Golden test harness with 14 fixture suites for deterministic output
- Native Dart runtime integration tests (`scripts/test_bindings.sh`)
- CI pipeline with separate build and test-bindings jobs

### Changed

- Modularized `dart/mod.rs` into 14 focused sub-modules
- Aligned with UniFFI 0.31 upstream semantics

[Unreleased]: https://github.com/aspect-build/uniffi-bindgen-dart/compare/v0.1.0...HEAD
