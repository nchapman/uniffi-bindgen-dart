use std::collections::HashMap;
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
    pub rename: HashMap<String, String>,
    pub exclude: Vec<String>,
    pub external_packages: HashMap<String, String>,
    pub custom_types: HashMap<String, CustomTypeConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CustomTypeConfig {
    /// Dart type to use in signatures (default: builtin type).
    pub type_name: Option<String>,
    /// Extra imports needed for the custom type.
    pub imports: Option<Vec<String>>,
    /// Template converting builtin → custom: `"Uri.parse({})"`.
    pub lift: Option<String>,
    /// Template converting custom → builtin: `"{}.toString()"`.
    pub lower: Option<String>,
}

impl CustomTypeConfig {
    /// Apply the lift template (builtin → custom). Identity when unset.
    pub fn lift_expr(&self, builtin_expr: &str) -> String {
        match &self.lift {
            Some(template) => template.replace("{}", builtin_expr),
            None => builtin_expr.to_string(),
        }
    }

    /// Apply the lower template (custom → builtin). Identity when unset.
    pub fn lower_expr(&self, custom_expr: &str) -> String {
        match &self.lower {
            Some(template) => template.replace("{}", custom_expr),
            None => custom_expr.to_string(),
        }
    }
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
rename = { old_name = "new_name", "Counter.value" = "reading" }
exclude = ["skip_me", "Counter.hidden"]
external_packages = { other_crate = "package:other_bindings/other_bindings.dart" }
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
        assert_eq!(
            cfg.rename.get("old_name").map(String::as_str),
            Some("new_name")
        );
        assert_eq!(
            cfg.rename.get("Counter.value").map(String::as_str),
            Some("reading")
        );
        assert!(cfg.exclude.iter().any(|e| e == "skip_me"));
        assert!(cfg.exclude.iter().any(|e| e == "Counter.hidden"));
        assert_eq!(
            cfg.external_packages.get("other_crate").map(String::as_str),
            Some("package:other_bindings/other_bindings.dart")
        );
    }

    #[test]
    fn parses_custom_types_config() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_path = temp.path().join("uniffi.toml");
        fs::write(
            &config_path,
            r#"
[bindings.dart.custom_types.Url]
type_name = "Uri"
imports = ["dart:core"]
lift = "Uri.parse({})"
lower = "{}.toString()"

[bindings.dart.custom_types.Count]
lower = "{}.toInt()"
lift = "Count({})"
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
        let url_cfg = cfg.custom_types.get("Url").expect("Url entry");
        assert_eq!(url_cfg.type_name.as_deref(), Some("Uri"));
        assert_eq!(
            url_cfg
                .imports
                .as_ref()
                .map(|v| v.iter().map(String::as_str).collect::<Vec<_>>()),
            Some(vec!["dart:core"])
        );
        assert_eq!(url_cfg.lift_expr("raw"), "Uri.parse(raw)");
        assert_eq!(url_cfg.lower_expr("value"), "value.toString()");

        let count_cfg = cfg.custom_types.get("Count").expect("Count entry");
        assert_eq!(count_cfg.type_name, None);
        assert_eq!(count_cfg.lift_expr("raw"), "Count(raw)");
        assert_eq!(count_cfg.lower_expr("value"), "value.toInt()");

        // No entry → not present
        assert!(!cfg.custom_types.contains_key("Label"));
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
