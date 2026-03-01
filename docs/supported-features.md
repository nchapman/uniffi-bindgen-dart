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
| Objects/interfaces | Partial | sync constructors/methods + lifecycle (`close`/finalizer) implemented; async wrappers for `[Async]` methods are generated across supported return families including bytes and string-keyed maps; trait edge-case parity still pending |
| Trait methods | Partial | object-level `Display`/`Debug`/`Hash`/`Eq`/`Ord` traits map to idiomatic Dart `toString()`/`hashCode`/`operator ==`/`Comparable<T>.compareTo`; broader trait edge-case fixture depth still pending |
| Records | Implemented | model generation + JSON codecs + `copyWith`; record field defaults are rendered in Dart constructors and respected in `fromJson` when keys are absent |
| Enums | Implemented | flat + data-carrying codecs |
| Errors (`[Error]` + `[Throws]`) | Partial | typed Dart exception mapping for supported runtime-compatible paths, including external enum throw-contract paths via `*ExceptionFfiCodec.decode` |
| Optionals/sequences/maps | Implemented | optionals/sequences are covered in top-level + object paths (including async bytes families); string-keyed maps use JSON codec in top-level and object sync/async paths; non-string-keyed maps (`record<u32, u64>`) use binary RustBuffer codec with `_UniFfiBinaryWriter`/`_UniFfiBinaryReader` |
| Builtins | Implemented | int/float/bool/string/bytes/timestamp/duration |
| Async futures | Partial | `[Async]` maps to idiomatic `Future<...>` APIs with rust-future poll/cancel/complete/free runtime flow for string, `void`, integer, object-handle (`u64`), bytes, optional-bytes, bytes-sequence, and string-keyed-map return families; builtin-backed custom typedefs in those families are supported; dedicated async golden coverage exists (`fixtures/futures-stress`) and runtime smoke includes failure + timeout/non-completion checks; external/custom parity is still incomplete |
| Callback interfaces | Partial | sync/async/throws callback argument paths for top-level + object methods are implemented, including callback-interface method-level async/throws generation for primitive + string/optional string/record/enum return families with runtime fixture coverage; async callback methods with builtin-backed custom aliases are now covered by regression goldens |
| Custom types | Partial | builtin-backed typedef unwrapping is implemented for runtime-compatible paths and validated for string, integer, bytes/optional-bytes, and string-keyed custom-map aliases across sync/async top-level + object calls; nested/container custom aliases (for example `record<string, sequence<Count>>` and `record<string, Blob?>`) are now covered in runtime smoke for top-level and object sync/async paths; dedicated `custom-types-demo` golden fixture is in place; broader custom lift/lower coverage is still pending |
| External/remote types | Partial | external record/enum/interface typedef references bind through runtime wrappers with mapped `external_packages` imports; generator emits stable public `*FfiCodec` helpers for enum/error/object cross-package conversion contracts (including external enum throw decode via `*ExceptionFfiCodec.decode`, external interface handle lower/lift paths in top-level runtime-compatible calls, and async external-interface handle return paths); dedicated `ext-types-demo` golden coverage is in place; full external parity (broader object/trait/error paths across crates) still needs deeper metadata/library-mode integration |
| Rename/exclude/docstrings | Implemented | `rename`/`exclude` config keys are implemented for generated Dart public API wrappers (top-level functions + object class/constructor/method names) with dedicated `rename-demo` golden coverage; docstring emission is implemented across generated public Dart API/model surfaces with dedicated `docstrings-demo` golden coverage |
| Library-mode metadata input | Implemented | `generate --library <cdylib>` now parses UniFFI metadata from library artifacts (with optional crate selection via `--crate`) instead of requiring UDL-only inputs |
| Record/enum methods (proc-macro metadata) | Partial | library-metadata-driven generation now emits idiomatic Dart record methods and enum methods (flat-enum extensions + sealed-enum instance methods) plus runtime FFI lookup/invoke wrappers; dedicated runtime fixture coverage for these surfaces is still pending |

## Known Limitations
- **`[ByRef]` / `[Self=ByArc]`**: These Rust calling-convention attributes are not reflected in generated Dart code. All values are copied across the FFI boundary.
- **Error interface methods**: Error types generated from `[Error]` enums do not support methods.

## Notes
- Current fixture coverage includes 14 golden tests across all major feature domains, anchored by `coverall-demo` (comprehensive feature combinations) and `simple-fns` (rich runtime interactions).
- Strict hygiene gate includes `cargo clippy --all-targets -- -D warnings` and full `./scripts/test_bindings.sh`.
