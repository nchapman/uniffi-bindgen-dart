# UniFFI Dart Bindgen Plan (Instance)

## Purpose
This plan is the implementation template for building production-grade UniFFI language backends.

## Source Template
1. Base template: `PLAN_TEMPLATE.md`.
2. This file resolves the template variables for Dart.
3. Reuse the same gates/policy for future language instances unless there is a hard technical reason to change.
4. Dart-specific commands and paths are captured below.

## Template Variables
| Variable | Dart Value | Notes |
|---|---|---|
| `LANG_NAME` | `Dart` | Human-readable language name |
| `LANG_ID` | `dart` | Used in config/table names |
| `BINARY_NAME` | `uniffi-bindgen-dart` | CLI binary |
| `CONFIG_TABLE` | `[bindings.dart]` | `uniffi.toml` section |
| `HOST_FORMAT_CMD` | `dart format --set-exit-if-changed` | Generated-code format check |
| `HOST_ANALYZE_CMD` | `dart analyze` | Static analysis |
| `HOST_TEST_CMD` | `dart test` | Runtime/behavior tests |
| `HOST_PACKAGE_FILE` | `pubspec.yaml` | Host package manifest |
| `OFFICIAL_INTEROP_REF` | `https://github.com/dart-lang/native` | Primary reference for Dart native interop conventions |

## Outcomes
### Primary Outcome
`Dart` backend that is safe, fully featured for core UniFFI use cases, and reliable enough to be a reference backend.

### Secondary Outcome
A repeatable backend-development process that can be applied with minimal changes to future UniFFI language generators.

## Progress Snapshot (March 1, 2026)
### Completed
- Phase 0: Bootstrap
- Phase 1: First End-to-End Path
- Phase 2: Core Type System
- Phase 3: Object Model and Lifetimes (sync object lifecycle paths)

### In Progress
- Phase 4: Enums/errors are in place; trait parity now covers object-level `Display`/`Debug`/`Hash`/`Eq`/`Ord` mapping with idiomatic Dart (`toString()`/`hashCode`/`operator ==`/`Comparable<T>.compareTo`) and runtime fixture validation.
- Phase 5: Async Rust-future ABI now covers string, `void`, integer, object-handle (`u64`), bytes, optional-bytes, bytes-sequence, and string-keyed map return/argument fixture paths for top-level functions and object methods, including builtin-backed custom typedefs (for example, `[Custom] typedef string Label`); dedicated futures-stress golden coverage is in place, and runtime smoke tests now cover error and timeout/non-completion edges with cancel/free counter assertions.
- Phase 6: Advanced config is partially implemented (`rename`/`exclude` for generated public API wrappers with dedicated `rename-demo` golden coverage) and docstring emission is now in place with dedicated `docstrings-demo` coverage; external type support now covers external record/enum typedef wrapper binding, external enum throw-contract decoding, and external interface typedef argument/return handle paths with `external_packages` import mapping, with broader external parity still pending. Library-mode metadata ingestion is now wired for `--library` source paths, and metadata-defined record/enum methods now generate idiomatic Dart API wrappers plus runtime FFI call paths.
- Phase 7+: Documentation hardening and release workflow completion pending.
- CI hardening: strict `clippy` gate and artifact-split runtime binding test matrix (Linux + macOS) are now wired in GitHub Actions.

### Blocked/Deferred
- None currently.

