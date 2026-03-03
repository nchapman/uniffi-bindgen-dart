use std::collections::HashMap;

use uniffi_bindgen::interface::{ffi::FfiType, Type};

use super::config::CustomTypeConfig;
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

pub(super) fn is_ffibuffer_eligible_function(function: &UdlFunction) -> bool {
    function.ffi_symbol.is_some() && !function.is_async
}

pub(super) fn is_runtime_unsupported_async_ffibuffer_eligible_function(
    function: &UdlFunction,
) -> bool {
    if function.runtime_unsupported.is_none() || !function.is_async || function.ffi_symbol.is_none()
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
    if method.runtime_unsupported.is_none() || !method.is_async || method.ffi_symbol.is_none() {
        return false;
    }
    async_rust_future_spec_from_uniffi_return_type(method.return_type.as_ref()).is_some()
}

pub(super) fn has_runtime_unsupported_async_ffibuffer_support(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    functions
        .iter()
        .any(is_runtime_unsupported_async_ffibuffer_eligible_function)
        || objects.iter().any(|o| {
            o.methods
                .iter()
                .any(is_runtime_unsupported_async_ffibuffer_eligible_method)
        })
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

/// Symbol names for the 5 `late final` lookup fields emitted by
/// [`render_ffibuffer_async_ffi_lookups`].
pub(super) struct FfiBufferAsyncSymbols<'a> {
    pub(super) method_field: &'a str,
    pub(super) ffibuffer_symbol: &'a str,
    pub(super) poll_field: &'a str,
    pub(super) poll_symbol: &'a str,
    pub(super) cancel_field: &'a str,
    pub(super) cancel_symbol: &'a str,
    pub(super) complete_field: &'a str,
    pub(super) complete_symbol: &'a str,
    pub(super) complete_native_sig: &'a str,
    pub(super) complete_dart_sig: &'a str,
    pub(super) free_field: &'a str,
    pub(super) free_symbol: &'a str,
}

/// Emits the 5 `late final` FFI lookup fields for ffi-buffer async methods:
/// the ffi-buffer call, poll, cancel, complete, and free.
pub(super) fn render_ffibuffer_async_ffi_lookups(out: &mut String, syms: &FfiBufferAsyncSymbols) {
    out.push('\n');
    out.push_str(&format!(
        "  late final void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr) {} = _lib.lookupFunction<ffi.Void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr), void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr)>('{}');\n",
        syms.method_field, syms.ffibuffer_symbol
    ));
    out.push_str(&format!(
        "  late final void Function(int handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, int callbackData) {} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, ffi.Uint64 callbackData), void Function(int handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, int callbackData)>('{}');\n",
        syms.poll_field, syms.poll_symbol
    ));
    out.push_str(&format!(
        "  late final void Function(int handle) {} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle), void Function(int handle)>('{}');\n",
        syms.cancel_field, syms.cancel_symbol
    ));
    out.push_str(&format!(
        "  late final {} {} = _lib.lookupFunction<{}, {}>('{}');\n",
        syms.complete_dart_sig,
        syms.complete_field,
        syms.complete_native_sig,
        syms.complete_dart_sig,
        syms.complete_symbol
    ));
    out.push_str(&format!(
        "  late final void Function(int handle) {} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle), void Function(int handle)>('{}');\n",
        syms.free_field, syms.free_symbol
    ));
}

