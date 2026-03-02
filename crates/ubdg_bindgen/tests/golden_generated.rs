use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn generate(source: &Path, out_dir: &Path, config: Option<&Path>) {
    let mut args = vec![
        "uniffi-bindgen-dart".to_string(),
        "generate".to_string(),
        source.display().to_string(),
        "--out-dir".to_string(),
        out_dir.display().to_string(),
    ];
    if let Some(config) = config {
        args.push("--config".to_string());
        args.push(config.display().to_string());
    }

    ubdg_bindgen::run(args).expect("generate bindings");
}

fn generate_library(source: &Path, crate_name: &str, out_dir: &Path) {
    let args = vec![
        "uniffi-bindgen-dart".to_string(),
        "generate".to_string(),
        source.display().to_string(),
        "--library".to_string(),
        "--crate".to_string(),
        crate_name.to_string(),
        "--out-dir".to_string(),
        out_dir.display().to_string(),
    ];
    ubdg_bindgen::run(args).expect("generate bindings (library mode)");
}

fn assert_matches_expected(actual: &Path, expected: &Path) {
    let actual_src = std::fs::read_to_string(actual).expect("read actual");
    let expected_src = std::fs::read_to_string(expected).expect("read expected");
    assert_eq!(
        actual_src,
        expected_src,
        "generated output diverged from golden file: {}",
        expected.display()
    );
}

#[test]
fn golden_simple_fns() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/simple-fns/src/simple-fns.udl");
    let config = root.join("fixtures/simple-fns/uniffi.toml");
    let expected = root.join("fixtures/simple-fns/expected/simple_fns.dart");

    generate(&source, &out_dir, Some(&config));
    assert_matches_expected(&out_dir.join("simple_fns.dart"), &expected);
}

#[test]
fn golden_compound_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/compound-demo/src/compound-demo.udl");
    let expected = root.join("fixtures/compound-demo/expected/compound_demo.dart");

    generate(&source, &out_dir, None);
    assert_matches_expected(&out_dir.join("compound_demo.dart"), &expected);
}

#[test]
fn golden_model_types_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/model-types-demo/src/model-types-demo.udl");
    let expected = root.join("fixtures/model-types-demo/expected/model_types_demo.dart");

    generate(&source, &out_dir, None);
    assert_matches_expected(&out_dir.join("model_types_demo.dart"), &expected);
}

#[test]
fn golden_futures_stress() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/futures-stress/src/futures-stress.udl");
    let expected = root.join("fixtures/futures-stress/expected/futures_stress.dart");

    generate(&source, &out_dir, None);
    assert_matches_expected(&out_dir.join("futures_stress.dart"), &expected);
}

#[test]
fn golden_custom_types_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/custom-types-demo/src/custom-types-demo.udl");
    let expected = root.join("fixtures/custom-types-demo/expected/custom_types_demo.dart");

    generate(&source, &out_dir, None);
    assert_matches_expected(&out_dir.join("custom_types_demo.dart"), &expected);
}

#[test]
fn golden_rename_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/rename-demo/src/rename-demo.udl");
    let config = root.join("fixtures/rename-demo/uniffi.toml");
    let expected = root.join("fixtures/rename-demo/expected/rename_demo.dart");

    generate(&source, &out_dir, Some(&config));
    assert_matches_expected(&out_dir.join("rename_demo.dart"), &expected);
}

#[test]
fn golden_ext_types_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/ext-types-demo/src/ext-types-demo.udl");
    let config = root.join("fixtures/ext-types-demo/uniffi.toml");
    let expected = root.join("fixtures/ext-types-demo/expected/ext_types_demo.dart");

    generate(&source, &out_dir, Some(&config));
    assert_matches_expected(&out_dir.join("ext_types_demo.dart"), &expected);
}

#[test]
fn golden_docstrings_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/docstrings-demo/src/docstrings-demo.udl");
    let expected = root.join("fixtures/docstrings-demo/expected/docstrings_demo.dart");

    generate(&source, &out_dir, None);
    assert_matches_expected(&out_dir.join("docstrings_demo.dart"), &expected);
}

#[test]
fn golden_regression_custom_shadow_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/regressions/custom-shadow-demo/src/custom-shadow-demo.udl");
    let expected =
        root.join("fixtures/regressions/custom-shadow-demo/expected/custom_shadow_demo.dart");

    generate(&source, &out_dir, None);
    assert_matches_expected(&out_dir.join("custom_shadow_demo.dart"), &expected);
}

#[test]
fn golden_regression_async_object_lift_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source =
        root.join("fixtures/regressions/async-object-lift-demo/src/async-object-lift-demo.udl");
    let config = root.join("fixtures/regressions/async-object-lift-demo/uniffi.toml");
    let expected = root
        .join("fixtures/regressions/async-object-lift-demo/expected/async_object_lift_demo.dart");

    generate(&source, &out_dir, Some(&config));
    assert_matches_expected(&out_dir.join("async_object_lift_demo.dart"), &expected);
}

