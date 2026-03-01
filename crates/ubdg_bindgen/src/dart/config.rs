use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::GenerateArgs;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DartBindingsConfig {
    pub module_name: Option<String>,
    pub ffi_class_name: Option<String>,
    pub library_name: Option<String>,
    pub dart_format: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RootConfig {
    #[serde(default)]
    bindings: BindingsConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct BindingsConfig {
    dart: DartBindingsConfig,
}

pub fn load(args: &GenerateArgs) -> Result<DartBindingsConfig> {
    let Some(config_path) = resolve_config_path(args) else {
        return Ok(DartBindingsConfig::default());
    };

    let src = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read config file: {}", config_path.display()))?;
    let parsed: RootConfig = toml::from_str(&src)
        .with_context(|| format!("failed to parse config file: {}", config_path.display()))?;

    Ok(parsed.bindings.dart)
}

fn resolve_config_path(args: &GenerateArgs) -> Option<PathBuf> {
    if let Some(path) = &args.config {
        return Some(path.clone());
    }
    find_uniffi_toml(&args.source)
}

fn find_uniffi_toml(source: &Path) -> Option<PathBuf> {
    source.canonicalize().ok().and_then(|path| {
        let mut cursor = if path.is_dir() {
            path
        } else {
            path.parent()?.to_path_buf()
        };

        loop {
            let candidate = cursor.join("uniffi.toml");
            if candidate.exists() {
                return Some(candidate);
            }
            if !cursor.pop() {
                return None;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn loads_explicit_config_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_path = temp.path().join("custom.toml");
        fs::write(
            &config_path,
            r#"
[bindings.dart]
module_name = "my_module"
ffi_class_name = "MyFfi"
library_name = "my_lib"
dart_format = false
"#,
        )
        .expect("write config");

        let args = GenerateArgs {
            source: temp.path().join("demo.udl"),
            out_dir: temp.path().join("out"),
            library: false,
            config: Some(config_path),
            crate_name: None,
            no_format: false,
        };

        let cfg = load(&args).expect("load config");
        assert_eq!(cfg.module_name.as_deref(), Some("my_module"));
        assert_eq!(cfg.ffi_class_name.as_deref(), Some("MyFfi"));
        assert_eq!(cfg.library_name.as_deref(), Some("my_lib"));
        assert_eq!(cfg.dart_format, Some(false));
    }

    #[test]
    fn auto_discovers_uniffi_toml_from_source_ancestors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("fixture");
        let src_dir = root.join("src");
        fs::create_dir_all(&src_dir).expect("create src dir");
        fs::write(
            root.join("uniffi.toml"),
            r#"
[bindings.dart]
module_name = "discovered"
"#,
        )
        .expect("write config");
        fs::write(src_dir.join("demo.udl"), "namespace demo {}").expect("write udl");

        let args = GenerateArgs {
            source: src_dir.join("demo.udl"),
            out_dir: temp.path().join("out"),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        let cfg = load(&args).expect("load config");
        assert_eq!(cfg.module_name.as_deref(), Some("discovered"));
    }
}