/// Emits the RustBuffer arg serialization block for a single argument.
///
/// Encodes the arg value into bytes, allocates a foreign buffer, calls
/// `_uniFfiRustBufferFromBytes`, checks the status, and writes the resulting
/// RustBuffer fields (capacity/len/data) into `argBuf` at `offset`.
pub(super) fn render_ffibuffer_rustbuffer_arg_serialization(
    out: &mut String,
    arg: &UdlArg,
    offset: usize,
    escaped_reason: &str,
    error_name: &str,
    enums: &[UdlEnum],
    custom_types: &HashMap<String, CustomTypeConfig>,
) {
    let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
    // Compute the lowered expression for custom types (used in encode paths
    // that bypass render_uniffi_binary_write_statement).
    let lowered_arg = if let Type::Custom { name, .. } = &arg.type_ {
        if let Some(cfg) = custom_types.get(name.as_str()) {
            cfg.lower_expr(&arg_name)
        } else {
            arg_name.clone()
        }
    } else {
        arg_name.clone()
    };
    let needs_writer = matches!(
        runtime_unwrapped_type(&arg.type_),
        Type::Map { .. }
            | Type::Sequence { .. }
            | Type::Optional { .. }
            | Type::Timestamp
            | Type::Duration
    );
    let writer_name = format!("{arg_name}Writer");
    let encode_expr = match runtime_unwrapped_type(&arg.type_) {
        Type::Record { name, .. } | Type::Enum { name, .. } => {
            format!("_uniffiEncode{}({lowered_arg})", to_upper_camel(name))
        }
        Type::String => {
            format!("Uint8List.fromList(utf8.encode({lowered_arg}))")
        }
        Type::Bytes => lowered_arg.clone(),
        Type::Map { .. }
        | Type::Sequence { .. }
        | Type::Optional { .. }
        | Type::Timestamp
        | Type::Duration => {
            format!("{writer_name}.toBytes()")
        }
        _ => {
            out.push_str(&format!(
                "      throw UnsupportedError('{escaped_reason} ({error_name})');\n"
            ));
            return;
        }
    };
    if needs_writer {
        let write_stmt = render_uniffi_binary_write_statement(
            &arg.type_,
            &arg_name,
            &writer_name,
            enums,
            "      ",
            custom_types,
        );
        out.push_str(&format!(
            "      final {writer_name} = _UniFfiBinaryWriter();\n"
        ));
        out.push_str(&write_stmt);
    }
    out.push_str(&format!(
        "      final Uint8List {arg_name}Bytes = {encode_expr};\n"
    ));
    out.push_str(&format!(
        "      final ffi.Pointer<ffi.Uint8> {arg_name}Ptr = {arg_name}Bytes.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>({arg_name}Bytes.length);\n"
    ));
    out.push_str(&format!(
        "      if ({arg_name}Bytes.isNotEmpty) {{ {arg_name}Ptr.asTypedList({arg_name}Bytes.length).setAll(0, {arg_name}Bytes); }}\n"
    ));
    out.push_str(&format!("      foreignArgPtrs.add({arg_name}Ptr);\n"));
    let from_bytes_status_ptr = format!("{arg_name}FromBytesStatusPtr");
    let from_bytes_code = format!("{arg_name}FromBytesCode");
    let from_bytes_err_buf = format!("{arg_name}FromBytesErrBuf");
    let from_bytes_err_buf_ptr = format!("{arg_name}FromBytesErrBufPtr");
    out.push_str(&format!(
        "      final ffi.Pointer<_UniFfiRustCallStatus> {from_bytes_status_ptr} = calloc<_UniFfiRustCallStatus>();\n"
    ));
    out.push_str(&format!(
        "      {from_bytes_status_ptr}.ref.code = _uniFfiRustCallStatusSuccess;\n"
    ));
    out.push_str(&format!("      {from_bytes_status_ptr}.ref.errorBuf\n"));
    out.push_str("        ..capacity = 0\n");
    out.push_str("        ..len = 0\n");
    out.push_str("        ..data = ffi.nullptr;\n");
    out.push_str(&format!(
        "      final ffi.Pointer<_UniFfiForeignBytes> {arg_name}ForeignPtr = calloc<_UniFfiForeignBytes>();\n"
    ));
    out.push_str(&format!(
        "      {arg_name}ForeignPtr.ref\n        ..len = {arg_name}Bytes.length\n        ..data = {arg_name}Ptr;\n"
    ));
    out.push_str(&format!(
        "      final _UniFfiRustBuffer {arg_name}RustBuffer = _uniFfiRustBufferFromBytes({arg_name}ForeignPtr.ref, {from_bytes_status_ptr});\n"
    ));
    out.push_str(&format!("      calloc.free({arg_name}ForeignPtr);\n"));
    out.push_str(&format!(
        "      final int {from_bytes_code} = {from_bytes_status_ptr}.ref.code;\n"
    ));
    out.push_str(&format!(
        "      final _UniFfiRustBuffer {from_bytes_err_buf} = {from_bytes_status_ptr}.ref.errorBuf;\n"
    ));
    out.push_str(&format!("      calloc.free({from_bytes_status_ptr});\n"));
    out.push_str(&format!(
        "      if ({from_bytes_code} != _uniFfiRustCallStatusSuccess) {{\n"
    ));
    out.push_str(&format!(
        "        final ffi.Pointer<_UniFfiRustBuffer> {from_bytes_err_buf_ptr} = calloc<_UniFfiRustBuffer>();\n"
    ));
    out.push_str(&format!(
        "        {from_bytes_err_buf_ptr}.ref\n          ..capacity = {from_bytes_err_buf}.capacity\n          ..len = {from_bytes_err_buf}.len\n          ..data = {from_bytes_err_buf}.data;\n"
    ));
    out.push_str(&format!(
        "        rustRetBufferPtrs.add({from_bytes_err_buf_ptr});\n"
    ));
    out.push_str(&format!(
        "        throw StateError('UniFFI rustbuffer_from_bytes failed with status ${from_bytes_code}');\n"
    ));
    out.push_str("      }\n");
    out.push_str(&format!(
        "      (argBuf + {}).ref.u64 = {arg_name}RustBuffer.capacity;\n",
        offset
    ));
    out.push_str(&format!(
        "      (argBuf + {}).ref.u64 = {arg_name}RustBuffer.len;\n",
        offset + 1
    ));
    out.push_str(&format!(
        "      (argBuf + {}).ref.ptr = {arg_name}RustBuffer.data.cast<ffi.Void>();\n",
        offset + 2
    ));
}

