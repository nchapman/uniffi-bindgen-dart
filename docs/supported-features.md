# Supported Features

This document tracks implemented UniFFI feature parity for Dart.

## Status Snapshot
Legend:
- `Implemented`: available in current prototype with tests.
- `Partial`: some paths implemented, parity still incomplete.
- `Planned`: not implemented yet.

| Area | Status | Notes |
|---|---|---|
| Top-level functions | Implemented | includes primitives, temporal, bytes, records/enums, typed throws envelope paths, and metadata-backed default-argument rendering in generated Dart wrapper signatures |
| Objects/interfaces | Implemented | sync/async constructors and methods + lifecycle (`close`/finalizer); async constructors use real Rust-future poll/complete lifecycle (including `[Async, Throws=X]`); async wrappers for `[Async]` methods generated across all supported return families; optional object parameters and returns (`Object?`) supported via handle-based null sentinel |
| Trait methods | Implemented | object-level `Display`/`Debug`/`Hash`/`Eq`/`Ord` traits map to idiomatic Dart `toString()`/`hashCode`/`operator ==`/`Comparable<T>.compareTo`; record and data-carrying enum trait synthesis (`[Traits=(Display, Eq, Hash)]`) generates `toString()`, `operator ==`, `hashCode` using `Object.hash`; flat enums use Dart-native implementations |
| Foreign-implementable traits | Implemented | `[Trait, WithForeign]` generates full vtable FFI glue with `NativeCallable.isolateLocal` dispatch, odd/even handle map, and Dart-to-Rust callback registration |
| Records | Implemented | model generation + JSON codecs + `copyWith`; record field defaults are rendered in Dart constructors and respected in `fromJson` when keys are absent |
| Enums | Implemented | flat + data-carrying codecs; `[NonExhaustive]` enums generate `unknown` fallback variant for forward-compatible deserialization; enum discriminant values supported via Dart 2.17 enhanced enums |
| Errors (`[Error]` + `[Throws]`) | Implemented | typed Dart exception mapping for enum and object error types, including external enum throw-contract paths via `*ExceptionFfiCodec.decode`; object-as-error (`[Throws=Interface]`) supported via handle-based lifting; `[NonExhaustive]` errors generate `Unknown` exception subclass |
| Optionals | Implemented | `Optional<String>`, `Optional<Bytes>`, `Optional<Object>` (handle sentinel), and `Optional<Primitive>` (JSON-encoded) all supported as parameters and return types across sync/async/callback paths |
| Sequences/maps | Implemented | sequences covered in top-level + object paths (including async bytes families); string-keyed maps use JSON codec; non-string-keyed maps (`record<u32, u64>`) use binary RustBuffer codec with `_UniFfiBinaryWriter`/`_UniFfiBinaryReader` |
| Builtins | Implemented | int/float/bool/string/bytes/timestamp/duration |
| Async futures | Implemented | `[Async]` maps to idiomatic `Future<...>` APIs with rust-future poll/cancel/complete/free runtime flow for all return families; `[Async, Throws=X]` functions/methods/constructors fully supported with typed error decoding; dedicated async golden coverage (`fixtures/futures-stress`) and runtime smoke with failure + timeout checks |
| Callback interfaces | Implemented | sync/async/throws callback argument paths for top-level functions and object methods; callback-interface method-level async/throws generation for all supported return families; vtable/handle-based dispatch with `NativeCallable.isolateLocal`; throwing functions with callback params supported |
| Custom types | Implemented | builtin-backed typedef unwrapping implemented across all runtime paths (same approach as Swift/Kotlin — validation happens in Rust scaffolding); validated for string, integer, bytes/optional-bytes, and string-keyed custom-map aliases across sync/async top-level + object calls; nested container custom aliases covered in runtime smoke |
| External/remote types | Implemented | external record/enum/interface typedef references bind through runtime wrappers with mapped `external_packages` imports; import collection scans all surfaces including record fields and enum payloads; generator emits stable public `*FfiCodec` helpers for cross-package conversion contracts; `Optional<ExternalRecord>`, `Sequence<ExternalRecord>`, and external-field-in-record paths covered by golden fixture |
| Rename/exclude/docstrings | Implemented | `rename`/`exclude` config keys implemented for generated Dart public API wrappers with dedicated `rename-demo` golden coverage; docstring emission across all generated surfaces with `docstrings-demo` golden coverage |
| Library-mode metadata input | Implemented | `generate --library <cdylib>` parses UniFFI metadata from library artifacts with optional crate selection via `--crate` |
| Record/enum methods (proc-macro metadata) | Implemented | library-metadata-driven generation emits idiomatic Dart record methods and enum methods (flat-enum extensions + sealed-enum instance methods) plus runtime FFI lookup/invoke wrappers; comprehensive runtime fixture coverage via `record-enum-methods` with sync/async/throws/custom-type method families |
| Non-exhaustive enums | Implemented | `[NonExhaustive]` flat enums and error enums generate unknown/fallback variants for forward-compatible deserialization; dedicated `non-exhaustive-demo` golden fixture |
| ABI integrity checks | Implemented | contract version and per-function checksum verification at library init; mismatches throw clear diagnostic errors |
| Skip warnings | Implemented | unsupported constructs emit warning comments in generated code and stderr messages during generation |

## Known Limitations
- **Recursive callback interfaces**: Callback interfaces whose methods take or return the same callback type (e.g., `NodeTrait` with `set_parent(NodeTrait?)`) cannot be used as function parameters.
- **Callback interface as return type**: Functions returning a callback interface type (e.g., `Getters make_rust_getters()`) are not yet supported.
- **Error interface methods**: Error types generated from `[Error]` enums do not support methods.

## Notes
- Current fixture coverage includes 19 golden tests across all major feature domains, anchored by `coverall-demo` (comprehensive feature combinations) and `simple-fns` (rich runtime interactions).
- Strict hygiene gate includes `cargo clippy --all-targets -- -D warnings` and full `./scripts/test_bindings.sh`.
