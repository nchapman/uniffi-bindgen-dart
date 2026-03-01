use uniffi_bindgen::interface::Type;

use super::*;

pub(super) fn has_runtime_async_rust_future_support(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    functions.iter().any(|f| {
        f.runtime_unsupported.is_none()
            && is_runtime_async_rust_future_compatible_function(
                f,
                callback_interfaces,
                records,
                enums,
            )
    }) || objects.iter().any(|o| {
        o.methods.iter().any(|m| {
            m.runtime_unsupported.is_none()
                && is_runtime_async_rust_future_compatible_method(
                    m,
                    callback_interfaces,
                    records,
                    enums,
                )
        })
    }) || records.iter().any(|r| {
        r.methods.iter().any(|m| {
            m.runtime_unsupported.is_none()
                && is_runtime_async_rust_future_compatible_method(
                    m,
                    callback_interfaces,
                    records,
                    enums,
                )
        })
    }) || enums.iter().any(|e| {
        e.methods.iter().any(|m| {
            m.runtime_unsupported.is_none()
                && is_runtime_async_rust_future_compatible_method(
                    m,
                    callback_interfaces,
                    records,
                    enums,
                )
        })
    })
}

pub(super) struct AsyncRustFutureSpec {
    pub(super) suffix: &'static str,
    pub(super) complete_native_type: &'static str,
    pub(super) complete_dart_type: &'static str,
}

pub(super) fn async_rust_future_spec(
    return_type: Option<&Type>,
    _records: &[UdlRecord],
    enums: &[UdlEnum],
) -> Option<AsyncRustFutureSpec> {
    match return_type.map(runtime_unwrapped_type) {
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
        Some(Type::Map { key_type, .. }) if is_runtime_string_type(key_type) => {
            Some(AsyncRustFutureSpec {
                suffix: "string",
                complete_native_type: "ffi.Pointer<Utf8>",
                complete_dart_type: "ffi.Pointer<Utf8>",
            })
        }
        Some(Type::Bytes) => Some(AsyncRustFutureSpec {
            suffix: "bytes",
            complete_native_type: "_RustBuffer",
            complete_dart_type: "_RustBuffer",
        }),
        Some(Type::Optional { inner_type }) if is_runtime_bytes_type(inner_type) => {
            Some(AsyncRustFutureSpec {
                suffix: "bytes_opt",
                complete_native_type: "_RustBufferOpt",
                complete_dart_type: "_RustBufferOpt",
            })
        }
        Some(Type::Sequence { inner_type }) if is_runtime_bytes_type(inner_type) => {
            Some(AsyncRustFutureSpec {
                suffix: "bytes_vec",
                complete_native_type: "_RustBufferVec",
                complete_dart_type: "_RustBufferVec",
            })
        }
        Some(Type::Object { .. }) => Some(AsyncRustFutureSpec {
            suffix: "u64",
            complete_native_type: "ffi.Uint64",
            complete_dart_type: "int",
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

pub(super) fn is_runtime_async_rust_future_compatible_function(
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

pub(super) fn is_runtime_async_rust_future_compatible_method(
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
