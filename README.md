# uniffi-bindgen-dart

[![Crates.io](https://img.shields.io/crates/v/uniffi-bindgen-dart)](https://crates.io/crates/uniffi-bindgen-dart)
[![CI](https://github.com/nchapman/uniffi-bindgen-dart/actions/workflows/ci.yml/badge.svg)](https://github.com/nchapman/uniffi-bindgen-dart/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

Call Rust code from Dart and Flutter.

`uniffi-bindgen-dart` generates idiomatic Dart bindings from [UniFFI](https://github.com/mozilla/uniffi-rs) interface definitions. Define your API once in Rust, and get production-grade Dart code that uses `dart:ffi` to call your compiled library -- on mobile, desktop, and server.

## Quickstart

**1. Define your interface** in a UDL file (`src/math.udl`):

```webidl
namespace math {
  u32 add(u32 left, u32 right);
  string greet(string name);
};
```

**2. Implement it in Rust** (`src/lib.rs`):

```rust
pub fn add(left: u32, right: u32) -> u32 {
    left + right
}

pub fn greet(name: String) -> String {
    format!("Hello, {name}!")
}
```

**3. Build** your Rust crate as a cdylib:

```bash
cargo build --release
```

**4. Generate Dart bindings:**

```bash
uniffi-bindgen-dart generate target/release/libmath.dylib --out-dir out/
```

**5. Use from Dart:**

```dart
import 'out/math.dart';

void main() {
  configureDefaultBindings(libraryPath: 'target/release/libmath.dylib');

  print(add(2, 3));        // 5
  print(greet('World'));   // Hello, World!
}
```

The generator reads your compiled library (or UDL file) and emits a self-contained `.dart` file with typed functions, classes, enums, and records -- ready to use.

## Install

Requires Rust 1.75 or later.

```bash
cargo install uniffi-bindgen-dart
```

Or install from source:

```bash
cargo install --git https://github.com/nchapman/uniffi-bindgen-dart
```

After installing, verify your environment:

```bash
uniffi-bindgen-dart doctor
```

This checks that required host tooling (Dart SDK, etc.) is available and prints diagnostics.

## What it generates

The generated Dart code is designed to look like something you would write by hand. Here are a few examples of what you get.

### Top-level functions

UDL:

```webidl
namespace math {
  u32 add(u32 left, u32 right);
  string greet(string name);
};
```

Generated Dart:

```dart
int add(int left, int right) {
  return _bindings().add(left, right);
}

String greet(String name) {
  return _bindings().greet(name);
}
```

### Records

UDL:

```webidl
dictionary Person {
  string name;
  u32 age;
  string? nickname;
};
```

Generated Dart:

```dart
class Person {
  const Person({
    required this.name,
    required this.age,
    required this.nickname,
  });

  final String name;
  final int age;
  final String? nickname;

  Person copyWith({
    String? name,
    int? age,
    Object? nickname = _sentinel,
  }) {
    return Person(
      name: name ?? this.name,
      age: age ?? this.age,
      nickname: nickname == _sentinel ? this.nickname : nickname as String?,
    );
  }

  // toString(), operator ==, hashCode, toJson(), fromJson() also generated
}
```

### Objects (interfaces)

UDL:

```webidl
interface Counter {
  constructor();
  u32 current_value();
};
```

Generated Dart:

```dart
final class Counter {
  // Prevent manual construction -- use factory constructors
  Counter._(this._ffi, this._handle) {
    _finalizer.attach(this, _CounterFinalizerToken(_ffi._counterFree, _handle), detach: this);
  }

  bool get isClosed => _closed;

  void close() { /* releases native resource */ }

  static Counter create() {
    return _bindings().counterCreateNew();
  }

  int currentValue() {
    _ensureOpen();
    return _ffi.counterInvokeCurrentValue(_handle);
  }
}
```

Objects are wrapped in lifecycle-safe classes with `NativeFinalizer` support, `close()` for deterministic cleanup, and guards against use-after-close.

### Enums

UDL:

```webidl
enum Color { "red", "green", "blue" };

[Enum]
interface Outcome {
  Success(string message);
  Failure(i32 code, string reason);
};
```

Generated Dart:

```dart
enum Color { red, green, blue }

sealed class Outcome {
  const Outcome();
}

final class OutcomeSuccess extends Outcome {
  const OutcomeSuccess({ required this.message });
  final String message;
}

final class OutcomeFailure extends Outcome {
  const OutcomeFailure({ required this.code, required this.reason });
  final int code;
  final String reason;
}
```

Flat enums map to Dart `enum`, data-carrying enums map to `sealed class` hierarchies with exhaustive pattern matching.

## Usage

### Generate command

```bash
uniffi-bindgen-dart generate <SOURCE> --out-dir <DIR> [OPTIONS]
```

The tool auto-detects the mode from the file extension:

- **Library mode** (`.dylib` / `.so` / `.dll`) -- reads metadata from a compiled UniFFI cdylib. This is the recommended approach for production.
- **UDL mode** (`.udl`) -- reads a UDL file directly. Useful during development.

| Flag | Description |
|---|---|
| `--out-dir <dir>` | Output directory for generated Dart files |
| `--config <file>` | Path to `uniffi.toml` configuration |
| `--crate <name>` | Generate bindings for this crate only (library mode) |
| `--no-format` | Skip running `dart format` on output |

### Configuration

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

Generated bindings load a native library at runtime via `DynamicLibrary.open()`. Make sure the compiled Rust cdylib is discoverable:

- **Linux**: add to `LD_LIBRARY_PATH` or install to a system library path.
- **macOS**: add to `DYLD_LIBRARY_PATH` or embed via `@rpath`.
- **Flutter**: use the standard native asset bundling for your target platform. Place the compiled `.so`/`.dylib`/`.dll` in the appropriate platform directory (`android/`, `ios/`, `macos/`, `linux/`, `windows/`) and it will be bundled automatically.

## Features

- All UniFFI primitives, strings, bytes, timestamps, and durations
- Records with field defaults, `copyWith` helpers, JSON serialization
- Flat enums (Dart `enum`) and data-carrying enums (`sealed class` hierarchies)
- Objects with constructors, methods, `close()` lifecycle, and `NativeFinalizer` safety net
- Typed Dart exceptions via `[Error]` and `[Throws]`
- Async functions and methods mapped to `Future<T>`
- Callback interfaces (sync, async, and throwing)
- Custom type aliases and external type imports across packages
- Trait synthesis: `Display` to `toString()`, `Hash` to `hashCode`, `Eq` to `operator ==`, `Ord` to `compareTo`
- Rename and exclude symbols via config
- UDL docstrings preserved as Dart doc comments
- Library-mode metadata extraction for proc-macro crates

## Compatibility

| uniffi-bindgen-dart | uniffi-rs |
|---|---|
| 0.1.x | 0.31.0 |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT
