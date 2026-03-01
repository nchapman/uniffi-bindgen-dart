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
    let functions = parse_udl_functions(&args.source, args.crate_name.as_deref())?;

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
    return_type: Option<Type>,
    args: Vec<UdlArg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlArg {
    name: String,
    type_: Type,
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
            .is_some_and(is_runtime_bytes_type)
            || self.args.iter().any(|a| is_runtime_bytes_type(&a.type_))
    }

    fn returns_runtime_bytes(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(is_runtime_bytes_type)
    }
}

fn parse_udl_functions(source: &Path, crate_name: Option<&str>) -> Result<Vec<UdlFunction>> {
    if source.extension().and_then(|e| e.to_str()) != Some("udl") {
        return Ok(Vec::new());
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
            return_type: f.return_type().cloned(),
            args: f
                .arguments()
                .into_iter()
                .map(|a| UdlArg {
                    name: a.name().to_string(),
                    type_: a.as_type(),
                })
                .collect(),
        })
        .collect();

    Ok(functions)
}

fn render_dart_scaffold(
    module_name: &str,
    ffi_class_name: &str,
    library_name: &str,
    functions: &[UdlFunction],
) -> String {
    let needs_typed_data = functions.iter().any(UdlFunction::uses_bytes);
    let needs_ffi_helpers = functions
        .iter()
        .any(|f| {
            is_runtime_ffi_compatible_function(f) && (f.uses_runtime_string() || f.uses_runtime_bytes())
        });
    let needs_runtime_bytes = functions
        .iter()
        .any(|f| is_runtime_ffi_compatible_function(f) && f.uses_runtime_bytes());

    let mut out = String::new();
    out.push_str("// Generated by uniffi-bindgen-dart. DO NOT EDIT.\n");
    out.push_str(&format!("library {module_name};\n\n"));
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
    out.push_str(&render_bound_methods(functions));
    out.push_str("}\n");
    out.push_str(&render_function_stubs(functions, ffi_class_name));
    out
}

fn render_bound_methods(functions: &[UdlFunction]) -> String {
    let mut out = String::new();
    let needs_string_free = functions
        .iter()
        .any(|f| is_runtime_ffi_compatible_function(f) && f.returns_runtime_string());
    let needs_bytes_free = functions
        .iter()
        .any(|f| is_runtime_ffi_compatible_function(f) && f.returns_runtime_bytes());

    if needs_string_free {
        out.push('\n');
        out.push_str("  late final void Function(ffi.Pointer<Utf8>) _rustStringFree = _lib.lookupFunction<ffi.Void Function(ffi.Pointer<Utf8>), void Function(ffi.Pointer<Utf8>)>('rust_string_free');\n");
    }
    if needs_bytes_free {
        out.push('\n');
        out.push_str("  late final void Function(_RustBuffer) _rustBytesFree = _lib.lookupFunction<ffi.Void Function(_RustBuffer), void Function(_RustBuffer)>('rust_bytes_free');\n");
    }

    for function in functions {
        if !is_runtime_ffi_compatible_function(function) {
            continue;
        }

        let method_name = safe_dart_identifier(&to_lower_camel(&function.name));
        let field_name = format!("_{}", method_name);
        let return_type = function
            .return_type
            .as_ref()
            .map(map_uniffi_type_to_dart)
            .unwrap_or_else(|| "void".to_string());
        let native_return = function
            .return_type
            .as_ref()
            .map(map_runtime_native_ffi_type)
            .unwrap_or(Some("ffi.Void"));
        let dart_ffi_return = function
            .return_type
            .as_ref()
            .map(map_runtime_dart_ffi_type)
            .unwrap_or(Some("void"));

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
            let Some(native_type) = map_runtime_native_ffi_type(&arg.type_) else {
                signature_compatible = false;
                break;
            };
            let Some(dart_ffi_type) = map_runtime_dart_ffi_type(&arg.type_) else {
                signature_compatible = false;
                break;
            };
            native_args.push(format!("{native_type} {arg_name}"));
            dart_ffi_args.push(format!("{dart_ffi_type} {arg_name}"));
            dart_args.push(format!("{} {}", map_uniffi_type_to_dart(&arg.type_), arg_name));
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
                pre_call.push(format!(
                    "    {buffer_ptr_name}.ref.data = {data_name};\n"
                ));
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
            } else {
                call_args.push(arg_name);
            }
        }

        if !signature_compatible {
            continue;
        }

        let native_sig = format!(
            "{native_return} Function({})",
            native_args.join(", ")
        );
        let dart_sig = format!("{dart_ffi_return} Function({})", dart_ffi_args.join(", "));

        out.push_str("\n");
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
                out.push_str(&format!(
                    "      final int micros = {call_expr};\n"
                ));
                out.push_str(
                    "      return DateTime.fromMicrosecondsSinceEpoch(micros, isUtc: true);\n",
                );
            }
            Some(type_) if is_runtime_duration_type(type_) => {
                out.push_str(&format!(
                    "      final int micros = {call_expr};\n"
                ));
                out.push_str("      return Duration(microseconds: micros);\n");
            }
            Some(type_) if is_runtime_bytes_type(type_) => {
                out.push_str(&format!(
                    "      final _RustBuffer resultBuf = {call_expr};\n"
                ));
                out.push_str("      final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n");
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
                out.push_str("        return Uint8List.fromList(resultData.asTypedList(resultLen));\n");
                out.push_str("      } finally {\n");
                out.push_str("        _rustBytesFree(resultBuf);\n");
                out.push_str("      }\n");
            }
            Some(_) => {
                out.push_str(&format!("      return {call_expr};\n"));
            }
            None => {
                out.push_str(&format!("      {call_expr};\n"));
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

    out
}

