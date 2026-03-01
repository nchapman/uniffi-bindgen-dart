# Configuration

`uniffi-bindgen-dart` will read `[bindings.dart]` from `uniffi.toml`.

## Supported Keys

- `module_name`: overrides generated Dart `library ...;` module name.
- `ffi_class_name`: overrides generated FFI class name.
- `library_name`: overrides dynamic library name used by `DynamicLibrary.open(...)`.
- `dart_format`: reserved; currently accepted but not yet wired to formatter behavior.
- `rename`: map of UDL API identifiers to Dart public API names.
- `exclude`: list of UDL API identifiers to omit from generated Dart public API surface.
- `external_packages`: map of external UniFFI crate names to Dart import URIs used for generated external-type references.

## `rename` and `exclude` Identifier Format

- Top-level function: `function_name`
- Object/interface class name: `ObjectName`
- Object constructor/method: `ObjectName.member_name`

Examples:
- `add_numbers`
- `Counter`
- `Counter.current_value`
- `Counter.with_seed`

## Example

```toml
[bindings.dart]
module_name = "demo_bindings"
ffi_class_name = "DemoInterop"
library_name = "demoffi"
rename = { add_numbers = "sumValues", Counter = "Meter", "Counter.current_value" = "valueNow" }
exclude = ["hidden_sum", "Counter.hidden_value"]
external_packages = { other_crate = "package:other_bindings/other_bindings.dart" }
```
