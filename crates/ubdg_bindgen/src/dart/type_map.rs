use std::collections::HashMap;
use uniffi_bindgen::interface::Type;

use super::config::CustomTypeConfig;
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

/// Returns true for any enum type that can be used as a throws type,
/// but only if the enum exists in the known enums list (local) or is
/// an external enum (not locally defined, but imported via external packages).
/// This prevents generating references to undefined `*ExceptionFfiCodec` symbols
/// for truly unknown types while still supporting external enum throws.
pub(super) fn is_runtime_throws_enum_type(type_: &Type, enums: &[UdlEnum]) -> bool {
    match runtime_unwrapped_type(type_) {
        Type::Enum { name, module_path } => {
            // Accept local enums that exist in our definitions.
            let is_local = enums.iter().any(|e| e.name == *name);
            // Accept external enums (non-empty module_path means the UDL parser
            // validated this type; if it's not local, it must be external with
            // an imported codec).
            let is_external = !is_local && !module_path.is_empty();
            is_local || is_external
        }
        // Accept object types used as errors (interface throws).
        // The UDL parser has already validated the type exists.
        Type::Object { .. } => true,
        _ => false,
    }
}

/// Extract the name from a throws type, whether it's an enum or an object.
pub(super) fn throws_name_from_type(type_: &Type) -> Option<&str> {
    match runtime_unwrapped_type(type_) {
        Type::Enum { name, .. } | Type::Object { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

/// Returns true when the throws type is an object (interface used as error).
pub(super) fn is_throws_object_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Object { .. })
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

pub(super) fn is_runtime_optional_object_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Optional { inner_type } if is_runtime_object_type(inner_type))
}

pub(super) fn is_runtime_optional_record_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Optional { inner_type } if is_runtime_record_type(inner_type))
}

pub(super) fn is_runtime_optional_enum_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Optional { inner_type } if matches!(**inner_type, Type::Enum { .. }))
}

pub(super) fn is_runtime_optional_primitive_type(type_: &Type) -> bool {
    match runtime_unwrapped_type(type_) {
        Type::Optional { inner_type } => matches!(
            runtime_unwrapped_type(inner_type),
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
        ),
        _ => false,
    }
}

pub(super) fn is_runtime_string_like_type(type_: &Type) -> bool {
    is_runtime_string_type(type_) || is_runtime_optional_string_type(type_)
}

pub(super) fn render_plain_ffi_decode_expr(
    type_: &Type,
    call_expr: &str,
    custom_types: &HashMap<String, CustomTypeConfig>,
) -> String {
    let decoded = match runtime_unwrapped_type(type_) {
        Type::Timestamp => format!("DateTime.fromMicrosecondsSinceEpoch({call_expr}, isUtc: true)"),
        Type::Duration => format!("Duration(microseconds: {call_expr})"),
        _ => call_expr.to_string(),
    };
    lift_custom_if_needed(&decoded, type_, custom_types)
}

/// Wrap a decoded expression with the custom-type lift template when configured.
/// Checks both top-level `Custom` and `Optional<Custom<...>>` since
/// `runtime_unwrapped_type` strips both wrappers in type-dispatch branches.
/// Pass the original type, not the result of `runtime_unwrapped_type`.
pub(super) fn lift_custom_if_needed(
    decoded_expr: &str,
    type_: &Type,
    custom_types: &HashMap<String, CustomTypeConfig>,
) -> String {
    if let Type::Custom { name, .. } = type_ {
        if let Some(cfg) = custom_types.get(name.as_str()) {
            return cfg.lift_expr(decoded_expr);
        }
    }
    if let Type::Optional { inner_type } = type_ {
        if let Type::Custom { name, .. } = inner_type.as_ref() {
            if let Some(cfg) = custom_types.get(name.as_str()) {
                return cfg.lift_expr(decoded_expr);
            }
        }
    }
    decoded_expr.to_string()
}

/// Wrap a value expression with the custom-type lower template when configured.
/// Mirror of `lift_custom_if_needed` for the arg/encode direction.
pub(super) fn lower_custom_if_needed(
    value_expr: &str,
    type_: &Type,
    custom_types: &HashMap<String, CustomTypeConfig>,
) -> String {
    if let Type::Custom { name, .. } = type_ {
        if let Some(cfg) = custom_types.get(name.as_str()) {
            return cfg.lower_expr(value_expr);
        }
    }
    if let Type::Optional { inner_type } = type_ {
        if let Type::Custom { name, .. } = inner_type.as_ref() {
            if let Some(cfg) = custom_types.get(name.as_str()) {
                return cfg.lower_expr(value_expr);
            }
        }
    }
    value_expr.to_string()
}

