# UniFFI Language Bindgen Plan Template

## Purpose
This plan is the implementation template for building production-grade UniFFI language backends.  
Use this as the source template for any UniFFI language backend. Create an instance file (for example, PLAN.md) by resolving the template variables and language-specific commands.

## How To Reuse This Template
1. Copy this file into a new backend repo.
2. Update values in the `Template Variables` table.
3. Keep phase gates, CI gates, and release policy unchanged unless there is a hard technical reason.
4. Add language-specific notes only in the marked sections.

## Template Variables
| Variable | Template Value | Notes |
|---|---|---|
| `LANG_NAME` | `<Language>` | Human-readable language name |
| `LANG_ID` | `<lang_id>` | Used in config/table names |
| `BINARY_NAME` | `uniffi-bindgen-<lang_id>` | CLI binary |
| `CONFIG_TABLE` | `[bindings.<lang_id>]` | `uniffi.toml` section |
| `HOST_FORMAT_CMD` | `<host format command>` | Generated-code format check |
| `HOST_ANALYZE_CMD` | `<host static analysis command>` | Static analysis |
| `HOST_TEST_CMD` | `<host test command>` | Runtime/behavior tests |
| `HOST_PACKAGE_FILE` | `<host package manifest>` | Host package manifest |
| `OFFICIAL_INTEROP_REF` | `<language official native interop docs/repo>` | Primary source for host-native interop conventions |

## Outcomes
### Primary Outcome
`LANG_NAME` backend that is safe, fully featured for core UniFFI use cases, and reliable enough to be a reference backend.

### Secondary Outcome
A repeatable backend-development process that can be applied with minimal changes to future UniFFI language generators.

## Progress Tracking (Instance-Only Section)
When creating a language-specific plan instance, add a dated progress snapshot with:
- Completed phases
- In-progress phases
- Blocked/deferred phases
- Implemented scope summary (what is actually working today)
- Current immediate next steps (remove finished items; keep this list forward-looking)

## Scope
### In Scope
- Full code generation pipeline for `LANG_NAME`.
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
6. Generated code is idiomatic for the target language and passes host style/lint checks.

## Idiomatic Code Contract
- Generated bindings must look and behave like hand-written host-language code, not Rust transliterations.
- Follow host language naming, visibility, error, async, and resource-lifecycle conventions.
- Prefer host standard library types and patterns over custom wrapper types when semantics allow.
- Host formatter and analyzer/linter checks are required quality gates.

## Reference Baselines
- `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs`
  - Canonical architecture and semantics for Swift/Kotlin/Python.
- `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-bindgen-react-native`
  - External generator structure, CLI composition, test utility patterns.
- `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-bindgen-go`
  - Host-language integration-test package layout, artifact-split CI, compatibility versioning strategy.
- `OFFICIAL_INTEROP_REF`
  - Authoritative host-language interop and native-asset guidance; use as primary style/packaging reference.

