use uniffi_bindgen::interface::{ffi::FfiType, Type};

use super::*;

pub(super) fn ffi_type_contains_rust_buffer(type_: &FfiType) -> bool {
    match type_ {
        FfiType::RustBuffer(_) => true,
        FfiType::Reference(inner) | FfiType::MutReference(inner) => {
            ffi_type_contains_rust_buffer(inner)
        }
        _ => false,
    }
}

pub(super) fn runtime_unsupported_reason_for_ffi_func(
    ffi_func: &uniffi_bindgen::interface::FfiFunction,
) -> Option<String> {
    if ffi_func.has_rust_call_status_arg() {
        return Some("runtime invocation for this UniFFI ABI (RustCallStatus out-arg) is not implemented yet".to_string());
    }
    if ffi_func
        .arguments()
        .iter()
        .any(|arg| ffi_type_contains_rust_buffer(&arg.type_()))
    {
        return Some(
            "runtime invocation for this UniFFI ABI (RustBuffer argument) is not implemented yet"
                .to_string(),
        );
    }
    if ffi_func
        .return_type()
        .is_some_and(ffi_type_contains_rust_buffer)
    {
        return Some(
            "runtime invocation for this UniFFI ABI (RustBuffer return) is not implemented yet"
                .to_string(),
        );
    }
    None
}

#[allow(dead_code)]
pub(super) fn is_ffibuffer_supported_ffi_type(type_: &FfiType) -> bool {
    match type_ {
        FfiType::UInt8
        | FfiType::Int8
        | FfiType::UInt16
        | FfiType::Int16
        | FfiType::UInt32
        | FfiType::Int32
        | FfiType::UInt64
        | FfiType::Int64
        | FfiType::Float32
        | FfiType::Float64
        | FfiType::Handle
        | FfiType::RustBuffer(_)
        | FfiType::RustCallStatus => true,
        FfiType::Reference(inner) | FfiType::MutReference(inner) => {
            matches!(inner.as_ref(), FfiType::VoidPointer)
        }
        _ => false,
    }
}

#[allow(dead_code)]
pub(super) fn is_ffibuffer_eligible_function(function: &UdlFunction) -> bool {
    function.ffi_symbol.is_some() && !function.is_async
}

pub(super) fn is_runtime_unsupported_async_ffibuffer_eligible_function(
    function: &UdlFunction,
) -> bool {
    if function.runtime_unsupported.is_none()
        || !function.is_async
        || function.throws_type.is_some()
        || function.ffi_symbol.is_none()
    {
        return false;
    }
    async_rust_future_spec_from_uniffi_return_type(function.return_type.as_ref()).is_some()
}

pub(super) fn ffibuffer_symbol_name(ffi_symbol: &str) -> String {
    if let Some(rest) = ffi_symbol.strip_prefix("uniffi_") {
        format!("uniffi_ffibuffer_{rest}")
    } else {
        format!("uniffi_ffibuffer_{ffi_symbol}")
    }
}

pub(super) fn ffibuffer_element_count(ffi_type: &FfiType) -> Option<usize> {
    match ffi_type {
        FfiType::UInt8
        | FfiType::Int8
        | FfiType::UInt16
        | FfiType::Int16
        | FfiType::UInt32
        | FfiType::Int32
        | FfiType::UInt64
        | FfiType::Int64
        | FfiType::Float32
        | FfiType::Float64
        | FfiType::Handle
        | FfiType::Reference(_)
        | FfiType::MutReference(_) => Some(1),
        FfiType::RustBuffer(_) => Some(3),
        FfiType::RustCallStatus => Some(4),
        _ => None,
    }
}

pub(super) fn ffibuffer_primitive_union_field(ffi_type: &FfiType) -> Option<&'static str> {
    match ffi_type {
        FfiType::UInt8 => Some("u8"),
        FfiType::Int8 => Some("i8"),
        FfiType::UInt16 => Some("u16"),
        FfiType::Int16 => Some("i16"),
        FfiType::UInt32 => Some("u32"),
        FfiType::Int32 => Some("i32"),
        FfiType::UInt64 | FfiType::Int64 | FfiType::Handle => Some("u64"),
        FfiType::Float32 => Some("float32"),
        FfiType::Float64 => Some("float64"),
        FfiType::Reference(inner) | FfiType::MutReference(inner)
            if matches!(inner.as_ref(), FfiType::VoidPointer) =>
        {
            Some("ptr")
        }
        _ => None,
    }
}