#[test]
fn golden_regression_callback_custom_async_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root
        .join("fixtures/regressions/callback-custom-async-demo/src/callback-custom-async-demo.udl");
    let expected = root.join(
        "fixtures/regressions/callback-custom-async-demo/expected/callback_custom_async_demo.dart",
    );

    generate(&source, &out_dir, None);
    assert_matches_expected(&out_dir.join("callback_custom_async_demo.dart"), &expected);
}

#[test]
fn golden_keywords_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/keywords-demo/src/keywords-demo.udl");
    let expected = root.join("fixtures/keywords-demo/expected/keywords_demo.dart");

    generate(&source, &out_dir, None);
    assert_matches_expected(&out_dir.join("keywords_demo.dart"), &expected);
}

#[test]
fn golden_type_limits_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/type-limits-demo/src/type-limits-demo.udl");
    let expected = root.join("fixtures/type-limits-demo/expected/type_limits_demo.dart");

    generate(&source, &out_dir, None);
    assert_matches_expected(&out_dir.join("type_limits_demo.dart"), &expected);
}

#[test]
fn golden_coverall_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/coverall-demo/src/coverall-demo.udl");
    let expected = root.join("fixtures/coverall-demo/expected/coverall_demo.dart");

    generate(&source, &out_dir, None);
    assert_matches_expected(&out_dir.join("coverall_demo.dart"), &expected);
}

#[test]
fn golden_error_types_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/error-types-demo/src/error-types-demo.udl");
    let config = root.join("fixtures/error-types-demo/uniffi.toml");
    let expected = root.join("fixtures/error-types-demo/expected/error_types_demo.dart");

    generate(&source, &out_dir, Some(&config));
    assert_matches_expected(&out_dir.join("error_types_demo.dart"), &expected);
}

#[test]
fn golden_non_exhaustive_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/non-exhaustive-demo/src/non-exhaustive-demo.udl");
    let expected = root.join("fixtures/non-exhaustive-demo/expected/non_exhaustive_demo.dart");

    generate(&source, &out_dir, None);
    assert_matches_expected(&out_dir.join("non_exhaustive_demo.dart"), &expected);
}

#[test]
fn golden_trait_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/trait-demo/src/trait-demo.udl");
    let expected = root.join("fixtures/trait-demo/expected/trait_demo.dart");

    generate(&source, &out_dir, None);
    assert_matches_expected(&out_dir.join("trait_demo.dart"), &expected);
}

#[test]
fn golden_regression_defaults_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/regressions/defaults-demo/src/defaults-demo.udl");
    let expected = root.join("fixtures/regressions/defaults-demo/expected/defaults_demo.dart");

    generate(&source, &out_dir, None);
    assert_matches_expected(&out_dir.join("defaults_demo.dart"), &expected);
}

#[test]
fn golden_regression_forward_refs_demo() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let source = root.join("fixtures/regressions/forward-refs-demo/src/forward-refs-demo.udl");
    let expected =
        root.join("fixtures/regressions/forward-refs-demo/expected/forward_refs_demo.dart");

    generate(&source, &out_dir, None);
    assert_matches_expected(&out_dir.join("forward_refs_demo.dart"), &expected);
}

// ── Library-mode golden tests ──────────────────────────────────────────────
// These require a compiled cdylib. They are gated on env vars that point to
// the built library path. Regular `cargo test` skips them; CI and
// test_bindings.sh set the vars after building the fixtures.

#[test]
fn golden_record_enum_methods_library() {
    let lib_path = match std::env::var("UBDG_RECORD_ENUM_METHODS_LIB") {
        Ok(p) => {
            let path = PathBuf::from(&p);
            if path.is_absolute() {
                path
            } else {
                repo_root().join(path)
            }
        }
        Err(_) => {
            eprintln!("UBDG_RECORD_ENUM_METHODS_LIB not set; skipping library-mode golden test");
            return;
        }
    };

    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let expected =
        root.join("fixtures/record-enum-methods/expected/record_enum_methods_library.dart");

    generate_library(&lib_path, "uniffi_record_enum_methods", &out_dir);
    assert_matches_expected(&out_dir.join("record_enum_methods.dart"), &expected);
}

#[test]
fn golden_library_mode_demo() {
    let lib_path = match std::env::var("UBDG_LIBRARY_MODE_DEMO_LIB") {
        Ok(p) => {
            let path = PathBuf::from(&p);
            if path.is_absolute() {
                path
            } else {
                repo_root().join(path)
            }
        }
        Err(_) => {
            eprintln!("UBDG_LIBRARY_MODE_DEMO_LIB not set; skipping library-mode golden test");
            return;
        }
    };

    let root = repo_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("out");

    let expected = root.join("fixtures/library-mode-demo/expected/library_mode_demo.dart");

    generate_library(&lib_path, "uniffi_library_mode_demo", &out_dir);
    assert_matches_expected(&out_dir.join("library_mode_demo.dart"), &expected);
}
