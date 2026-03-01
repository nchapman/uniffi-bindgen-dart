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

fn assert_matches_expected(actual: &Path, expected: &Path) {
    let actual_src = std::fs::read_to_string(actual).expect("read actual");
    let expected_src = std::fs::read_to_string(expected).expect("read expected");
    assert_eq!(
        actual_src, expected_src,
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