/// Emits a primitive (non-RustBuffer) arg write into `argBuf`.
///
/// Handles boolean→i8 coercion and pointer casts.
pub(super) fn render_ffibuffer_primitive_arg_write(
    out: &mut String,
    arg: &UdlArg,
    ffi_type: &FfiType,
    offset: usize,
    escaped_reason: &str,
    error_name: &str,
    custom_types: &HashMap<String, CustomTypeConfig>,
) {
    let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
    // Apply lower transform for custom types backed by primitives.
    let lowered_arg = if let Type::Custom { name, .. } = &arg.type_ {
        if let Some(cfg) = custom_types.get(name.as_str()) {
            cfg.lower_expr(&arg_name)
        } else {
            arg_name.clone()
        }
    } else {
        arg_name.clone()
    };
    let Some(union_field) = ffibuffer_primitive_union_field(ffi_type) else {
        out.push_str(&format!(
            "      throw UnsupportedError('{escaped_reason} ({error_name})');\n"
        ));
        return;
    };
    if union_field == "ptr" {
        out.push_str(&format!(
            "      (argBuf + {}).ref.ptr = {}.cast<ffi.Void>();\n",
            offset, lowered_arg
        ));
    } else {
        let value_expr =
            if union_field == "i8" && matches!(runtime_unwrapped_type(&arg.type_), Type::Boolean) {
                format!("{lowered_arg} ? 1 : 0")
            } else {
                lowered_arg.clone()
            };
        out.push_str(&format!(
            "      (argBuf + {}).ref.{} = {};\n",
            offset, union_field, value_expr
        ));
    }
}

