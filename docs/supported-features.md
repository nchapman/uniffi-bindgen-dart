# Supported Features

This document tracks implemented UniFFI feature parity for Dart.

## Status Snapshot
Legend:
- `Implemented`: available in current prototype with tests.
- `Partial`: some paths implemented, parity still incomplete.
- `Planned`: not implemented yet.

| Area | Status | Notes |
|---|---|---|
| Top-level functions | Implemented | includes primitives, temporal, bytes, records/enums, and typed throws envelope paths |
| Objects/interfaces | Partial | sync constructors/methods + lifecycle (`close`/finalizer) implemented; async wrappers for `[Async]` methods are generated across supported return families including bytes and string-keyed maps; trait edge-case parity still pending |
| Trait methods | Partial | object-level `Display`/`Debug`/`Hash`/`Eq` traits map to idiomatic Dart `toString()`/`hashCode`/`operator ==`; `Ord` is currently blocked in this toolchain by UniFFI UDL parser support (`Invalid trait name: Ord`) |
| Records | Implemented | model generation + JSON codecs + `copyWith` |
| Enums | Implemented | flat + data-carrying codecs |
| Errors (`[Error]` + `[Throws]`) | Partial | typed Dart exception mapping for supported runtime-compatible paths, including external enum throw-contract paths via `*ExceptionFfiCodec.decode` |
| Optionals/sequences/maps | Partial | optionals/sequences are covered in top-level + object paths (including async bytes families); string-keyed maps are covered in top-level and object sync/async paths; broader nested map parity still pending |
| Builtins | Implemented | int/float/bool/string/bytes/timestamp/duration |
| Async futures | Partial | `[Async]` maps to idiomatic `Future<...>` APIs with rust-future poll/cancel/complete/free runtime flow for string, `void`, integer, object-handle (`u64`), bytes, optional-bytes, bytes-sequence, and string-keyed-map return families; builtin-backed custom typedefs in those families are supported; dedicated async golden coverage exists (`fixtures/futures-stress`) and runtime smoke includes failure + timeout/non-completion checks; external/custom parity is still incomplete |
| Callback interfaces | Partial | sync/async/throws callback argument paths for top-level + object methods are implemented, including callback-interface method-level async/throws generation for primitive + string/optional string/record/enum return families with runtime fixture coverage |
| Custom types | Partial | builtin-backed typedef unwrapping is implemented for runtime-compatible paths and validated for string, integer, bytes/optional-bytes, and string-keyed custom-map aliases across sync/async top-level + object calls; nested/container custom aliases (for example `record<string, sequence<Count>>` and `record<string, Blob?>`) are now covered in runtime smoke for top-level and object sync/async paths; dedicated `custom-types-demo` golden fixture is in place; broader custom lift/lower coverage is still pending |
| External/remote types | Partial | external record/enum/interface typedef references bind through runtime wrappers with mapped `external_packages` imports; generator emits stable public `*FfiCodec` helpers for enum/error/object cross-package conversion contracts (including external enum throw decode via `*ExceptionFfiCodec.decode`, external interface handle lower/lift paths in top-level runtime-compatible calls, and async external-interface handle return paths); dedicated `ext-types-demo` golden coverage is in place; full external parity (broader object/trait/error paths across crates) still needs deeper metadata/library-mode integration |
| Rename/exclude/docstrings | Implemented | `rename`/`exclude` config keys are implemented for generated Dart public API wrappers (top-level functions + object class/constructor/method names) with dedicated `rename-demo` golden coverage; docstring emission is implemented across generated public Dart API/model surfaces with dedicated `docstrings-demo` golden coverage |

## Notes
- Current fixture coverage is centered on `simple-fns` (rich runtime interactions) plus focused golden fixtures.
- Strict hygiene gate includes `cargo clippy --all-targets -- -D warnings` and full `./scripts/test_bindings.sh`.
