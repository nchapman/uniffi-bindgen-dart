use uniffi_bindgen::interface::Type;

use super::*;

pub(super) fn is_runtime_string_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::String)
}

pub(super) fn is_runtime_timestamp_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Timestamp)
}

pub(super) fn is_runtime_duration_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Duration)
}

pub(super) fn is_runtime_bytes_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Bytes)
}

pub(super) fn is_runtime_record_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Record { .. })
}

pub(super) fn is_runtime_enum_type(type_: &Type, _enums: &[UdlEnum]) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Enum { .. })
}

pub(super) fn is_runtime_object_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Object { .. })
}

pub(super) fn is_runtime_error_enum_type(type_: &Type, enums: &[UdlEnum]) -> bool {
    let Some(name) = enum_name_from_type(type_) else {
        return false;
    };
    enums.iter().any(|e| e.name == name && e.is_error)
}

pub(super) fn is_runtime_throws_enum_type(type_: &Type, enums: &[UdlEnum]) -> bool {
    if is_runtime_error_enum_type(type_, enums) {
        return true;
    }
    matches!(runtime_unwrapped_type(type_), Type::Enum { .. })
}

pub(super) fn is_runtime_record_or_enum_string_type(type_: &Type, enums: &[UdlEnum]) -> bool {
    is_runtime_record_type(type_) || is_runtime_enum_type(type_, enums)
}

pub(super) fn enum_name_from_type(type_: &Type) -> Option<&str> {
    match runtime_unwrapped_type(type_) {
        Type::Enum { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

pub(super) fn record_name_from_type(type_: &Type) -> Option<&str> {
    match runtime_unwrapped_type(type_) {
        Type::Record { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

pub(super) fn object_name_from_type(type_: &Type) -> Option<&str> {
    match runtime_unwrapped_type(type_) {
        Type::Object { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

pub(super) fn is_external_object_type(type_: &Type, local_module_path: &str) -> bool {
    let local_crate = local_module_path.split("::").next().unwrap_or_default();
    match runtime_unwrapped_type(type_) {
        Type::Object { module_path, .. } => {
            let crate_name = module_path.split("::").next().unwrap_or_default();
            !crate_name.is_empty() && !local_crate.is_empty() && crate_name != local_crate
        }
        _ => false,
    }
}

pub(super) fn is_runtime_optional_bytes_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Optional { inner_type } if is_runtime_bytes_type(inner_type))
}

pub(super) fn is_runtime_sequence_bytes_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Sequence { inner_type } if is_runtime_bytes_type(inner_type))
}

pub(super) fn is_runtime_bytes_like_type(type_: &Type) -> bool {
    is_runtime_bytes_type(type_)
        || is_runtime_optional_bytes_type(type_)
        || is_runtime_sequence_bytes_type(type_)
}

pub(super) fn is_runtime_optional_string_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Optional { inner_type } if is_runtime_string_type(inner_type))
}

pub(super) fn is_runtime_string_like_type(type_: &Type) -> bool {
    is_runtime_string_type(type_) || is_runtime_optional_string_type(type_)
}

pub(super) fn render_plain_ffi_decode_expr(type_: &Type, call_expr: &str) -> String {
    match runtime_unwrapped_type(type_) {
        Type::Timestamp => format!("DateTime.fromMicrosecondsSinceEpoch({call_expr}, isUtc: true)"),
        Type::Duration => format!("Duration(microseconds: {call_expr})"),
        _ => call_expr.to_string(),
    }
}

pub(super) fn map_uniffi_type_to_dart(type_: &Type) -> String {
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
        | Type::CallbackInterface { name, .. } => to_upper_camel(name),
        Type::Custom { builtin, .. } => map_uniffi_type_to_dart(builtin),
    }
}

pub(super) fn uniffi_type_uses_json(type_: &Type) -> bool {
    match type_ {
        Type::Record { .. } | Type::Enum { .. } | Type::Map { .. } => true,
        Type::Optional { inner_type } | Type::Sequence { inner_type } => {
            uniffi_type_uses_json(inner_type)
        }
        Type::Custom { builtin, .. } => uniffi_type_uses_json(builtin),
        _ => false,
    }
}

pub(super) fn uniffi_type_uses_bytes(type_: &Type) -> bool {
    match type_ {
        Type::Bytes => true,
        Type::Optional { inner_type } | Type::Sequence { inner_type } => {
            uniffi_type_uses_bytes(inner_type)
        }
        Type::Map {
            key_type,
            value_type,
        } => uniffi_type_uses_bytes(key_type) || uniffi_type_uses_bytes(value_type),
        Type::Custom { builtin, .. } => uniffi_type_uses_bytes(builtin),
        _ => false,
    }
}

pub(super) fn runtime_unwrapped_type(type_: &Type) -> &Type {
    match type_ {
        Type::Custom { builtin, .. } => runtime_unwrapped_type(builtin),
        _ => type_,
    }
}

pub(super) fn is_runtime_map_with_string_key_type(type_: &Type) -> bool {
    match runtime_unwrapped_type(type_) {
        Type::Map { key_type, .. } => is_runtime_string_type(key_type),
        _ => false,
    }
}

pub(super) fn is_runtime_utf8_pointer_marshaled_type(
    type_: &Type,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    map_runtime_native_ffi_type(type_, records, enums) == Some("ffi.Pointer<Utf8>")
}

pub(super) fn function_uses_bytes(f: &UdlFunction) -> bool {
    f.return_type.as_ref().is_some_and(uniffi_type_uses_bytes)
        || f.args.iter().any(|a| uniffi_type_uses_bytes(&a.type_))
}

pub(super) fn function_uses_runtime_string(f: &UdlFunction) -> bool {
    f.return_type
        .as_ref()
        .is_some_and(is_runtime_string_like_type)
        || f.args.iter().any(|a| is_runtime_string_like_type(&a.type_))
}

pub(super) fn function_returns_runtime_string(f: &UdlFunction) -> bool {
    f.return_type
        .as_ref()
        .is_some_and(is_runtime_string_like_type)
}

pub(super) fn function_uses_runtime_bytes(f: &UdlFunction) -> bool {
    f.return_type
        .as_ref()
        .is_some_and(is_runtime_bytes_like_type)
        || f.args.iter().any(|a| is_runtime_bytes_like_type(&a.type_))
}

pub(super) fn function_returns_runtime_bytes(f: &UdlFunction) -> bool {
    f.return_type
        .as_ref()
        .is_some_and(is_runtime_bytes_like_type)
}

pub(super) fn is_runtime_ffi_compatible_function(
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
                    && is_runtime_throws_enum_type(t, enums)
            })
            .unwrap_or(true)
}

