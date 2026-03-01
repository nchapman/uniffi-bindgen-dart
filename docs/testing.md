# Testing

## Workspace tests

```bash
cargo test --workspace
```

## Strict lint gate

```bash
cargo clippy --all-targets -- -D warnings
```

## Formatting

```bash
cargo fmt --check
```

## Binding tests

```bash
./scripts/test_bindings.sh
```

`./scripts/test_bindings.sh` performs:
- workspace Rust tests
- Dart binding generation
- native fixture build (`cdylib` for FFI runtime tests)
- Dart analyze/test when the Dart SDK is available