/// Emits the ffi-buffer async complete call, return-type decode dispatch,
/// cancelled/error handling, and inner try/catch/finally (cancel + free).
///
/// This block starts right after the poll loop `}` and ends just before the
/// outer cleanup block.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_ffibuffer_async_complete_and_decode(
    out: &mut String,
    complete_field: &str,
    cancel_field: &str,
    free_field: &str,
    async_spec: &AsyncRustFutureSpec,
    return_type: Option<&Type>,
    throws_type: Option<&Type>,
    error_name: &str,
    local_module_path: &str,
    objects: &[UdlObject],
    enums: &[UdlEnum],
    custom_types: &HashMap<String, CustomTypeConfig>,
) {
    out.push_str(
        "        final ffi.Pointer<_UniFfiRustCallStatus> outStatusPtr = calloc<_UniFfiRustCallStatus>();\n",
    );
    out.push_str("        outStatusPtr.ref.code = _uniFfiRustCallStatusSuccess;\n");
    out.push_str("        outStatusPtr.ref.errorBuf\n");
    out.push_str("          ..capacity = 0\n");
    out.push_str("          ..len = 0\n");
    out.push_str("          ..data = ffi.nullptr;\n");
    out.push_str("        try {\n");
    if return_type.is_none() {
        out.push_str(&format!(
            "          {complete_field}(futureHandle, outStatusPtr);\n"
        ));
    } else {
        out.push_str(&format!(
            "          final {} resultValue = {complete_field}(futureHandle, outStatusPtr);\n",
            async_spec.complete_dart_type
        ));
    }
    out.push_str("          final int completeStatusCode = outStatusPtr.ref.code;\n");
    out.push_str("          if (completeStatusCode == _uniFfiRustCallStatusSuccess) {\n");
    if return_type.is_none() {
        out.push_str("            return;\n");
    } else if async_spec.suffix == "rust_buffer" {
        if let Some(ret_type) = return_type {
            let decode_expr = match runtime_unwrapped_type(ret_type) {
                Type::String => {
                    lift_custom_if_needed("utf8.decode(resultBytes)", ret_type, custom_types)
                }
                Type::Bytes => lift_custom_if_needed("resultBytes", ret_type, custom_types),
                Type::Record { name, .. } | Type::Enum { name, .. } => {
                    format!("_uniffiDecode{}(resultBytes)", to_upper_camel(name))
                }
                _ => render_uniffi_binary_read_expression(
                    ret_type,
                    "resultReader",
                    enums,
                    custom_types,
                ),
            };
            out.push_str(
                "            final ffi.Pointer<_UniFfiRustBuffer> resultBufPtr = calloc<_UniFfiRustBuffer>();\n",
            );
            out.push_str(
                "            resultBufPtr.ref\n              ..capacity = resultValue.capacity\n              ..len = resultValue.len\n              ..data = resultValue.data;\n",
            );
            out.push_str("            rustRetBufferPtrs.add(resultBufPtr);\n");
            out.push_str(
                "            final Uint8List resultBytes = resultBufPtr.ref.len == 0 ? Uint8List(0) : Uint8List.fromList(resultBufPtr.ref.data.asTypedList(resultBufPtr.ref.len));\n",
            );
            if matches!(
                runtime_unwrapped_type(ret_type),
                Type::String | Type::Bytes | Type::Record { .. } | Type::Enum { .. }
            ) {
                out.push_str(&format!("            return {decode_expr};\n"));
            } else {
                out.push_str(
                    "            final _UniFfiBinaryReader resultReader = _UniFfiBinaryReader(resultBytes);\n",
                );
                out.push_str(&format!(
                    "            final decodedValue = {decode_expr};\n"
                ));
                out.push_str("            if (!resultReader.isDone) {\n");
                out.push_str(
                    "              throw StateError('extra bytes remaining while decoding UniFFI rust future payload');\n",
                );
                out.push_str("            }\n");
                out.push_str("            return decodedValue;\n");
            }
        }
    } else if let Some(ret_type) = return_type {
        if is_runtime_object_type(ret_type) {
            let lift = render_object_lift_expr_with_objects(
                ret_type,
                "resultValue",
                local_module_path,
                "this",
                objects,
            );
            out.push_str(&format!("            return {lift};\n"));
        } else if is_runtime_optional_object_type(ret_type) {
            let inner = match runtime_unwrapped_type(ret_type) {
                Type::Optional { inner_type } => inner_type,
                other => unreachable!("expected Optional, got {other:?}"),
            };
            let lift = render_object_lift_expr_with_objects(
                inner,
                "resultValue",
                local_module_path,
                "this",
                objects,
            );
            out.push_str("            if (resultValue == 0) {\n");
            out.push_str("              return null;\n");
            out.push_str("            }\n");
            out.push_str(&format!("            return {lift};\n"));
        } else if is_runtime_timestamp_type(ret_type) {
            out.push_str(
                "            return DateTime.fromMicrosecondsSinceEpoch(resultValue, isUtc: true);\n",
            );
        } else if is_runtime_duration_type(ret_type) {
            out.push_str("            return Duration(microseconds: resultValue);\n");
        } else if is_runtime_optional_primitive_type(ret_type) {
            let decode = render_json_decode_expr("decoded", ret_type, custom_types);
            out.push_str("            if (resultValue == ffi.nullptr) {\n");
            out.push_str(&format!(
                "              throw StateError('Rust returned null pointer for {error_name}');\n"
            ));
            out.push_str("            }\n");
            out.push_str("            try {\n");
            out.push_str("              final String payload = resultValue.toDartString();\n");
            out.push_str("              final Object? decoded = jsonDecode(payload);\n");
            out.push_str(&format!("              return {decode};\n"));
            out.push_str("            } finally {\n");
            out.push_str("              _rustStringFree(resultValue);\n");
            out.push_str("            }\n");
        } else {
            let decode = render_plain_ffi_decode_expr(ret_type, "resultValue", custom_types);
            out.push_str(&format!("            return {decode};\n"));
        }
    }
    out.push_str("          }\n");
    out.push_str("          if (completeStatusCode == _uniFfiRustCallStatusCancelled) {\n");
    out.push_str(&format!(
        "            throw StateError('Rust future was cancelled for {error_name}');\n"
    ));
    out.push_str("          }\n");
    out.push_str("          final _UniFfiRustBuffer errorBuf = outStatusPtr.ref.errorBuf;\n");
    out.push_str(
        "          if (!(errorBuf.data == ffi.nullptr && errorBuf.len == 0 && errorBuf.capacity == 0)) {\n",
    );
    out.push_str(
        "            final ffi.Pointer<_UniFfiRustBuffer> errorBufPtr = calloc<_UniFfiRustBuffer>();\n",
    );
    out.push_str(
        "            errorBufPtr.ref\n              ..capacity = errorBuf.capacity\n              ..len = errorBuf.len\n              ..data = errorBuf.data;\n",
    );
    out.push_str("            rustRetBufferPtrs.add(errorBufPtr);\n");
    out.push_str(
        "            final Uint8List errorBytes = errorBufPtr.ref.len == 0 ? Uint8List(0) : Uint8List.fromList(errorBufPtr.ref.data.asTypedList(errorBufPtr.ref.len));\n",
    );
    if let Some(throws_type) = throws_type {
        if let Some(throws_name) = throws_name_from_type(throws_type).map(to_upper_camel) {
            out.push_str(
                "            if (completeStatusCode == _uniFfiRustCallStatusError && errorBytes.isNotEmpty) {\n",
            );
            if is_throws_object_type(throws_type) {
                out.push_str(
                    "              final ByteData _errBd = ByteData.sublistView(errorBytes);\n",
                );
                out.push_str(
                    "              final int _errHandle = _errBd.getUint64(0, Endian.little);\n",
                );
                out.push_str(&format!(
                    "              throw {throws_name}._(this, _errHandle);\n"
                ));
            } else {
                let exception_name = format!("{throws_name}Exception");
                out.push_str(&format!(
                    "              throw _uniffiLift{exception_name}(errorBytes);\n"
                ));
            }
            out.push_str("            }\n");
        }
    }
    out.push_str("            if (errorBytes.isNotEmpty) {\n");
    out.push_str(
        "              throw StateError(utf8.decode(errorBytes, allowMalformed: true));\n",
    );
    out.push_str("            }\n");
    out.push_str("          }\n");
    out.push_str(&format!(
        "          throw StateError('Rust future failed for {error_name} with status code: $completeStatusCode');\n"
    ));
    out.push_str("        } finally {\n");
    out.push_str("          calloc.free(outStatusPtr);\n");
    out.push_str("        }\n");
    out.push_str("      } catch (_) {\n");
    out.push_str(&format!("        {cancel_field}(futureHandle);\n"));
    out.push_str("        rethrow;\n");
    out.push_str("      } finally {\n");
    out.push_str("        await pollEvents.close();\n");
    out.push_str("        callback.close();\n");
    out.push_str(&format!("        {free_field}(futureHandle);\n"));
    out.push_str("      }\n");
}

