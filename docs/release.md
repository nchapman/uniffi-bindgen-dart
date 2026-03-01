# Release Process

## Versioning Policy

This project follows [Semantic Versioning](https://semver.org/):

- **Major**: Breaking changes to generated Dart API surface or CLI interface
- **Minor**: New UDL features, new config options, non-breaking generator improvements
- **Patch**: Bug fixes, golden test corrections, documentation updates

The workspace version in the root `Cargo.toml` is the single source of truth.
All crates (`ubdg_bindgen`, `ubdg_runtime`, `ubdg_testing`) inherit it via `version.workspace = true`.

## Pre-release Checklist

1. Ensure CI is green: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
2. Run the full binding test suite: `./scripts/test_bindings.sh`
3. Verify golden tests pass (they run as part of `cargo test`)
4. Review `git log` since the last release for any missed changelog entries

## Version Bump

Update the version in one place:

```toml
# Cargo.toml (workspace root)
[workspace.package]
version = "X.Y.Z"
```

All crates pick up the new version automatically.

## Changelog Update

1. Move items from `## [Unreleased]` into a new `## [X.Y.Z] - YYYY-MM-DD` section
2. Add a fresh empty `## [Unreleased]` section at the top
3. Group entries under Added / Changed / Fixed as appropriate

## Publish

```bash
# Publish crates in dependency order
cargo publish -p ubdg_runtime
cargo publish -p ubdg_bindgen

# Tag the release
git tag vX.Y.Z
git push origin vX.Y.Z
```

Wait for each `cargo publish` to propagate before publishing dependent crates.

## Post-release

- Create a GitHub release from the tag with the changelog section as the body
- Bump the workspace version to the next dev pre-release if desired (e.g., `0.2.0-dev`)
