use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::GenerateArgs;

pub mod callback_interface;
pub mod compounds;
pub mod config;
pub mod custom;
pub mod enum_;
pub mod error;
pub mod object;
pub mod oracle;
pub mod primitives;
pub mod record;

pub fn generate_bindings(args: &GenerateArgs) -> Result<()> {
    let namespace = namespace_from_source(&args.source)?;
    let cfg = config::load(args)?;
    let functions = parse_udl_functions(&args.source)?;

    let module_name = cfg
        .module_name
        .clone()
        .unwrap_or_else(|| dart_identifier(&namespace));
    let ffi_class_name = cfg
        .ffi_class_name
        .clone()
        .unwrap_or_else(|| format!("{}Ffi", to_upper_camel(&namespace)));
    let library_name = cfg
        .library_name
        .clone()
        .or_else(|| args.crate_name.clone())
        .unwrap_or_else(|| format!("uniffi_{}", namespace.replace('-', "_")));

    fs::create_dir_all(&args.out_dir)?;

    let output_file = args.out_dir.join(format!("{namespace}.dart"));
    let content = render_dart_scaffold(&module_name, &ffi_class_name, &library_name, &functions);
    fs::write(&output_file, content).with_context(|| {
        format!(
            "failed to write generated dart bindings: {}",
            output_file.display()
        )
    })?;

    Ok(())
}

fn namespace_from_source(source: &Path) -> Result<String> {
    if let Some(namespace) = extract_namespace_from_udl(source) {
        return Ok(namespace);
    }

    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("source path must have a valid UTF-8 file stem"))?;
    if stem.trim().is_empty() {
        bail!("source path stem cannot be empty");
    }
    Ok(stem)
}

