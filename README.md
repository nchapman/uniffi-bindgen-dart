# uniffi-bindgen-dart

[![CI](https://github.com/aspect-build/uniffi-bindgen-dart/actions/workflows/ci.yml/badge.svg)](https://github.com/aspect-build/uniffi-bindgen-dart/actions/workflows/ci.yml)
[![License: MPL-2.0](https://img.shields.io/badge/License-MPL_2.0-blue.svg)](https://opensource.org/licenses/MPL-2.0)

Generate idiomatic [Dart](https://dart.dev/) bindings for [UniFFI](https://github.com/mozilla/uniffi-rs) components.

`uniffi-bindgen-dart` is a third-party bindings generator that produces
production-grade Dart code from UniFFI interface definitions. It targets
`uniffi-rs` version **0.31.0**.

## Features

- All UniFFI primitives, strings, bytes, timestamps, and durations
- Records with field defaults and `copyWith` helpers
- Flat and data-carrying enums with codec support
- Objects with constructors, methods, `close()` lifecycle, and `NativeFinalizer` safety net
- Typed Dart exceptions via `[Error]` and `[Throws]`
- Async functions and methods mapped to `Future<T>`
- Callback interfaces (sync, async, and throwing)
- Custom type aliases and external/remote type imports
- Trait method mapping (`Display` → `toString()`, `Hash` → `hashCode`, `Eq` → `operator ==`, `Ord` → `compareTo`)
- Rename, exclude, and docstring support
- Library-mode metadata extraction (`--library`) for proc-macro crates

## Install

Requires Rust 1.75 or later.

```bash
cargo install --git https://github.com/aspect-build/uniffi-bindgen-dart
```

Or build from source:

```bash
git clone https://github.com/aspect-build/uniffi-bindgen-dart
cd uniffi-bindgen-dart
cargo build --release
```

## Usage

Generate bindings from a UDL file:

```bash
uniffi-bindgen-dart generate path/to/definitions.udl --out-dir out/
```

Generate from a compiled library (proc-macro / library mode):

```bash
uniffi-bindgen-dart generate path/to/libmycrate.so --library --out-dir out/
```

### CLI flags

| Flag | Description |
|---|---|
| `--out-dir <dir>` | Output directory for generated Dart files |
| `--library` | Treat source as a compiled cdylib (library mode) |
| `--config <file>` | Path to `uniffi.toml` configuration |
| `--crate <name>` | In library mode, generate bindings for this crate only |
| `--no-format` | Skip `dart format` on generated output |

### Doctor

Check that host tooling is available:

```bash
uniffi-bindgen-dart doctor
```

## Configuration

Place a `[bindings.dart]` section in your `uniffi.toml`:

```toml
[bindings.dart]
module_name = "my_bindings"
ffi_class_name = "MyInterop"
library_name = "myffi"
rename = { add_numbers = "sumValues", "Counter.current_value" = "valueNow" }
exclude = ["internal_helper"]
external_packages = { other_crate = "package:other_bindings/other_bindings.dart" }
```

See [docs/configuration.md](docs/configuration.md) for the full reference.

## Linking

Generated bindings load a native library at runtime via `DynamicLibrary.open()`.
Make sure the compiled Rust cdylib is discoverable:

- **Linux**: add to `LD_LIBRARY_PATH` or install to a system library path
- **macOS**: add to `DYLD_LIBRARY_PATH` or embed via `@rpath`
- **Flutter**: use the standard native asset bundling for your target platform

## Compatibility

| uniffi-bindgen-dart | uniffi-rs |
|---|---|
| 0.1.x | 0.31.0 |

## Feature status

See [docs/supported-features.md](docs/supported-features.md) for a detailed
feature parity matrix.

## Development

```bash
# Run all Rust tests (unit + golden)
cargo test --workspace

# Build fixtures, generate bindings, and run Dart runtime tests
./scripts/test_bindings.sh
```

See [docs/testing.md](docs/testing.md) for the full test workflow.

## Documentation

- [Supported features](docs/supported-features.md)
- [Configuration reference](docs/configuration.md)
- [Testing guide](docs/testing.md)
- [Release process](docs/release.md)

## License

MPL-2.0
