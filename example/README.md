# Example: Rust to Dart via UniFFI

This example shows the full workflow for generating Dart bindings from a Rust crate.

## What's here

```
example/
  rust/           # Minimal Rust crate with UDL interface
    Cargo.toml
    src/lib.rs
    src/example.udl
  generated/      # Pre-generated Dart output (for reference)
    example.dart
```

## Workflow

### 1. Build the Rust shared library

```bash
cd example/rust
cargo build --release
# Produces: target/release/libuniffi_example.dylib (macOS) / libuniffi_example.so (Linux) / uniffi_example.dll (Windows)
```

### 2. Generate Dart bindings

```bash
# From the repository root:
cargo run -- generate example/rust/src/example.udl --out-dir example/generated/
```

This reads the UDL interface definition and produces `example.dart`.

### 3. Use in Dart

```dart
import 'example.dart';

void main() {
  // Point to the compiled Rust library
  configureDefaultBindings(libraryPath: 'path/to/libuniffi_example.dylib');

  // Call Rust functions directly
  print(greet('World')); // "Hello, World!"

  // Use generated record types
  final todo = TodoEntry(title: 'Learn UniFFI', done: false);
  print('${todo.title}: ${todo.done}');
}
```

## UDL interface

The interface is defined in [`rust/src/example.udl`](rust/src/example.udl):

```
namespace example {
  string greet(string name);
};

dictionary TodoEntry {
  string title;
  boolean done;
};
```

The matching Rust implementation is in [`rust/src/lib.rs`](rust/src/lib.rs). The `#[uniffi::export]` and `#[derive(uniffi::Record)]` macros wire the Rust code to the UDL definitions. The `.udl` file is read by `uniffi-bindgen-dart generate` — it is not used by the Rust compiler (the proc macros handle that side).