pub(super) fn map_uniffi_type_to_dart(
    type_: &Type,
    custom_types: &HashMap<String, CustomTypeConfig>,
) -> String {
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
        Type::Optional { inner_type } => {
            format!("{}?", map_uniffi_type_to_dart(inner_type, custom_types))
        }
        Type::Sequence { inner_type } => format!(
            "List<{}>",
            map_uniffi_type_to_dart(inner_type, custom_types)
        ),
        Type::Map {
            key_type,
            value_type,
        } => format!(
            "Map<{}, {}>",
            map_uniffi_type_to_dart(key_type, custom_types),
            map_uniffi_type_to_dart(value_type, custom_types)
        ),
        Type::Enum { name, .. }
        | Type::Object { name, .. }
        | Type::Record { name, .. }
        | Type::CallbackInterface { name, .. } => to_upper_camel(name),
        Type::Custom { name, builtin, .. } => {
            if let Some(cfg) = custom_types.get(name.as_str()) {
                if let Some(type_name) = &cfg.type_name {
                    return type_name.clone();
                }
            }
            map_uniffi_type_to_dart(builtin, custom_types)
        }
    }
}

pub(super) fn uniffi_type_uses_json(type_: &Type) -> bool {
    match type_ {
        Type::Record { .. } | Type::Enum { .. } => true,
        Type::Map { key_type, .. } if is_runtime_string_type(key_type) => true,
        Type::Optional { inner_type } => {
            // Optional primitives are JSON-encoded at the FFI boundary
            is_runtime_optional_primitive_type(type_) || uniffi_type_uses_json(inner_type)
        }
        Type::Sequence { inner_type } => uniffi_type_uses_json(inner_type),
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

/// Collect all `Type::Custom` entries found in a type tree into the given map.
/// Maps custom type name → builtin Dart type string.
pub(super) fn collect_custom_types(
    type_: &Type,
    custom_types_config: &HashMap<String, CustomTypeConfig>,
    customs: &mut std::collections::BTreeMap<String, String>,
) {
    match type_ {
        Type::Custom { name, builtin, .. } => {
            customs
                .entry(name.clone())
                .or_insert_with(|| map_uniffi_type_to_dart(builtin, custom_types_config));
        }
        Type::Optional { inner_type } | Type::Sequence { inner_type } => {
            collect_custom_types(inner_type, custom_types_config, customs);
        }
        Type::Map {
            key_type,
            value_type,
        } => {
            collect_custom_types(key_type, custom_types_config, customs);
            collect_custom_types(value_type, custom_types_config, customs);
        }
        _ => {}
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

pub(super) fn is_runtime_map_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Map { .. })
}

pub(super) fn is_runtime_non_string_map_type(type_: &Type) -> bool {
    is_runtime_map_type(type_) && !is_runtime_map_with_string_key_type(type_)
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
        Type::Sequence { .. } => Some("ffi.Pointer<Utf8>"),
        Type::Map { key_type, .. } if is_runtime_string_type(key_type) => Some("ffi.Pointer<Utf8>"),
        Type::Map { .. } => Some("_RustBuffer"),
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
            Some("ffi.Pointer<Utf8>")
        }
        Type::Optional { inner_type } if is_runtime_object_type(inner_type) => Some("ffi.Uint64"),
        Type::Optional { .. } if is_runtime_optional_primitive_type(type_) => {
            Some("ffi.Pointer<Utf8>")
        }
        Type::Optional { .. } if is_runtime_optional_record_type(type_) => {
            Some("ffi.Pointer<Utf8>")
        }
        Type::Optional { .. } if is_runtime_optional_enum_type(type_) => Some("ffi.Pointer<Utf8>"),
        Type::Record { .. } => Some("ffi.Pointer<Utf8>"),
        Type::Enum { .. } => Some("ffi.Pointer<Utf8>"),
        Type::Object { .. } => Some("ffi.Uint64"),
        _ => None,
    }
}

pub(super) fn is_runtime_sequence_json_type(type_: &Type) -> bool {
    match runtime_unwrapped_type(type_) {
        Type::Sequence { inner_type } => {
            !is_runtime_bytes_type(inner_type)
                && !matches!(
                    runtime_unwrapped_type(inner_type),
                    Type::Object { .. } | Type::CallbackInterface { .. }
                )
        }
        _ => false,
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
        Type::Sequence { .. } => Some("ffi.Pointer<Utf8>"),
        Type::Map { key_type, .. } if is_runtime_string_type(key_type) => Some("ffi.Pointer<Utf8>"),
        Type::Map { .. } => Some("_RustBuffer"),
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
            Some("ffi.Pointer<Utf8>")
        }
        Type::Optional { inner_type } if is_runtime_object_type(inner_type) => Some("int"),
        Type::Optional { .. } if is_runtime_optional_primitive_type(type_) => {
            Some("ffi.Pointer<Utf8>")
        }
        Type::Optional { .. } if is_runtime_optional_record_type(type_) => {
            Some("ffi.Pointer<Utf8>")
        }
        Type::Optional { .. } if is_runtime_optional_enum_type(type_) => Some("ffi.Pointer<Utf8>"),
        Type::Record { .. } => Some("ffi.Pointer<Utf8>"),
        Type::Enum { .. } => Some("ffi.Pointer<Utf8>"),
        Type::Object { .. } => Some("int"),
        _ => None,
    }
}
