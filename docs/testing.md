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
- workspace Rust tests (UDL golden tests run unconditionally)
- native fixture build (`cdylib` for FFI runtime tests)
- Dart binding generation for all fixtures (UDL and library mode)
- library-mode golden tests (run with env vars pointing to built cdylibs)
- Dart analyze/test when the Dart SDK is available