pub(super) fn ffibuffer_ffi_type_from_uniffi_type(type_: &Type) -> Option<FfiType> {
    if let Type::Custom { builtin, .. } = type_ {
        return ffibuffer_ffi_type_from_uniffi_type(builtin);
    }
    match type_ {
        Type::UInt8 => Some(FfiType::UInt8),
        Type::Int8 => Some(FfiType::Int8),
        Type::UInt16 => Some(FfiType::UInt16),
        Type::Int16 => Some(FfiType::Int16),
        Type::UInt32 => Some(FfiType::UInt32),
        Type::Int32 => Some(FfiType::Int32),
        Type::UInt64 => Some(FfiType::UInt64),
        Type::Int64 => Some(FfiType::Int64),
        Type::Float32 => Some(FfiType::Float32),
        Type::Float64 => Some(FfiType::Float64),
        Type::Boolean => Some(FfiType::Int8),
        Type::Object { .. } | Type::CallbackInterface { .. } => Some(FfiType::Handle),
        Type::String
        | Type::Bytes
        | Type::Timestamp
        | Type::Duration
        | Type::Optional { .. }
        | Type::Sequence { .. }
        | Type::Map { .. }
        | Type::Record { .. }
        | Type::Enum { .. } => Some(FfiType::RustBuffer(None)),
        _ => None,
    }
}

pub(super) fn is_ffibuffer_eligible_object_member(method: &UdlObjectMethod) -> bool {
    method.ffi_symbol.is_some() && !method.is_async
}

pub(super) fn is_ffibuffer_eligible_object_constructor(ctor: &UdlObjectConstructor) -> bool {
    ctor.ffi_symbol.is_some() && !ctor.is_async
}

pub(super) fn is_runtime_unsupported_async_ffibuffer_eligible_method(
    method: &UdlObjectMethod,
) -> bool {
    if method.runtime_unsupported.is_none()
        || !method.is_async
        || method.throws_type.is_some()
        || method.ffi_symbol.is_none()
    {
        return false;
    }
    async_rust_future_spec_from_uniffi_return_type(method.return_type.as_ref()).is_some()
}

pub(super) fn has_runtime_unsupported_async_ffibuffer_support(
    functions: &[UdlFunction],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    functions
        .iter()
        .any(is_runtime_unsupported_async_ffibuffer_eligible_function)
        || records.iter().any(|r| {
            r.methods
                .iter()
                .any(is_runtime_unsupported_async_ffibuffer_eligible_method)
        })
        || enums.iter().any(|e| {
            e.methods
                .iter()
                .any(is_runtime_unsupported_async_ffibuffer_eligible_method)
        })
}

pub(super) fn async_rust_future_spec_from_uniffi_return_type(
    return_type: Option<&Type>,
) -> Option<AsyncRustFutureSpec> {
    let return_ffi_type = return_type.and_then(ffibuffer_ffi_type_from_uniffi_type);
    match return_ffi_type {
        None => Some(AsyncRustFutureSpec {
            suffix: "void",
            complete_native_type: "ffi.Void",
            complete_dart_type: "void",
        }),
        Some(FfiType::UInt8) => Some(AsyncRustFutureSpec {
            suffix: "u8",
            complete_native_type: "ffi.Uint8",
            complete_dart_type: "int",
        }),
        Some(FfiType::Int8) => Some(AsyncRustFutureSpec {
            suffix: "i8",
            complete_native_type: "ffi.Int8",
            complete_dart_type: "int",
        }),
        Some(FfiType::UInt16) => Some(AsyncRustFutureSpec {
            suffix: "u16",
            complete_native_type: "ffi.Uint16",
            complete_dart_type: "int",
        }),
        Some(FfiType::Int16) => Some(AsyncRustFutureSpec {
            suffix: "i16",
            complete_native_type: "ffi.Int16",
            complete_dart_type: "int",
        }),
        Some(FfiType::UInt32) => Some(AsyncRustFutureSpec {
            suffix: "u32",
            complete_native_type: "ffi.Uint32",
            complete_dart_type: "int",
        }),
        Some(FfiType::Int32) => Some(AsyncRustFutureSpec {
            suffix: "i32",
            complete_native_type: "ffi.Int32",
            complete_dart_type: "int",
        }),
        Some(FfiType::UInt64) | Some(FfiType::Handle) => Some(AsyncRustFutureSpec {
            suffix: "u64",
            complete_native_type: "ffi.Uint64",
            complete_dart_type: "int",
        }),
        Some(FfiType::Int64) => Some(AsyncRustFutureSpec {
            suffix: "i64",
            complete_native_type: "ffi.Int64",
            complete_dart_type: "int",
        }),
        Some(FfiType::Float32) => Some(AsyncRustFutureSpec {
            suffix: "f32",
            complete_native_type: "ffi.Float",
            complete_dart_type: "double",
        }),
        Some(FfiType::Float64) => Some(AsyncRustFutureSpec {
            suffix: "f64",
            complete_native_type: "ffi.Double",
            complete_dart_type: "double",
        }),
        Some(FfiType::RustBuffer(_)) => Some(AsyncRustFutureSpec {
            suffix: "rust_buffer",
            complete_native_type: "_UniFfiRustBuffer",
            complete_dart_type: "_UniFfiRustBuffer",
        }),
        _ => None,
    }
}
