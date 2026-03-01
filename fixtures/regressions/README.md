# Regression Fixtures

Add a reproducer fixture here for every bug fix before applying the code change.

Current fixtures:
- `custom-shadow-demo`: guards nested custom JSON encode/decode closures against local-variable shadowing in generated Dart.
- `async-object-lift-demo`: guards async object return lifting semantics for local objects (`._(this, handle)`) vs external objects (`*FfiCodec.lift(handle)`).
- `callback-custom-async-demo`: guards async callback-interface methods with custom alias return types.
