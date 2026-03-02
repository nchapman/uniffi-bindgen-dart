use std::collections::{HashMap, HashSet};

use uniffi_bindgen::interface::{DefaultValue, Literal, Radix, Type};

use super::*;

pub(super) fn render_doc_comment(docstring: Option<&str>, indent: &str) -> String {
    let Some(raw) = docstring.map(str::trim) else {
        return String::new();
    };
    if raw.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for line in raw.lines() {
        let clean = line.trim();
        if clean.is_empty() {
            out.push_str(&format!("{indent}///\n"));
        } else {
            out.push_str(&format!("{indent}/// {clean}\n"));
        }
    }
    out
}

/// Render the throw expression for an error in a JSON-envelope function.
///
/// For enum errors: `throw ErrorExceptionFfiCodec.decode(errRaw);`
/// For object errors: `throw ErrorName._(this, (errRaw as num).toInt());`
pub(super) fn render_throws_expr(throws_type: &Type, err_value: &str, indent: &str) -> String {
    if is_throws_object_type(throws_type) {
        let name = throws_name_from_type(throws_type)
            .map(to_upper_camel)
            .unwrap_or_else(|| "Object".to_string());
        format!("{indent}throw {name}._(this, ({err_value} as num).toInt());\n")
    } else {
        let name = throws_name_from_type(throws_type)
            .map(to_upper_camel)
            .unwrap_or_else(|| "Unknown".to_string());
        format!("{indent}throw {name}ExceptionFfiCodec.decode({err_value});\n")
    }
}

