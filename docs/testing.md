# Testing

## Workspace tests

```bash
cargo test --workspace
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