### Implemented So Far (Prototype Baseline)
- CLI `generate` flow with `uniffi.toml` config resolution.
- Deterministic generation with golden tests for `simple-fns`, `compound-demo`, and `model-types-demo`.
- Deterministic generation with golden tests for `simple-fns`, `compound-demo`, `model-types-demo`, `futures-stress`, and `custom-types-demo`.
- Top-level functions for primitive, temporal, bytes, record, enum, and fallible envelope paths.
- Record models with JSON codec helpers and `copyWith`.
- Record field default-value rendering in generated Dart constructors and `fromJson` fallback paths.
- Flat + data-carrying enums with runtime encode/decode helpers.
- Typed Dart exception hierarchy and throw mapping from `[Throws=...]` / `[Error]`.
- Object wrappers with explicit `close()` plus finalizer fallback.
- Object constructors/methods with runtime marshalling across supported FFI-compatible types.
- Runtime fixture/native library coverage for strings, bytes, records, enums, objects, and typed errors.
- Async `[Async]` wrappers and Rust-future poll/cancel/complete/free lifecycle coverage across string, `void`, integer, bytes, optional-bytes, bytes-sequence, and string-keyed map fixture paths.
- Runtime/custom-type unwrapping for builtin-backed custom typedefs is validated across string, `u32`, and bytes (including optional-bytes) families in sync/async top-level and object method paths, including nested/container aliases (`record<string, sequence<Count>>` and `record<string, Blob?>`) with runtime smoke coverage.
- Dedicated async fixture golden coverage at `fixtures/futures-stress` plus runtime stress checks for failure and timeout/non-completion behavior through `binding_tests/test/smoke_test.dart`.
- Advanced config support now includes `rename`/`exclude` for top-level wrappers and object class/constructor/method API names, with deterministic `rename-demo` golden coverage.
- External record/enum/interface typedef wrapper generation is now wired with `external_packages` import mapping, including external enum throw-contract decode and external interface handle lower/lift paths, with deterministic `ext-types-demo` golden coverage.
- Callback interface bridge support for sync/async/throwing top-level and object-method function-argument callbacks, including callback-interface method-level async/throws paths for primitive + string/optional string/record/enum return families, with fixture/runtime verification.
- Trait helper mapping for UDL `[Traits=(...)]` object interfaces now generates idiomatic Dart `toString()`, `hashCode`, and `operator ==` via UniFFI trait-method exports (`Display`/`Debug`/`Hash`/`Eq`), with generator + runtime fixture coverage.
- Callable default-value rendering helpers are in place for metadata-backed defaults, generating idiomatic named Dart parameters with defaults when default metadata is available.
- Library-metadata-driven record/enum method generation is in place: generated Dart now emits record instance methods and enum methods (flat-enum extensions + sealed-enum instance methods) backed by runtime FFI wrappers.

## Scope
### In Scope
- Full code generation pipeline for `Dart`.
- Runtime support needed by generated bindings.
- Test harness and fixture coverage.
- CI, release, and compatibility process.
- End-user docs and configuration docs.

### Out of Scope (Initial)
- IDE plugins.
- Framework-specific wrappers beyond core runtime interop.
- Performance micro-optimization before correctness and parity are complete.

## Quality Bar
All must be true before stable release:
1. Feature completeness against agreed parity contract.
2. Deterministic generation outputs for golden-tested fixtures.
3. Runtime fixture suite green on required platforms.
4. No known unsound lifetime/memory behavior in object/callback paths.
5. Clear compatibility mapping to target `uniffi-rs` version.
6. Generated Dart is idiomatic and passes Dart formatting and analysis gates.

## Idiomatic Code Contract
- Generated bindings must read like native Dart code, not Rust-shaped code translated into Dart syntax.
- Follow Dart naming/style conventions and common API ergonomics.
- Use Dart-standard async/error/resource patterns where semantics allow.
- `dart format` and `dart analyze` are required quality gates for generated output.

## Reference Baselines
- `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs`
  - Canonical architecture and semantics for Swift/Kotlin/Python.
- `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-bindgen-react-native`
  - External generator structure, CLI composition, test utility patterns.
- `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-bindgen-go`
  - Host-language integration-test package layout, artifact-split CI, compatibility versioning strategy.