/// Emits the ffi-buffer async poll loop: StreamController, NativeCallable listener,
/// while(true) poll/wake/ready loop.
///
/// `poll_field` is the Dart field name for the poll function.
/// `error_name` is the identifier used in the StateError message.
pub(super) fn render_ffibuffer_async_poll_loop(
    out: &mut String,
    poll_field: &str,
    error_name: &str,
) {
    out.push_str(
        "      final StreamController<int> pollEvents = StreamController<int>.broadcast();\n",
    );
    out.push_str(
        "      final callback = ffi.NativeCallable<ffi.Void Function(ffi.Uint64, ffi.Int8)>.listener((int _, int pollResult) {\n",
    );
    out.push_str("        pollEvents.add(pollResult);\n");
    out.push_str("      });\n");
    out.push_str("      try {\n");
    out.push_str(&format!(
        "        {poll_field}(futureHandle, callback.nativeFunction, 0);\n"
    ));
    out.push_str("        while (true) {\n");
    out.push_str("          final int pollResult = await pollEvents.stream.first;\n");
    out.push_str("          if (pollResult == _rustFuturePollReady) {\n");
    out.push_str("            break;\n");
    out.push_str("          }\n");
    out.push_str("          if (pollResult == _rustFuturePollWake) {\n");
    out.push_str(&format!(
        "            {poll_field}(futureHandle, callback.nativeFunction, 0);\n"
    ));
    out.push_str("            continue;\n");
    out.push_str("          }\n");
    out.push_str(&format!(
        "          throw StateError('Rust future poll returned invalid status for {error_name}: $pollResult');\n"
    ));
    out.push_str("        }\n");
}