pub(super) fn is_runtime_throwing_ffi_compatible_function(
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
                && is_runtime_throws_enum_type(t, enums)
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

pub(super) fn is_runtime_ffi_compatible_type(
    type_: &Type,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    map_runtime_native_ffi_type(type_, records, enums).is_some()
}

pub(super) fn map_runtime_native_ffi_type(
    type_: &Type,
    _records: &[UdlRecord],
    _enums: &[UdlEnum],
) -> Option<&'static str> {
    if let Type::Custom { builtin, .. } = type_ {
        return map_runtime_native_ffi_type(builtin, _records, _enums);
    }

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
        Type::Map { key_type, .. } if is_runtime_string_type(key_type) => Some("ffi.Pointer<Utf8>"),
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
            Some("ffi.Pointer<Utf8>")
        }
        Type::Record { .. } => Some("ffi.Pointer<Utf8>"),
        Type::Enum { .. } => Some("ffi.Pointer<Utf8>"),
        Type::Object { .. } => Some("ffi.Uint64"),
        _ => None,
    }
}

pub(super) fn map_runtime_dart_ffi_type(
    type_: &Type,
    _records: &[UdlRecord],
    _enums: &[UdlEnum],
) -> Option<&'static str> {
    if let Type::Custom { builtin, .. } = type_ {
        return map_runtime_dart_ffi_type(builtin, _records, _enums);
    }

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
        Type::Map { key_type, .. } if is_runtime_string_type(key_type) => Some("ffi.Pointer<Utf8>"),
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
            Some("ffi.Pointer<Utf8>")
        }
        Type::Record { .. } => Some("ffi.Pointer<Utf8>"),
        Type::Enum { .. } => Some("ffi.Pointer<Utf8>"),
        Type::Object { .. } => Some("int"),
        _ => None,
    }
}
