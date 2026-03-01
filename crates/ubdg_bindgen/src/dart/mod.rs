use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use uniffi_bindgen::interface::{AsType, ComponentInterface, Type};

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
    let metadata = parse_udl_metadata(&args.source, args.crate_name.as_deref())?;

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
    let content = render_dart_scaffold(
        &module_name,
        &ffi_class_name,
        &library_name,
        &metadata.functions,
        &metadata.objects,
        &metadata.callback_interfaces,
        &metadata.records,
        &metadata.enums,
    );
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
    is_async: bool,
    return_type: Option<Type>,
    throws_type: Option<Type>,
    args: Vec<UdlArg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlArg {
    name: String,
    type_: Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlObject {
    name: String,
    constructors: Vec<UdlObjectConstructor>,
    methods: Vec<UdlObjectMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlObjectConstructor {
    name: String,
    is_async: bool,
    args: Vec<UdlArg>,
    throws_type: Option<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlObjectMethod {
    name: String,
    is_async: bool,
    return_type: Option<Type>,
    throws_type: Option<Type>,
    args: Vec<UdlArg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlCallbackInterface {
    name: String,
    methods: Vec<UdlCallbackMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlCallbackMethod {
    name: String,
    is_async: bool,
    return_type: Option<Type>,
    throws_type: Option<Type>,
    args: Vec<UdlArg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlRecord {
    name: String,
    fields: Vec<UdlArg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlEnum {
    name: String,
    is_error: bool,
    variants: Vec<UdlEnumVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlEnumVariant {
    name: String,
    fields: Vec<UdlArg>,
}

impl UdlFunction {
    fn uses_bytes(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(uniffi_type_uses_bytes)
            || self.args.iter().any(|a| uniffi_type_uses_bytes(&a.type_))
    }

    fn uses_runtime_string(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(is_runtime_string_like_type)
            || self
                .args
                .iter()
                .any(|a| is_runtime_string_like_type(&a.type_))
    }

    fn returns_runtime_string(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(is_runtime_string_like_type)
    }

    fn uses_runtime_bytes(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(is_runtime_bytes_like_type)
            || self
                .args
                .iter()
                .any(|a| is_runtime_bytes_like_type(&a.type_))
    }

    fn returns_runtime_bytes(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(is_runtime_bytes_like_type)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct UdlMetadata {
    functions: Vec<UdlFunction>,
    objects: Vec<UdlObject>,
    callback_interfaces: Vec<UdlCallbackInterface>,
    records: Vec<UdlRecord>,
    enums: Vec<UdlEnum>,
}

fn parse_udl_metadata(source: &Path, crate_name: Option<&str>) -> Result<UdlMetadata> {
    if source.extension().and_then(|e| e.to_str()) != Some("udl") {
        return Ok(UdlMetadata::default());
    }

    let udl = fs::read_to_string(source)
        .with_context(|| format!("failed to read UDL source: {}", source.display()))?;
    let module_path = crate_name.unwrap_or("crate_name");
    let ci = ComponentInterface::from_webidl(&udl, module_path)
        .with_context(|| format!("failed to parse UDL: {}", source.display()))?;

    let functions = ci
        .function_definitions()
        .iter()
        .map(|f| UdlFunction {
            name: f.name().to_string(),
            is_async: f.is_async(),
            return_type: f.return_type().cloned(),
            throws_type: f.throws_type().cloned(),
            args: f
                .arguments()
                .into_iter()
                .map(|a| UdlArg {
                    name: a.name().to_string(),
                    type_: a.as_type(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let records = ci
        .record_definitions()
        .map(|record| UdlRecord {
            name: record.name().to_string(),
            fields: record
                .fields()
                .iter()
                .map(|field| UdlArg {
                    name: field.name().to_string(),
                    type_: field.as_type(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let enums = ci
        .enum_definitions()
        .map(|enum_| UdlEnum {
            name: enum_.name().to_string(),
            is_error: ci.is_name_used_as_error(enum_.name()),
            variants: enum_
                .variants()
                .iter()
                .map(|variant| UdlEnumVariant {
                    name: variant.name().to_string(),
                    fields: variant
                        .fields()
                        .iter()
                        .map(|field| UdlArg {
                            name: field.name().to_string(),
                            type_: field.as_type(),
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let objects = ci
        .object_definitions()
        .iter()
        .map(|obj| UdlObject {
            name: obj.name().to_string(),
            constructors: obj
                .constructors()
                .into_iter()
                .map(|ctor| UdlObjectConstructor {
                    name: ctor.name().to_string(),
                    is_async: ctor.is_async(),
                    args: ctor
                        .arguments()
                        .into_iter()
                        .map(|a| UdlArg {
                            name: a.name().to_string(),
                            type_: a.as_type(),
                        })
                        .collect(),
                    throws_type: ctor.throws_type().cloned(),
                })
                .collect(),
            methods: obj
                .methods()
                .into_iter()
                .map(|m| UdlObjectMethod {
                    name: m.name().to_string(),
                    is_async: m.is_async(),
                    return_type: m.return_type().cloned(),
                    throws_type: m.throws_type().cloned(),
                    args: m
                        .arguments()
                        .into_iter()
                        .map(|a| UdlArg {
                            name: a.name().to_string(),
                            type_: a.as_type(),
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let callback_interfaces = ci
        .callback_interface_definitions()
        .iter()
        .map(|cb| UdlCallbackInterface {
            name: cb.name().to_string(),
            methods: cb
                .methods()
                .into_iter()
                .map(|m| UdlCallbackMethod {
                    name: m.name().to_string(),
                    is_async: m.is_async(),
                    return_type: m.return_type().cloned(),
                    throws_type: m.throws_type().cloned(),
                    args: m
                        .arguments()
                        .into_iter()
                        .map(|a| UdlArg {
                            name: a.name().to_string(),
                            type_: a.as_type(),
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();

    Ok(UdlMetadata {
        functions,
        objects,
        callback_interfaces,
        records,
        enums,
    })
}

#[allow(clippy::too_many_arguments)]
fn render_dart_scaffold(
    module_name: &str,
    ffi_class_name: &str,
    library_name: &str,
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let needs_callback_runtime =
        has_runtime_callback_support(functions, objects, callback_interfaces, records, enums);
    let needs_async_rust_future = has_runtime_async_rust_future_support(
        functions,
        objects,
        callback_interfaces,
        records,
        enums,
    );
    let needs_rust_call_status = needs_async_rust_future || needs_callback_runtime;
    let needs_typed_data = functions.iter().any(UdlFunction::uses_bytes)
        || objects.iter().any(|o| {
            o.methods.iter().any(|m| {
                m.return_type.as_ref().is_some_and(uniffi_type_uses_bytes)
                    || m.args.iter().any(|a| uniffi_type_uses_bytes(&a.type_))
            })
        });
    let needs_json_convert = !records.is_empty() || !enums.is_empty();
    let needs_ffi_helpers = needs_async_rust_future
        || needs_callback_runtime
        || functions.iter().any(|f| {
            is_runtime_ffi_compatible_function(f, records, enums)
                && (f.uses_runtime_string()
                    || f.uses_runtime_bytes()
                    || f.return_type
                        .as_ref()
                        .is_some_and(|t| is_runtime_record_or_enum_string_type(t, enums))
                    || f.args
                        .iter()
                        .any(|a| is_runtime_record_or_enum_string_type(&a.type_, enums)))
        })
        || !objects.is_empty();
    let needs_runtime_bytes = functions
        .iter()
        .any(|f| is_runtime_ffi_compatible_function(f, records, enums) && f.uses_runtime_bytes())
        || objects.iter().any(|o| {
            o.methods.iter().any(|m| {
                m.return_type
                    .as_ref()
                    .is_some_and(is_runtime_bytes_like_type)
                    || m.args.iter().any(|a| is_runtime_bytes_like_type(&a.type_))
            })
        });
    let needs_runtime_optional_bytes = functions.iter().any(|f| {
        is_runtime_ffi_compatible_function(f, records, enums)
            && (f
                .return_type
                .as_ref()
                .is_some_and(is_runtime_optional_bytes_type)
                || f.args
                    .iter()
                    .any(|a| is_runtime_optional_bytes_type(&a.type_)))
    }) || objects.iter().any(|o| {
        o.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_optional_bytes_type)
                || m.args
                    .iter()
                    .any(|a| is_runtime_optional_bytes_type(&a.type_))
        })
    });
    let needs_runtime_sequence_bytes = functions.iter().any(|f| {
        is_runtime_ffi_compatible_function(f, records, enums)
            && (f
                .return_type
                .as_ref()
                .is_some_and(is_runtime_sequence_bytes_type)
                || f.args
                    .iter()
                    .any(|a| is_runtime_sequence_bytes_type(&a.type_)))
    }) || objects.iter().any(|o| {
        o.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_sequence_bytes_type)
                || m.args
                    .iter()
                    .any(|a| is_runtime_sequence_bytes_type(&a.type_))
        })
    });

    let mut out = String::new();
    out.push_str("// Generated by uniffi-bindgen-dart. DO NOT EDIT.\n");
    out.push_str("// ignore_for_file: unused_element\n");
    out.push_str(&format!("library {module_name};\n\n"));
    if needs_async_rust_future {
        out.push_str("import 'dart:async';\n");
    }
    if needs_json_convert {
        out.push_str("import 'dart:convert';\n");
    }
    out.push_str("import 'dart:ffi' as ffi;\n");
    if needs_ffi_helpers {
        out.push_str("import 'package:ffi/ffi.dart';\n");
    }
    if needs_typed_data {
        out.push_str("import 'dart:typed_data';\n");
    }
    out.push('\n');
    if needs_runtime_bytes {
        out.push_str("final class _RustBuffer extends ffi.Struct {\n");
        out.push_str("  external ffi.Pointer<ffi.Uint8> data;\n\n");
        out.push_str("  @ffi.Uint64()\n");
        out.push_str("  external int len;\n");
        out.push_str("}\n\n");
    }
    if needs_runtime_optional_bytes {
        out.push_str("final class _RustBufferOpt extends ffi.Struct {\n");
        out.push_str("  @ffi.Uint8()\n");
        out.push_str("  external int isSome;\n\n");
        out.push_str("  external _RustBuffer value;\n");
        out.push_str("}\n\n");
    }
    if needs_runtime_sequence_bytes {
        out.push_str("final class _RustBufferVec extends ffi.Struct {\n");
        out.push_str("  external ffi.Pointer<_RustBuffer> data;\n\n");
        out.push_str("  @ffi.Uint64()\n");
        out.push_str("  external int len;\n");
        out.push_str("}\n\n");
    }
    if needs_rust_call_status {
        out.push_str("final class _RustCallStatus extends ffi.Struct {\n");
        out.push_str("  @ffi.Int8()\n");
        out.push_str("  external int code;\n\n");
        out.push_str("  external ffi.Pointer<Utf8> errorBuf;\n");
        out.push_str("}\n\n");
        out.push_str("const int _rustCallStatusSuccess = 0;\n");
        out.push_str("const int _rustCallStatusError = 1;\n");
        out.push_str("const int _rustCallStatusUnexpectedError = 2;\n");
        out.push_str("const int _rustCallStatusCancelled = 3;\n");
    }
    if needs_async_rust_future {
        out.push_str("const int _rustFuturePollReady = 0;\n");
        out.push_str("const int _rustFuturePollWake = 1;\n\n");
    }
    out.push_str(&render_data_models(records, enums));
    out.push_str(&render_callback_interfaces(callback_interfaces));
    out.push_str(&render_callback_bridges(
        functions,
        objects,
        callback_interfaces,
        records,
        enums,
    ));
    out.push_str(&format!(
        "class {ffi_class_name} {{\n  {ffi_class_name}({{ffi.DynamicLibrary? dynamicLibrary, String? libraryPath}})\n      : _dynamicLibrary = dynamicLibrary,\n        _libraryPath = libraryPath;\n\n"
    ));
    out.push_str("  final ffi.DynamicLibrary? _dynamicLibrary;\n");
    out.push_str("  final String? _libraryPath;\n\n");
    out.push_str(&format!(
        "  static const String libraryName = '{library_name}';\n\n"
    ));
    out.push_str("  ffi.DynamicLibrary open() {\n");
    out.push_str("    final provided = _dynamicLibrary;\n");
    out.push_str("    if (provided != null) {\n");
    out.push_str("      return provided;\n");
    out.push_str("    }\n");
    out.push_str("    return ffi.DynamicLibrary.open(_libraryPath ?? libraryName);\n");
    out.push_str("  }\n\n");
    out.push_str("  late final ffi.DynamicLibrary _lib = open();\n");
    out.push_str(&render_bound_methods(
        functions,
        objects,
        callback_interfaces,
        ffi_class_name,
        records,
        enums,
    ));
    out.push_str("}\n");
    out.push_str(&render_object_classes(
        objects,
        callback_interfaces,
        ffi_class_name,
        records,
        enums,
    ));
    out.push_str(&render_function_stubs(
        functions,
        objects,
        callback_interfaces,
        ffi_class_name,
        records,
        enums,
    ));
    out
}

fn render_data_models(records: &[UdlRecord], enums: &[UdlEnum]) -> String {
    let mut out = String::new();

    for record in records {
        let class_name = to_upper_camel(&record.name);
        out.push_str(&format!("class {class_name} {{\n"));
        out.push_str(&format!("  const {class_name}({{\n"));
        for field in &record.fields {
            let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
            out.push_str(&format!("    required this.{field_name},\n"));
        }
        out.push_str("  });\n\n");
        for field in &record.fields {
            let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
            out.push_str(&format!(
                "  final {} {field_name};\n",
                map_uniffi_type_to_dart(&field.type_)
            ));
        }
        out.push('\n');
        out.push_str("  Map<String, dynamic> toJson() {\n");
        out.push_str("    return {\n");
        for field in &record.fields {
            let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
            let expr = render_json_encode_expr(&format!("this.{field_name}"), &field.type_);
            out.push_str(&format!("      '{field_name}': {expr},\n"));
        }
        out.push_str("    };\n");
        out.push_str("  }\n\n");
        out.push_str(&format!(
            "  static {class_name} fromJson(Map<String, dynamic> json) {{\n"
        ));
        out.push_str(&format!("    return {class_name}(\n"));
        for field in &record.fields {
            let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
            let decode = render_json_decode_expr(&format!("json['{field_name}']"), &field.type_);
            out.push_str(&format!("      {field_name}: {decode},\n"));
        }
        out.push_str("    );\n");
        out.push_str("  }\n");
        if !record.fields.is_empty() {
            out.push('\n');
            out.push_str(&format!("  {class_name} copyWith({{\n"));
            for field in &record.fields {
                let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                let field_type = map_uniffi_type_to_dart(&field.type_);
                let copy_with_type = if field_type.ends_with('?') {
                    field_type
                } else {
                    format!("{field_type}?")
                };
                out.push_str(&format!("    {copy_with_type} {field_name},\n"));
            }
            out.push_str("  }) {\n");
            out.push_str(&format!("    return {class_name}(\n"));
            for field in &record.fields {
                let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                out.push_str(&format!(
                    "      {field_name}: {field_name} ?? this.{field_name},\n"
                ));
            }
            out.push_str("    );\n");
            out.push_str("  }\n");
        }
        out.push_str("}\n\n");
    }

    for enum_ in enums {
        let enum_name = to_upper_camel(&enum_.name);
        let has_data = enum_.variants.iter().any(|v| !v.fields.is_empty());
        if !has_data && !enum_.is_error {
            out.push_str(&format!("enum {enum_name} {{\n"));
            for variant in &enum_.variants {
                out.push_str(&format!(
                    "  {},\n",
                    safe_dart_identifier(&to_lower_camel(&variant.name))
                ));
            }
            out.push_str("}\n\n");
            continue;
        }

        out.push_str(&format!(
            "sealed class {enum_name} {{\n  const {enum_name}();\n}}\n\n"
        ));
        for variant in &enum_.variants {
            let variant_name = to_upper_camel(&variant.name);
            let class_name = format!("{enum_name}{variant_name}");
            out.push_str(&format!(
                "final class {class_name} extends {enum_name} {{\n"
            ));
            if variant.fields.is_empty() {
                out.push_str(&format!("  const {class_name}();\n"));
            } else {
                out.push_str(&format!("  const {class_name}({{\n"));
                for field in &variant.fields {
                    let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                    out.push_str(&format!("    required this.{field_name},\n"));
                }
                out.push_str("  });\n");
            }
            for field in &variant.fields {
                let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                out.push_str(&format!(
                    "  final {} {field_name};\n",
                    map_uniffi_type_to_dart(&field.type_)
                ));
            }
            out.push_str("}\n\n");
        }
    }

    for enum_ in enums {
        if !enum_.is_error {
            continue;
        }
        let enum_name = to_upper_camel(&enum_.name);
        let exception_name = format!("{enum_name}Exception");
        out.push_str(&format!(
            "sealed class {exception_name} implements Exception {{\n  const {exception_name}();\n}}\n\n"
        ));
        for variant in &enum_.variants {
            let variant_name = to_upper_camel(&variant.name);
            let variant_exception = format!("{exception_name}{variant_name}");
            out.push_str(&format!(
                "final class {variant_exception} extends {exception_name} {{\n"
            ));
            if variant.fields.is_empty() {
                out.push_str(&format!("  const {variant_exception}();\n"));
            } else {
                out.push_str(&format!("  const {variant_exception}({{\n"));
                for field in &variant.fields {
                    let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                    out.push_str(&format!("    required this.{field_name},\n"));
                }
                out.push_str("  });\n");
                for field in &variant.fields {
                    let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                    out.push_str(&format!(
                        "  final {} {field_name};\n",
                        map_uniffi_type_to_dart(&field.type_)
                    ));
                }
            }
            out.push_str("}\n\n");
        }
    }

    for enum_ in enums {
        let enum_name = to_upper_camel(&enum_.name);
        let has_data = enum_.variants.iter().any(|v| !v.fields.is_empty());
        out.push_str(&format!(
            "String _encode{enum_name}({enum_name} value) {{\n"
        ));
        if !has_data && !enum_.is_error {
            out.push_str("  return switch (value) {\n");
            for variant in &enum_.variants {
                let variant_name = safe_dart_identifier(&to_lower_camel(&variant.name));
                out.push_str(&format!(
                    "    {enum_name}.{variant_name} => '{variant_name}',\n"
                ));
            }
            out.push_str("  };\n");
            out.push_str("}\n\n");

            out.push_str(&format!("{enum_name} _decode{enum_name}(String raw) {{\n"));
            out.push_str("  switch (raw) {\n");
            for variant in &enum_.variants {
                let variant_name = safe_dart_identifier(&to_lower_camel(&variant.name));
                out.push_str(&format!("    case '{variant_name}':\n"));
                out.push_str(&format!("      return {enum_name}.{variant_name};\n"));
            }
            out.push_str("    default:\n");
            out.push_str(&format!(
                "      throw StateError('Unknown {} variant: $raw');\n",
                enum_name
            ));
            out.push_str("  }\n");
            out.push_str("}\n\n");
            continue;
        }

        for variant in &enum_.variants {
            let variant_class = format!("{}{}", enum_name, to_upper_camel(&variant.name));
            out.push_str(&format!("  if (value is {variant_class}) {{\n"));
            out.push_str("    return jsonEncode({\n");
            let variant_tag = safe_dart_identifier(&to_lower_camel(&variant.name));
            out.push_str(&format!("      'tag': '{variant_tag}',\n"));
            for field in &variant.fields {
                let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                let expr = render_json_encode_expr(&format!("value.{field_name}"), &field.type_);
                out.push_str(&format!("      '{field_name}': {expr},\n"));
            }
            out.push_str("    });\n");
            out.push_str("  }\n");
        }
        out.push_str(&format!(
            "  throw StateError('Unknown {} variant instance: $value');\n",
            enum_name
        ));
        out.push_str("}\n\n");

        out.push_str(&format!("{enum_name} _decode{enum_name}(String raw) {{\n"));
        out.push_str(
            "  final Map<String, dynamic> map = jsonDecode(raw) as Map<String, dynamic>;\n",
        );
        out.push_str("  final String? tag = map['tag'] as String?;\n");
        out.push_str("  switch (tag) {\n");
        for variant in &enum_.variants {
            let variant_tag = safe_dart_identifier(&to_lower_camel(&variant.name));
            let variant_class = format!("{}{}", enum_name, to_upper_camel(&variant.name));
            out.push_str(&format!("    case '{variant_tag}':\n"));
            out.push_str(&format!("      return {variant_class}(\n"));
            for field in &variant.fields {
                let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                let decode = render_json_decode_expr(&format!("map['{field_name}']"), &field.type_);
                out.push_str(&format!("        {field_name}: {decode},\n"));
            }
            out.push_str("      );\n");
        }
        out.push_str("    default:\n");
        out.push_str(&format!(
            "      throw StateError('Unknown {} variant tag: $tag');\n",
            enum_name
        ));
        out.push_str("  }\n");
        out.push_str("}\n\n");
    }

    for enum_ in enums {
        if !enum_.is_error {
            continue;
        }
        let enum_name = to_upper_camel(&enum_.name);
        let exception_name = format!("{enum_name}Exception");
        out.push_str(&format!(
            "{exception_name} _decode{exception_name}(Object? raw) {{\n"
        ));
        out.push_str(
            "  final Map<String, dynamic> map = raw is String ? (jsonDecode(raw) as Map<String, dynamic>) : (raw as Map<String, dynamic>);\n",
        );
        out.push_str("  final String? tag = map['tag'] as String?;\n");
        out.push_str("  switch (tag) {\n");
        for variant in &enum_.variants {
            let variant_tag = safe_dart_identifier(&to_lower_camel(&variant.name));
            let variant_name = to_upper_camel(&variant.name);
            let variant_exception = format!("{exception_name}{variant_name}");
            out.push_str(&format!("    case '{variant_tag}':\n"));
            if variant.fields.is_empty() {
                out.push_str(&format!("      return const {variant_exception}();\n"));
            } else {
                out.push_str(&format!("      return {variant_exception}(\n"));
                for field in &variant.fields {
                    let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                    let decode =
                        render_json_decode_expr(&format!("map['{field_name}']"), &field.type_);
                    out.push_str(&format!("        {field_name}: {decode},\n"));
                }
                out.push_str("      );\n");
            }
        }
        out.push_str("    default:\n");
        out.push_str(&format!(
            "      throw StateError('Unknown {} exception tag: $tag');\n",
            exception_name
        ));
        out.push_str("  }\n");
        out.push_str("}\n\n");
    }

    out
}

fn render_json_encode_expr(value_expr: &str, type_: &Type) -> String {
    match type_ {
        Type::Timestamp => format!("{value_expr}.toUtc().microsecondsSinceEpoch"),
        Type::Duration => format!("{value_expr}.inMicroseconds"),
        Type::Bytes => format!("base64Encode({value_expr})"),
        Type::Optional { inner_type } => {
            let inner = render_json_encode_expr("value", inner_type);
            format!("{value_expr} == null ? null : (() {{ final value = {value_expr}; return {inner}; }})()")
        }
        Type::Sequence { inner_type } => {
            let inner = render_json_encode_expr("item", inner_type);
            format!("{value_expr}.map((item) => {inner}).toList()")
        }
        Type::Map {
            key_type: _,
            value_type,
        } => {
            let inner = render_json_encode_expr("value", value_type);
            format!("{value_expr}.map((key, value) => MapEntry(key, {inner}))")
        }
        Type::Record { .. } => format!("{value_expr}.toJson()"),
        Type::Enum { name, .. } => format!("_encode{}({value_expr})", to_upper_camel(name)),
        _ => value_expr.to_string(),
    }
}

fn render_json_decode_expr(value_expr: &str, type_: &Type) -> String {
    match type_ {
        Type::UInt8
        | Type::Int8
        | Type::UInt16
        | Type::Int16
        | Type::UInt32
        | Type::Int32
        | Type::UInt64
        | Type::Int64 => format!("({value_expr} as num).toInt()"),
        Type::Float32 | Type::Float64 => format!("({value_expr} as num).toDouble()"),
        Type::Boolean => format!("{value_expr} as bool"),
        Type::String => format!("{value_expr} as String"),
        Type::Timestamp => format!(
            "DateTime.fromMicrosecondsSinceEpoch(({value_expr} as num).toInt(), isUtc: true)"
        ),
        Type::Duration => format!("Duration(microseconds: ({value_expr} as num).toInt())"),
        Type::Bytes => format!("base64Decode({value_expr} as String)"),
        Type::Optional { inner_type } => {
            let inner = render_json_decode_expr("value", inner_type);
            format!("{value_expr} == null ? null : (() {{ final value = {value_expr}; return {inner}; }})()")
        }
        Type::Sequence { inner_type } => {
            let inner = render_json_decode_expr("item", inner_type);
            format!("({value_expr} as List).map((item) => {inner}).toList()")
        }
        Type::Map {
            key_type: _,
            value_type,
        } => {
            let inner = render_json_decode_expr("value", value_type);
            format!("({value_expr} as Map<String, dynamic>).map((key, value) => MapEntry(key, {inner}))")
        }
        Type::Record { name, .. } => format!(
            "{}.fromJson({value_expr} as Map<String, dynamic>)",
            to_upper_camel(name)
        ),
        Type::Enum { name, .. } => {
            format!("_decode{}({value_expr} as String)", to_upper_camel(name))
        }
        _ => "throw UnimplementedError('unsupported json decode type')".to_string(),
    }
}

fn append_runtime_arg_marshalling(
    arg_name: &str,
    type_: &Type,
    enums: &[UdlEnum],
    pre_call: &mut Vec<String>,
    post_call: &mut Vec<String>,
    call_args: &mut Vec<String>,
) {
    if is_runtime_string_type(type_) {
        let native_name = format!("{arg_name}Native");
        pre_call.push(format!(
            "    final ffi.Pointer<Utf8> {native_name} = {arg_name}.toNativeUtf8();\n"
        ));
        post_call.push(format!("    calloc.free({native_name});\n"));
        call_args.push(native_name);
    } else if is_runtime_optional_string_type(type_) {
        let native_name = format!("{arg_name}Native");
        pre_call.push(format!(
            "    final ffi.Pointer<Utf8> {native_name} = {arg_name} == null ? ffi.nullptr : {arg_name}.toNativeUtf8();\n"
        ));
        post_call.push(format!(
            "    if ({native_name} != ffi.nullptr) calloc.free({native_name});\n"
        ));
        call_args.push(native_name);
    } else if is_runtime_timestamp_type(type_) {
        call_args.push(format!("{arg_name}.toUtc().microsecondsSinceEpoch"));
    } else if is_runtime_duration_type(type_) {
        call_args.push(format!("{arg_name}.inMicroseconds"));
    } else if is_runtime_bytes_type(type_) {
        let data_name = format!("{arg_name}Data");
        let buffer_ptr_name = format!("{arg_name}BufferPtr");
        let native_name = format!("{arg_name}Native");
        pre_call.push(format!(
            "    final ffi.Pointer<ffi.Uint8> {data_name} = {arg_name}.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>({arg_name}.length);\n"
        ));
        pre_call.push(format!(
            "    if ({data_name} != ffi.nullptr) {{\n      {data_name}.asTypedList({arg_name}.length).setAll(0, {arg_name});\n    }}\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<_RustBuffer> {buffer_ptr_name} = calloc<_RustBuffer>();\n"
        ));
        pre_call.push(format!("    {buffer_ptr_name}.ref.data = {data_name};\n"));
        pre_call.push(format!(
            "    {buffer_ptr_name}.ref.len = {arg_name}.length;\n"
        ));
        pre_call.push(format!(
            "    final _RustBuffer {native_name} = {buffer_ptr_name}.ref;\n"
        ));
        post_call.push(format!(
            "    if ({data_name} != ffi.nullptr) calloc.free({data_name});\n"
        ));
        post_call.push(format!("    calloc.free({buffer_ptr_name});\n"));
        call_args.push(native_name);
    } else if is_runtime_optional_bytes_type(type_) {
        let data_name = format!("{arg_name}Data");
        let buffer_ptr_name = format!("{arg_name}BufferPtr");
        let opt_ptr_name = format!("{arg_name}OptPtr");
        let native_name = format!("{arg_name}Native");
        let value_name = format!("{arg_name}Value");
        pre_call.push(format!(
            "    final bool {arg_name}IsSome = {arg_name} != null;\n"
        ));
        pre_call.push(format!(
            "    final Uint8List {value_name} = {arg_name} ?? Uint8List(0);\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<ffi.Uint8> {data_name} = !{arg_name}IsSome || {value_name}.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>({value_name}.length);\n"
        ));
        pre_call.push(format!(
            "    if ({data_name} != ffi.nullptr) {{\n      {data_name}.asTypedList({value_name}.length).setAll(0, {value_name});\n    }}\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<_RustBuffer> {buffer_ptr_name} = calloc<_RustBuffer>();\n"
        ));
        pre_call.push(format!("    {buffer_ptr_name}.ref.data = {data_name};\n"));
        pre_call.push(format!(
            "    {buffer_ptr_name}.ref.len = {arg_name}IsSome ? {value_name}.length : 0;\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<_RustBufferOpt> {opt_ptr_name} = calloc<_RustBufferOpt>();\n"
        ));
        pre_call.push(format!(
            "    {opt_ptr_name}.ref.isSome = {arg_name}IsSome ? 1 : 0;\n"
        ));
        pre_call.push(format!(
            "    {opt_ptr_name}.ref.value = {buffer_ptr_name}.ref;\n"
        ));
        pre_call.push(format!(
            "    final _RustBufferOpt {native_name} = {opt_ptr_name}.ref;\n"
        ));
        post_call.push(format!(
            "    if ({data_name} != ffi.nullptr) calloc.free({data_name});\n"
        ));
        post_call.push(format!("    calloc.free({buffer_ptr_name});\n"));
        post_call.push(format!("    calloc.free({opt_ptr_name});\n"));
        call_args.push(native_name);
    } else if is_runtime_sequence_bytes_type(type_) {
        let data_name = format!("{arg_name}Data");
        let vec_ptr_name = format!("{arg_name}VecPtr");
        let native_name = format!("{arg_name}Native");
        pre_call.push(format!(
            "    final ffi.Pointer<_RustBuffer> {data_name} = {arg_name}.isEmpty ? ffi.nullptr : calloc<_RustBuffer>({arg_name}.length);\n"
        ));
        pre_call.push(format!(
            "    if ({data_name} != ffi.nullptr) {{\n      for (var i = 0; i < {arg_name}.length; i++) {{\n        final item = {arg_name}[i];\n        final ffi.Pointer<ffi.Uint8> itemData = item.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>(item.length);\n        if (itemData != ffi.nullptr) {{\n          itemData.asTypedList(item.length).setAll(0, item);\n        }}\n        ({data_name} + i).ref\n          ..data = itemData\n          ..len = item.length;\n      }}\n    }}\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<_RustBufferVec> {vec_ptr_name} = calloc<_RustBufferVec>();\n"
        ));
        pre_call.push(format!(
            "    {vec_ptr_name}.ref\n      ..data = {data_name}\n      ..len = {arg_name}.length;\n"
        ));
        pre_call.push(format!(
            "    final _RustBufferVec {native_name} = {vec_ptr_name}.ref;\n"
        ));
        post_call.push(format!(
            "    if ({data_name} != ffi.nullptr) {{\n      for (var i = 0; i < {arg_name}.length; i++) {{\n        final data = ({data_name} + i).ref.data;\n        if (data != ffi.nullptr) calloc.free(data);\n      }}\n      calloc.free({data_name});\n    }}\n"
        ));
        post_call.push(format!("    calloc.free({vec_ptr_name});\n"));
        call_args.push(native_name);
    } else if is_runtime_record_type(type_) {
        let native_name = format!("{arg_name}Native");
        pre_call.push(format!(
            "    final String {native_name}Json = jsonEncode({arg_name}.toJson());\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<Utf8> {native_name} = {native_name}Json.toNativeUtf8();\n"
        ));
        post_call.push(format!("    calloc.free({native_name});\n"));
        call_args.push(native_name);
    } else if is_runtime_enum_type(type_, enums) {
        let native_name = format!("{arg_name}Native");
        let enum_name = enum_name_from_type(type_).unwrap_or("Enum");
        pre_call.push(format!(
            "    final String {native_name}Json = _encode{}({arg_name});\n",
            to_upper_camel(enum_name)
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<Utf8> {native_name} = {native_name}Json.toNativeUtf8();\n"
        ));
        post_call.push(format!("    calloc.free({native_name});\n"));
        call_args.push(native_name);
    } else {
        call_args.push(arg_name.to_string());
    }
}

fn render_callback_interfaces(callback_interfaces: &[UdlCallbackInterface]) -> String {
    if callback_interfaces.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for callback_interface in callback_interfaces {
        let class_name = to_upper_camel(&callback_interface.name);
        out.push_str(&format!("abstract interface class {class_name} {{\n"));
        for method in &callback_interface.methods {
            let value_return_type = method
                .return_type
                .as_ref()
                .map(map_uniffi_type_to_dart)
                .unwrap_or_else(|| "void".to_string());
            let signature_return_type = if method.is_async {
                format!("Future<{value_return_type}>")
            } else {
                value_return_type
            };
            let args = method
                .args
                .iter()
                .map(|a| {
                    format!(
                        "{} {}",
                        map_uniffi_type_to_dart(&a.type_),
                        safe_dart_identifier(&to_lower_camel(&a.name))
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let method_name = safe_dart_identifier(&to_lower_camel(&method.name));
            out.push_str(&format!(
                "  {signature_return_type} {method_name}({args});\n"
            ));
        }
        out.push_str("}\n\n");
    }
    out
}

fn render_callback_bridges(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let used = callback_interfaces_used_for_runtime(
        functions,
        objects,
        callback_interfaces,
        records,
        enums,
    );
    if used.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let has_async_callback_methods = used
        .iter()
        .any(|cb| cb.methods.iter().any(|method| method.is_async));
    if has_async_callback_methods {
        out.push_str("final class _ForeignFutureDroppedCallbackStruct extends ffi.Struct {\n");
        out.push_str("  @ffi.Uint64()\n");
        out.push_str("  external int handle;\n\n");
        out.push_str(
            "  external ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 handle)>> callback;\n",
        );
        out.push_str("}\n\n");
    }

    for callback_interface in used {
        let class_name = to_upper_camel(&callback_interface.name);
        let vtable_name = callback_vtable_struct_name(&callback_interface.name);
        let bridge_name = callback_bridge_class_name(&callback_interface.name);
        for method in &callback_interface.methods {
            if !method.is_async {
                continue;
            }
            let result_struct_name =
                callback_async_result_struct_name(&callback_interface.name, &method.name);
            out.push_str(&format!(
                "final class {result_struct_name} extends ffi.Struct {{\n"
            ));
            if let Some(return_type) = method.return_type.as_ref() {
                let return_field = render_callback_async_result_return_field(return_type)
                    .expect("validated runtime callback async return type");
                out.push_str(&return_field);
            }
            out.push_str("  external _RustCallStatus callStatus;\n");
            out.push_str("}\n\n");
        }
        out.push_str(&format!(
            "final class {vtable_name} extends ffi.Struct {{\n"
        ));
        out.push_str(
            "  external ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 handle)>> uniffiFree;\n\n",
        );
        out.push_str(
            "  external ffi.Pointer<ffi.NativeFunction<ffi.Uint64 Function(ffi.Uint64 handle)>> uniffiClone;\n\n",
        );
        for method in &callback_interface.methods {
            let method_name = safe_dart_identifier(&to_lower_camel(&method.name));
            let mut ffi_args = vec!["ffi.Uint64 handle".to_string()];
            for arg in &method.args {
                let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                let arg_native = map_runtime_native_ffi_type(&arg.type_, records, enums)
                    .expect("validated runtime callback arg type");
                ffi_args.push(format!("{arg_native} {arg_name}"));
            }
            if method.is_async {
                let result_struct_name =
                    callback_async_result_struct_name(&callback_interface.name, &method.name);
                ffi_args.push(format!(
                    "ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, {result_struct_name} result)>> uniffiFutureCallback"
                ));
                ffi_args.push("ffi.Uint64 callbackData".to_string());
                ffi_args.push(
                    "ffi.Pointer<_ForeignFutureDroppedCallbackStruct> uniffiOutDroppedCallback"
                        .to_string(),
                );
            } else {
                if let Some(return_type) = method.return_type.as_ref() {
                    let out_type = map_runtime_native_ffi_type(return_type, records, enums)
                        .expect("validated runtime callback return type");
                    ffi_args.push(format!("ffi.Pointer<{out_type}> outReturn"));
                } else {
                    ffi_args.push("ffi.Pointer<ffi.Void> outReturn".to_string());
                }
                ffi_args.push("ffi.Pointer<_RustCallStatus> outStatus".to_string());
            }
            out.push_str(&format!(
                "  external ffi.Pointer<ffi.NativeFunction<ffi.Void Function({})>> {method_name};\n\n",
                ffi_args.join(", ")
            ));
        }
        out.push_str("}\n\n");

        out.push_str(&format!("final class {bridge_name} {{\n"));
        out.push_str(&format!("  {bridge_name}._();\n"));
        out.push_str(&format!(
            "  static final {bridge_name} instance = {bridge_name}._();\n\n"
        ));
        out.push_str(&format!(
            "  final Map<int, {class_name}> _callbacks = <int, {class_name}>{{}};\n"
        ));
        out.push_str("  final Map<int, int> _refCounts = <int, int>{};\n");
        out.push_str("  int _nextHandle = 1;\n\n");
        out.push_str(&format!("  int register({class_name} callback) {{\n"));
        out.push_str("    final int handle = _nextHandle++;\n");
        out.push_str("    _callbacks[handle] = callback;\n");
        out.push_str("    _refCounts[handle] = 1;\n");
        out.push_str("    return handle;\n");
        out.push_str("  }\n\n");
        out.push_str("  void release(int handle) {\n");
        out.push_str("    final int? refs = _refCounts[handle];\n");
        out.push_str("    if (refs == null) {\n");
        out.push_str("      return;\n");
        out.push_str("    }\n");
        out.push_str("    if (refs <= 1) {\n");
        out.push_str("      _refCounts.remove(handle);\n");
        out.push_str("      _callbacks.remove(handle);\n");
        out.push_str("      return;\n");
        out.push_str("    }\n");
        out.push_str("    _refCounts[handle] = refs - 1;\n");
        out.push_str("  }\n\n");
        out.push_str("  int cloneHandle(int handle) {\n");
        out.push_str("    final int? refs = _refCounts[handle];\n");
        out.push_str("    if (refs == null) {\n");
        out.push_str("      return handle;\n");
        out.push_str("    }\n");
        out.push_str("    _refCounts[handle] = refs + 1;\n");
        out.push_str("    return handle;\n");
        out.push_str("  }\n\n");
        out.push_str(&format!(
            "  {class_name}? lookup(int handle) => _callbacks[handle];\n\n"
        ));
        out.push_str(
            "  static final ffi.NativeCallable<ffi.Void Function(ffi.Uint64 handle)> _freeNative = ffi.NativeCallable<ffi.Void Function(ffi.Uint64 handle)>.isolateLocal((int handle) {\n",
        );
        out.push_str("    instance.release(handle);\n");
        out.push_str("  });\n\n");
        out.push_str(
            "  static final ffi.NativeCallable<ffi.Uint64 Function(ffi.Uint64 handle)> _cloneNative = ffi.NativeCallable<ffi.Uint64 Function(ffi.Uint64 handle)>.isolateLocal((int handle) {\n",
        );
        out.push_str("    return instance.cloneHandle(handle);\n");
        out.push_str("  }, exceptionalReturn: 0);\n\n");

        for method in &callback_interface.methods {
            let method_name = safe_dart_identifier(&to_lower_camel(&method.name));
            let native_callable_name = format!("_{}Native", method_name);
            let mut ffi_args = vec!["ffi.Uint64 handle".to_string()];
            let mut dart_args = vec!["int handle".to_string()];
            let mut callback_args = Vec::new();
            for arg in &method.args {
                let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                let arg_native = map_runtime_native_ffi_type(&arg.type_, records, enums)
                    .expect("validated runtime callback arg type");
                let arg_dart = map_runtime_dart_ffi_type(&arg.type_, records, enums)
                    .expect("validated runtime callback arg type");
                ffi_args.push(format!("{arg_native} {arg_name}"));
                dart_args.push(format!("{arg_dart} {arg_name}"));
                callback_args.push(render_callback_arg_decode_expr(
                    &arg.type_, &arg_name, records, enums,
                ));
            }
            if method.is_async {
                let result_struct_name =
                    callback_async_result_struct_name(&callback_interface.name, &method.name);
                ffi_args.push(format!(
                    "ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, {result_struct_name} result)>> uniffiFutureCallback"
                ));
                ffi_args.push("ffi.Uint64 callbackData".to_string());
                ffi_args.push(
                    "ffi.Pointer<_ForeignFutureDroppedCallbackStruct> uniffiOutDroppedCallback"
                        .to_string(),
                );
                dart_args.push(format!(
                    "ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, {result_struct_name} result)>> uniffiFutureCallback"
                ));
                dart_args.push("int callbackData".to_string());
                dart_args.push(
                    "ffi.Pointer<_ForeignFutureDroppedCallbackStruct> uniffiOutDroppedCallback"
                        .to_string(),
                );

                out.push_str(&format!(
                    "  static final ffi.NativeCallable<ffi.Void Function({})> {native_callable_name} = ffi.NativeCallable<ffi.Void Function({})>.isolateLocal(({}) {{\n",
                    ffi_args.join(", "),
                    ffi_args.join(", "),
                    dart_args.join(", ")
                ));
                out.push_str(&format!(
                    "    final {class_name}? callback = instance.lookup(handle);\n"
                ));
                out.push_str("    final complete = uniffiFutureCallback.asFunction<void Function(int callbackData, ");
                out.push_str(&result_struct_name);
                out.push_str(" result)>();\n");
                out.push_str("    if (uniffiOutDroppedCallback != ffi.nullptr) {\n");
                out.push_str("      uniffiOutDroppedCallback.ref\n");
                out.push_str("        ..handle = 0\n");
                out.push_str("        ..callback = ffi.nullptr;\n");
                out.push_str("    }\n");
                out.push_str("    if (callback == null) {\n");
                out.push_str(&format!(
                    "      final ffi.Pointer<{result_struct_name}> resultPtr = calloc<{result_struct_name}>();\n"
                ));
                if let Some(return_type) = method.return_type.as_ref() {
                    let default_value = callback_async_default_return_expr(return_type);
                    out.push_str(&format!(
                        "      resultPtr.ref.returnValue = {default_value};\n"
                    ));
                }
                out.push_str("      resultPtr.ref.callStatus\n");
                out.push_str("        ..code = _rustCallStatusUnexpectedError\n");
                out.push_str("        ..errorBuf = ffi.nullptr;\n");
                out.push_str("      complete(callbackData, resultPtr.ref);\n");
                out.push_str("      calloc.free(resultPtr);\n");
                out.push_str("      return;\n");
                out.push_str("    }\n");
                out.push_str("    () async {\n");
                out.push_str(&format!(
                    "      final ffi.Pointer<{result_struct_name}> resultPtr = calloc<{result_struct_name}>();\n"
                ));
                if let Some(return_type) = method.return_type.as_ref() {
                    let default_value = callback_async_default_return_expr(return_type);
                    out.push_str(&format!(
                        "      resultPtr.ref.returnValue = {default_value};\n"
                    ));
                }
                out.push_str("      try {\n");
                if let Some(return_type) = method.return_type.as_ref() {
                    out.push_str(&format!(
                        "        final result = await callback.{method_name}({});\n",
                        callback_args.join(", ")
                    ));
                    let encoded =
                        render_callback_return_encode_expr(return_type, "result", records, enums);
                    out.push_str(&format!("        resultPtr.ref.returnValue = {encoded};\n"));
                } else {
                    out.push_str(&format!(
                        "        await callback.{method_name}({});\n",
                        callback_args.join(", ")
                    ));
                }
                out.push_str("        resultPtr.ref.callStatus\n");
                out.push_str("          ..code = _rustCallStatusSuccess\n");
                out.push_str("          ..errorBuf = ffi.nullptr;\n");
                out.push_str("      } catch (_) {\n");
                if method.throws_type.is_some() {
                    out.push_str("        resultPtr.ref.callStatus\n");
                    out.push_str("          ..code = _rustCallStatusError\n");
                    out.push_str("          ..errorBuf = ffi.nullptr;\n");
                } else {
                    out.push_str("        resultPtr.ref.callStatus\n");
                    out.push_str("          ..code = _rustCallStatusUnexpectedError\n");
                    out.push_str("          ..errorBuf = ffi.nullptr;\n");
                }
                out.push_str("      } finally {\n");
                out.push_str("        complete(callbackData, resultPtr.ref);\n");
                out.push_str("        calloc.free(resultPtr);\n");
                out.push_str("      }\n");
                out.push_str("    }();\n");
            } else {
                if let Some(return_type) = method.return_type.as_ref() {
                    let out_type = map_runtime_native_ffi_type(return_type, records, enums)
                        .expect("validated runtime callback return type");
                    ffi_args.push(format!("ffi.Pointer<{out_type}> outReturn"));
                    dart_args.push(format!("ffi.Pointer<{out_type}> outReturn"));
                } else {
                    ffi_args.push("ffi.Pointer<ffi.Void> outReturn".to_string());
                    dart_args.push("ffi.Pointer<ffi.Void> outReturn".to_string());
                }
                ffi_args.push("ffi.Pointer<_RustCallStatus> outStatus".to_string());
                dart_args.push("ffi.Pointer<_RustCallStatus> outStatus".to_string());

                out.push_str(&format!(
                    "  static final ffi.NativeCallable<ffi.Void Function({})> {native_callable_name} = ffi.NativeCallable<ffi.Void Function({})>.isolateLocal(({}) {{\n",
                    ffi_args.join(", "),
                    ffi_args.join(", "),
                    dart_args.join(", ")
                ));
                out.push_str(&format!(
                    "    final {class_name}? callback = instance.lookup(handle);\n"
                ));
                out.push_str("    if (callback == null) {\n");
                out.push_str("      outStatus.ref\n");
                out.push_str("        ..code = _rustCallStatusUnexpectedError\n");
                out.push_str("        ..errorBuf = ffi.nullptr;\n");
                out.push_str("      return;\n");
                out.push_str("    }\n");
                out.push_str("    try {\n");
                if let Some(return_type) = method.return_type.as_ref() {
                    out.push_str(&format!(
                        "      final result = callback.{method_name}({});\n",
                        callback_args.join(", ")
                    ));
                    let encoded =
                        render_callback_return_encode_expr(return_type, "result", records, enums);
                    out.push_str(&format!("      outReturn.value = {encoded};\n"));
                } else {
                    out.push_str(&format!(
                        "      callback.{method_name}({});\n",
                        callback_args.join(", ")
                    ));
                }
                out.push_str("      outStatus.ref\n");
                out.push_str("        ..code = _rustCallStatusSuccess\n");
                out.push_str("        ..errorBuf = ffi.nullptr;\n");
                out.push_str("    } catch (_) {\n");
                if method.throws_type.is_some() {
                    out.push_str("      outStatus.ref\n");
                    out.push_str("        ..code = _rustCallStatusError\n");
                    out.push_str("        ..errorBuf = ffi.nullptr;\n");
                } else {
                    out.push_str("      outStatus.ref\n");
                    out.push_str("        ..code = _rustCallStatusUnexpectedError\n");
                    out.push_str("        ..errorBuf = ffi.nullptr;\n");
                }
                out.push_str("    }\n");
            }
            out.push_str("  });\n\n");
        }

        out.push_str(&format!(
            "  static ffi.Pointer<{vtable_name}> createVTable() {{\n"
        ));
        out.push_str(&format!(
            "    final ffi.Pointer<{vtable_name}> vtablePtr = calloc<{vtable_name}>();\n"
        ));
        out.push_str("    vtablePtr.ref\n");
        out.push_str("      ..uniffiFree = _freeNative.nativeFunction\n");
        out.push_str("      ..uniffiClone = _cloneNative.nativeFunction\n");
        for method in &callback_interface.methods {
            let method_name = safe_dart_identifier(&to_lower_camel(&method.name));
            out.push_str(&format!(
                "      ..{method_name} = _{method_name}Native.nativeFunction\n"
            ));
        }
        out.push_str("    ;\n");
        out.push_str("    return vtablePtr;\n");
        out.push_str("  }\n");
        out.push_str("}\n\n");
    }

    out
}

fn render_bound_methods(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    _ffi_class_name: &str,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let mut out = String::new();
    let callback_runtime_interfaces = callback_interfaces_used_for_runtime(
        functions,
        objects,
        callback_interfaces,
        records,
        enums,
    );
    let needs_async_rust_future = has_runtime_async_rust_future_support(
        functions,
        objects,
        callback_interfaces,
        records,
        enums,
    );
    let needs_string_free = needs_async_rust_future
        || functions.iter().any(|f| {
            is_runtime_ffi_compatible_function(f, records, enums)
                && (f.returns_runtime_string()
                    || is_runtime_throwing_ffi_compatible_function(
                        f,
                        callback_interfaces,
                        records,
                        enums,
                    )
                    || f.return_type
                        .as_ref()
                        .is_some_and(|t| is_runtime_record_or_enum_string_type(t, enums)))
        });
    let needs_bytes_free = functions.iter().any(|f| {
        is_runtime_ffi_compatible_function(f, records, enums) && f.returns_runtime_bytes()
    });
    let needs_bytes_vec_free = functions.iter().any(|f| {
        is_runtime_ffi_compatible_function(f, records, enums)
            && f.return_type
                .as_ref()
                .is_some_and(is_runtime_sequence_bytes_type)
    });

    if needs_string_free {
        out.push('\n');
        out.push_str("  late final void Function(ffi.Pointer<Utf8>) _rustStringFree = _lib.lookupFunction<ffi.Void Function(ffi.Pointer<Utf8>), void Function(ffi.Pointer<Utf8>)>('rust_string_free');\n");
    }
    if needs_bytes_free {
        out.push('\n');
        out.push_str("  late final void Function(_RustBuffer) _rustBytesFree = _lib.lookupFunction<ffi.Void Function(_RustBuffer), void Function(_RustBuffer)>('rust_bytes_free');\n");
    }
    if needs_bytes_vec_free {
        out.push('\n');
        out.push_str("  late final void Function(_RustBufferVec) _rustBytesVecFree = _lib.lookupFunction<ffi.Void Function(_RustBufferVec), void Function(_RustBufferVec)>('rust_bytes_vec_free');\n");
    }
    for callback_interface in &callback_runtime_interfaces {
        let callback_name = &callback_interface.name;
        let vtable_name = callback_vtable_struct_name(callback_name);
        let init_field = callback_init_field_name(callback_name);
        let init_done_field = callback_init_done_field_name(callback_name);
        let vtable_field = callback_vtable_field_name(callback_name);
        let bridge_name = callback_bridge_class_name(callback_name);
        let init_symbol = callback_init_symbol(callback_name);
        out.push('\n');
        out.push_str(&format!(
            "  late final void Function(ffi.Pointer<{vtable_name}>) {init_field} = _lib.lookupFunction<ffi.Void Function(ffi.Pointer<{vtable_name}>), void Function(ffi.Pointer<{vtable_name}>)>('{init_symbol}');\n"
        ));
        out.push_str(&format!(
            "  late final ffi.Pointer<{vtable_name}> {vtable_field} = {bridge_name}.createVTable();\n"
        ));
        out.push_str(&format!(
            "  late final bool {init_done_field} = (() {{\n    {init_field}({vtable_field});\n    return true;\n  }})();\n"
        ));
    }

    for function in functions {
        let is_runtime_supported = is_runtime_ffi_compatible_function(function, records, enums);
        let is_sync_callback_supported =
            is_runtime_callback_compatible_function(function, callback_interfaces, records, enums);
        let has_callback_args =
            has_runtime_callback_args_in_args(&function.args, callback_interfaces, records, enums);
        if !is_runtime_supported && !is_sync_callback_supported && !has_callback_args {
            continue;
        }

        let method_name = safe_dart_identifier(&to_lower_camel(&function.name));
        let field_name = format!("_{}", method_name);
        if is_sync_callback_supported {
            let return_type = function
                .return_type
                .as_ref()
                .map(map_uniffi_type_to_dart)
                .unwrap_or_else(|| "void".to_string());
            let native_return = function
                .return_type
                .as_ref()
                .and_then(|t| map_runtime_native_ffi_type(t, records, enums))
                .unwrap_or("ffi.Void");
            let dart_ffi_return = function
                .return_type
                .as_ref()
                .and_then(|t| map_runtime_dart_ffi_type(t, records, enums))
                .unwrap_or("void");

            let mut native_args = Vec::new();
            let mut dart_ffi_args = Vec::new();
            let mut dart_args = Vec::new();
            let mut call_args = Vec::new();
            let mut pre_call = Vec::new();
            let mut post_call = Vec::new();

            for arg in &function.args {
                let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                dart_args.push(format!(
                    "{} {}",
                    map_uniffi_type_to_dart(&arg.type_),
                    arg_name
                ));
                if let Some(callback_name) = callback_interface_name_from_type(&arg.type_) {
                    let init_done_field = callback_init_done_field_name(callback_name);
                    let bridge_name = callback_bridge_class_name(callback_name);
                    let handle_name = format!("{arg_name}Handle");
                    native_args.push(format!("ffi.Uint64 {arg_name}"));
                    dart_ffi_args.push(format!("int {arg_name}"));
                    pre_call.push(format!("    {init_done_field};\n"));
                    pre_call.push(format!(
                        "    final int {handle_name} = {bridge_name}.instance.register({arg_name});\n"
                    ));
                    post_call.push(format!(
                        "    {bridge_name}.instance.release({handle_name});\n"
                    ));
                    call_args.push(handle_name);
                    continue;
                }
                let native_type = map_runtime_native_ffi_type(&arg.type_, records, enums)
                    .expect("validated callback-compatible arg type");
                let dart_ffi_type = map_runtime_dart_ffi_type(&arg.type_, records, enums)
                    .expect("validated callback-compatible arg type");
                native_args.push(format!("{native_type} {arg_name}"));
                dart_ffi_args.push(format!("{dart_ffi_type} {arg_name}"));
                append_runtime_arg_marshalling(
                    &arg_name,
                    &arg.type_,
                    enums,
                    &mut pre_call,
                    &mut post_call,
                    &mut call_args,
                );
            }

            let native_sig = format!("{native_return} Function({})", native_args.join(", "));
            let dart_ffi_sig = format!("{dart_ffi_return} Function({})", dart_ffi_args.join(", "));
            let dart_sig = dart_args.join(", ");

            out.push('\n');
            out.push_str(&format!(
                "  late final {dart_ffi_sig} {field_name} = _lib.lookupFunction<{native_sig}, {dart_ffi_sig}>('{}');\n",
                function.name
            ));
            out.push('\n');
            out.push_str(&format!("  {return_type} {method_name}({dart_sig}) {{\n"));
            for line in &pre_call {
                out.push_str(line);
            }
            if !post_call.is_empty() {
                out.push_str("    try {\n");
            }
            let call = format!("{field_name}({})", call_args.join(", "));
            match function.return_type.as_ref() {
                None => out.push_str(&format!("    {call};\n")),
                Some(ret_type) => {
                    let decode = render_plain_ffi_decode_expr(ret_type, &call);
                    out.push_str(&format!("    return {decode};\n"));
                }
            }
            if !post_call.is_empty() {
                out.push_str("    } finally {\n");
                for line in &post_call {
                    out.push_str(line);
                }
                out.push_str("    }\n");
            }
            out.push_str("  }\n");
            continue;
        }

        let return_type = function
            .return_type
            .as_ref()
            .map(map_uniffi_type_to_dart)
            .unwrap_or_else(|| "void".to_string());
        let is_throwing = is_runtime_throwing_ffi_compatible_function(
            function,
            callback_interfaces,
            records,
            enums,
        );
        let native_return = function
            .return_type
            .as_ref()
            .map(|t| {
                if is_throwing {
                    Some("ffi.Pointer<Utf8>")
                } else {
                    map_runtime_native_ffi_type(t, records, enums)
                }
            })
            .unwrap_or_else(|| {
                if is_throwing {
                    Some("ffi.Pointer<Utf8>")
                } else {
                    Some("ffi.Void")
                }
            });
        let dart_ffi_return = function
            .return_type
            .as_ref()
            .map(|t| {
                if is_throwing {
                    Some("ffi.Pointer<Utf8>")
                } else {
                    map_runtime_dart_ffi_type(t, records, enums)
                }
            })
            .unwrap_or_else(|| {
                if is_throwing {
                    Some("ffi.Pointer<Utf8>")
                } else {
                    Some("void")
                }
            });

        let Some(native_return) = native_return else {
            continue;
        };
        let Some(dart_ffi_return) = dart_ffi_return else {
            continue;
        };

        let mut native_args = Vec::new();
        let mut dart_ffi_args = Vec::new();
        let mut dart_args = Vec::new();
        let mut call_args = Vec::new();
        let mut pre_call = Vec::new();
        let mut post_call = Vec::new();
        let mut signature_compatible = true;

        for arg in &function.args {
            let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
            dart_args.push(format!(
                "{} {}",
                map_uniffi_type_to_dart(&arg.type_),
                arg_name
            ));
            if let Some(callback_name) = callback_interface_name_from_type(&arg.type_) {
                let init_done_field = callback_init_done_field_name(callback_name);
                let bridge_name = callback_bridge_class_name(callback_name);
                let handle_name = format!("{arg_name}Handle");
                native_args.push(format!("ffi.Uint64 {arg_name}"));
                dart_ffi_args.push(format!("int {arg_name}"));
                pre_call.push(format!("    {init_done_field};\n"));
                pre_call.push(format!(
                    "    final int {handle_name} = {bridge_name}.instance.register({arg_name});\n"
                ));
                post_call.push(format!(
                    "    {bridge_name}.instance.release({handle_name});\n"
                ));
                call_args.push(handle_name);
                continue;
            }
            let Some(native_type) = map_runtime_native_ffi_type(&arg.type_, records, enums) else {
                signature_compatible = false;
                break;
            };
            let Some(dart_ffi_type) = map_runtime_dart_ffi_type(&arg.type_, records, enums) else {
                signature_compatible = false;
                break;
            };
            native_args.push(format!("{native_type} {arg_name}"));
            dart_ffi_args.push(format!("{dart_ffi_type} {arg_name}"));
            if is_runtime_string_type(&arg.type_) {
                let native_name = format!("{arg_name}Native");
                pre_call.push(format!(
                    "    final ffi.Pointer<Utf8> {native_name} = {arg_name}.toNativeUtf8();\n"
                ));
                post_call.push(format!("    calloc.free({native_name});\n"));
                call_args.push(native_name);
            } else if is_runtime_optional_string_type(&arg.type_) {
                let native_name = format!("{arg_name}Native");
                pre_call.push(format!(
                    "    final ffi.Pointer<Utf8> {native_name} = {arg_name} == null ? ffi.nullptr : {arg_name}.toNativeUtf8();\n"
                ));
                post_call.push(format!(
                    "    if ({native_name} != ffi.nullptr) calloc.free({native_name});\n"
                ));
                call_args.push(native_name);
            } else if is_runtime_timestamp_type(&arg.type_) {
                call_args.push(format!("{arg_name}.toUtc().microsecondsSinceEpoch"));
            } else if is_runtime_duration_type(&arg.type_) {
                call_args.push(format!("{arg_name}.inMicroseconds"));
            } else if is_runtime_bytes_type(&arg.type_) {
                let data_name = format!("{arg_name}Data");
                let buffer_ptr_name = format!("{arg_name}BufferPtr");
                let native_name = format!("{arg_name}Native");
                pre_call.push(format!(
                    "    final ffi.Pointer<ffi.Uint8> {data_name} = {arg_name}.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>({arg_name}.length);\n"
                ));
                pre_call.push(format!(
                    "    if ({data_name} != ffi.nullptr) {{\n      {data_name}.asTypedList({arg_name}.length).setAll(0, {arg_name});\n    }}\n"
                ));
                pre_call.push(format!(
                    "    final ffi.Pointer<_RustBuffer> {buffer_ptr_name} = calloc<_RustBuffer>();\n"
                ));
                pre_call.push(format!("    {buffer_ptr_name}.ref.data = {data_name};\n"));
                pre_call.push(format!(
                    "    {buffer_ptr_name}.ref.len = {arg_name}.length;\n"
                ));
                pre_call.push(format!(
                    "    final _RustBuffer {native_name} = {buffer_ptr_name}.ref;\n"
                ));
                post_call.push(format!(
                    "    if ({data_name} != ffi.nullptr) calloc.free({data_name});\n"
                ));
                post_call.push(format!("    calloc.free({buffer_ptr_name});\n"));
                call_args.push(native_name);
            } else if is_runtime_optional_bytes_type(&arg.type_) {
                let data_name = format!("{arg_name}Data");
                let buffer_ptr_name = format!("{arg_name}BufferPtr");
                let opt_ptr_name = format!("{arg_name}OptPtr");
                let native_name = format!("{arg_name}Native");
                let value_name = format!("{arg_name}Value");
                pre_call.push(format!(
                    "    final bool {arg_name}IsSome = {arg_name} != null;\n"
                ));
                pre_call.push(format!(
                    "    final Uint8List {value_name} = {arg_name} ?? Uint8List(0);\n"
                ));
                pre_call.push(format!(
                    "    final ffi.Pointer<ffi.Uint8> {data_name} = !{arg_name}IsSome || {value_name}.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>({value_name}.length);\n"
                ));
                pre_call.push(format!(
                    "    if ({data_name} != ffi.nullptr) {{\n      {data_name}.asTypedList({value_name}.length).setAll(0, {value_name});\n    }}\n"
                ));
                pre_call.push(format!(
                    "    final ffi.Pointer<_RustBuffer> {buffer_ptr_name} = calloc<_RustBuffer>();\n"
                ));
                pre_call.push(format!("    {buffer_ptr_name}.ref.data = {data_name};\n"));
                pre_call.push(format!(
                    "    {buffer_ptr_name}.ref.len = {arg_name}IsSome ? {value_name}.length : 0;\n"
                ));
                pre_call.push(format!(
                    "    final ffi.Pointer<_RustBufferOpt> {opt_ptr_name} = calloc<_RustBufferOpt>();\n"
                ));
                pre_call.push(format!(
                    "    {opt_ptr_name}.ref.isSome = {arg_name}IsSome ? 1 : 0;\n"
                ));
                pre_call.push(format!(
                    "    {opt_ptr_name}.ref.value = {buffer_ptr_name}.ref;\n"
                ));
                pre_call.push(format!(
                    "    final _RustBufferOpt {native_name} = {opt_ptr_name}.ref;\n"
                ));
                post_call.push(format!(
                    "    if ({data_name} != ffi.nullptr) calloc.free({data_name});\n"
                ));
                post_call.push(format!("    calloc.free({buffer_ptr_name});\n"));
                post_call.push(format!("    calloc.free({opt_ptr_name});\n"));
                call_args.push(native_name);
            } else if is_runtime_sequence_bytes_type(&arg.type_) {
                let data_name = format!("{arg_name}Data");
                let vec_ptr_name = format!("{arg_name}VecPtr");
                let native_name = format!("{arg_name}Native");
                pre_call.push(format!(
                    "    final ffi.Pointer<_RustBuffer> {data_name} = {arg_name}.isEmpty ? ffi.nullptr : calloc<_RustBuffer>({arg_name}.length);\n"
                ));
                pre_call.push(format!(
                    "    if ({data_name} != ffi.nullptr) {{\n      for (var i = 0; i < {arg_name}.length; i++) {{\n        final item = {arg_name}[i];\n        final ffi.Pointer<ffi.Uint8> itemData = item.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>(item.length);\n        if (itemData != ffi.nullptr) {{\n          itemData.asTypedList(item.length).setAll(0, item);\n        }}\n        ({data_name} + i).ref\n          ..data = itemData\n          ..len = item.length;\n      }}\n    }}\n"
                ));
                pre_call.push(format!(
                    "    final ffi.Pointer<_RustBufferVec> {vec_ptr_name} = calloc<_RustBufferVec>();\n"
                ));
                pre_call.push(format!(
                    "    {vec_ptr_name}.ref\n      ..data = {data_name}\n      ..len = {arg_name}.length;\n"
                ));
                pre_call.push(format!(
                    "    final _RustBufferVec {native_name} = {vec_ptr_name}.ref;\n"
                ));
                post_call.push(format!(
                    "    if ({data_name} != ffi.nullptr) {{\n      for (var i = 0; i < {arg_name}.length; i++) {{\n        final data = ({data_name} + i).ref.data;\n        if (data != ffi.nullptr) calloc.free(data);\n      }}\n      calloc.free({data_name});\n    }}\n"
                ));
                post_call.push(format!("    calloc.free({vec_ptr_name});\n"));
                call_args.push(native_name);
            } else if is_runtime_record_type(&arg.type_) {
                let native_name = format!("{arg_name}Native");
                pre_call.push(format!(
                    "    final String {native_name}Json = jsonEncode({arg_name}.toJson());\n"
                ));
                pre_call.push(format!(
                    "    final ffi.Pointer<Utf8> {native_name} = {native_name}Json.toNativeUtf8();\n"
                ));
                post_call.push(format!("    calloc.free({native_name});\n"));
                call_args.push(native_name);
            } else if is_runtime_enum_type(&arg.type_, enums) {
                let native_name = format!("{arg_name}Native");
                let enum_name = enum_name_from_type(&arg.type_).unwrap_or("Enum");
                pre_call.push(format!(
                    "    final String {native_name}Json = _encode{}({arg_name});\n",
                    to_upper_camel(enum_name)
                ));
                pre_call.push(format!(
                    "    final ffi.Pointer<Utf8> {native_name} = {native_name}Json.toNativeUtf8();\n"
                ));
                post_call.push(format!("    calloc.free({native_name});\n"));
                call_args.push(native_name);
            } else {
                call_args.push(arg_name);
            }
        }

        if !signature_compatible {
            continue;
        }

        if is_runtime_async_rust_future_compatible_function(
            function,
            callback_interfaces,
            records,
            enums,
        ) {
            let Some(async_spec) =
                async_rust_future_spec(function.return_type.as_ref(), records, enums)
            else {
                continue;
            };
            let start_native_sig = format!("ffi.Uint64 Function({})", native_args.join(", "));
            let start_dart_sig = format!("int Function({})", dart_ffi_args.join(", "));
            let poll_field = format!("{field_name}RustFuturePoll");
            let cancel_field = format!("{field_name}RustFutureCancel");
            let complete_field = format!("{field_name}RustFutureComplete");
            let free_field = format!("{field_name}RustFutureFree");
            let complete_symbol = format!("rust_future_complete_{}", async_spec.suffix);
            let poll_symbol = format!("rust_future_poll_{}", async_spec.suffix);
            let cancel_symbol = format!("rust_future_cancel_{}", async_spec.suffix);
            let free_symbol = format!("rust_future_free_{}", async_spec.suffix);
            let complete_native_sig = format!(
                "{} Function(ffi.Uint64 handle, ffi.Pointer<_RustCallStatus> outStatus)",
                async_spec.complete_native_type
            );
            let complete_dart_sig = format!(
                "{} Function(int handle, ffi.Pointer<_RustCallStatus> outStatus)",
                async_spec.complete_dart_type
            );

            out.push('\n');
            out.push_str(&format!(
                "  late final {start_dart_sig} {field_name} = _lib.lookupFunction<{start_native_sig}, {start_dart_sig}>('{}');\n",
                function.name
            ));
            out.push_str(&format!(
                "  late final void Function(int handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, int callbackData) {poll_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, ffi.Uint64 callbackData), void Function(int handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, int callbackData)>('{poll_symbol}');\n"
            ));
            out.push_str(&format!(
                "  late final void Function(int handle) {cancel_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle), void Function(int handle)>('{cancel_symbol}');\n"
            ));
            out.push_str(&format!(
                "  late final {complete_dart_sig} {complete_field} = _lib.lookupFunction<{complete_native_sig}, {complete_dart_sig}>('{complete_symbol}');\n"
            ));
            out.push_str(&format!(
                "  late final void Function(int handle) {free_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle), void Function(int handle)>('{free_symbol}');\n"
            ));
            out.push('\n');
            out.push_str(&format!(
                "  Future<{return_type}> {method_name}({}) async {{\n",
                dart_args.join(", ")
            ));
            for line in &pre_call {
                out.push_str(line);
            }
            out.push_str("    final int futureHandle;\n");
            if !post_call.is_empty() {
                out.push_str("    try {\n");
                out.push_str(&format!(
                    "      futureHandle = {field_name}({});\n",
                    call_args.join(", ")
                ));
                out.push_str("    } finally {\n");
                for line in &post_call {
                    out.push_str(line);
                }
                out.push_str("    }\n");
            } else {
                out.push_str(&format!(
                    "    futureHandle = {field_name}({});\n",
                    call_args.join(", ")
                ));
            }
            out.push_str(
                "    final StreamController<int> pollEvents = StreamController<int>.broadcast();\n",
            );
            out.push_str(
                "    final callback = ffi.NativeCallable<ffi.Void Function(ffi.Uint64, ffi.Int8)>.listener((int _, int pollResult) {\n",
            );
            out.push_str("      pollEvents.add(pollResult);\n");
            out.push_str("    });\n");
            out.push_str("    try {\n");
            out.push_str(&format!(
                "      {poll_field}(futureHandle, callback.nativeFunction, 0);\n"
            ));
            out.push_str("      while (true) {\n");
            out.push_str("        final int pollResult = await pollEvents.stream.first;\n");
            out.push_str("        if (pollResult == _rustFuturePollReady) {\n");
            out.push_str("          break;\n");
            out.push_str("        }\n");
            out.push_str("        if (pollResult == _rustFuturePollWake) {\n");
            out.push_str(&format!(
                "          {poll_field}(futureHandle, callback.nativeFunction, 0);\n"
            ));
            out.push_str("          continue;\n");
            out.push_str("        }\n");
            out.push_str(&format!(
                "        throw StateError('Rust future poll returned invalid status for {}: $pollResult');\n",
                function.name
            ));
            out.push_str("      }\n");
            out.push_str(
                "      final ffi.Pointer<_RustCallStatus> outStatusPtr = calloc<_RustCallStatus>();\n",
            );
            out.push_str("      try {\n");
            if function.return_type.is_none() {
                out.push_str(&format!(
                    "        {complete_field}(futureHandle, outStatusPtr);\n"
                ));
            } else if let Some(ret_type) = function.return_type.as_ref() {
                if is_runtime_string_type(ret_type)
                    || is_runtime_optional_string_type(ret_type)
                    || is_runtime_record_type(ret_type)
                    || is_runtime_enum_type(ret_type, enums)
                {
                    out.push_str(&format!(
                        "        final ffi.Pointer<Utf8> resultPtr = {complete_field}(futureHandle, outStatusPtr);\n"
                    ));
                } else {
                    out.push_str(&format!(
                        "        final {} resultValue = {complete_field}(futureHandle, outStatusPtr);\n",
                        async_spec.complete_dart_type
                    ));
                }
            } else {
                out.push_str(&format!(
                    "        final {} resultValue = {complete_field}(futureHandle, outStatusPtr);\n",
                    async_spec.complete_dart_type
                ));
            }
            out.push_str("        final int statusCode = outStatusPtr.ref.code;\n");
            out.push_str("        if (statusCode == _rustCallStatusSuccess) {\n");
            if let Some(ret_type) = function.return_type.as_ref() {
                if is_runtime_string_type(ret_type) {
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            return resultPtr.toDartString();\n");
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if is_runtime_optional_string_type(ret_type) {
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("            return null;\n");
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            return resultPtr.toDartString();\n");
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if is_runtime_record_type(ret_type) {
                    let record_name = record_name_from_type(ret_type).unwrap_or("Record");
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "            return {}.fromJson(jsonDecode(payload) as Map<String, dynamic>);\n",
                        to_upper_camel(record_name)
                    ));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if is_runtime_enum_type(ret_type, enums) {
                    let enum_name = enum_name_from_type(ret_type).unwrap_or("Enum");
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "            return _decode{}(payload);\n",
                        to_upper_camel(enum_name)
                    ));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if is_runtime_timestamp_type(ret_type) {
                    out.push_str(
                        "          return DateTime.fromMicrosecondsSinceEpoch(resultValue, isUtc: true);\n",
                    );
                } else if is_runtime_duration_type(ret_type) {
                    out.push_str("          return Duration(microseconds: resultValue);\n");
                } else {
                    let decode = render_plain_ffi_decode_expr(ret_type, "resultValue");
                    out.push_str(&format!("          return {decode};\n"));
                }
            } else {
                out.push_str("          return;\n");
            }
            out.push_str("        }\n");
            out.push_str("        if (statusCode == _rustCallStatusCancelled) {\n");
            out.push_str(&format!(
                "          throw StateError('Rust future was cancelled for {}');\n",
                function.name
            ));
            out.push_str("        }\n");
            out.push_str("        final ffi.Pointer<Utf8> errorPtr = outStatusPtr.ref.errorBuf;\n");
            out.push_str("        if (errorPtr != ffi.nullptr) {\n");
            out.push_str("          try {\n");
            out.push_str("            throw StateError(errorPtr.toDartString());\n");
            out.push_str("          } finally {\n");
            out.push_str("            _rustStringFree(errorPtr);\n");
            out.push_str("          }\n");
            out.push_str("        }\n");
            out.push_str(&format!(
                "        throw StateError('Rust future failed for {} with status code: $statusCode');\n",
                function.name
            ));
            out.push_str("      } finally {\n");
            out.push_str("        calloc.free(outStatusPtr);\n");
            out.push_str("      }\n");
            out.push_str("    } catch (_) {\n");
            out.push_str(&format!("      {cancel_field}(futureHandle);\n"));
            out.push_str("      rethrow;\n");
            out.push_str("    } finally {\n");
            out.push_str("      await pollEvents.close();\n");
            out.push_str("      callback.close();\n");
            out.push_str(&format!("      {free_field}(futureHandle);\n"));
            out.push_str("    }\n");
            out.push_str("  }\n");
            continue;
        }

        let native_sig = format!("{native_return} Function({})", native_args.join(", "));
        let dart_sig = format!("{dart_ffi_return} Function({})", dart_ffi_args.join(", "));

        out.push('\n');
        out.push_str(&format!(
            "  late final {dart_sig} {field_name} = _lib.lookupFunction<{native_sig}, {dart_sig}>('{}');\n",
            function.name
        ));
        out.push('\n');
        out.push_str(&format!(
            "  {return_type} {method_name}({}) {{\n",
            dart_args.join(", ")
        ));
        for line in &pre_call {
            out.push_str(line);
        }
        if !post_call.is_empty() {
            out.push_str("    try {\n");
        }
        let call_expr = format!("{field_name}({})", call_args.join(", "));
        if is_throwing {
            let Some(throws_name) = function
                .throws_type
                .as_ref()
                .and_then(enum_name_from_type)
                .map(to_upper_camel)
            else {
                continue;
            };
            let ok_decode = function
                .return_type
                .as_ref()
                .map(|t| render_json_decode_expr("okRaw", t));
            out.push_str(&format!(
                "      final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
            ));
            out.push_str("      if (resultPtr == ffi.nullptr) {\n");
            out.push_str(&format!(
                "        throw StateError('Rust returned null for {}');\n",
                function.name
            ));
            out.push_str("      }\n");
            out.push_str("      final String payload;\n");
            out.push_str("      try {\n");
            out.push_str("        payload = resultPtr.toDartString();\n");
            out.push_str("      } finally {\n");
            out.push_str("        _rustStringFree(resultPtr);\n");
            out.push_str("      }\n");
            out.push_str(
                "      final Map<String, dynamic> envelope = jsonDecode(payload) as Map<String, dynamic>;\n",
            );
            out.push_str("      final Object? errRaw = envelope['err'];\n");
            out.push_str("      if (errRaw != null) {\n");
            out.push_str(&format!(
                "        throw _decode{}Exception(errRaw);\n",
                throws_name
            ));
            out.push_str("      }\n");
            if let Some(ok_decode) = ok_decode {
                out.push_str("      if (!envelope.containsKey('ok')) {\n");
                out.push_str(&format!(
                    "        throw StateError('Rust returned malformed result for {}');\n",
                    function.name
                ));
                out.push_str("      }\n");
                out.push_str("      final Object? okRaw = envelope['ok'];\n");
                out.push_str(&format!("      return {ok_decode};\n"));
            } else {
                out.push_str("      return;\n");
            }
        } else {
            match function.return_type.as_ref() {
                Some(type_) if is_runtime_string_type(type_) => {
                    out.push_str(&format!(
                        "      final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("      if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "        throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str("        return resultPtr.toDartString();\n");
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustStringFree(resultPtr);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_optional_string_type(type_) => {
                    out.push_str(&format!(
                        "      final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("      if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("        return null;\n");
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str("        return resultPtr.toDartString();\n");
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustStringFree(resultPtr);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_timestamp_type(type_) => {
                    out.push_str(&format!("      final int micros = {call_expr};\n"));
                    out.push_str(
                        "      return DateTime.fromMicrosecondsSinceEpoch(micros, isUtc: true);\n",
                    );
                }
                Some(type_) if is_runtime_duration_type(type_) => {
                    out.push_str(&format!("      final int micros = {call_expr};\n"));
                    out.push_str("      return Duration(microseconds: micros);\n");
                }
                Some(type_) if is_runtime_bytes_type(type_) => {
                    out.push_str(&format!(
                        "      final _RustBuffer resultBuf = {call_expr};\n"
                    ));
                    out.push_str(
                        "      final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n",
                    );
                    out.push_str("      final int resultLen = resultBuf.len;\n");
                    out.push_str("      if (resultData == ffi.nullptr) {\n");
                    out.push_str("        if (resultLen == 0) {\n");
                    out.push_str("          _rustBytesFree(resultBuf);\n");
                    out.push_str("          return Uint8List(0);\n");
                    out.push_str("        }\n");
                    out.push_str(&format!(
                        "        throw StateError('Rust returned invalid buffer for {}');\n",
                        function.name
                    ));
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str(
                        "        return Uint8List.fromList(resultData.asTypedList(resultLen));\n",
                    );
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustBytesFree(resultBuf);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_optional_bytes_type(type_) => {
                    out.push_str(&format!(
                        "      final _RustBufferOpt resultOpt = {call_expr};\n"
                    ));
                    out.push_str("      if (resultOpt.isSome == 0) {\n");
                    out.push_str("        return null;\n");
                    out.push_str("      }\n");
                    out.push_str("      final _RustBuffer resultBuf = resultOpt.value;\n");
                    out.push_str(
                        "      final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n",
                    );
                    out.push_str("      final int resultLen = resultBuf.len;\n");
                    out.push_str("      if (resultData == ffi.nullptr) {\n");
                    out.push_str("        if (resultLen == 0) {\n");
                    out.push_str("          _rustBytesFree(resultBuf);\n");
                    out.push_str("          return Uint8List(0);\n");
                    out.push_str("        }\n");
                    out.push_str(&format!(
                    "        throw StateError('Rust returned invalid optional buffer for {}');\n",
                    function.name
                ));
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str(
                        "        return Uint8List.fromList(resultData.asTypedList(resultLen));\n",
                    );
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustBytesFree(resultBuf);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_sequence_bytes_type(type_) => {
                    out.push_str(&format!(
                        "      final _RustBufferVec resultVec = {call_expr};\n"
                    ));
                    out.push_str(
                        "      final ffi.Pointer<_RustBuffer> resultData = resultVec.data;\n",
                    );
                    out.push_str("      final int resultLen = resultVec.len;\n");
                    out.push_str("      if (resultData == ffi.nullptr) {\n");
                    out.push_str("        if (resultLen == 0) {\n");
                    out.push_str("          _rustBytesVecFree(resultVec);\n");
                    out.push_str("          return <Uint8List>[];\n");
                    out.push_str("        }\n");
                    out.push_str(&format!(
                        "        throw StateError('Rust returned invalid byte vector for {}');\n",
                        function.name
                    ));
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str("        final out = <Uint8List>[];\n");
                    out.push_str("        for (var i = 0; i < resultLen; i++) {\n");
                    out.push_str("          final _RustBuffer item = (resultData + i).ref;\n");
                    out.push_str("          final ffi.Pointer<ffi.Uint8> itemData = item.data;\n");
                    out.push_str("          final int itemLen = item.len;\n");
                    out.push_str("          if (itemData == ffi.nullptr) {\n");
                    out.push_str("            if (itemLen == 0) {\n");
                    out.push_str("              out.add(Uint8List(0));\n");
                    out.push_str("              continue;\n");
                    out.push_str("            }\n");
                    out.push_str(&format!(
                    "            throw StateError('Rust returned invalid nested buffer for {}');\n",
                    function.name
                ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str(
                        "            out.add(Uint8List.fromList(itemData.asTypedList(itemLen)));\n",
                    );
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustBytesFree(item);\n");
                    out.push_str("          }\n");
                    out.push_str("        }\n");
                    out.push_str("        return out;\n");
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustBytesVecFree(resultVec);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_record_type(type_) => {
                    let record_name = record_name_from_type(type_).unwrap_or("Record");
                    out.push_str(&format!(
                        "      final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("      if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "        throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str("        final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                    "        return {}.fromJson(jsonDecode(payload) as Map<String, dynamic>);\n",
                    to_upper_camel(record_name)
                ));
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustStringFree(resultPtr);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_enum_type(type_, enums) => {
                    let enum_name = enum_name_from_type(type_).unwrap_or("Enum");
                    out.push_str(&format!(
                        "      final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("      if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "        throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str("        final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "        return _decode{}(payload);\n",
                        to_upper_camel(enum_name)
                    ));
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustStringFree(resultPtr);\n");
                    out.push_str("      }\n");
                }
                Some(_) => {
                    out.push_str(&format!("      return {call_expr};\n"));
                }
                None => {
                    out.push_str(&format!("      {call_expr};\n"));
                }
            }
        }
        if !post_call.is_empty() {
            out.push_str("    } finally {\n");
            for line in &post_call {
                out.push_str(line);
            }
            out.push_str("    }\n");
        }
        out.push_str("  }\n");
    }

    for object in objects {
        let object_name = to_upper_camel(&object.name);
        let object_lower = safe_dart_identifier(&to_lower_camel(&object.name));
        let object_symbol = dart_identifier(&object.name);
        let free_field = format!("_{}Free", object_lower);
        out.push('\n');
        out.push_str(&format!(
            "  late final void Function(int handle) {free_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle), void Function(int handle)>('{object_symbol}_free');\n"
        ));

        for ctor in &object.constructors {
            if !ctor
                .args
                .iter()
                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums))
            {
                continue;
            }
            let ctor_camel = to_upper_camel(&ctor.name);
            let ctor_field = format!("_{}Ctor{}", object_lower, ctor_camel);
            let ctor_method = format!("{}Create{}", object_lower, ctor_camel);
            let ctor_symbol = format!("{}_{}", object_symbol, dart_identifier(&ctor.name));
            let is_throwing = ctor.throws_type.is_some();
            let native_return = if is_throwing {
                "ffi.Pointer<Utf8>"
            } else {
                "ffi.Uint64"
            };
            let dart_return = if is_throwing {
                "ffi.Pointer<Utf8>"
            } else {
                "int"
            };
            let mut native_args = Vec::new();
            let mut dart_args = Vec::new();
            let mut dart_ffi_args = Vec::new();
            let mut call_args = Vec::new();
            let mut pre_call = Vec::new();
            let mut post_call = Vec::new();
            let mut signature_compatible = true;
            for arg in &ctor.args {
                let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                let Some(native_ty) = map_runtime_native_ffi_type(&arg.type_, records, enums)
                else {
                    signature_compatible = false;
                    break;
                };
                let Some(dart_ffi_ty) = map_runtime_dart_ffi_type(&arg.type_, records, enums)
                else {
                    signature_compatible = false;
                    break;
                };
                native_args.push(format!("{native_ty} {arg_name}"));
                dart_ffi_args.push(format!("{dart_ffi_ty} {arg_name}"));
                dart_args.push(format!(
                    "{} {arg_name}",
                    map_uniffi_type_to_dart(&arg.type_)
                ));
                append_runtime_arg_marshalling(
                    &arg_name,
                    &arg.type_,
                    enums,
                    &mut pre_call,
                    &mut post_call,
                    &mut call_args,
                );
            }
            if !signature_compatible {
                continue;
            }
            out.push('\n');
            out.push_str(&format!(
                "  late final {dart_return} Function({}) {ctor_field} = _lib.lookupFunction<{native_return} Function({}), {dart_return} Function({})>('{ctor_symbol}');\n",
                dart_ffi_args.join(", "),
                native_args.join(", "),
                dart_ffi_args.join(", ")
            ));
            out.push('\n');
            out.push_str(&format!(
                "  {object_name} {ctor_method}({}) {{\n",
                dart_args.join(", ")
            ));
            for line in &pre_call {
                out.push_str(line);
            }
            if !post_call.is_empty() {
                out.push_str("    try {\n");
            }
            if is_throwing {
                let Some(throws_name) = ctor
                    .throws_type
                    .as_ref()
                    .and_then(enum_name_from_type)
                    .map(to_upper_camel)
                else {
                    continue;
                };
                out.push_str(&format!(
                    "    final ffi.Pointer<Utf8> resultPtr = {ctor_field}({});\n",
                    call_args.join(", ")
                ));
                out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                out.push_str(&format!(
                    "      throw StateError('Rust returned null for {}');\n",
                    ctor_symbol
                ));
                out.push_str("    }\n");
                out.push_str("    final String payload;\n");
                out.push_str("    try {\n");
                out.push_str("      payload = resultPtr.toDartString();\n");
                out.push_str("    } finally {\n");
                out.push_str("      _rustStringFree(resultPtr);\n");
                out.push_str("    }\n");
                out.push_str(
                    "    final Map<String, dynamic> envelope = jsonDecode(payload) as Map<String, dynamic>;\n",
                );
                out.push_str("    final Object? errRaw = envelope['err'];\n");
                out.push_str("    if (errRaw != null) {\n");
                out.push_str(&format!(
                    "      throw _decode{}Exception(errRaw);\n",
                    throws_name
                ));
                out.push_str("    }\n");
                out.push_str("    final Object? okRaw = envelope['ok'];\n");
                out.push_str("    final int handle = (okRaw as num).toInt();\n");
                out.push_str(&format!("    return {object_name}._(this, handle);\n"));
            } else {
                out.push_str(&format!(
                    "    final int handle = {ctor_field}({});\n",
                    call_args.join(", ")
                ));
                out.push_str(&format!("    return {object_name}._(this, handle);\n"));
            }
            if !post_call.is_empty() {
                out.push_str("    } finally {\n");
                for line in &post_call {
                    out.push_str(line);
                }
                out.push_str("    }\n");
            }
            out.push_str("  }\n");
        }

        for method in &object.methods {
            let has_callback_args = has_runtime_callback_args_in_args(
                &method.args,
                callback_interfaces,
                records,
                enums,
            );
            let supported_return = method
                .return_type
                .as_ref()
                .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
                .unwrap_or(true);
            let supports_runtime_args = method
                .args
                .iter()
                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums));
            if !has_callback_args && (!supported_return || !supports_runtime_args) {
                continue;
            }
            let method_camel = to_upper_camel(&method.name);
            let method_field = format!("_{}{}", object_lower, method_camel);
            let method_invoke = format!("{}Invoke{}", object_lower, method_camel);
            let method_symbol = format!("{}_{}", object_symbol, dart_identifier(&method.name));
            let is_throwing = method.throws_type.is_some();

            let mut native_args = vec!["ffi.Uint64 handle".to_string()];
            let mut dart_ffi_args = vec!["int handle".to_string()];
            let mut dart_args = vec!["int handle".to_string()];
            let mut call_args = vec!["handle".to_string()];
            let mut pre_call = Vec::new();
            let mut post_call = Vec::new();
            let mut signature_compatible = true;
            for arg in &method.args {
                let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                dart_args.push(format!(
                    "{} {arg_name}",
                    map_uniffi_type_to_dart(&arg.type_)
                ));
                if has_callback_args {
                    if let Some(callback_name) = callback_interface_name_from_type(&arg.type_) {
                        let init_done_field = callback_init_done_field_name(callback_name);
                        let bridge_name = callback_bridge_class_name(callback_name);
                        let handle_name = format!("{arg_name}Handle");
                        native_args.push(format!("ffi.Uint64 {arg_name}"));
                        dart_ffi_args.push(format!("int {arg_name}"));
                        pre_call.push(format!("    {init_done_field};\n"));
                        pre_call.push(format!(
                            "    final int {handle_name} = {bridge_name}.instance.register({arg_name});\n"
                        ));
                        post_call.push(format!(
                            "    {bridge_name}.instance.release({handle_name});\n"
                        ));
                        call_args.push(handle_name);
                        continue;
                    }
                    let Some(native_ty) = map_runtime_native_ffi_type(&arg.type_, records, enums)
                    else {
                        signature_compatible = false;
                        break;
                    };
                    let Some(dart_ffi_ty) = map_runtime_dart_ffi_type(&arg.type_, records, enums)
                    else {
                        signature_compatible = false;
                        break;
                    };
                    native_args.push(format!("{native_ty} {arg_name}"));
                    dart_ffi_args.push(format!("{dart_ffi_ty} {arg_name}"));
                    append_runtime_arg_marshalling(
                        &arg_name,
                        &arg.type_,
                        enums,
                        &mut pre_call,
                        &mut post_call,
                        &mut call_args,
                    );
                } else {
                    let Some(native_ty) = map_runtime_native_ffi_type(&arg.type_, records, enums)
                    else {
                        signature_compatible = false;
                        break;
                    };
                    let Some(dart_ffi_ty) = map_runtime_dart_ffi_type(&arg.type_, records, enums)
                    else {
                        signature_compatible = false;
                        break;
                    };
                    native_args.push(format!("{native_ty} {arg_name}"));
                    dart_ffi_args.push(format!("{dart_ffi_ty} {arg_name}"));
                    append_runtime_arg_marshalling(
                        &arg_name,
                        &arg.type_,
                        enums,
                        &mut pre_call,
                        &mut post_call,
                        &mut call_args,
                    );
                }
            }
            if !signature_compatible {
                continue;
            }
            let return_type = method
                .return_type
                .as_ref()
                .map(map_uniffi_type_to_dart)
                .unwrap_or_else(|| "void".to_string());
            let native_return = if is_throwing {
                "ffi.Pointer<Utf8>".to_string()
            } else {
                method
                    .return_type
                    .as_ref()
                    .and_then(|t| map_runtime_native_ffi_type(t, records, enums))
                    .unwrap_or("ffi.Void")
                    .to_string()
            };
            let dart_return = if is_throwing {
                "ffi.Pointer<Utf8>".to_string()
            } else {
                method
                    .return_type
                    .as_ref()
                    .and_then(|t| map_runtime_dart_ffi_type(t, records, enums))
                    .unwrap_or("void")
                    .to_string()
            };

            if is_runtime_async_rust_future_compatible_method(
                method,
                callback_interfaces,
                records,
                enums,
            ) {
                let Some(async_spec) =
                    async_rust_future_spec(method.return_type.as_ref(), records, enums)
                else {
                    continue;
                };
                let start_native_sig = format!("ffi.Uint64 Function({})", native_args.join(", "));
                let start_dart_sig = format!("int Function({})", dart_ffi_args.join(", "));
                let poll_field = format!("{method_field}RustFuturePoll");
                let cancel_field = format!("{method_field}RustFutureCancel");
                let complete_field = format!("{method_field}RustFutureComplete");
                let free_field = format!("{method_field}RustFutureFree");
                let complete_symbol = format!("rust_future_complete_{}", async_spec.suffix);
                let poll_symbol = format!("rust_future_poll_{}", async_spec.suffix);
                let cancel_symbol = format!("rust_future_cancel_{}", async_spec.suffix);
                let free_symbol = format!("rust_future_free_{}", async_spec.suffix);
                let complete_native_sig = format!(
                    "{} Function(ffi.Uint64 handle, ffi.Pointer<_RustCallStatus> outStatus)",
                    async_spec.complete_native_type
                );
                let complete_dart_sig = format!(
                    "{} Function(int handle, ffi.Pointer<_RustCallStatus> outStatus)",
                    async_spec.complete_dart_type
                );

                out.push('\n');
                out.push_str(&format!(
                    "  late final {start_dart_sig} {method_field} = _lib.lookupFunction<{start_native_sig}, {start_dart_sig}>('{method_symbol}');\n",
                ));
                out.push_str(&format!(
                    "  late final void Function(int handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, int callbackData) {poll_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, ffi.Uint64 callbackData), void Function(int handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, int callbackData)>('{poll_symbol}');\n"
                ));
                out.push_str(&format!(
                    "  late final void Function(int handle) {cancel_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle), void Function(int handle)>('{cancel_symbol}');\n"
                ));
                out.push_str(&format!(
                    "  late final {complete_dart_sig} {complete_field} = _lib.lookupFunction<{complete_native_sig}, {complete_dart_sig}>('{complete_symbol}');\n"
                ));
                out.push_str(&format!(
                    "  late final void Function(int handle) {free_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle), void Function(int handle)>('{free_symbol}');\n"
                ));
                out.push('\n');
                out.push_str(&format!(
                    "  Future<{return_type}> {method_invoke}({}) async {{\n",
                    dart_args.join(", ")
                ));
                for line in &pre_call {
                    out.push_str(line);
                }
                out.push_str("    final int futureHandle;\n");
                if !post_call.is_empty() {
                    out.push_str("    try {\n");
                    out.push_str(&format!(
                        "      futureHandle = {method_field}({});\n",
                        call_args.join(", ")
                    ));
                    out.push_str("    } finally {\n");
                    for line in &post_call {
                        out.push_str(line);
                    }
                    out.push_str("    }\n");
                } else {
                    out.push_str(&format!(
                        "    futureHandle = {method_field}({});\n",
                        call_args.join(", ")
                    ));
                }
                out.push_str(
                    "    final StreamController<int> pollEvents = StreamController<int>.broadcast();\n",
                );
                out.push_str(
                    "    final callback = ffi.NativeCallable<ffi.Void Function(ffi.Uint64, ffi.Int8)>.listener((int _, int pollResult) {\n",
                );
                out.push_str("      pollEvents.add(pollResult);\n");
                out.push_str("    });\n");
                out.push_str("    try {\n");
                out.push_str(&format!(
                    "      {poll_field}(futureHandle, callback.nativeFunction, 0);\n"
                ));
                out.push_str("      while (true) {\n");
                out.push_str("        final int pollResult = await pollEvents.stream.first;\n");
                out.push_str("        if (pollResult == _rustFuturePollReady) {\n");
                out.push_str("          break;\n");
                out.push_str("        }\n");
                out.push_str("        if (pollResult == _rustFuturePollWake) {\n");
                out.push_str(&format!(
                    "          {poll_field}(futureHandle, callback.nativeFunction, 0);\n"
                ));
                out.push_str("          continue;\n");
                out.push_str("        }\n");
                out.push_str(&format!(
                    "        throw StateError('Rust future poll returned invalid status for {}: $pollResult');\n",
                    method_symbol
                ));
                out.push_str("      }\n");
                out.push_str(
                    "      final ffi.Pointer<_RustCallStatus> outStatusPtr = calloc<_RustCallStatus>();\n",
                );
                out.push_str("      try {\n");
                if method.return_type.is_none() {
                    out.push_str(&format!(
                        "        {complete_field}(futureHandle, outStatusPtr);\n"
                    ));
                } else if method.return_type.as_ref().is_some_and(|t| {
                    is_runtime_string_type(t)
                        || is_runtime_optional_string_type(t)
                        || is_runtime_record_type(t)
                        || is_runtime_enum_type(t, enums)
                }) {
                    out.push_str(&format!(
                        "        final ffi.Pointer<Utf8> resultPtr = {complete_field}(futureHandle, outStatusPtr);\n"
                    ));
                } else {
                    out.push_str(&format!(
                        "        final {} resultValue = {complete_field}(futureHandle, outStatusPtr);\n",
                        async_spec.complete_dart_type
                    ));
                }
                out.push_str("        final int statusCode = outStatusPtr.ref.code;\n");
                out.push_str("        if (statusCode == _rustCallStatusSuccess) {\n");
                if method.return_type.is_none() {
                    out.push_str("          return;\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_string_type)
                {
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            return resultPtr.toDartString();\n");
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_optional_string_type)
                {
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("            return null;\n");
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            return resultPtr.toDartString();\n");
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_record_type)
                {
                    let record_name = method
                        .return_type
                        .as_ref()
                        .and_then(record_name_from_type)
                        .unwrap_or("Record");
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "            return {}.fromJson(jsonDecode(payload) as Map<String, dynamic>);\n",
                        to_upper_camel(record_name)
                    ));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(|t| is_runtime_enum_type(t, enums))
                {
                    let enum_name = method
                        .return_type
                        .as_ref()
                        .and_then(enum_name_from_type)
                        .unwrap_or("Enum");
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "            return _decode{}(payload);\n",
                        to_upper_camel(enum_name)
                    ));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_timestamp_type)
                {
                    out.push_str(
                        "          return DateTime.fromMicrosecondsSinceEpoch(resultValue, isUtc: true);\n",
                    );
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_duration_type)
                {
                    out.push_str("          return Duration(microseconds: resultValue);\n");
                } else if let Some(ret_type) = method.return_type.as_ref() {
                    let decode = render_plain_ffi_decode_expr(ret_type, "resultValue");
                    out.push_str(&format!("          return {decode};\n"));
                }
                out.push_str("        }\n");
                out.push_str("        if (statusCode == _rustCallStatusCancelled) {\n");
                out.push_str(&format!(
                    "          throw StateError('Rust future was cancelled for {}');\n",
                    method_symbol
                ));
                out.push_str("        }\n");
                out.push_str(
                    "        final ffi.Pointer<Utf8> errorPtr = outStatusPtr.ref.errorBuf;\n",
                );
                out.push_str("        if (errorPtr != ffi.nullptr) {\n");
                out.push_str("          try {\n");
                out.push_str("            throw StateError(errorPtr.toDartString());\n");
                out.push_str("          } finally {\n");
                out.push_str("            _rustStringFree(errorPtr);\n");
                out.push_str("          }\n");
                out.push_str("        }\n");
                out.push_str(&format!(
                    "        throw StateError('Rust future failed for {} with status code: $statusCode');\n",
                    method_symbol
                ));
                out.push_str("      } finally {\n");
                out.push_str("        calloc.free(outStatusPtr);\n");
                out.push_str("      }\n");
                out.push_str("    } catch (_) {\n");
                out.push_str(&format!("      {cancel_field}(futureHandle);\n"));
                out.push_str("      rethrow;\n");
                out.push_str("    } finally {\n");
                out.push_str("      await pollEvents.close();\n");
                out.push_str("      callback.close();\n");
                out.push_str(&format!("      {free_field}(futureHandle);\n"));
                out.push_str("    }\n");
                out.push_str("  }\n");
                continue;
            }

            out.push('\n');
            out.push_str(&format!(
                "  late final {dart_return} Function({}) {method_field} = _lib.lookupFunction<{native_return} Function({}), {dart_return} Function({})>('{method_symbol}');\n",
                dart_ffi_args.join(", "),
                native_args.join(", "),
                dart_ffi_args.join(", ")
            ));
            out.push('\n');
            out.push_str(&format!(
                "  {return_type} {method_invoke}({}) {{\n",
                dart_args.join(", ")
            ));
            for line in &pre_call {
                out.push_str(line);
            }
            if !post_call.is_empty() {
                out.push_str("    try {\n");
            }
            if is_throwing {
                let Some(throws_name) = method
                    .throws_type
                    .as_ref()
                    .and_then(enum_name_from_type)
                    .map(to_upper_camel)
                else {
                    continue;
                };
                out.push_str(&format!(
                    "    final ffi.Pointer<Utf8> resultPtr = {method_field}({});\n",
                    call_args.join(", ")
                ));
                out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                out.push_str(&format!(
                    "      throw StateError('Rust returned null for {}');\n",
                    method_symbol
                ));
                out.push_str("    }\n");
                out.push_str("    final String payload;\n");
                out.push_str("    try {\n");
                out.push_str("      payload = resultPtr.toDartString();\n");
                out.push_str("    } finally {\n");
                out.push_str("      _rustStringFree(resultPtr);\n");
                out.push_str("    }\n");
                out.push_str(
                    "    final Map<String, dynamic> envelope = jsonDecode(payload) as Map<String, dynamic>;\n",
                );
                out.push_str("    final Object? errRaw = envelope['err'];\n");
                out.push_str("    if (errRaw != null) {\n");
                out.push_str(&format!(
                    "      throw _decode{}Exception(errRaw);\n",
                    throws_name
                ));
                out.push_str("    }\n");
                if let Some(ret_type) = method.return_type.as_ref() {
                    out.push_str("    final Object? okRaw = envelope['ok'];\n");
                    let decode = render_json_decode_expr("okRaw", ret_type);
                    out.push_str(&format!("    return {decode};\n"));
                } else {
                    out.push_str("    return;\n");
                }
            } else if let Some(ret) = &method.return_type {
                let call_expr = format!("{method_field}({})", call_args.join(", "));
                if is_runtime_string_type(ret) {
                    out.push_str(&format!(
                        "    final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "      throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str("      return resultPtr.toDartString();\n");
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustStringFree(resultPtr);\n");
                    out.push_str("    }\n");
                } else if is_runtime_optional_string_type(ret) {
                    out.push_str(&format!(
                        "    final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("      return null;\n");
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str("      return resultPtr.toDartString();\n");
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustStringFree(resultPtr);\n");
                    out.push_str("    }\n");
                } else if is_runtime_record_type(ret) {
                    let record_name = record_name_from_type(ret).unwrap_or("Record");
                    out.push_str(&format!(
                        "    final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "      throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str("      final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "      return {}.fromJson(jsonDecode(payload) as Map<String, dynamic>);\n",
                        to_upper_camel(record_name)
                    ));
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustStringFree(resultPtr);\n");
                    out.push_str("    }\n");
                } else if is_runtime_enum_type(ret, enums) {
                    let enum_name = enum_name_from_type(ret).unwrap_or("Enum");
                    out.push_str(&format!(
                        "    final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "      throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str("      final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "      return _decode{}(payload);\n",
                        to_upper_camel(enum_name)
                    ));
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustStringFree(resultPtr);\n");
                    out.push_str("    }\n");
                } else if is_runtime_bytes_type(ret) {
                    out.push_str(&format!("    final _RustBuffer resultBuf = {call_expr};\n"));
                    out.push_str("    final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n");
                    out.push_str("    final int resultLen = resultBuf.len;\n");
                    out.push_str("    if (resultData == ffi.nullptr) {\n");
                    out.push_str("      if (resultLen == 0) {\n");
                    out.push_str("        _rustBytesFree(resultBuf);\n");
                    out.push_str("        return Uint8List(0);\n");
                    out.push_str("      }\n");
                    out.push_str(&format!(
                        "      throw StateError('Rust returned invalid buffer for {}');\n",
                        method_symbol
                    ));
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str(
                        "      return Uint8List.fromList(resultData.asTypedList(resultLen));\n",
                    );
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustBytesFree(resultBuf);\n");
                    out.push_str("    }\n");
                } else {
                    let decode = render_plain_ffi_decode_expr(ret, &call_expr);
                    out.push_str(&format!("    return {decode};\n"));
                }
            } else {
                out.push_str(&format!("    {method_field}({});\n", call_args.join(", ")));
            }
            if !post_call.is_empty() {
                out.push_str("    } finally {\n");
                for line in &post_call {
                    out.push_str(line);
                }
                out.push_str("    }\n");
            }
            out.push_str("  }\n");
        }
    }

    out
}

fn render_object_classes(
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    ffi_class_name: &str,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let mut out = String::new();
    for object in objects {
        let object_name = to_upper_camel(&object.name);
        let object_lower = safe_dart_identifier(&to_lower_camel(&object.name));
        let free_field = format!("_{}Free", object_lower);
        let token_name = format!("_{}FinalizerToken", object_name);
        out.push('\n');
        out.push_str(&format!(
            "final class {token_name} {{\n  const {token_name}(this.free, this.handle);\n  final void Function(int) free;\n  final int handle;\n}}\n\n"
        ));
        out.push_str(&format!("final class {object_name} {{\n"));
        out.push_str(&format!("  {object_name}._(this._ffi, this._handle) {{\n"));
        out.push_str(&format!(
            "    _finalizer.attach(this, {token_name}(_ffi.{free_field}, _handle), detach: this);\n"
        ));
        out.push_str("  }\n\n");
        out.push_str(&format!("  final {ffi_class_name} _ffi;\n"));
        out.push_str("  int _handle;\n");
        out.push_str("  bool _closed = false;\n\n");
        out.push_str(&format!(
            "  static final Finalizer<{token_name}> _finalizer = Finalizer((token) {{\n"
        ));
        out.push_str("    token.free(token.handle);\n");
        out.push_str("  });\n\n");
        out.push_str("  bool get isClosed => _closed;\n\n");
        out.push_str("  void close() {\n");
        out.push_str("    if (_closed) {\n");
        out.push_str("      return;\n");
        out.push_str("    }\n");
        out.push_str("    _closed = true;\n");
        out.push_str("    _finalizer.detach(this);\n");
        out.push_str(&format!("    _ffi.{free_field}(_handle);\n"));
        out.push_str("  }\n\n");
        out.push_str("  void _ensureOpen() {\n");
        out.push_str("    if (_closed) {\n");
        out.push_str(&format!(
            "      throw StateError('{object_name} is closed');\n"
        ));
        out.push_str("    }\n");
        out.push_str("  }\n\n");

        for ctor in &object.constructors {
            if !ctor
                .args
                .iter()
                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums))
            {
                continue;
            }
            let ctor_camel = to_upper_camel(&ctor.name);
            let ctor_invoker = format!("{}Create{}", object_lower, ctor_camel);
            let static_name = if ctor.name == "new" {
                "create".to_string()
            } else {
                safe_dart_identifier(&to_lower_camel(&ctor.name))
            };
            let args = ctor
                .args
                .iter()
                .map(|a| {
                    format!(
                        "{} {}",
                        map_uniffi_type_to_dart(&a.type_),
                        safe_dart_identifier(&to_lower_camel(&a.name))
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let arg_names = ctor
                .args
                .iter()
                .map(|a| safe_dart_identifier(&to_lower_camel(&a.name)))
                .collect::<Vec<_>>()
                .join(", ");
            let invoke_expr = format!("_bindings().{ctor_invoker}({arg_names})");
            let signature_return = if ctor.is_async {
                format!("Future<{object_name}>")
            } else {
                object_name.clone()
            };
            out.push_str(&format!(
                "  static {signature_return} {static_name}({args}) {{\n"
            ));
            if ctor.is_async {
                out.push_str(&format!("    return Future(() => {invoke_expr});\n"));
            } else {
                out.push_str(&format!("    return {invoke_expr};\n"));
            }
            out.push_str("  }\n\n");
        }

        for method in &object.methods {
            let has_callback_args = has_runtime_callback_args_in_args(
                &method.args,
                callback_interfaces,
                records,
                enums,
            );
            let supported_return = method
                .return_type
                .as_ref()
                .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
                .unwrap_or(true);
            let supports_runtime_args = method
                .args
                .iter()
                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums));
            if !has_callback_args && (!supported_return || !supports_runtime_args) {
                continue;
            }
            let method_name = safe_dart_identifier(&to_lower_camel(&method.name));
            let method_camel = to_upper_camel(&method.name);
            let invoke_name = format!("{}Invoke{}", object_lower, method_camel);
            let return_type = method
                .return_type
                .as_ref()
                .map(map_uniffi_type_to_dart)
                .unwrap_or_else(|| "void".to_string());
            let signature_return = if method.is_async {
                format!("Future<{return_type}>")
            } else {
                return_type.clone()
            };
            let args = method
                .args
                .iter()
                .map(|a| {
                    format!(
                        "{} {}",
                        map_uniffi_type_to_dart(&a.type_),
                        safe_dart_identifier(&to_lower_camel(&a.name))
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let arg_names = method
                .args
                .iter()
                .map(|a| safe_dart_identifier(&to_lower_camel(&a.name)))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("  {signature_return} {method_name}({args}) {{\n"));
            out.push_str("    _ensureOpen();\n");
            let invoke_args = if arg_names.is_empty() {
                "_handle".to_string()
            } else {
                format!("_handle, {arg_names}")
            };
            if method.is_async {
                if is_runtime_async_rust_future_compatible_method(
                    method,
                    callback_interfaces,
                    records,
                    enums,
                ) {
                    out.push_str(&format!("    return _ffi.{invoke_name}({invoke_args});\n"));
                } else {
                    out.push_str(&format!(
                        "    return Future(() => _ffi.{invoke_name}({invoke_args}));\n"
                    ));
                }
            } else if method.return_type.is_some() {
                out.push_str(&format!("    return _ffi.{invoke_name}({invoke_args});\n"));
            } else {
                out.push_str(&format!("    _ffi.{invoke_name}({invoke_args});\n"));
            }
            out.push_str("  }\n\n");
        }
        out.push_str("}\n");
    }

    out
}

fn render_function_stubs(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    ffi_class_name: &str,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    if functions.is_empty() && objects.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    let has_runtime_functions = functions.iter().any(|f| {
        is_runtime_ffi_compatible_function(f, records, enums)
            || is_runtime_callback_compatible_function(f, callback_interfaces, records, enums)
            || has_runtime_callback_args_in_args(&f.args, callback_interfaces, records, enums)
    }) || !objects.is_empty();
    out.push('\n');
    if has_runtime_functions {
        out.push_str(&format!("{ffi_class_name}? _defaultBindings;\n\n"));
        out.push_str(&format!(
            "{ffi_class_name} _bindings() => _defaultBindings ??= {ffi_class_name}();\n\n"
        ));
        out.push_str(
            "void configureDefaultBindings({ffi.DynamicLibrary? dynamicLibrary, String? libraryPath}) {\n",
        );
        out.push_str(&format!(
            "  _defaultBindings = {ffi_class_name}(dynamicLibrary: dynamicLibrary, libraryPath: libraryPath);\n"
        ));
        out.push_str("}\n\n");
        out.push_str("void resetDefaultBindings() {\n");
        out.push_str("  _defaultBindings = null;\n");
        out.push_str("}\n\n");
    }
    for f in functions {
        let fn_name = safe_dart_identifier(&to_lower_camel(&f.name));
        let value_return_type = f
            .return_type
            .as_ref()
            .map(map_uniffi_type_to_dart)
            .unwrap_or_else(|| "void".to_string());
        let signature_return_type = if f.is_async {
            format!("Future<{value_return_type}>")
        } else {
            value_return_type.clone()
        };
        let args = f
            .args
            .iter()
            .map(|a| {
                format!(
                    "{} {}",
                    map_uniffi_type_to_dart(&a.type_),
                    safe_dart_identifier(&to_lower_camel(&a.name))
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let arg_names = f
            .args
            .iter()
            .map(|a| safe_dart_identifier(&to_lower_camel(&a.name)))
            .collect::<Vec<_>>()
            .join(", ");

        out.push_str(&format!("{signature_return_type} {fn_name}({args}) {{\n"));
        if is_runtime_ffi_compatible_function(f, records, enums)
            || is_runtime_callback_compatible_function(f, callback_interfaces, records, enums)
            || has_runtime_callback_args_in_args(&f.args, callback_interfaces, records, enums)
        {
            if f.is_async {
                if is_runtime_async_rust_future_compatible_function(
                    f,
                    callback_interfaces,
                    records,
                    enums,
                ) {
                    out.push_str(&format!("  return _bindings().{fn_name}({arg_names});\n"));
                } else {
                    out.push_str(&format!(
                        "  return Future(() => _bindings().{fn_name}({arg_names}));\n"
                    ));
                }
            } else if f.return_type.is_some() {
                out.push_str(&format!("  return _bindings().{fn_name}({arg_names});\n"));
            } else {
                out.push_str(&format!("  _bindings().{fn_name}({arg_names});\n"));
            }
        } else if f.is_async {
            out.push_str("  return Future.error(UnimplementedError('TODO: bind to Rust FFI'));\n");
        } else {
            out.push_str("  throw UnimplementedError('TODO: bind to Rust FFI');\n");
        }
        out.push_str("}\n\n");
    }
    out
}

fn has_runtime_callback_support(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    functions
        .iter()
        .any(|f| has_runtime_callback_args_in_args(&f.args, callback_interfaces, records, enums))
        || objects.iter().any(|o| {
            o.methods.iter().any(|m| {
                has_runtime_callback_args_in_args(&m.args, callback_interfaces, records, enums)
            })
        })
}

fn is_runtime_callback_compatible_function(
    function: &UdlFunction,
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    if function.is_async || function.throws_type.is_some() {
        return false;
    }
    let return_supported = function
        .return_type
        .as_ref()
        .map(is_runtime_callback_function_return_compatible_type)
        .unwrap_or(true);
    if !return_supported {
        return false;
    }
    has_runtime_callback_args_in_args(&function.args, callback_interfaces, records, enums)
}

fn callback_interfaces_used_for_runtime<'a>(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &'a [UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> Vec<&'a UdlCallbackInterface> {
    callback_interfaces
        .iter()
        .filter(|callback_interface| {
            is_runtime_callback_interface_compatible(callback_interface, records, enums)
                && (functions.iter().any(|function| {
                    has_runtime_callback_args_in_args(
                        &function.args,
                        callback_interfaces,
                        records,
                        enums,
                    ) && function.args.iter().any(|arg| {
                        callback_interface_name_from_type(&arg.type_)
                            .is_some_and(|name| name == callback_interface.name)
                    })
                }) || objects.iter().any(|object| {
                    object.methods.iter().any(|method| {
                        has_runtime_callback_args_in_args(
                            &method.args,
                            callback_interfaces,
                            records,
                            enums,
                        ) && method.args.iter().any(|arg| {
                            callback_interface_name_from_type(&arg.type_)
                                .is_some_and(|name| name == callback_interface.name)
                        })
                    })
                }))
        })
        .collect()
}

fn runtime_args_compatible_with_optional_callbacks(
    args: &[UdlArg],
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> Option<bool> {
    let mut saw_callback = false;
    for arg in args {
        if let Some(callback_name) = callback_interface_name_from_type(&arg.type_) {
            saw_callback = true;
            let callback_interface = callback_interfaces
                .iter()
                .find(|cb| cb.name == callback_name)?;
            if !is_runtime_callback_interface_compatible(callback_interface, records, enums) {
                return None;
            }
            continue;
        }
        if !is_runtime_ffi_compatible_type(&arg.type_, records, enums) {
            return None;
        }
    }
    Some(saw_callback)
}

fn has_runtime_callback_args_in_args(
    args: &[UdlArg],
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    runtime_args_compatible_with_optional_callbacks(args, callback_interfaces, records, enums)
        .unwrap_or(false)
}

fn callback_interface_name_from_type(type_: &Type) -> Option<&str> {
    match type_ {
        Type::CallbackInterface { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

fn is_runtime_callback_interface_compatible(
    callback_interface: &UdlCallbackInterface,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    callback_interface
        .methods
        .iter()
        .all(|method| is_runtime_callback_method_compatible(method, records, enums))
}

fn is_runtime_callback_method_compatible(
    method: &UdlCallbackMethod,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    method
        .throws_type
        .as_ref()
        .map(|t| {
            is_runtime_ffi_compatible_type(t, records, enums)
                && is_runtime_error_enum_type(t, enums)
        })
        .unwrap_or(true)
        && method
            .return_type
            .as_ref()
            .map(|t| {
                if method.is_async {
                    is_runtime_callback_async_return_type_compatible(t)
                } else {
                    is_runtime_callback_method_type_compatible(t, records, enums)
                }
            })
            .unwrap_or(true)
        && method
            .args
            .iter()
            .all(|arg| is_runtime_callback_method_type_compatible(&arg.type_, records, enums))
}

fn is_runtime_callback_function_return_compatible_type(type_: &Type) -> bool {
    matches!(
        type_,
        Type::UInt8
            | Type::Int8
            | Type::UInt16
            | Type::Int16
            | Type::UInt32
            | Type::Int32
            | Type::UInt64
            | Type::Int64
            | Type::Float32
            | Type::Float64
            | Type::Boolean
            | Type::Timestamp
            | Type::Duration
    )
}

fn is_runtime_callback_method_type_compatible(
    type_: &Type,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    is_runtime_callback_function_return_compatible_type(type_)
        || is_runtime_string_type(type_)
        || is_runtime_optional_string_type(type_)
        || records
            .iter()
            .any(|r| record_name_from_type(type_) == Some(r.name.as_str()))
        || is_runtime_enum_type(type_, enums)
}

fn is_runtime_callback_async_return_type_compatible(type_: &Type) -> bool {
    is_runtime_callback_function_return_compatible_type(type_)
}

fn callback_bridge_class_name(callback_name: &str) -> String {
    format!("_{}CallbackBridge", to_upper_camel(callback_name))
}

fn callback_vtable_struct_name(callback_name: &str) -> String {
    format!("_{}VTable", to_upper_camel(callback_name))
}

fn callback_init_symbol(callback_name: &str) -> String {
    format!("{}_callback_init", callback_name.to_ascii_lowercase())
}

fn callback_init_field_name(callback_name: &str) -> String {
    safe_dart_identifier(&format!("_{}CallbackInit", to_lower_camel(callback_name)))
}

fn callback_init_done_field_name(callback_name: &str) -> String {
    safe_dart_identifier(&format!(
        "_{}CallbackInitDone",
        to_lower_camel(callback_name)
    ))
}

fn callback_vtable_field_name(callback_name: &str) -> String {
    safe_dart_identifier(&format!("_{}CallbackVTable", to_lower_camel(callback_name)))
}

fn callback_async_result_struct_name(callback_name: &str, method_name: &str) -> String {
    format!(
        "_{}{}AsyncResult",
        to_upper_camel(callback_name),
        to_upper_camel(method_name)
    )
}

fn render_callback_async_result_return_field(type_: &Type) -> Option<String> {
    match type_ {
        Type::UInt8 => Some("  @ffi.Uint8()\n  external int returnValue;\n\n".to_string()),
        Type::Int8 => Some("  @ffi.Int8()\n  external int returnValue;\n\n".to_string()),
        Type::UInt16 => Some("  @ffi.Uint16()\n  external int returnValue;\n\n".to_string()),
        Type::Int16 => Some("  @ffi.Int16()\n  external int returnValue;\n\n".to_string()),
        Type::UInt32 => Some("  @ffi.Uint32()\n  external int returnValue;\n\n".to_string()),
        Type::Int32 => Some("  @ffi.Int32()\n  external int returnValue;\n\n".to_string()),
        Type::UInt64 => Some("  @ffi.Uint64()\n  external int returnValue;\n\n".to_string()),
        Type::Int64 => Some("  @ffi.Int64()\n  external int returnValue;\n\n".to_string()),
        Type::Float32 => Some("  @ffi.Float()\n  external double returnValue;\n\n".to_string()),
        Type::Float64 => Some("  @ffi.Double()\n  external double returnValue;\n\n".to_string()),
        Type::Boolean => Some("  @ffi.Uint8()\n  external int returnValue;\n\n".to_string()),
        Type::Timestamp | Type::Duration => {
            Some("  @ffi.Int64()\n  external int returnValue;\n\n".to_string())
        }
        _ => None,
    }
}

fn callback_async_default_return_expr(type_: &Type) -> &'static str {
    match type_ {
        Type::Float32 | Type::Float64 => "0.0",
        _ => "0",
    }
}

fn render_callback_arg_decode_expr(
    type_: &Type,
    arg_name: &str,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    match type_ {
        Type::String => format!(
            "{arg_name} == ffi.nullptr ? (throw StateError('Rust passed null string callback arg')) : {arg_name}.toDartString()"
        ),
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
            format!("{arg_name} == ffi.nullptr ? null : {arg_name}.toDartString()")
        }
        Type::Record { .. } if records
            .iter()
            .any(|r| record_name_from_type(type_) == Some(r.name.as_str())) =>
        {
            let record_name = record_name_from_type(type_).unwrap_or("Record");
            format!(
                "{arg_name} == ffi.nullptr ? (throw StateError('Rust passed null record callback arg')) : {}.fromJson(jsonDecode({arg_name}.toDartString()) as Map<String, dynamic>)",
                to_upper_camel(record_name)
            )
        }
        Type::Enum { .. } if is_runtime_enum_type(type_, enums) => {
            let enum_name = enum_name_from_type(type_).unwrap_or("Enum");
            format!(
                "{arg_name} == ffi.nullptr ? (throw StateError('Rust passed null enum callback arg')) : _decode{}({arg_name}.toDartString())",
                to_upper_camel(enum_name)
            )
        }
        Type::Timestamp => {
            format!("DateTime.fromMicrosecondsSinceEpoch({arg_name}, isUtc: true)")
        }
        Type::Duration => format!("Duration(microseconds: {arg_name})"),
        _ => arg_name.to_string(),
    }
}

fn render_callback_return_encode_expr(
    type_: &Type,
    value_expr: &str,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    match type_ {
        Type::String => format!("{value_expr}.toNativeUtf8()"),
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
            format!("{value_expr} == null ? ffi.nullptr : {value_expr}.toNativeUtf8()")
        }
        Type::Record { .. }
            if records
                .iter()
                .any(|r| record_name_from_type(type_) == Some(r.name.as_str())) =>
        {
            format!("jsonEncode({value_expr}.toJson()).toNativeUtf8()")
        }
        Type::Enum { .. } if is_runtime_enum_type(type_, enums) => {
            let enum_name = enum_name_from_type(type_).unwrap_or("Enum");
            format!(
                "_encode{}({value_expr}).toNativeUtf8()",
                to_upper_camel(enum_name)
            )
        }
        Type::Timestamp => format!("{value_expr}.toUtc().microsecondsSinceEpoch"),
        Type::Duration => format!("{value_expr}.inMicroseconds"),
        Type::Boolean => format!("{value_expr} ? 1 : 0"),
        _ => value_expr.to_string(),
    }
}

fn has_runtime_async_rust_future_support(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    functions.iter().any(|f| {
        is_runtime_async_rust_future_compatible_function(f, callback_interfaces, records, enums)
    }) || objects.iter().any(|o| {
        o.methods.iter().any(|m| {
            is_runtime_async_rust_future_compatible_method(m, callback_interfaces, records, enums)
        })
    })
}

struct AsyncRustFutureSpec {
    suffix: &'static str,
    complete_native_type: &'static str,
    complete_dart_type: &'static str,
}

fn async_rust_future_spec(
    return_type: Option<&Type>,
    _records: &[UdlRecord],
    enums: &[UdlEnum],
) -> Option<AsyncRustFutureSpec> {
    match return_type {
        None => Some(AsyncRustFutureSpec {
            suffix: "void",
            complete_native_type: "ffi.Void",
            complete_dart_type: "void",
        }),
        Some(type_) if is_runtime_string_type(type_) => Some(AsyncRustFutureSpec {
            suffix: "string",
            complete_native_type: "ffi.Pointer<Utf8>",
            complete_dart_type: "ffi.Pointer<Utf8>",
        }),
        Some(type_) if is_runtime_optional_string_type(type_) => Some(AsyncRustFutureSpec {
            suffix: "string",
            complete_native_type: "ffi.Pointer<Utf8>",
            complete_dart_type: "ffi.Pointer<Utf8>",
        }),
        Some(type_) if is_runtime_record_type(type_) => Some(AsyncRustFutureSpec {
            suffix: "string",
            complete_native_type: "ffi.Pointer<Utf8>",
            complete_dart_type: "ffi.Pointer<Utf8>",
        }),
        Some(type_) if is_runtime_enum_type(type_, enums) => Some(AsyncRustFutureSpec {
            suffix: "string",
            complete_native_type: "ffi.Pointer<Utf8>",
            complete_dart_type: "ffi.Pointer<Utf8>",
        }),
        Some(Type::UInt8) => Some(AsyncRustFutureSpec {
            suffix: "u8",
            complete_native_type: "ffi.Uint8",
            complete_dart_type: "int",
        }),
        Some(Type::Int8) => Some(AsyncRustFutureSpec {
            suffix: "i8",
            complete_native_type: "ffi.Int8",
            complete_dart_type: "int",
        }),
        Some(Type::UInt16) => Some(AsyncRustFutureSpec {
            suffix: "u16",
            complete_native_type: "ffi.Uint16",
            complete_dart_type: "int",
        }),
        Some(Type::Int16) => Some(AsyncRustFutureSpec {
            suffix: "i16",
            complete_native_type: "ffi.Int16",
            complete_dart_type: "int",
        }),
        Some(Type::UInt32) => Some(AsyncRustFutureSpec {
            suffix: "u32",
            complete_native_type: "ffi.Uint32",
            complete_dart_type: "int",
        }),
        Some(Type::Int32) => Some(AsyncRustFutureSpec {
            suffix: "i32",
            complete_native_type: "ffi.Int32",
            complete_dart_type: "int",
        }),
        Some(Type::UInt64) => Some(AsyncRustFutureSpec {
            suffix: "u64",
            complete_native_type: "ffi.Uint64",
            complete_dart_type: "int",
        }),
        Some(Type::Int64) => Some(AsyncRustFutureSpec {
            suffix: "i64",
            complete_native_type: "ffi.Int64",
            complete_dart_type: "int",
        }),
        Some(Type::Float32) => Some(AsyncRustFutureSpec {
            suffix: "f32",
            complete_native_type: "ffi.Float",
            complete_dart_type: "double",
        }),
        Some(Type::Float64) => Some(AsyncRustFutureSpec {
            suffix: "f64",
            complete_native_type: "ffi.Double",
            complete_dart_type: "double",
        }),
        Some(Type::Timestamp) => Some(AsyncRustFutureSpec {
            suffix: "i64",
            complete_native_type: "ffi.Int64",
            complete_dart_type: "int",
        }),
        Some(Type::Duration) => Some(AsyncRustFutureSpec {
            suffix: "i64",
            complete_native_type: "ffi.Int64",
            complete_dart_type: "int",
        }),
        _ => None,
    }
}

fn is_runtime_async_rust_future_compatible_function(
    function: &UdlFunction,
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    function.is_async
        && function.throws_type.is_none()
        && function
            .return_type
            .as_ref()
            .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
            .unwrap_or(true)
        && runtime_args_compatible_with_optional_callbacks(
            &function.args,
            callback_interfaces,
            records,
            enums,
        )
        .is_some()
        && async_rust_future_spec(function.return_type.as_ref(), records, enums).is_some()
}

fn is_runtime_async_rust_future_compatible_method(
    method: &UdlObjectMethod,
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    method.is_async
        && method.throws_type.is_none()
        && async_rust_future_spec(method.return_type.as_ref(), records, enums).is_some()
        && runtime_args_compatible_with_optional_callbacks(
            &method.args,
            callback_interfaces,
            records,
            enums,
        )
        .is_some()
}

fn is_runtime_ffi_compatible_function(
    function: &UdlFunction,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    function
        .return_type
        .as_ref()
        .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
        .unwrap_or(true)
        && function
            .args
            .iter()
            .all(|arg| is_runtime_ffi_compatible_type(&arg.type_, records, enums))
        && function
            .throws_type
            .as_ref()
            .map(|t| {
                is_runtime_ffi_compatible_type(t, records, enums)
                    && is_runtime_error_enum_type(t, enums)
            })
            .unwrap_or(true)
}

fn is_runtime_throwing_ffi_compatible_function(
    function: &UdlFunction,
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    function
        .throws_type
        .as_ref()
        .map(|t| {
            is_runtime_ffi_compatible_type(t, records, enums)
                && is_runtime_error_enum_type(t, enums)
        })
        .unwrap_or(false)
        && function
            .return_type
            .as_ref()
            .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
            .unwrap_or(true)
        && runtime_args_compatible_with_optional_callbacks(
            &function.args,
            callback_interfaces,
            records,
            enums,
        )
        .is_some()
}

fn is_runtime_ffi_compatible_type(type_: &Type, records: &[UdlRecord], enums: &[UdlEnum]) -> bool {
    map_runtime_native_ffi_type(type_, records, enums).is_some()
}

fn map_runtime_native_ffi_type(
    type_: &Type,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> Option<&'static str> {
    match type_ {
        Type::UInt8 => Some("ffi.Uint8"),
        Type::Int8 => Some("ffi.Int8"),
        Type::UInt16 => Some("ffi.Uint16"),
        Type::Int16 => Some("ffi.Int16"),
        Type::UInt32 => Some("ffi.Uint32"),
        Type::Int32 => Some("ffi.Int32"),
        Type::UInt64 => Some("ffi.Uint64"),
        Type::Int64 => Some("ffi.Int64"),
        Type::Float32 => Some("ffi.Float"),
        Type::Float64 => Some("ffi.Double"),
        Type::Boolean => Some("ffi.Bool"),
        Type::String => Some("ffi.Pointer<Utf8>"),
        Type::Timestamp => Some("ffi.Int64"),
        Type::Duration => Some("ffi.Int64"),
        Type::Bytes => Some("_RustBuffer"),
        Type::Optional { inner_type } if is_runtime_bytes_type(inner_type) => {
            Some("_RustBufferOpt")
        }
        Type::Sequence { inner_type } if is_runtime_bytes_type(inner_type) => {
            Some("_RustBufferVec")
        }
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
            Some("ffi.Pointer<Utf8>")
        }
        Type::Record { name, .. } if records.iter().any(|r| r.name == *name) => {
            Some("ffi.Pointer<Utf8>")
        }
        Type::Enum { name, .. } if enums.iter().any(|e| e.name == *name) => {
            Some("ffi.Pointer<Utf8>")
        }
        _ => None,
    }
}

fn map_runtime_dart_ffi_type(
    type_: &Type,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> Option<&'static str> {
    match type_ {
        Type::UInt8
        | Type::Int8
        | Type::UInt16
        | Type::Int16
        | Type::UInt32
        | Type::Int32
        | Type::UInt64
        | Type::Int64 => Some("int"),
        Type::Float32 | Type::Float64 => Some("double"),
        Type::Boolean => Some("bool"),
        Type::String => Some("ffi.Pointer<Utf8>"),
        Type::Timestamp | Type::Duration => Some("int"),
        Type::Bytes => Some("_RustBuffer"),
        Type::Optional { inner_type } if is_runtime_bytes_type(inner_type) => {
            Some("_RustBufferOpt")
        }
        Type::Sequence { inner_type } if is_runtime_bytes_type(inner_type) => {
            Some("_RustBufferVec")
        }
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
            Some("ffi.Pointer<Utf8>")
        }
        Type::Record { name, .. } if records.iter().any(|r| r.name == *name) => {
            Some("ffi.Pointer<Utf8>")
        }
        Type::Enum { name, .. } if enums.iter().any(|e| e.name == *name) => {
            Some("ffi.Pointer<Utf8>")
        }
        _ => None,
    }
}

fn is_runtime_string_type(type_: &Type) -> bool {
    matches!(type_, Type::String)
}

fn is_runtime_timestamp_type(type_: &Type) -> bool {
    matches!(type_, Type::Timestamp)
}

fn is_runtime_duration_type(type_: &Type) -> bool {
    matches!(type_, Type::Duration)
}

fn is_runtime_bytes_type(type_: &Type) -> bool {
    matches!(type_, Type::Bytes)
}

fn is_runtime_record_type(type_: &Type) -> bool {
    matches!(type_, Type::Record { .. })
}

fn is_runtime_enum_type(type_: &Type, enums: &[UdlEnum]) -> bool {
    let Some(name) = enum_name_from_type(type_) else {
        return false;
    };
    enums.iter().any(|e| e.name == name)
}

fn is_runtime_error_enum_type(type_: &Type, enums: &[UdlEnum]) -> bool {
    let Some(name) = enum_name_from_type(type_) else {
        return false;
    };
    enums.iter().any(|e| e.name == name && e.is_error)
}

fn is_runtime_record_or_enum_string_type(type_: &Type, enums: &[UdlEnum]) -> bool {
    is_runtime_record_type(type_) || is_runtime_enum_type(type_, enums)
}

fn enum_name_from_type(type_: &Type) -> Option<&str> {
    match type_ {
        Type::Enum { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

fn record_name_from_type(type_: &Type) -> Option<&str> {
    match type_ {
        Type::Record { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

fn is_runtime_optional_bytes_type(type_: &Type) -> bool {
    matches!(type_, Type::Optional { inner_type } if is_runtime_bytes_type(inner_type))
}

fn is_runtime_sequence_bytes_type(type_: &Type) -> bool {
    matches!(type_, Type::Sequence { inner_type } if is_runtime_bytes_type(inner_type))
}

fn is_runtime_bytes_like_type(type_: &Type) -> bool {
    is_runtime_bytes_type(type_)
        || is_runtime_optional_bytes_type(type_)
        || is_runtime_sequence_bytes_type(type_)
}

fn is_runtime_optional_string_type(type_: &Type) -> bool {
    matches!(type_, Type::Optional { inner_type } if is_runtime_string_type(inner_type))
}

fn is_runtime_string_like_type(type_: &Type) -> bool {
    is_runtime_string_type(type_) || is_runtime_optional_string_type(type_)
}

fn render_plain_ffi_decode_expr(type_: &Type, call_expr: &str) -> String {
    match type_ {
        Type::Timestamp => format!("DateTime.fromMicrosecondsSinceEpoch({call_expr}, isUtc: true)"),
        Type::Duration => format!("Duration(microseconds: {call_expr})"),
        _ => call_expr.to_string(),
    }
}

fn map_uniffi_type_to_dart(type_: &Type) -> String {
    match type_ {
        Type::UInt8
        | Type::Int8
        | Type::UInt16
        | Type::Int16
        | Type::UInt32
        | Type::Int32
        | Type::UInt64
        | Type::Int64 => "int".to_string(),
        Type::Float32 | Type::Float64 => "double".to_string(),
        Type::Boolean => "bool".to_string(),
        Type::String => "String".to_string(),
        Type::Bytes => "Uint8List".to_string(),
        Type::Timestamp => "DateTime".to_string(),
        Type::Duration => "Duration".to_string(),
        Type::Optional { inner_type } => format!("{}?", map_uniffi_type_to_dart(inner_type)),
        Type::Sequence { inner_type } => format!("List<{}>", map_uniffi_type_to_dart(inner_type)),
        Type::Map {
            key_type,
            value_type,
        } => format!(
            "Map<{}, {}>",
            map_uniffi_type_to_dart(key_type),
            map_uniffi_type_to_dart(value_type)
        ),
        Type::Enum { name, .. }
        | Type::Object { name, .. }
        | Type::Record { name, .. }
        | Type::Custom { name, .. }
        | Type::CallbackInterface { name, .. } => to_upper_camel(name),
    }
}

fn uniffi_type_uses_bytes(type_: &Type) -> bool {
    match type_ {
        Type::Bytes => true,
        Type::Optional { inner_type } | Type::Sequence { inner_type } => {
            uniffi_type_uses_bytes(inner_type)
        }
        Type::Map {
            key_type,
            value_type,
        } => uniffi_type_uses_bytes(key_type) || uniffi_type_uses_bytes(value_type),
        _ => false,
    }
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
                out.push(c);
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
                out.push(c);
            }
        }
    }
    if out.is_empty() {
        "value".to_string()
    } else {
        out
    }
}

fn safe_dart_identifier(input: &str) -> String {
    if is_dart_keyword(input) {
        format!("{input}_")
    } else {
        input.to_string()
    }
}

fn is_dart_keyword(input: &str) -> bool {
    matches!(
        input,
        "abstract"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "base"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "covariant"
            | "default"
            | "deferred"
            | "do"
            | "dynamic"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "extension"
            | "external"
            | "factory"
            | "false"
            | "final"
            | "finally"
            | "for"
            | "Function"
            | "get"
            | "hide"
            | "if"
            | "implements"
            | "import"
            | "in"
            | "interface"
            | "is"
            | "late"
            | "library"
            | "mixin"
            | "new"
            | "null"
            | "on"
            | "operator"
            | "part"
            | "required"
            | "rethrow"
            | "return"
            | "sealed"
            | "set"
            | "show"
            | "static"
            | "super"
            | "switch"
            | "sync"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typedef"
            | "var"
            | "void"
            | "when"
            | "while"
            | "with"
            | "yield"
    )
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
        fs::write(&source, "namespace simple_fns {};").expect("write source");

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
        fs::write(&source, "namespace demo {};").expect("write source");
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
    fn renders_async_functions_and_methods_as_futures() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("async_demo.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace async_demo {
  [Async]
  string greet_async(string name);
  [Async]
  u32 add_async(u32 left, u32 right);
  [Async]
  void tick_async();
};

interface Counter {
  constructor(u32 initial);
  [Async]
  string async_describe();
  [Async]
  u32 async_value();
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
        let content = fs::read_to_string(out_dir.join("async_demo.dart")).expect("read generated");
        assert!(content.contains("Future<String> greetAsync(String name) {"));
        assert!(content.contains("return _bindings().greetAsync(name);"));
        assert!(content.contains("Future<int> addAsync(int left, int right) {"));
        assert!(content.contains("Future<void> tickAsync() {"));
        assert!(content.contains("rust_future_poll_string"));
        assert!(content.contains("rust_future_complete_string"));
        assert!(content.contains("rust_future_poll_u32"));
        assert!(content.contains("rust_future_complete_u32"));
        assert!(content.contains("rust_future_poll_void"));
        assert!(content.contains("rust_future_complete_void"));
        assert!(content.contains("rust_future_free_string"));
        assert!(content.contains("final class _RustCallStatus extends ffi.Struct {"));
        assert!(content.contains("Future<String> asyncDescribe() {"));
        assert!(content.contains("return _ffi.counterInvokeAsyncDescribe(_handle);"));
        assert!(content.contains("Future<int> asyncValue() {"));
    }

    #[test]
    fn renders_runtime_callback_interface_bindings() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("callbacks.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace callbacks {
  u32 apply_adder(Adder adder, u32 left, u32 right);
  [Async]
  u32 apply_adder_async(Adder adder, u32 left, u32 right);
  [Throws=MathError]
  u32 checked_apply_adder(Adder adder, u32 left, u32 right);
  u32 apply_formatter(Formatter formatter, string? prefix, Person person, Outcome outcome);
};

callback interface Adder {
  u32 add(u32 left, u32 right);
  [Async]
  u32 add_async(u32 left, u32 right);
  [Throws=MathError]
  u32 checked_add(u32 left, u32 right);
};

callback interface Formatter {
  string format(string? prefix, Person person, Outcome outcome);
};

dictionary Person {
  string name;
  u32 age;
};

[Enum]
interface Outcome {
  Success(string message);
  Failure(i32 code, string reason);
};

interface Counter {
  constructor();
  u32 apply_adder_with(Adder adder, u32 left, u32 right);
  [Async]
  u32 apply_adder_async_with(Adder adder, u32 left, u32 right);
  [Throws=MathError]
  u32 checked_apply_adder_with(Adder adder, u32 left, u32 right);
};

[Error]
interface MathError {
  DivisionByZero();
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
        let content = fs::read_to_string(out_dir.join("callbacks.dart")).expect("read generated");
        assert!(content.contains("abstract interface class Adder {"));
        assert!(content.contains("int add(int left, int right);"));
        assert!(content.contains("Future<int> addAsync(int left, int right);"));
        assert!(content.contains("int checkedAdd(int left, int right);"));
        assert!(content.contains("final class _AdderVTable extends ffi.Struct {"));
        assert!(content.contains("final class _AdderCallbackBridge {"));
        assert!(content
            .contains("final class _ForeignFutureDroppedCallbackStruct extends ffi.Struct {"));
        assert!(content.contains("final class _AdderAddAsyncAsyncResult extends ffi.Struct {"));
        assert!(content.contains("lookupFunction<ffi.Void Function(ffi.Pointer<_AdderVTable>)"));
        assert!(content.contains("'adder_callback_init'"));
        assert!(content.contains("int applyAdder(Adder adder, int left, int right) {"));
        assert!(content.contains("Future<int> applyAdderAsync(Adder adder, int left, int right) {"));
        assert!(content.contains("int checkedApplyAdder(Adder adder, int left, int right) {"));
        assert!(content.contains("return _bindings().applyAdder(adder, left, right);"));
        assert!(content.contains("return _bindings().applyAdderAsync(adder, left, right);"));
        assert!(content.contains("return _bindings().checkedApplyAdder(adder, left, right);"));
        assert!(content.contains("abstract interface class Formatter {"));
        assert!(content.contains("String format(String? prefix, Person person, Outcome outcome);"));
        assert!(content.contains("'formatter_callback_init'"));
        assert!(content.contains("final class _FormatterVTable extends ffi.Struct {"));
        assert!(content.contains("final class _FormatterCallbackBridge {"));
        assert!(content
            .contains("int applyFormatter(Formatter formatter, String? prefix, Person person, Outcome outcome) {"));
        assert!(content.contains(
            "int counterInvokeApplyAdderWith(int handle, Adder adder, int left, int right) {"
        ));
        assert!(content.contains("int applyAdderWith(Adder adder, int left, int right) {"));
        assert!(content.contains(
            "Future<int> counterInvokeApplyAdderAsyncWith(int handle, Adder adder, int left, int right) async {"
        ));
        assert!(content.contains(
            "int counterInvokeCheckedApplyAdderWith(int handle, Adder adder, int left, int right) {"
        ));
        assert!(
            content.contains("Future<int> applyAdderAsyncWith(Adder adder, int left, int right) {")
        );
        assert!(content.contains("int checkedApplyAdderWith(Adder adder, int left, int right) {"));
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
        assert!(content.contains("import 'package:ffi/ffi.dart';"));
        assert!(content.contains("final class _RustBuffer extends ffi.Struct {"));
        assert!(content.contains("Uint8List echoBytes(Uint8List input) {"));
        assert!(content.contains("late final void Function(_RustBuffer) _rustBytesFree ="));
        assert!(content.contains("return Uint8List.fromList(resultData.asTypedList(resultLen));"));
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
    fn rewrites_reserved_identifiers() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("keywords.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace keywords {
  u32 class(u32 switch);
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
        let content = fs::read_to_string(out_dir.join("keywords.dart")).expect("read generated");
        assert!(content.contains("int class_(int switch_) {"));
    }

    #[test]
    fn delegates_supported_top_level_functions_to_default_bindings() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("simple.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace simple {
  u32 add(u32 left, u32 right);
  string greeting(string name);
  sequence<u32> not_supported(sequence<u32> value);
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
        let content = fs::read_to_string(out_dir.join("simple.dart")).expect("read generated");
        assert!(content.contains("SimpleFfi? _defaultBindings;"));
        assert!(content.contains("void configureDefaultBindings("));
        assert!(content.contains("return _bindings().add(left, right);"));
        assert!(content.contains("throw UnimplementedError('TODO: bind to Rust FFI');"));
    }

    #[test]
    fn renders_runtime_string_marshalling_and_free() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("strings.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace strings {
  string greet(string name);
  string? maybe_greet(string? name);
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
        let content = fs::read_to_string(out_dir.join("strings.dart")).expect("read generated");
        assert!(content.contains("import 'package:ffi/ffi.dart';"));
        assert!(content.contains("ffi.Pointer<Utf8> nameNative = name.toNativeUtf8();"));
        assert!(content.contains("calloc.free(nameNative);"));
        assert!(content.contains("_rustStringFree(resultPtr);"));
        assert!(
            content.contains(
                "final ffi.Pointer<Utf8> nameNative = name == null ? ffi.nullptr : name.toNativeUtf8();"
            )
        );
        assert!(content.contains("if (nameNative != ffi.nullptr) calloc.free(nameNative);"));
        assert!(content.contains("if (resultPtr == ffi.nullptr) {"));
        assert!(content.contains("return null;"));
    }

    #[test]
    fn renders_runtime_bytes_marshalling_and_free() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("bytes.udl");
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
        assert!(content.contains("final class _RustBuffer extends ffi.Struct {"));
        assert!(content.contains("late final void Function(_RustBuffer) _rustBytesFree ="));
        assert!(content.contains("final ffi.Pointer<ffi.Uint8> inputData = input.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>(input.length);"));
        assert!(content.contains("inputData.asTypedList(input.length).setAll(0, input);"));
        assert!(content.contains("final _RustBuffer resultBuf = _echoBytes(inputNative);"));
        assert!(content.contains("return Uint8List.fromList(resultData.asTypedList(resultLen));"));
    }

    #[test]
    fn renders_timestamp_and_duration_runtime_conversions() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("temporal.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace temporal {
  timestamp add_seconds(timestamp when, i64 seconds);
  duration multiply_duration(duration value, u32 factor);
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
        let content = fs::read_to_string(out_dir.join("temporal.dart")).expect("read generated");
        assert!(content.contains("DateTime addSeconds(DateTime when_, int seconds) {"));
        assert!(
            content.contains("return DateTime.fromMicrosecondsSinceEpoch(micros, isUtc: true);")
        );
        assert!(content.contains("when_.toUtc().microsecondsSinceEpoch"));
        assert!(content.contains("Duration multiplyDuration(Duration value, int factor) {"));
        assert!(content.contains("return Duration(microseconds: micros);"));
        assert!(content.contains("value.inMicroseconds"));
    }

    #[test]
    fn renders_throwing_functions_with_typed_exceptions() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("errors.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace errors {
  [Throws=MathError]
  i32 checked_divide(i32 left, i32 right);
};

[Error]
interface MathError {
  DivisionByZero();
  NegativeInput(i32 value);
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
        let content = fs::read_to_string(out_dir.join("errors.dart")).expect("read generated");
        assert!(content.contains("int checkedDivide(int left, int right) {"));
        assert!(content.contains(
            "late final ffi.Pointer<Utf8> Function(int left, int right) _checkedDivide ="
        ));
        assert!(content.contains("sealed class MathErrorException implements Exception {"));
        assert!(content
            .contains("final class MathErrorExceptionDivisionByZero extends MathErrorException {"));
        assert!(content
            .contains("final class MathErrorExceptionNegativeInput extends MathErrorException {"));
        assert!(content.contains("MathErrorException _decodeMathErrorException(Object? raw) {"));
        assert!(content.contains("throw _decodeMathErrorException(errRaw);"));
        assert!(content.contains("_rustStringFree(resultPtr);"));
    }

    #[test]
    fn renders_object_classes_with_lifecycle_and_throws() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("objects.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace objects {};

[Error]
interface MathError {
  DivisionByZero();
};

dictionary Person {
  string name;
  u32 age;
};

[Enum]
interface Outcome {
  Success(string message);
  Failure(i32 code, string reason);
};

interface Counter {
  constructor(u32 initial);
  [Name=with_person]
  constructor(Person seed);
  void add_value(u32 amount);
  u32 current_value();
  void set_label(string label);
  string maybe_label(string? suffix);
  void ingest_person(Person input);
  Outcome flip_outcome(Outcome input);
  u32 bytes_len(bytes input);
  u32 optional_bytes_len(bytes? input);
  u32 chunks_total_len(sequence<bytes> input);
  string describe();
  Person snapshot_person();
  Outcome snapshot_outcome();
  bytes snapshot_bytes();
  [Throws=MathError]
  i32 divide_by(i32 divisor);
  [Throws=MathError]
  Outcome risky_outcome(i32 divisor);
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
        let content = fs::read_to_string(out_dir.join("objects.dart")).expect("read generated");
        assert!(content.contains("final class Counter {"));
        assert!(content.contains("Counter._(this._ffi, this._handle) {"));
        assert!(content.contains("void close() {"));
        assert!(content.contains("static Counter withPerson(Person seed) {"));
        assert!(content.contains("void addValue(int amount) {"));
        assert!(content.contains("int currentValue() {"));
        assert!(content.contains("void setLabel(String label) {"));
        assert!(content.contains("String maybeLabel(String? suffix) {"));
        assert!(content.contains("void ingestPerson(Person input) {"));
        assert!(content.contains("Outcome flipOutcome(Outcome input) {"));
        assert!(content.contains("int bytesLen(Uint8List input) {"));
        assert!(content.contains("int optionalBytesLen(Uint8List? input) {"));
        assert!(content.contains("int chunksTotalLen(List<Uint8List> input) {"));
        assert!(content.contains("String describe() {"));
        assert!(content.contains("Person snapshotPerson() {"));
        assert!(content.contains("Outcome snapshotOutcome() {"));
        assert!(content.contains("Uint8List snapshotBytes() {"));
        assert!(content.contains("int divideBy(int divisor) {"));
        assert!(content.contains("Outcome riskyOutcome(int divisor) {"));
        assert!(content.contains("late final void Function(int handle) _counterFree ="));
        assert!(content.contains("Counter counterCreateNew(int initial) {"));
        assert!(content.contains("int counterInvokeCurrentValue(int handle) {"));
        assert!(content.contains("throw _decodeMathErrorException(errRaw);"));
    }

    #[test]
    fn renders_record_and_enum_models() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("models.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace models {};

dictionary Person {
  string name;
  u32 age;
  string? nickname;
};

enum Color { "red", "blue" };

[Enum]
interface Outcome {
  Success(string message);
  Failure(i32 code, string reason);
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
        let content = fs::read_to_string(out_dir.join("models.dart")).expect("read generated");
        assert!(content.contains("class Person {"));
        assert!(content.contains("const Person({"));
        assert!(content.contains("Person copyWith({"));
        assert!(content.contains("enum Color {"));
        assert!(content.contains("sealed class Outcome {"));
        assert!(content.contains("final class OutcomeSuccess extends Outcome {"));
        assert!(content.contains("final class OutcomeFailure extends Outcome {"));
        assert!(content.contains("String _encodeOutcome(Outcome value) {"));
        assert!(content.contains("Outcome _decodeOutcome(String raw) {"));
    }
}