- [dart-lang/native](https://github.com/dart-lang/native)
  - Official Dart-language native interop ecosystem reference (FFI/native assets/code generation patterns).

## Repository Blueprint
```text
.
├── Cargo.toml
├── PLAN.md
├── crates/
│   ├── ubdg_bindgen/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── cli.rs
│   │   │   └── dart/
│   │   │       ├── mod.rs
│   │   │       ├── config.rs
│   │   │       ├── oracle.rs
│   │   │       ├── primitives.rs
│   │   │       ├── compounds.rs
│   │   │       ├── record.rs
│   │   │       ├── object.rs
│   │   │       ├── enum_.rs
│   │   │       ├── error.rs
│   │   │       ├── callback_interface.rs
│   │   │       ├── custom.rs
│   │   │       └── templates/
│   │   └── tests/
│   ├── ubdg_runtime/
│   └── ubdg_testing/
├── fixtures/
│   └── regressions/
├── binding_tests/
│   ├── generated/
│   ├── test/
│   └── pubspec.yaml
├── integration/
│   └── dart_package_template/
├── docs/
│   ├── configuration.md
│   ├── supported-features.md
│   ├── testing.md
│   └── release.md
├── scripts/
│   ├── build.sh
│   ├── build_bindings.sh
│   └── test_bindings.sh
├── docker_build.sh
├── docker_test_bindings.sh
└── .github/workflows/
```

## CLI Contract
### Required Command
- `generate <source> --out-dir <dir> [--library] [--config <file>] [--crate <name>] [--no-format]`

### Optional Commands (Post-MVP)
- `doctor` for environment diagnostics.
- `print-config` for resolved configuration debugging.

## Configuration Contract (`[bindings.dart]`)
### Required Keys (MVP)
- `library_name`
- `module_name`
- `ffi_class_name`
- `generate_immutable_records`
- `mutable_records`
- `custom_types`
- `rename`
- `exclude`
- `omit_checksums`

### Strongly Recommended Keys
- `external_packages` or equivalent import-map config
- `dart_format`
- any language runtime-specific safety switches

## Feature Parity Contract
Every row must have generator tests + runtime tests.

| Area | Required in v0.x | Notes |
|---|---|---|
| Top-level functions | Yes | sync + fallible |
| Objects/interfaces | Yes | constructors, methods, static methods, lifecycle |
| Records | Yes | defaults + mutability controls |
| Enums | Yes | flat + data-carrying |
| Errors | Yes | typed exception mapping |
| Optionals/sequences/maps | Yes | nested combinations included |
| Builtins | Yes | ints, floats, bool, string, bytes, duration, timestamp |
| Async futures | Yes | Rust async -> host futures/promises |
| Callback interfaces | Yes | sync + async callback paths |
| Custom types | Yes | lift/lower |
| External/remote types | Yes | cross-crate support |
| Renaming/exclusion | Yes | parity with Swift/Kotlin behavior |
| Docstrings | Yes | language-appropriate emission |

## UDL Coverage Ledger (Mandatory)
Use this ledger as the execution checklist for full parity. This is the operational source of truth for implementation status.

### Canonical Source
- For each row, confirm semantics against `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs` before implementation and before marking complete.
- If behavior is unclear, add a note with the exact upstream file/module reviewed.

### Row Rules
1. One row per UDL semantic unit (not per file).
2. Every row must include generator and runtime tests.
3. A row is not complete until required gates and docs updates pass.
4. If a bug is found, add a regression row before implementing the fix.

### Row Execution Playbook (Formulaic)
1. Select next `Not started` or `In progress` row from this ledger.
2. Add/extend fixture and write failing runtime test for that row.
3. Add failing generator-level test (unit or golden).
4. Implement minimal generator/runtime changes to satisfy semantics.
5. Run required gates and update docs.
6. Mark row `Done` with evidence references.

### UDL Coverage Table (Dart Status)
| UDL Unit | Rust Semantics Source (`uniffi-rs`) | Dart API Shape | Generator Changes | Runtime Changes | Required Tests (unit/golden/runtime) | Status | Evidence/Notes |
|---|---|---|---|---|---|---|---|
| Top-level functions (sync) | `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs` | top-level Dart functions | complete for supported types | complete for supported types | `dart::tests::renders_top_level_function_stubs_from_udl`; golden `simple-fns`; `binding_tests/test/smoke_test.dart` | Done | primitives/bytes/temporal/record+enum paths covered |
| Top-level functions (`[Throws]`) | `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs` | typed Dart exceptions | complete for current envelope mapping | complete for current envelope mapping | `dart::tests::renders_throwing_functions_with_typed_exceptions`; `smoke_test.dart` throw path | Done | `[Throws]` + `[Error]` typed mapping implemented for supported runtime paths |
| Records (defaults/mutability) | `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs` | Dart model classes + `copyWith` + codecs | complete for current fixture scope | complete for current fixture scope | `dart::tests::renders_record_and_enum_models`; golden `model-types-demo`; runtime smoke assertions | Done | mutability controls remain to expand with dedicated fixtures |
| Enums (flat/data-carrying) | `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs` | Dart sealed-style model conventions | complete for current fixture scope | complete for current fixture scope | `dart::tests::renders_record_and_enum_models`; golden `compound-demo`; runtime smoke assertions | Done | flat + data-carrying enums covered |
| Objects/interfaces lifecycle | `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs` | wrappers with `close()` + finalizer fallback | sync constructors/methods complete; async method wrappers generated for supported return families | sync lifecycle paths complete; async method runtime fixture coverage includes string/integer/bytes paths | `dart::tests::renders_object_classes_with_lifecycle_and_throws`; golden `simple-fns`; runtime smoke object calls | In progress | core lifecycle + async object method parity is in place for current fixture; trait edge cases and broader fixture depth still pending |
| Trait methods | `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs` | trait mapping into idiomatic Dart APIs | UDL `[Traits=(...)]` parsing + object trait-method synthesis for `Display`/`Debug`/`Hash`/`Eq`/`Ord`; object class generation emits idiomatic `toString()`, `hashCode`, `operator ==`, and `Comparable<T>.compareTo` overrides | fixture native library exports `counter_uniffi_trait_display` / `counter_uniffi_trait_hash` / `counter_uniffi_trait_eq` / `counter_uniffi_trait_ord_cmp` and runtime assertions validate behavior | `dart::tests::renders_object_classes_with_lifecycle_and_throws`; golden `simple-fns`; `binding_tests/test/smoke_test.dart` trait assertions | In progress | historical `Invalid trait name: Ord` reports came from pre-`0.30` UniFFI (`0.29.5`) toolchains; current `0.31.x` parser accepts `Ord`; broader trait edge-case fixture depth still pending |
| Async futures | `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs` | `Future<T>`-based idiomatic APIs | top-level/object API wrappers for `[Async]` with return-type driven rust-future symbol selection, including object-handle (`u64`), bytes/optional-bytes/bytes-sequence/string-keyed-map completion families and builtin-backed custom typedef unwrapping | rust-future poll/cancel/complete/free flow implemented for string, `void`, integer, object-handle (`u64`), bytes, optional-bytes, bytes-sequence, and string-keyed-map fixture paths for top-level + object methods | `dart::tests::renders_async_functions_and_methods_as_futures`; goldens `simple-fns` + `futures-stress`; runtime smoke includes failure + timeout/non-completion lifecycle checks plus async object creation/close coverage | In progress | async lifecycle is runtime-driven (poll/wake/ready/cancel/free); dedicated futures-stress fixture is in place, with runtime cancel/free counters exercised via failure path and timeout checks |
| Callback interfaces (sync/async) | `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs` | callback APIs with Dart callable conventions | callback interface model + vtable bridge generation for top-level functions and object methods across sync/async/throwing function-argument paths, plus callback-interface method-level async/throws generation for primitive + string/optional string/record/enum return families | fixture native library supports callback-vtable init + callback invocation lifecycle (`clone`/`free` + method dispatch) across adder + formatter callback paths for top-level and object methods, including async future and throws envelope flows with callback args and callback-interface async method completion callbacks | `dart::tests::renders_runtime_callback_interface_bindings`; golden `simple-fns`; `binding_tests/test/smoke_test.dart` callback runtime assertions | In progress | callback argument parity is green across sync/async/throws for top-level + object methods; runtime fixture coverage is green for async string/optional string/record/enum callback methods |
| Custom types | `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs` | Dart conversion helpers | builtin-backed typedef unwrapping in type mapping + marshalling is implemented for runtime-compatible paths | fixture/runtime coverage now validates string, `u32`, bytes/optional-bytes, string-keyed `record<string, Count>` custom-map paths, and nested/container custom aliases (`record<string, sequence<Count>>`, `record<string, Blob?>`) across sync/async top-level + object calls; dedicated `custom-types-demo` golden fixture added | extend non-runtime-compatible conversion families and additional edge-case matrices | In progress | current coverage proves builtin-backed custom typedef parity for primary + nested/container runtime families with dedicated fixture artifacts; broader non-runtime-compatible custom conversion matrix still pending |
| External/remote types | `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs` | cross-package import/type mapping | `external_packages` config + external crate usage import emission implemented; external record + enum typedefs are treated as runtime-compatible pointer/JSON paths; external `[Throws=ExternalEnum]` runtime paths now bind via `*ExceptionFfiCodec.decode`; external interface typedef argument/return handle paths now bind via `*FfiCodec.lower/lift`, including async external-interface return paths | dedicated `ext-types-demo` golden validates external-record + external-enum + external-throws + external-interface (sync/async) wrapper generation and package import mapping; full cross-crate runtime fixture matrix is still pending | `dart::tests::imports_external_package_and_binds_external_record_and_enum_types`; golden `ext-types-demo` | In progress | UniFFI removed `Type::External`; current path infers externality from `module_path` crate segment; external object top-level runtime-compatible paths are now covered, while broader external object/trait parity still needs deeper metadata/library-mode integration |
| Rename/exclude/docstrings | `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs` | idiomatic naming/docs in Dart output | `rename`/`exclude` config parsing + wrapper-level API rename/exclude generation implemented; docstring emission implemented for generated public Dart API/model surfaces; `rename-demo` + `docstrings-demo` golden fixtures added | runtime semantics are unaffected by rename/exclude/docstrings (API-surface only); runtime smoke remains green against canonical fixture names | `dart::tests::applies_rename_and_exclude_overrides`; `dart::tests::renders_docstrings_for_public_api_surfaces`; goldens `rename-demo` + `docstrings-demo` | Done | current rename/exclude scope is generated public API wrappers (top-level + object class/constructor/method names); docstrings now emitted on wrappers/models/callback interfaces |
| Regression rows (`regressions/*`) | `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs` | N/A | policy in place; dedicated regression fixture coverage now includes `custom-shadow-demo`, `async-object-lift-demo`, and `callback-custom-async-demo` golden guards | policy in place | `golden_regression_custom_shadow_demo`; `golden_regression_async_object_lift_demo`; `golden_regression_callback_custom_async_demo`; fixtures under `fixtures/regressions/*` | In progress | regression fixtures now guard nested custom JSON closure shadowing, async object lift semantics (local vs external), and async callback custom-alias return generation; continue adding one row per bug class |

## Test Strategy (TDD-First)
### Test Layers
1. Unit tests (Rust): naming, type maps, config parse, edge semantics.
2. Golden generation tests: deterministic generated source outputs.
3. Host compile/analyze tests: generated code quality gate.
4. Runtime integration tests: real FFI interaction through fixtures.
5. CLI behavior tests: argument handling, defaults, error diagnostics.
6. Regression tests: each bug gets a reproducer fixture/test first.

### Per-Feature TDD Workflow (Mandatory)
1. Add/extend fixture and write failing runtime test.
2. Add failing generator-level test (unit or golden).
3. Implement minimal generator/runtime code.
4. Pass all relevant layers locally.
5. Add regression coverage if fixing a bug.
6. Document behavior/config in docs.
7. For memory/free-counter assertions, prefer delta-based checks over brittle absolute totals when new valid call paths may affect counts.

### Definition Of Done For Any Feature
- Unit/golden/runtime tests exist and pass.
- No formatter/analyzer warnings in generated code.
- `cargo clippy --all-targets -- -D warnings` passes.
- Docs updated.
- CI gates remain green.

## Prototype Hygiene Gates
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --workspace`
- `./scripts/test_bindings.sh`
- For generated Dart: analyzer clean (`dart analyze`) and runtime tests green (`dart test` via script)

## Fixture Matrix (Minimum)
| Fixture Group | Purpose |
|---|---|
| `simple-fns` | Baseline call flow and primitives |
| `simple-iface` | Object lifecycle and methods |
| `enum-types` | enum generation semantics |
| `error-types` | throw/catch and error payload behavior |
| `callbacks` | callback interface wiring |
| `trait-methods` | trait-related callable behavior |
| `futures` | async call semantics |
| `ext-types` | external type import/resolution |
| `custom-types` | custom conversion correctness |
| `keywords` | identifier escaping and naming |
| `rename` | rename rule application |
| `mutable-records` | mutability config behavior |
| `regressions/*` | permanent bug prevention |

## Fixture Strategy (Current Project Shape)
- Keep one rich end-to-end fixture (`simple-fns`) that exercises mixed feature interactions and memory/resource behavior.
- Keep focused fixture demos (`compound-demo`, `model-types-demo`) for deterministic golden coverage on type-shape changes.
- Add targeted `regressions/*` fixtures when fixing generator/runtime bugs to prevent reintroductions.

## Execution Phases and Gates
### Phase 0: Bootstrap
#### Deliverables
- Workspace skeleton and base crates.
- CLI shell with help output.
- `scripts/build.sh`, `scripts/build_bindings.sh`, `scripts/test_bindings.sh`.
- CI skeleton.

#### Gate
- `cargo test` green.
- CLI parse tests in place.

### Phase 1: First End-to-End Path
#### Deliverables
- Minimal generator using UniFFI loader.
- Generate and execute simple function calls via `binding_tests`.

#### Gate
- `simple-fns` runtime test green.

### Phase 2: Core Type System
#### Deliverables
- Builtins + optionals/lists/maps.
- Records with defaults and mutability controls.

#### Gate
- Core type fixture suite green.

### Phase 3: Object Model and Lifetimes
#### Deliverables
- Full object API generation.
- Safe handle management (`close` + finalizer fallback).

#### Gate
- Lifetime and double-free safety tests green.

### Phase 4: Enums, Errors, and Traits
#### Deliverables
- Flat/data enums.
- Typed error mappings.
- Trait method support required by parity contract.

#### Gate
- enum/error/trait fixture suite green.

### Phase 5: Async and Callbacks
#### Deliverables
- Async function/method support.
- Callback interfaces, sync and async.

#### Gate
- futures/callback runtime tests stable across CI platforms.

### Phase 6: Advanced Config and External Types
#### Deliverables
- custom types, external types, rename/exclude/docstrings.
- checksum policy and controls.

#### Gate
- Advanced config and external type fixtures green.

### Phase 7: DX and Documentation
#### Deliverables
- Complete user docs.
- Troubleshooting guidance.
- Example package/project.

#### Gate
- New user path validated from docs only.

### Phase 8: Hardening and Release
#### Deliverables
- Compatibility and stability checks.
- Release workflow and policy enforcement.

#### Gate
- Release dry run passes.
- Changelog and compatibility table prepared.

## CI Blueprint
### Required PR Jobs
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --workspace`
- Golden generation tests
- Build bindings artifacts
- Host analyze/compile checks
- Runtime integration tests consuming build artifacts

### Required Job Topology
1. `build` job:
   - compile generator and fixture libraries
   - generate bindings
   - upload artifacts
2. `test-bindings` job:
   - download artifacts
   - run host language test package (`binding_tests`)

### Scheduled Jobs
- Expanded fixture matrix.
- Latest UniFFI compatibility validation.

### Platforms
- Linux required.
- macOS required.

## Release and Compatibility Policy
### Versioning
- Version backend independently from `uniffi-rs`.
- Explicitly declare target `uniffi-rs` version for every backend release.
- Prefer compatibility metadata format: `vX.Y.Z+vA.B.C` where `A.B.C` is upstream UniFFI target.

### Release Checklist
1. Update backend version.
2. Update compatibility table (`backend version -> uniffi-rs version`).
3. Update changelog (`BREAKING` and `IMPORTANT` markers where applicable).
4. Run release dry-run workflow.
5. Tag and publish.

## Repeatability Rules For Future Languages
These rules are part of the template and should not be skipped:
1. Preserve the same phase gates and CI gate types.
2. Keep a dedicated host-language `binding_tests` package/project.
3. Keep artifact-split CI model (`build` then `test-bindings`).
4. Require regression fixture/tests for every bug fix.
5. Maintain explicit compatibility mapping to upstream UniFFI.
6. Require docs parity: configuration, supported features, testing, release.

## Operating Model (Template)
### Cadence
- Track progress per phase gate, not by raw task count.
- Require at least one green runtime fixture expansion in each feature-heavy PR series.

### Change Control
- Any deviation from this template must be documented in `docs/release.md` with rationale.
- Any temporary skipped test must include a linked issue and expiry/removal condition.

### Git Commit Workflow
- Initialize Git at project start and keep history linear.
- Commit continuously as coherent units of change; do not batch unrelated work.
- Use descriptive commit messages that explain what changed and why.
- Do not use commit messages framed as milestone or step progress.
- Run relevant tests before each commit that changes behavior.

## Risk Register (Template)
| Risk | Impact | Mitigation |
|---|---|---|
| Drift from UniFFI semantics | Behavioral incompatibility | Add parity fixtures and compare against Swift/Kotlin outcomes |
| Generator/runtime mismatch | Runtime failures | Enforce runtime integration tests as required PR gate |
| Flaky async/callback tests | CI instability | Isolate timing assumptions and add deterministic test harness helpers |
| External type resolution regressions | Cross-crate breakage | Keep dedicated `ext-types` fixtures in required matrix |
| Release compatibility confusion | Consumer integration failures | Maintain explicit backend-to-UniFFI compatibility table |

## Immediate Next Steps (Dart Instance)
1. Add broader trait fixture coverage around `Eq` semantics (edge cases/lifecycle scenarios) and keep trait behavior synchronized with upstream `uniffi-rs`.
2. Extend remaining async parity for external/custom families and add broader cancellation-path runtime hooks where host-side explicit cancellation is available.
3. Add dedicated runtime fixture coverage for UniFFI `0.31` record/enum method surfaces now that library-metadata generation is implemented.
4. Extend record/enum method parity tests to include async/throws/custom/external combinations as fixture depth increases.
5. Keep `docs/supported-features.md` synchronized with every parity change.
6. Extend remaining custom/external parity (nested custom/container lift/lower families + cross-crate/ext-type scenarios).
7. Keep CI coverage aligned with new fixture/runtime surfaces as parity expands (strict clippy + artifact-split Linux/macOS runtime matrix now implemented).