fn render_function_stubs(functions: &[UdlFunction], ffi_class_name: &str) -> String {
    if functions.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    let has_runtime_functions = functions.iter().any(is_runtime_ffi_compatible_function);
    out.push('\n');
    if has_runtime_functions {
        out.push_str(&format!(
            "{ffi_class_name}? _defaultBindings;\n\n"
        ));
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
        let return_type = f
            .return_type
            .as_ref()
            .map(map_uniffi_type_to_dart)
            .unwrap_or_else(|| "void".to_string());
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

        out.push_str(&format!("{return_type} {fn_name}({args}) {{\n"));
        if is_runtime_ffi_compatible_function(f) {
            if f.return_type.is_some() {
                out.push_str(&format!("  return _bindings().{fn_name}({arg_names});\n"));
            } else {
                out.push_str(&format!("  _bindings().{fn_name}({arg_names});\n"));
            }
        } else {
            out.push_str("  throw UnimplementedError('TODO: bind to Rust FFI');\n");
        }
        out.push_str("}\n\n");
    }
    out
}

fn is_runtime_ffi_compatible_function(function: &UdlFunction) -> bool {
    function
        .return_type
        .as_ref()
        .map(is_runtime_ffi_compatible_type)
        .unwrap_or(true)
        && function
            .args
            .iter()
            .all(|arg| is_runtime_ffi_compatible_type(&arg.type_))
}

fn is_runtime_ffi_compatible_type(type_: &Type) -> bool {
    map_runtime_native_ffi_type(type_).is_some()
}

fn map_runtime_native_ffi_type(type_: &Type) -> Option<&'static str> {
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
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
            Some("ffi.Pointer<Utf8>")
        }
        _ => None,
    }
}

fn map_runtime_dart_ffi_type(type_: &Type) -> Option<&'static str> {
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
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
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

fn is_runtime_optional_string_type(type_: &Type) -> bool {
    matches!(type_, Type::Optional { inner_type } if is_runtime_string_type(inner_type))
}

fn is_runtime_string_like_type(type_: &Type) -> bool {
    is_runtime_string_type(type_) || is_runtime_optional_string_type(type_)
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
        assert!(content.contains("return DateTime.fromMicrosecondsSinceEpoch(micros, isUtc: true);"));
        assert!(content.contains("when_.toUtc().microsecondsSinceEpoch"));
        assert!(content.contains("Duration multiplyDuration(Duration value, int factor) {"));
        assert!(content.contains("return Duration(microseconds: micros);"));
        assert!(content.contains("value.inMicroseconds"));
    }
}