/// Emits the outer `finally` cleanup block shared by all ffi-buffer code paths.
///
/// Frees `foreignArgPtrs`, calls `_uniFfiRustBufferFree` on each `rustRetBufferPtrs`
/// entry, and frees `argBuf` / `returnBuf`.
pub(super) fn render_ffibuffer_outer_cleanup(out: &mut String) {
    out.push_str("    } finally {\n");
    out.push_str("      for (final ptr in foreignArgPtrs) {\n");
    out.push_str("        if (ptr != ffi.nullptr) {\n");
    out.push_str("          calloc.free(ptr);\n");
    out.push_str("        }\n");
    out.push_str("      }\n");
    out.push_str("      for (final bufPtr in rustRetBufferPtrs) {\n");
    out.push_str(
        "        if (bufPtr.ref.data == ffi.nullptr && bufPtr.ref.len == 0 && bufPtr.ref.capacity == 0) {\n",
    );
    out.push_str("          continue;\n");
    out.push_str("        }\n");
    out.push_str(
        "        final ffi.Pointer<_UniFfiRustCallStatus> freeStatusPtr = calloc<_UniFfiRustCallStatus>();\n",
    );
    out.push_str("        freeStatusPtr.ref.code = _uniFfiRustCallStatusSuccess;\n");
    out.push_str("        freeStatusPtr.ref.errorBuf\n");
    out.push_str("          ..capacity = 0\n");
    out.push_str("          ..len = 0\n");
    out.push_str("          ..data = ffi.nullptr;\n");
    out.push_str("        _uniFfiRustBufferFree(bufPtr.ref, freeStatusPtr);\n");
    out.push_str("        calloc.free(freeStatusPtr);\n");
    out.push_str("        calloc.free(bufPtr);\n");
    out.push_str("      }\n");
    out.push_str("      calloc.free(argBuf);\n");
    out.push_str("      calloc.free(returnBuf);\n");
    out.push_str("    }\n");
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

#[cfg(test)]
mod tests {
    use super::*;
    use uniffi_bindgen::interface::{ffi::FfiType, ObjectImpl, Type};

    // ── ffibuffer_symbol_name ──────────────────────────────────────────

    #[test]
    fn symbol_name_strips_uniffi_prefix() {
        assert_eq!(
            ffibuffer_symbol_name("uniffi_my_crate_fn_add"),
            "uniffi_ffibuffer_my_crate_fn_add"
        );
    }

    #[test]
    fn symbol_name_bare_symbol() {
        assert_eq!(
            ffibuffer_symbol_name("some_bare_symbol"),
            "uniffi_ffibuffer_some_bare_symbol"
        );
    }

    #[test]
    fn symbol_name_just_prefix() {
        assert_eq!(ffibuffer_symbol_name("uniffi_"), "uniffi_ffibuffer_");
    }

    // ── ffibuffer_element_count ────────────────────────────────────────

    #[test]
    fn element_count_uint8() {
        assert_eq!(ffibuffer_element_count(&FfiType::UInt8), Some(1));
    }

    #[test]
    fn element_count_int64() {
        assert_eq!(ffibuffer_element_count(&FfiType::Int64), Some(1));
    }

    #[test]
    fn element_count_float32() {
        assert_eq!(ffibuffer_element_count(&FfiType::Float32), Some(1));
    }

    #[test]
    fn element_count_handle() {
        assert_eq!(ffibuffer_element_count(&FfiType::Handle), Some(1));
    }

    #[test]
    fn element_count_rust_buffer() {
        assert_eq!(ffibuffer_element_count(&FfiType::RustBuffer(None)), Some(3));
    }

    #[test]
    fn element_count_rust_call_status() {
        assert_eq!(ffibuffer_element_count(&FfiType::RustCallStatus), Some(4));
    }

    #[test]
    fn element_count_void_pointer_unsupported() {
        assert_eq!(ffibuffer_element_count(&FfiType::VoidPointer), None);
    }

    // ── ffibuffer_primitive_union_field ─────────────────────────────────

    #[test]
    fn union_field_uint8() {
        assert_eq!(ffibuffer_primitive_union_field(&FfiType::UInt8), Some("u8"));
    }

    #[test]
    fn union_field_int8() {
        assert_eq!(ffibuffer_primitive_union_field(&FfiType::Int8), Some("i8"));
    }

    #[test]
    fn union_field_uint32() {
        assert_eq!(
            ffibuffer_primitive_union_field(&FfiType::UInt32),
            Some("u32")
        );
    }

    #[test]
    fn union_field_uint64() {
        assert_eq!(
            ffibuffer_primitive_union_field(&FfiType::UInt64),
            Some("u64")
        );
    }

    #[test]
    fn union_field_float32() {
        assert_eq!(
            ffibuffer_primitive_union_field(&FfiType::Float32),
            Some("float32")
        );
    }

    #[test]
    fn union_field_float64() {
        assert_eq!(
            ffibuffer_primitive_union_field(&FfiType::Float64),
            Some("float64")
        );
    }

    #[test]
    fn union_field_handle() {
        assert_eq!(
            ffibuffer_primitive_union_field(&FfiType::Handle),
            Some("u64")
        );
    }

    #[test]
    fn union_field_void_pointer_ref() {
        assert_eq!(
            ffibuffer_primitive_union_field(&FfiType::Reference(Box::new(FfiType::VoidPointer))),
            Some("ptr")
        );
    }

    #[test]
    fn union_field_rust_buffer_none() {
        assert_eq!(
            ffibuffer_primitive_union_field(&FfiType::RustBuffer(None)),
            None
        );
    }

    // ── ffibuffer_ffi_type_from_uniffi_type ─────────────────────────────

    #[test]
    fn ffi_type_from_uint8() {
        assert_eq!(
            ffibuffer_ffi_type_from_uniffi_type(&Type::UInt8),
            Some(FfiType::UInt8)
        );
    }

    #[test]
    fn ffi_type_from_string() {
        assert_eq!(
            ffibuffer_ffi_type_from_uniffi_type(&Type::String),
            Some(FfiType::RustBuffer(None))
        );
    }

    #[test]
    fn ffi_type_from_boolean() {
        assert_eq!(
            ffibuffer_ffi_type_from_uniffi_type(&Type::Boolean),
            Some(FfiType::Int8)
        );
    }

    #[test]
    fn ffi_type_from_object() {
        assert_eq!(
            ffibuffer_ffi_type_from_uniffi_type(&Type::Object {
                name: "Foo".into(),
                module_path: "".into(),
                imp: ObjectImpl::Struct,
            }),
            Some(FfiType::Handle)
        );
    }

    #[test]
    fn ffi_type_from_float64() {
        assert_eq!(
            ffibuffer_ffi_type_from_uniffi_type(&Type::Float64),
            Some(FfiType::Float64)
        );
    }

    // ── eligibility predicates ──────────────────────────────────────────

    fn test_function(ffi_symbol: Option<String>, is_async: bool) -> UdlFunction {
        UdlFunction {
            name: "test".to_string(),
            ffi_symbol,
            ffi_arg_types: vec![],
            ffi_return_type: None,
            ffi_has_rust_call_status: false,
            runtime_unsupported: None,
            docstring: None,
            is_async,
            return_type: None,
            throws_type: None,
            args: vec![],
        }
    }

    #[test]
    fn eligible_sync_with_symbol() {
        let f = test_function(Some("uniffi_test".into()), false);
        assert!(is_ffibuffer_eligible_function(&f));
    }

    #[test]
    fn ineligible_no_symbol() {
        let f = test_function(None, false);
        assert!(!is_ffibuffer_eligible_function(&f));
    }

    #[test]
    fn ineligible_async() {
        let f = test_function(Some("uniffi_test".into()), true);
        assert!(!is_ffibuffer_eligible_function(&f));
    }
}