fn extract_namespace_from_udl(source: &Path) -> Option<String> {
    if source.extension().and_then(|e| e.to_str()) != Some("udl") {
        return None;
    }

    let udl = fs::read_to_string(source).ok()?;
    let marker = "namespace";
    let start = udl.find(marker)?;
    let mut chars = udl[start + marker.len()..].chars().peekable();

    while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
        chars.next();
    }

    let mut ns = String::new();
    while matches!(chars.peek(), Some(c) if c.is_ascii_alphanumeric() || *c == '_') {
        ns.push(chars.next()?);
    }

    if ns.is_empty() {
        None
    } else {
        Some(ns)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlFunction {
    name: String,
    return_type: String,
    args: Vec<UdlArg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlArg {
    name: String,
    type_name: String,
}

impl UdlFunction {
    fn uses_bytes(&self) -> bool {
        udl_type_uses_bytes(&self.return_type)
            || self.args.iter().any(|a| udl_type_uses_bytes(&a.type_name))
    }
}

fn parse_udl_functions(source: &Path) -> Result<Vec<UdlFunction>> {
    if source.extension().and_then(|e| e.to_str()) != Some("udl") {
        return Ok(Vec::new());
    }

    let udl = fs::read_to_string(source)
        .with_context(|| format!("failed to read UDL source: {}", source.display()))?;
    Ok(parse_udl_functions_from_str(&udl))
}

fn parse_udl_functions_from_str(udl: &str) -> Vec<UdlFunction> {
    let body = extract_namespace_body(udl).unwrap_or(udl);
    let mut output = Vec::new();
    for statement in body.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let statement = strip_leading_attributes(statement);
        if statement.is_empty() || !statement.contains('(') {
            continue;
        }

        let Some(open_paren) = statement.find('(') else {
            continue;
        };
        let Some(close_paren) = statement.rfind(')') else {
            continue;
        };
        if close_paren < open_paren {
            continue;
        }

        let before = statement[..open_paren].trim();
        let args_part = statement[open_paren + 1..close_paren].trim();

        let mut head_parts = before.split_whitespace().collect::<Vec<_>>();
        if head_parts.len() < 2 {
            continue;
        }

        let name = head_parts.pop().unwrap_or_default().to_string();
        let return_type = head_parts.join(" ");
        if name.is_empty() || return_type.is_empty() {
            continue;
        }

        let args = if args_part.is_empty() {
            Vec::new()
        } else {
            split_top_level_generic_args(args_part)
                .into_iter()
                .filter_map(|arg| parse_udl_arg(arg.trim()))
                .collect::<Vec<_>>()
        };

        output.push(UdlFunction {
            name,
            return_type,
            args,
        });
    }
    output
}

fn extract_namespace_body(udl: &str) -> Option<&str> {
    let namespace_pos = udl.find("namespace")?;
    let brace_start_rel = udl[namespace_pos..].find('{')?;
    let brace_start = namespace_pos + brace_start_rel;
    let brace_end = udl.rfind('}')?;
    if brace_end <= brace_start {
        return None;
    }
    Some(&udl[brace_start + 1..brace_end])
}

fn strip_leading_attributes(statement: &str) -> &str {
    let mut s = statement.trim_start();
    loop {
        if !s.starts_with('[') {
            break;
        }
        let Some(close_idx) = s.find(']') else {
            break;
        };
        s = s[close_idx + 1..].trim_start();
    }
    s
}

fn parse_udl_arg(arg: &str) -> Option<UdlArg> {
    if arg.is_empty() {
        return None;
    }
    let mut parts = arg.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 2 {
        return None;
    }
    let name = parts.pop()?.to_string();
    let type_name = parts.join(" ");
    Some(UdlArg { name, type_name })
}

fn render_dart_scaffold(
    module_name: &str,
    ffi_class_name: &str,
    library_name: &str,
    functions: &[UdlFunction],
) -> String {
    let needs_typed_data = functions.iter().any(UdlFunction::uses_bytes);
    let mut out = String::new();
    out.push_str("// Generated by uniffi-bindgen-dart. DO NOT EDIT.\n");
    out.push_str(&format!("library {module_name};\n\n"));
    out.push_str("import 'dart:ffi' as ffi;\n");
    if needs_typed_data {
        out.push_str("import 'dart:typed_data';\n");
    }
    out.push('\n');
    out.push_str(&format!(
        "class {ffi_class_name} {{\n  const {ffi_class_name}();\n\n"
    ));
    out.push_str(&format!(
        "  static const String libraryName = '{library_name}';\n\n"
    ));
    out.push_str("  ffi.DynamicLibrary open() {\n");
    out.push_str("    return ffi.DynamicLibrary.open(libraryName);\n");
    out.push_str("  }\n}\n");
    out.push_str(&render_function_stubs(functions));
    out
}

fn render_function_stubs(functions: &[UdlFunction]) -> String {
    if functions.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push('\n');
    for f in functions {
        let fn_name = to_lower_camel(&f.name);
        let return_type = map_udl_type_to_dart(&f.return_type);
        let args = f
            .args
            .iter()
            .map(|a| {
                format!(
                    "{} {}",
                    map_udl_type_to_dart(&a.type_name),
                    to_lower_camel(&a.name)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        out.push_str(&format!("{return_type} {fn_name}({args}) {{\n"));
        out.push_str("  throw UnimplementedError('TODO: bind to Rust FFI');\n");
        out.push_str("}\n\n");
    }
    out
}

fn map_udl_type_to_dart(udl_type: &str) -> String {
    let t = udl_type.trim();
    if let Some(inner) = t.strip_suffix('?') {
        return format!("{}?", map_udl_type_to_dart(inner));
    }
    if let Some(inner) = t
        .strip_prefix("sequence<")
        .and_then(|s| s.strip_suffix('>'))
    {
        return format!("List<{}>", map_udl_type_to_dart(inner));
    }
    if let Some(inner) = t.strip_prefix("record<").and_then(|s| s.strip_suffix('>')) {
        let args = split_top_level_generic_args(inner);
        if args.len() == 2 {
            return format!(
                "Map<{}, {}>",
                map_udl_type_to_dart(args[0]),
                map_udl_type_to_dart(args[1])
            );
        }
        return "Map<dynamic, dynamic>".to_string();
    }

    match t {
        "void" => "void".to_string(),
        "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" => "int".to_string(),
        "f32" | "f64" => "double".to_string(),
        "boolean" => "bool".to_string(),
        "string" => "String".to_string(),
        "bytes" => "Uint8List".to_string(),
        _ => t.to_string(),
    }
}

fn udl_type_uses_bytes(udl_type: &str) -> bool {
    let t = udl_type.trim();
    if t == "bytes" {
        return true;
    }
    if let Some(inner) = t.strip_suffix('?') {
        return udl_type_uses_bytes(inner);
    }
    if let Some(inner) = t
        .strip_prefix("sequence<")
        .and_then(|s| s.strip_suffix('>'))
    {
        return udl_type_uses_bytes(inner);
    }
    if let Some(inner) = t.strip_prefix("record<").and_then(|s| s.strip_suffix('>')) {
        return split_top_level_generic_args(inner)
            .into_iter()
            .any(udl_type_uses_bytes);
    }
    false
}

fn split_top_level_generic_args(input: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, c) in input.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                out.push(input[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < input.len() {
        out.push(input[start..].trim());
    }
    out.into_iter().filter(|s| !s.is_empty()).collect()
}

fn to_upper_camel(input: &str) -> String {
    let mut out = String::new();
    for segment in input.split(|c: char| !c.is_ascii_alphanumeric()) {
        if segment.is_empty() {
            continue;
        }
        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            for c in chars {
                out.push(c.to_ascii_lowercase());
            }
        }
    }
    if out.is_empty() {
        "Uniffi".to_string()
    } else {
        out
    }
}

fn dart_identifier(input: &str) -> String {
    let parts = input
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(|p| p.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        "uniffi_bindings".to_string()
    } else {
        parts.join("_")
    }
}

fn to_lower_camel(input: &str) -> String {
    let mut out = String::new();
    for (i, segment) in input
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .enumerate()
    {
        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            if i == 0 {
                out.push(first.to_ascii_lowercase());
            } else {
                out.push(first.to_ascii_uppercase());
            }
            for c in chars {
                out.push(c.to_ascii_lowercase());
            }
        }
    }
    if out.is_empty() {
        "value".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn generates_dart_file_with_defaults() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("simple-fns.udl");
        let out_dir = temp.path().join("out");
        fs::write(&source, "namespace simple_fns {}").expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");

        let generated = out_dir.join("simple_fns.dart");
        assert!(generated.exists());
        let content = fs::read_to_string(generated).expect("read generated file");
        assert!(content.contains("library simple_fns;"));
        assert!(content.contains("class SimpleFnsFfi {"));
        assert!(content.contains("libraryName = 'uniffi_simple_fns';"));
    }

    #[test]
    fn uses_udl_namespace_when_available() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("filename_does_not_match.udl");
        let out_dir = temp.path().join("out");
        fs::write(&source, "namespace from_udl { u32 add(u32 a, u32 b); };").expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("from_udl.dart")).expect("read generated");
        assert!(content.contains("library from_udl;"));
    }

    #[test]
    fn applies_config_overrides() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("demo.udl");
        let out_dir = temp.path().join("out");
        let config = temp.path().join("uniffi.toml");
        fs::write(&source, "namespace demo {}").expect("write source");
        fs::write(
            &config,
            r#"
[bindings.dart]
module_name = "demo_bindings"
ffi_class_name = "DemoInterop"
library_name = "demoffi"
"#,
        )
        .expect("write config");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: Some(config),
            crate_name: Some("crate_override".to_string()),
            no_format: false,
        };

        generate_bindings(&args).expect("generate");

        let content = fs::read_to_string(out_dir.join("demo.dart")).expect("read generated file");
        assert!(content.contains("library demo_bindings;"));
        assert!(content.contains("class DemoInterop {"));
        assert!(content.contains("libraryName = 'demoffi';"));
    }

    #[test]
    fn renders_top_level_function_stubs_from_udl() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("demo.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace demo {
  u32 add_numbers(u32 left_value, u32 right_value);
  boolean is_even(i32 input_value);
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("demo.dart")).expect("read generated file");
        assert!(content.contains("int addNumbers(int leftValue, int rightValue) {"));
        assert!(content.contains("bool isEven(int inputValue) {"));
    }

    #[test]
    fn adds_typed_data_import_for_bytes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("bytes_demo.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace bytes_demo {
  bytes echo_bytes(bytes input);
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("bytes_demo.dart")).expect("read generated");
        assert!(content.contains("import 'dart:typed_data';"));
        assert!(content.contains("Uint8List echoBytes(Uint8List input) {"));
    }

    #[test]
    fn renders_compound_udl_types_to_idiomatic_dart() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("compound_demo.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace compound_demo {
  sequence<u32> listify(sequence<u32> values);
  record<string, u64> counts(record<string, u64> items);
  string? maybe_name(string? value);
  sequence<bytes> chunk(sequence<bytes> input);
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content =
            fs::read_to_string(out_dir.join("compound_demo.dart")).expect("read generated file");
        assert!(content.contains("List<int> listify(List<int> values) {"));
        assert!(content.contains("Map<String, int> counts(Map<String, int> items) {"));
        assert!(content.contains("String? maybeName(String? value) {"));
        assert!(content.contains("List<Uint8List> chunk(List<Uint8List> input) {"));
        assert!(content.contains("import 'dart:typed_data';"));
    }

    #[test]
    fn parses_udl_functions_from_source_text() {
        let parsed = parse_udl_functions_from_str(
            r#"
namespace demo {
  u32 add(u32 left, u32 right);
  string hello_world();
};
"#,
        );
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "add");
        assert_eq!(parsed[0].return_type, "u32");
        assert_eq!(parsed[0].args.len(), 2);
        assert_eq!(parsed[1].name, "hello_world");
        assert_eq!(parsed[1].args.len(), 0);
    }

    #[test]
    fn parses_functions_with_attributes() {
        let parsed = parse_udl_functions_from_str(
            r#"
namespace demo {
  [Throws=ExampleError]
  string risky_call(u32 count);
};
"#,
        );
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "risky_call");
        assert_eq!(parsed[0].return_type, "string");
        assert_eq!(parsed[0].args.len(), 1);
    }
}