## Repository Blueprint
```text
.
├── Cargo.toml
├── PLAN.md
├── crates/
│   ├── <project_prefix>_bindgen/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── cli.rs
│   │   │   └── <lang_id>/
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
│   ├── <project_prefix>_runtime/
│   └── <project_prefix>_testing/
├── fixtures/
│   └── regressions/
├── binding_tests/
│   ├── generated/
│   ├── test/
│   └── <host package manifest>
├── integration/
│   └── <host_package_template>/
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
- `generate <source> --out-dir <dir> [--config <file>] [--crate <name>] [--no-format]` (mode auto-detected from file extension)

### Optional Commands (Post-MVP)
- `doctor` for environment diagnostics.
- `print-config` for resolved configuration debugging.

## Configuration Contract (`[bindings.<lang_id>]`)
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
- `<lang_id>_format` (or equivalent generated-code format switch)
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
Use this ledger as the execution checklist for full parity. This is the operational source of truth for implementation status, not optional documentation.

### Canonical Source
- For each ledger row, confirm semantics against `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs` before implementation and before marking complete.
- If upstream behavior is unclear, add a note with the exact upstream file/module reviewed.

### Row Rules
1. One row per UDL semantic unit (not per file).
2. Every row must include generator and runtime tests.
3. A row is not complete until all required gates and docs updates pass.
4. If a bug is found, add a regression row before implementing the fix.

### Row Checklist (Formulaic, Required Per Row)
Use this checklist in the row notes before changing status to `Done`:
- `[]` Rust semantics cross-checked against `/Users/nchapman/Drive/Code/lessisbetter/refs/uniffi-rs` (file/module recorded).
- `[]` Failing runtime test added first.
- `[]` Failing generator test added first (unit or golden).
- `[]` Implementation merged with idiomatic target-language API surface review completed.
- `[]` Required gates passed (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, host analyze/test script).
- `[]` Docs and plan status updated with evidence links.

### Row Execution Playbook (Formulaic)
1. Select next `Not started` row from this ledger.
2. Add/extend fixture and write failing runtime test for that row.
3. Add failing generator-level test (unit or golden).
4. Implement minimal generator/runtime changes to satisfy semantics.
5. Run required gates and update docs.
6. Mark row `Done` with evidence links (tests/PR/commit references).

### UDL Coverage Table (Template)
| UDL Unit | Rust Semantics Source (`uniffi-rs`) | Target-Language API Shape | Generator Changes | Runtime Changes | Required Tests (unit/golden/runtime) | Status | Evidence/Notes |
|---|---|---|---|---|---|---|---|
| Top-level functions (sync) | `<module/file>` | `<idiomatic host signature>` | `<needed>` | `<needed>` | `<test ids>` | Not started | |
| Top-level functions (`[Throws]`) | `<module/file>` | `<typed host exceptions>` | `<needed>` | `<needed>` | `<test ids>` | Not started | |
| Records (defaults/mutability) | `<module/file>` | `<host model conventions>` | `<needed>` | `<needed>` | `<test ids>` | Not started | |
| Enums (flat/data-carrying) | `<module/file>` | `<host enum conventions>` | `<needed>` | `<needed>` | `<test ids>` | Not started | |
| Objects/interfaces lifecycle | `<module/file>` | `<construct/close/finalize pattern>` | `<needed>` | `<needed>` | `<test ids>` | Not started | |
| Trait methods | `<module/file>` | `<host trait/protocol mapping>` | `<needed>` | `<needed>` | `<test ids>` | Not started | |
| Async futures | `<module/file>` | `<future/promise mapping>` | `<needed>` | `<needed>` | `<test ids>` | Not started | |
| Callback interfaces (sync/async) | `<module/file>` | `<callback idioms>` | `<needed>` | `<needed>` | `<test ids>` | Not started | |
| Custom types | `<module/file>` | `<host conversion wrappers>` | `<needed>` | `<needed>` | `<test ids>` | Not started | |
| External/remote types | `<module/file>` | `<cross-crate/package idioms>` | `<needed>` | `<needed>` | `<test ids>` | Not started | include cross-package codec contract for enum/error/object (`*FfiCodec` / `*ExceptionFfiCodec`) |
| Rename/exclude/docstrings | `<module/file>` | `<naming/docs idioms>` | `<needed>` | `<needed>` | `<test ids>` | Not started | |
| Regression: <bug-id> | `<related upstream behavior>` | `<expected behavior>` | `<needed>` | `<needed>` | `<test ids>` | Not started | |

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
7. Prefer delta-based assertions for memory/free counters in runtime tests when valid call-surface growth can change absolute totals.

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
- Host-language analyze/test clean on generated bindings

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

## Fixture Strategy (Recommended)
- Keep one rich end-to-end fixture that mixes features and exercises lifecycle/memory behavior.
- Keep focused fixtures for deterministic golden coverage of specific type categories.
- Add regression fixtures for every bug fix before implementing the fix.
- Use relative/delta assertions for allocation/free counters to keep tests stable as valid feature coverage expands.

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

## Immediate Next Steps (Template)
1. Create workspace skeleton (`<project_prefix>_bindgen`, `<project_prefix>_runtime`, `<project_prefix>_testing`).
2. Implement CLI parse and no-op `generate` with tests.
3. Set up host-language `binding_tests` package/project and first runtime smoke test.
4. Implement minimal top-level function generation.
5. Add and enforce CI `build` + `test-bindings` job split.
6. Replace this section in the language instance plan with current next steps once Phase 1 is complete.
