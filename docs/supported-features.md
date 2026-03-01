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
| Objects/interfaces | Partial | sync constructors/methods + lifecycle (`close`/finalizer) implemented; async/trait parity pending |
| Trait methods | Planned | pending full parity coverage |
| Records | Implemented | model generation + JSON codecs + `copyWith` |
| Enums | Implemented | flat + data-carrying codecs |
| Errors (`[Error]` + `[Throws]`) | Partial | typed Dart exception mapping for supported runtime-compatible paths |
| Optionals/sequences/maps | Partial | covered in top-level and selected object paths; broader nesting parity still pending |
| Builtins | Implemented | int/float/bool/string/bytes/timestamp/duration |
| Async futures | Planned | pending |
| Callback interfaces | Planned | pending |
| Custom types | Planned | pending |
| External/remote types | Planned | pending |
| Rename/exclude/docstrings | Planned | pending |

## Notes
- Current fixture coverage is centered on `simple-fns` (rich runtime interactions) plus focused golden fixtures.
- Strict hygiene gate includes `cargo clippy --all-targets -- -D warnings` and full `./scripts/test_bindings.sh`.