pub(super) fn escape_dart_string_literal(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

pub(super) fn render_default_value_expr(
    default: &DefaultValue,
    type_: &Type,
    enums: &[UdlEnum],
) -> Option<String> {
    match default {
        DefaultValue::Default => render_type_default_expr(type_, enums),
        DefaultValue::Literal(lit) => render_literal_default_expr(lit, type_, enums),
    }
}

pub(super) fn render_type_default_expr(type_: &Type, enums: &[UdlEnum]) -> Option<String> {
    match type_ {
        Type::Boolean => Some("false".to_string()),
        Type::String => Some("''".to_string()),
        Type::Int8
        | Type::Int16
        | Type::Int32
        | Type::Int64
        | Type::UInt8
        | Type::UInt16
        | Type::UInt32
        | Type::UInt64 => Some("0".to_string()),
        Type::Float32 | Type::Float64 => Some("0.0".to_string()),
        Type::Bytes => Some("Uint8List(0)".to_string()),
        Type::Timestamp => Some("DateTime.fromMicrosecondsSinceEpoch(0, isUtc: true)".to_string()),
        Type::Duration => Some("Duration.zero".to_string()),
        Type::Optional { .. } => Some("null".to_string()),
        Type::Sequence { .. } => Some("const []".to_string()),
        Type::Map { .. } => Some("const {}".to_string()),
        Type::Custom { builtin, .. } => render_type_default_expr(builtin, enums),
        Type::Enum { name, .. } => {
            let enum_name = to_upper_camel(name);
            let enum_def = enums
                .iter()
                .find(|e| to_upper_camel(&e.name) == enum_name)?;
            let variant = enum_def.variants.first()?;
            if variant.fields.is_empty() && !enum_def.is_error {
                Some(format!(
                    "{enum_name}.{}",
                    safe_dart_identifier(&to_lower_camel(&variant.name))
                ))
            } else {
                None
            }
        }
        _ => None,
    }
}

pub(super) fn render_literal_default_expr(
    lit: &Literal,
    type_: &Type,
    enums: &[UdlEnum],
) -> Option<String> {
    match lit {
        Literal::Boolean(v) => Some(v.to_string()),
        Literal::String(v) => Some(format!("'{}'", escape_dart_string_literal(v))),
        Literal::UInt(v, radix, _) => Some(match radix {
            Radix::Decimal => v.to_string(),
            Radix::Octal => format!("0{o:o}", o = v),
            Radix::Hexadecimal => format!("0x{v:x}"),
        }),
        Literal::Int(v, _radix, _) => Some(v.to_string()),
        Literal::Float(v, _) => Some(v.to_string()),
        Literal::Enum(variant, enum_type) => {
            let enum_name = match enum_type {
                Type::Enum { name, .. } => to_upper_camel(name),
                _ => match type_ {
                    Type::Enum { name, .. } => to_upper_camel(name),
                    _ => return None,
                },
            };
            Some(format!(
                "{enum_name}.{}",
                safe_dart_identifier(&to_lower_camel(variant))
            ))
        }
        Literal::EmptySequence => Some("const []".to_string()),
        Literal::EmptyMap => Some("const {}".to_string()),
        Literal::None => Some("null".to_string()),
        Literal::Some { inner } => render_default_value_expr(inner, type_, enums),
    }
}

pub(super) fn render_callable_args_signature(args: &[UdlArg], enums: &[UdlEnum]) -> String {
    let defaults = args
        .iter()
        .map(|a| {
            a.default
                .as_ref()
                .and_then(|d| render_default_value_expr(d, &a.type_, enums))
        })
        .collect::<Vec<_>>();
    let has_defaults = defaults.iter().any(|d| d.is_some());
    if !has_defaults {
        return args
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
    }

    let params = args
        .iter()
        .zip(defaults.iter())
        .map(|(a, default_expr)| {
            let field_type = map_uniffi_type_to_dart(&a.type_);
            let field_name = safe_dart_identifier(&to_lower_camel(&a.name));
            if let Some(default_expr) = default_expr {
                format!("{field_type} {field_name} = {default_expr}")
            } else {
                format!("required {field_type} {field_name}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{params}}}")
}

pub(super) fn render_callable_arg_names(args: &[UdlArg]) -> String {
    args.iter()
        .map(|a| safe_dart_identifier(&to_lower_camel(&a.name)))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn append_runtime_arg_marshalling(
    arg_name: &str,
    type_: &Type,
    enums: &[UdlEnum],
    pre_call: &mut Vec<String>,
    post_call: &mut Vec<String>,
    call_args: &mut Vec<String>,
) {
    if let Type::Custom { builtin, .. } = type_ {
        append_runtime_arg_marshalling(arg_name, builtin, enums, pre_call, post_call, call_args);
        return;
    }

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
    } else if is_runtime_sequence_json_type(type_) || is_runtime_map_with_string_key_type(type_) {
        let native_name = format!("{arg_name}Native");
        let json_name = format!("{native_name}Json");
        let payload_expr = render_json_encode_expr(arg_name, type_);
        pre_call.push(format!(
            "    final String {json_name} = jsonEncode({payload_expr});\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<Utf8> {native_name} = {json_name}.toNativeUtf8();\n"
        ));
        post_call.push(format!("    calloc.free({native_name});\n"));
        call_args.push(native_name);
    } else if is_runtime_map_type(type_) {
        let data_name = format!("{arg_name}Data");
        let buffer_ptr_name = format!("{arg_name}BufferPtr");
        let native_name = format!("{arg_name}Native");
        let writer_name = format!("{arg_name}Writer");
        let write_stmt =
            render_uniffi_binary_write_statement(type_, arg_name, &writer_name, enums, "    ");
        pre_call.push(format!(
            "    final {writer_name} = _UniFfiBinaryWriter();\n"
        ));
        pre_call.push(write_stmt);
        pre_call.push(format!(
            "    final Uint8List {data_name}Bytes = {writer_name}.toBytes();\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<ffi.Uint8> {data_name} = {data_name}Bytes.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>({data_name}Bytes.length);\n"
        ));
        pre_call.push(format!(
            "    if ({data_name} != ffi.nullptr) {{\n      {data_name}.asTypedList({data_name}Bytes.length).setAll(0, {data_name}Bytes);\n    }}\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<_RustBuffer> {buffer_ptr_name} = calloc<_RustBuffer>();\n"
        ));
        pre_call.push(format!("    {buffer_ptr_name}.ref.data = {data_name};\n"));
        pre_call.push(format!(
            "    {buffer_ptr_name}.ref.len = {data_name}Bytes.length;\n"
        ));
        pre_call.push(format!(
            "    final _RustBuffer {native_name} = {buffer_ptr_name}.ref;\n"
        ));
        post_call.push(format!(
            "    if ({data_name} != ffi.nullptr) calloc.free({data_name});\n"
        ));
        post_call.push(format!("    calloc.free({buffer_ptr_name});\n"));
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
            "    final String {native_name}Json = {}FfiCodec.encode({arg_name});\n",
            to_upper_camel(enum_name)
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<Utf8> {native_name} = {native_name}Json.toNativeUtf8();\n"
        ));
        post_call.push(format!("    calloc.free({native_name});\n"));
        call_args.push(native_name);
    } else if is_runtime_object_type(type_) {
        let handle_name = format!("{arg_name}Handle");
        let object_name = object_name_from_type(type_).unwrap_or("Object");
        pre_call.push(format!(
            "    final int {handle_name} = {}FfiCodec.lower({arg_name});\n",
            to_upper_camel(object_name)
        ));
        call_args.push(handle_name);
    } else {
        call_args.push(arg_name.to_string());
    }
}

pub(super) fn render_object_lift_expr_with_objects(
    type_: &Type,
    handle_expr: &str,
    local_module_path: &str,
    binding_expr: &str,
    objects: &[UdlObject],
) -> String {
    let raw_name = object_name_from_type(type_).unwrap_or("Object");
    let object_name = to_upper_camel(raw_name);
    if is_external_object_type(type_, local_module_path) {
        format!("{object_name}FfiCodec.lift({handle_expr})")
    } else {
        let is_trait = objects
            .iter()
            .any(|o| o.name == raw_name && o.has_callback_interface);
        if is_trait {
            let impl_name = format!("_{object_name}Impl");
            format!("{impl_name}._({binding_expr}, {handle_expr})")
        } else {
            format!("{object_name}._({binding_expr}, {handle_expr})")
        }
    }
}

pub(super) fn crate_name_from_module_path(module_path: &str) -> &str {
    module_path.split("::").next().unwrap_or(module_path)
}

pub(super) fn collect_external_import_uris(
    local_module_path: &str,
    external_packages: &HashMap<String, String>,
    functions: &[UdlFunction],
    objects: &[UdlObject],
) -> Vec<String> {
    if local_module_path.is_empty() || external_packages.is_empty() {
        return Vec::new();
    }

    let local_crate = crate_name_from_module_path(local_module_path);
    let mut crates = HashSet::new();

    for f in functions {
        if let Some(t) = f.return_type.as_ref() {
            collect_external_crates_from_type(t, local_crate, &mut crates);
        }
        if let Some(t) = f.throws_type.as_ref() {
            collect_external_crates_from_type(t, local_crate, &mut crates);
        }
        for a in &f.args {
            collect_external_crates_from_type(&a.type_, local_crate, &mut crates);
        }
    }

    for o in objects {
        for ctor in &o.constructors {
            if let Some(t) = ctor.throws_type.as_ref() {
                collect_external_crates_from_type(t, local_crate, &mut crates);
            }
            for a in &ctor.args {
                collect_external_crates_from_type(&a.type_, local_crate, &mut crates);
            }
        }
        for m in &o.methods {
            if let Some(t) = m.return_type.as_ref() {
                collect_external_crates_from_type(t, local_crate, &mut crates);
            }
            if let Some(t) = m.throws_type.as_ref() {
                collect_external_crates_from_type(t, local_crate, &mut crates);
            }
            for a in &m.args {
                collect_external_crates_from_type(&a.type_, local_crate, &mut crates);
            }
        }
    }

    let mut uris = crates
        .into_iter()
        .filter_map(|crate_name| external_packages.get(crate_name).cloned())
        .collect::<Vec<_>>();
    uris.sort();
    uris
}

pub(super) fn collect_external_crates_from_type<'a>(
    type_: &'a Type,
    local_crate: &str,
    out: &mut HashSet<&'a str>,
) {
    match type_ {
        Type::Object { module_path, .. }
        | Type::Record { module_path, .. }
        | Type::Enum { module_path, .. }
        | Type::CallbackInterface { module_path, .. }
        | Type::Custom { module_path, .. } => {
            let crate_name = crate_name_from_module_path(module_path);
            if crate_name != local_crate {
                out.insert(crate_name);
            }
            if let Type::Custom { builtin, .. } = type_ {
                collect_external_crates_from_type(builtin, local_crate, out);
            }
        }
        Type::Optional { inner_type } | Type::Sequence { inner_type } => {
            collect_external_crates_from_type(inner_type, local_crate, out);
        }
        Type::Map {
            key_type,
            value_type,
        } => {
            collect_external_crates_from_type(key_type, local_crate, out);
            collect_external_crates_from_type(value_type, local_crate, out);
        }
        _ => {}
    }
}

/// Format a human-readable argument signature for warning messages.
/// Produces strings like `"int32 x, String name"`.
pub(super) fn format_args_for_warning(args: &[UdlArg]) -> String {
    args.iter()
        .map(|a| format!("{} {}", map_uniffi_type_to_dart(&a.type_), a.name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Emit a Dart comment warning that a constructor was skipped during generation,
/// and print a corresponding warning to stderr.
pub(super) fn emit_constructor_skip_warning(
    out: &mut String,
    object_name: &str,
    ctor_name: &str,
    args: &[UdlArg],
    indent: &str,
) {
    let sig = format_args_for_warning(args);
    let display_name = if ctor_name == "new" {
        format!("{object_name}({sig})")
    } else {
        format!("{object_name}.{ctor_name}({sig})")
    };
    out.push_str(&format!(
        "{indent}// WARNING: Constructor '{display_name}' was omitted because\n"
    ));
    out.push_str(&format!(
        "{indent}// the constructor signature is not yet supported in this FFI binding mode.\n\n"
    ));
    eprintln!(
        "WARNING: Skipping constructor '{}' on '{}' — unsupported argument types",
        ctor_name, object_name,
    );
}

/// Emit a Dart comment warning that a method was skipped during generation,
/// and print a corresponding warning to stderr.
pub(super) fn emit_method_skip_warning(
    out: &mut String,
    object_name: &str,
    method_name: &str,
    args: &[UdlArg],
    indent: &str,
) {
    let sig = format_args_for_warning(args);
    let display_name = format!("{object_name}.{method_name}({sig})");
    out.push_str(&format!(
        "{indent}// WARNING: Method '{display_name}' was omitted because\n"
    ));
    out.push_str(&format!(
        "{indent}// the method signature is not yet supported in this FFI binding mode.\n\n"
    ));
    eprintln!(
        "WARNING: Skipping method '{}' on '{}' — unsupported signature",
        method_name, object_name,
    );
}

/// Emit a Dart comment warning that a top-level function was skipped during generation,
/// and print a corresponding warning to stderr.
pub(super) fn emit_function_skip_warning(
    out: &mut String,
    function_name: &str,
    args: &[UdlArg],
    indent: &str,
) {
    let sig = format_args_for_warning(args);
    let display_name = format!("{function_name}({sig})");
    out.push_str(&format!(
        "{indent}// WARNING: Function '{display_name}' was omitted because\n"
    ));
    out.push_str(&format!(
        "{indent}// the function signature is not yet supported in this FFI binding mode.\n\n"
    ));
    eprintln!(
        "WARNING: Skipping function '{}' — unsupported signature",
        function_name,
    );
}
