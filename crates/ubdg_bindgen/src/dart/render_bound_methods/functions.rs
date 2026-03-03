use uniffi_bindgen::interface::{ffi::FfiType, Type};

use super::super::*;
use super::context::RenderMethodContext;

/// Renders all top-level function bindings (the `for function in &runtime_functions` loop).
pub(super) fn render_toplevel_functions(
    out: &mut String,
    runtime_functions: &[UdlFunction],
    ctx: &RenderMethodContext,
) {
    let ffi_namespace = ctx.ffi_namespace;
    for function in runtime_functions {
        let method_name = safe_dart_identifier(&to_lower_camel(&function.name));
        if let Some(reason) = function.runtime_unsupported.as_ref() {
            let ffibuffer_eligible =
                is_ffibuffer_eligible_function(function) && function.ffi_symbol.is_some();
            let runtime_unsupported_async_ffibuffer_eligible =
                is_runtime_unsupported_async_ffibuffer_eligible_function(function);
            if runtime_unsupported_async_ffibuffer_eligible {
                let value_return_type = function
                    .return_type
                    .as_ref()
                    .map(|t| map_uniffi_type_to_dart(t, ctx.custom_types))
                    .unwrap_or_else(|| "void".to_string());
                let signature_return_type = format!("Future<{value_return_type}>");
                let dart_sig = function
                    .args
                    .iter()
                    .map(|a| {
                        format!(
                            "{} {}",
                            map_uniffi_type_to_dart(&a.type_, ctx.custom_types),
                            safe_dart_identifier(&to_lower_camel(&a.name))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let escaped_reason = reason.replace('\'', "\\'");
                let method_field = format!("_{method_name}FfiBuffer");
                let poll_field = format!("{method_field}RustFuturePoll");
                let cancel_field = format!("{method_field}RustFutureCancel");
                let complete_field = format!("{method_field}RustFutureComplete");
                let free_field = format!("{method_field}RustFutureFree");
                let ffi_symbol = function.ffi_symbol.as_deref().unwrap_or(&function.name);
                let ffibuffer_symbol = ffibuffer_symbol_name(ffi_symbol);
                let ffi_start_return_type =
                    function.ffi_return_type.clone().unwrap_or(FfiType::UInt64);
                let Some(return_ffi_elements) = ffibuffer_element_count(&ffi_start_return_type)
                else {
                    out.push('\n');
                    out.push_str(&format!(
                        "  {signature_return_type} {method_name}({dart_sig}) async {{\n"
                    ));
                    out.push_str(&format!(
                        "    throw UnsupportedError('{escaped_reason} ({})');\n",
                        function.name
                    ));
                    out.push_str("  }\n");
                    continue;
                };
                let Some(async_spec) =
                    async_rust_future_spec_from_uniffi_return_type(function.return_type.as_ref())
                else {
                    out.push('\n');
                    out.push_str(&format!(
                        "  {signature_return_type} {method_name}({dart_sig}) async {{\n"
                    ));
                    out.push_str(&format!(
                        "    throw UnsupportedError('{escaped_reason} ({})');\n",
                        function.name
                    ));
                    out.push_str("  }\n");
                    continue;
                };
                let ffi_arg_types = if function.ffi_arg_types.len() == function.args.len() {
                    function.ffi_arg_types.clone()
                } else {
                    function
                        .args
                        .iter()
                        .filter_map(|a| ffibuffer_ffi_type_from_uniffi_type(&a.type_))
                        .collect::<Vec<_>>()
                };
                let mut arg_ffi_offsets = Vec::new();
                let mut arg_cursor = 0usize;
                let mut signature_compatible = ffi_arg_types.len() == function.args.len();
                if signature_compatible {
                    for ffi_type in &ffi_arg_types {
                        let Some(size) = ffibuffer_element_count(ffi_type) else {
                            signature_compatible = false;
                            break;
                        };
                        arg_ffi_offsets.push(arg_cursor);
                        arg_cursor += size;
                    }
                }
                let start_return_union_field =
                    ffibuffer_primitive_union_field(&ffi_start_return_type);
                if !signature_compatible || start_return_union_field.is_none() {
                    out.push('\n');
                    out.push_str(&format!(
                        "  {signature_return_type} {method_name}({dart_sig}) async {{\n"
                    ));
                    out.push_str(&format!(
                        "    throw UnsupportedError('{escaped_reason} ({})');\n",
                        function.name
                    ));
                    out.push_str("  }\n");
                    continue;
                }
                let start_return_union_field = start_return_union_field.unwrap_or("u64");
                let poll_symbol =
                    format!("ffi_{ffi_namespace}_rust_future_poll_{}", async_spec.suffix);
                let cancel_symbol = format!(
                    "ffi_{ffi_namespace}_rust_future_cancel_{}",
                    async_spec.suffix
                );
                let complete_symbol = format!(
                    "ffi_{ffi_namespace}_rust_future_complete_{}",
                    async_spec.suffix
                );
                let free_symbol =
                    format!("ffi_{ffi_namespace}_rust_future_free_{}", async_spec.suffix);
                let complete_native_sig = format!(
                    "{} Function(ffi.Uint64 handle, ffi.Pointer<_UniFfiRustCallStatus> outStatus)",
                    async_spec.complete_native_type
                );
                let complete_dart_sig = format!(
                    "{} Function(int handle, ffi.Pointer<_UniFfiRustCallStatus> outStatus)",
                    async_spec.complete_dart_type
                );

                super::super::ffi_buffer::render_ffibuffer_async_ffi_lookups(
                    out,
                    &super::super::ffi_buffer::FfiBufferAsyncSymbols {
                        method_field: &method_field,
                        ffibuffer_symbol: &ffibuffer_symbol,
                        poll_field: &poll_field,
                        poll_symbol: &poll_symbol,
                        cancel_field: &cancel_field,
                        cancel_symbol: &cancel_symbol,
                        complete_field: &complete_field,
                        complete_symbol: &complete_symbol,
                        complete_native_sig: &complete_native_sig,
                        complete_dart_sig: &complete_dart_sig,
                        free_field: &free_field,
                        free_symbol: &free_symbol,
                    },
                );
                out.push('\n');
                out.push_str(&format!(
                    "  {signature_return_type} {method_name}({dart_sig}) async {{\n"
                ));
                out.push_str(&format!(
                    "    final ffi.Pointer<_UniFfiFfiBufferElement> argBuf = calloc<_UniFfiFfiBufferElement>({arg_cursor});\n"
                ));
                out.push_str(&format!(
                    "    final ffi.Pointer<_UniFfiFfiBufferElement> returnBuf = calloc<_UniFfiFfiBufferElement>({});\n",
                    return_ffi_elements + 4
                ));
                out.push_str("    final foreignArgPtrs = <ffi.Pointer<ffi.Uint8>>[];\n");
                out.push_str("    final rustRetBufferPtrs = <ffi.Pointer<_UniFfiRustBuffer>>[];\n");
                out.push_str("    try {\n");

                for ((arg, ffi_type), offset) in function
                    .args
                    .iter()
                    .zip(ffi_arg_types.iter())
                    .zip(arg_ffi_offsets.iter())
                {
                    match ffi_type {
                        FfiType::RustBuffer(_) => {
                            render_ffibuffer_rustbuffer_arg_serialization(
                                out,
                                arg,
                                *offset,
                                &escaped_reason,
                                &function.name,
                                ctx.enums,
                                ctx.custom_types,
                            );
                        }
                        _ => {
                            render_ffibuffer_primitive_arg_write(
                                out,
                                arg,
                                ffi_type,
                                *offset,
                                &escaped_reason,
                                &function.name,
                                ctx.custom_types,
                            );
                        }
                    }
                }

                out.push_str(&format!("      {method_field}(argBuf, returnBuf);\n"));
                out.push_str(&format!(
                    "      final int statusCode = (returnBuf + {}).ref.i8;\n",
                    return_ffi_elements
                ));
                out.push_str("      if (statusCode != _uniFfiRustCallStatusSuccess) {\n");
                out.push_str(&format!(
                    "        final ffi.Pointer<_UniFfiRustBuffer> errBufPtr = calloc<_UniFfiRustBuffer>();\n        errBufPtr.ref\n          ..capacity = (returnBuf + {}).ref.u64\n          ..len = (returnBuf + {}).ref.u64\n          ..data = (returnBuf + {}).ref.ptr.cast<ffi.Uint8>();\n",
                    return_ffi_elements + 1,
                    return_ffi_elements + 2,
                    return_ffi_elements + 3
                ));
                out.push_str("        rustRetBufferPtrs.add(errBufPtr);\n");
                out.push_str(
                    "        throw StateError('UniFFI ffibuffer async start failed with status $statusCode');\n",
                );
                out.push_str("      }\n");
                if start_return_union_field == "ptr" {
                    out.push_str(
                        "      final int futureHandle = (returnBuf + 0).ref.ptr.address;\n",
                    );
                } else {
                    out.push_str(&format!(
                        "      final int futureHandle = (returnBuf + 0).ref.{start_return_union_field};\n"
                    ));
                }
                render_ffibuffer_async_poll_loop(out, &poll_field, &function.name);
                render_ffibuffer_async_complete_and_decode(
                    out,
                    &complete_field,
                    &cancel_field,
                    &free_field,
                    &async_spec,
                    function.return_type.as_ref(),
                    function.throws_type.as_ref(),
                    &function.name,
                    ctx.local_module_path,
                    ctx.objects,
                    ctx.enums,
                    ctx.custom_types,
                );
                render_ffibuffer_outer_cleanup(out);
                out.push_str("  }\n");
                continue;
            }
            if ffibuffer_eligible {
                let value_return_type = function
                    .return_type
                    .as_ref()
                    .map(|t| map_uniffi_type_to_dart(t, ctx.custom_types))
                    .unwrap_or_else(|| "void".to_string());
                let signature_return_type = if function.is_async {
                    format!("Future<{value_return_type}>")
                } else {
                    value_return_type.clone()
                };
                let dart_sig = function
                    .args
                    .iter()
                    .map(|a| {
                        format!(
                            "{} {}",
                            map_uniffi_type_to_dart(&a.type_, ctx.custom_types),
                            safe_dart_identifier(&to_lower_camel(&a.name))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let method_field = format!("_{method_name}FfiBuffer");
                let ffi_symbol = function.ffi_symbol.as_deref().unwrap_or(&function.name);
                let ffibuffer_symbol = ffibuffer_symbol_name(ffi_symbol);
                let ffi_return_type = function.ffi_return_type.clone().or_else(|| {
                    function
                        .return_type
                        .as_ref()
                        .and_then(ffibuffer_ffi_type_from_uniffi_type)
                });
                // For void-returning functions, return_ffi_elements is 0 (no return
                // value slots); the return buffer only holds the RustCallStatus
                // fields (4 elements).
                let return_ffi_elements = match &ffi_return_type {
                    Some(fft) => {
                        let Some(count) = ffibuffer_element_count(fft) else {
                            continue;
                        };
                        count
                    }
                    None => 0,
                };
                let ffi_arg_types = if function.ffi_arg_types.len() == function.args.len() {
                    function.ffi_arg_types.clone()
                } else {
                    function
                        .args
                        .iter()
                        .filter_map(|a| ffibuffer_ffi_type_from_uniffi_type(&a.type_))
                        .collect::<Vec<_>>()
                };
                let mut arg_ffi_offsets = Vec::new();
                let mut arg_cursor = 0usize;
                let mut signature_compatible = ffi_arg_types.len() == function.args.len();
                if signature_compatible {
                    for ffi_type in &ffi_arg_types {
                        let Some(size) = ffibuffer_element_count(ffi_type) else {
                            signature_compatible = false;
                            break;
                        };
                        arg_ffi_offsets.push(arg_cursor);
                        arg_cursor += size;
                    }
                }
                if !signature_compatible {
                    let escaped_reason = reason.replace('\'', "\\'");
                    out.push('\n');
                    out.push_str(&format!(
                        "  {signature_return_type} {method_name}({dart_sig}){} {{\n",
                        if function.is_async { " async" } else { "" }
                    ));
                    out.push_str(&format!(
                        "    throw UnsupportedError('{escaped_reason} ({})');\n",
                        function.name
                    ));
                    out.push_str("  }\n");
                    continue;
                }

                out.push('\n');
                out.push_str(&format!(
                    "  late final void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr) {method_field} = _lib.lookupFunction<ffi.Void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr), void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr)>('{ffibuffer_symbol}');\n"
                ));
                out.push('\n');
                out.push_str(&format!(
                    "  {signature_return_type} {method_name}({dart_sig}){} {{\n",
                    if function.is_async { " async" } else { "" }
                ));
                out.push_str(&format!(
                    "    final ffi.Pointer<_UniFfiFfiBufferElement> argBuf = calloc<_UniFfiFfiBufferElement>({arg_cursor});\n"
                ));
                out.push_str(&format!(
                    "    final ffi.Pointer<_UniFfiFfiBufferElement> returnBuf = calloc<_UniFfiFfiBufferElement>({});\n",
                    return_ffi_elements + 4
                ));
                out.push_str("    final foreignArgPtrs = <ffi.Pointer<ffi.Uint8>>[];\n");
                out.push_str("    final rustRetBufferPtrs = <ffi.Pointer<_UniFfiRustBuffer>>[];\n");
                out.push_str("    try {\n");

                let escaped_reason = reason.replace('\'', "\\'");
                for ((arg, ffi_type), offset) in function
                    .args
                    .iter()
                    .zip(ffi_arg_types.iter())
                    .zip(arg_ffi_offsets.iter())
                {
                    match ffi_type {
                        FfiType::RustBuffer(_) => {
                            render_ffibuffer_rustbuffer_arg_serialization(
                                out,
                                arg,
                                *offset,
                                &escaped_reason,
                                &function.name,
                                ctx.enums,
                                ctx.custom_types,
                            );
                        }
                        _ => {
                            render_ffibuffer_primitive_arg_write(
                                out,
                                arg,
                                ffi_type,
                                *offset,
                                &escaped_reason,
                                &function.name,
                                ctx.custom_types,
                            );
                        }
                    }
                }

                out.push_str(&format!("      {method_field}(argBuf, returnBuf);\n"));
                out.push_str(&format!(
                    "      final int statusCode = (returnBuf + {}).ref.i8;\n",
                    return_ffi_elements
                ));
                out.push_str("      if (statusCode != _uniFfiRustCallStatusSuccess) {\n");
                out.push_str(&format!(
                    "        final ffi.Pointer<_UniFfiRustBuffer> errBufPtr = calloc<_UniFfiRustBuffer>();\n        errBufPtr.ref\n          ..capacity = (returnBuf + {}).ref.u64\n          ..len = (returnBuf + {}).ref.u64\n          ..data = (returnBuf + {}).ref.ptr.cast<ffi.Uint8>();\n",
                    return_ffi_elements + 1,
                    return_ffi_elements + 2,
                    return_ffi_elements + 3
                ));
                out.push_str("        rustRetBufferPtrs.add(errBufPtr);\n");
                if let Some(throws_type) = function.throws_type.as_ref() {
                    if let Some(throws_name) =
                        throws_name_from_type(throws_type).map(to_upper_camel)
                    {
                        out.push_str("        if (statusCode == _uniFfiRustCallStatusError) {\n");
                        out.push_str(
                            "          final Uint8List errBytes = errBufPtr.ref.len == 0 ? Uint8List(0) : Uint8List.fromList(errBufPtr.ref.data.asTypedList(errBufPtr.ref.len));\n",
                        );
                        if is_throws_object_type(throws_type) {
                            out.push_str("          final ByteData _errBd = ByteData.sublistView(errBytes);\n");
                            out.push_str("          final int _errHandle = _errBd.getUint64(0, Endian.little);\n");
                            out.push_str(&format!(
                                "          throw {throws_name}._(this, _errHandle);\n"
                            ));
                        } else {
                            let exception_name = format!("{throws_name}Exception");
                            out.push_str(&format!(
                                "          throw _uniffiLift{exception_name}(errBytes);\n"
                            ));
                        }
                        out.push_str("        }\n");
                    }
                }
                out.push_str(
                    "        throw StateError('UniFFI ffibuffer call failed with status $statusCode');\n",
                );
                out.push_str("      }\n");

                match function.return_type.as_ref() {
                    None => out.push_str("      return;\n"),
                    Some(ret_type) => match ffi_return_type.as_ref() {
                        Some(FfiType::RustBuffer(_)) => {
                            let decode_expr = match runtime_unwrapped_type(ret_type) {
                                Type::String => lift_custom_if_needed(
                                    "utf8.decode(retBytes)",
                                    ret_type,
                                    ctx.custom_types,
                                ),
                                Type::Bytes => {
                                    lift_custom_if_needed("retBytes", ret_type, ctx.custom_types)
                                }
                                Type::Record { name, .. } | Type::Enum { name, .. } => {
                                    format!("_uniffiDecode{}(retBytes)", to_upper_camel(name))
                                }
                                _ => render_uniffi_binary_read_expression(
                                    ret_type,
                                    "retReader",
                                    ctx.enums,
                                    ctx.custom_types,
                                ),
                            };
                            out.push_str(
                                "      final ffi.Pointer<_UniFfiRustBuffer> retBufPtr = calloc<_UniFfiRustBuffer>();\n",
                            );
                            out.push_str(
                                "      retBufPtr.ref\n        ..capacity = (returnBuf + 0).ref.u64\n        ..len = (returnBuf + 1).ref.u64\n        ..data = (returnBuf + 2).ref.ptr.cast<ffi.Uint8>();\n",
                            );
                            out.push_str("      rustRetBufferPtrs.add(retBufPtr);\n");
                            out.push_str(
                                "      final Uint8List retBytes = retBufPtr.ref.len == 0 ? Uint8List(0) : Uint8List.fromList(retBufPtr.ref.data.asTypedList(retBufPtr.ref.len));\n",
                            );
                            if matches!(
                                runtime_unwrapped_type(ret_type),
                                Type::String
                                    | Type::Bytes
                                    | Type::Record { .. }
                                    | Type::Enum { .. }
                            ) {
                                out.push_str(&format!(
                                    "      final decodedValue = {decode_expr};\n"
                                ));
                            } else {
                                out.push_str(
                                    "      final _UniFfiBinaryReader retReader = _UniFfiBinaryReader(retBytes);\n",
                                );
                                out.push_str(&format!(
                                    "      final decodedValue = {decode_expr};\n"
                                ));
                                out.push_str("      if (!retReader.isDone) {\n");
                                out.push_str(
                                    "        throw StateError('extra bytes remaining while decoding UniFFI ffibuffer return payload');\n",
                                );
                                out.push_str("      }\n");
                            }
                            out.push_str("      return decodedValue;\n");
                        }
                        _ => {
                            let Some(union_field) = ffi_return_type
                                .as_ref()
                                .and_then(ffibuffer_primitive_union_field)
                            else {
                                let escaped_reason = reason.replace('\'', "\\'");
                                out.push_str(&format!(
                                    "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                    function.name
                                ));
                                out.push_str("      return;\n");
                                out.push_str("    } finally {\n");
                                out.push_str("      calloc.free(argBuf);\n");
                                out.push_str("      calloc.free(returnBuf);\n");
                                out.push_str("    }\n");
                                out.push_str("  }\n");
                                continue;
                            };
                            if union_field == "ptr" {
                                out.push_str("      return (returnBuf + 0).ref.ptr;\n");
                            } else {
                                out.push_str(&format!(
                                    "      return (returnBuf + 0).ref.{union_field};\n"
                                ));
                            }
                        }
                    },
                }
                render_ffibuffer_outer_cleanup(out);
                out.push_str("  }\n");
                continue;
            }

            let value_return_type = function
                .return_type
                .as_ref()
                .map(|t| map_uniffi_type_to_dart(t, ctx.custom_types))
                .unwrap_or_else(|| "void".to_string());
            let signature_return_type = if function.is_async {
                format!("Future<{value_return_type}>")
            } else {
                value_return_type
            };
            let dart_sig = function
                .args
                .iter()
                .map(|a| {
                    format!(
                        "{} {}",
                        map_uniffi_type_to_dart(&a.type_, ctx.custom_types),
                        safe_dart_identifier(&to_lower_camel(&a.name))
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let escaped_reason = reason.replace('\'', "\\'");
            out.push('\n');
            if function.is_async {
                out.push_str(&format!(
                    "  {signature_return_type} {method_name}({dart_sig}) async {{\n"
                ));
            } else {
                out.push_str(&format!(
                    "  {signature_return_type} {method_name}({dart_sig}) {{\n"
                ));
            }
            out.push_str(&format!(
                "    throw UnsupportedError('{escaped_reason} ({})');\n",
                function.name
            ));
            out.push_str("  }\n");
            continue;
        }

        let is_runtime_supported =
            is_runtime_ffi_compatible_function(function, ctx.records, ctx.enums);
        let is_sync_callback_supported = is_runtime_callback_compatible_function(
            function,
            ctx.callback_interfaces,
            ctx.records,
            ctx.enums,
        );
        let has_callback_args = has_runtime_callback_args_in_args(
            &function.args,
            ctx.callback_interfaces,
            ctx.records,
            ctx.enums,
        );
        if !is_runtime_supported && !is_sync_callback_supported && !has_callback_args {
            emit_function_skip_warning(out, &function.name, &function.args, ctx.custom_types, "  ");
            continue;
        }
        let field_name = format!("_{}", method_name);
        let function_symbol = function.ffi_symbol.as_deref().unwrap_or(&function.name);
        if is_sync_callback_supported {
            let return_type = function
                .return_type
                .as_ref()
                .map(|t| map_uniffi_type_to_dart(t, ctx.custom_types))
                .unwrap_or_else(|| "void".to_string());
            let native_return = function
                .return_type
                .as_ref()
                .and_then(|t| map_runtime_native_ffi_type(t, ctx.records, ctx.enums))
                .unwrap_or("ffi.Void");
            let dart_ffi_return = function
                .return_type
                .as_ref()
                .and_then(|t| map_runtime_dart_ffi_type(t, ctx.records, ctx.enums))
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
                    map_uniffi_type_to_dart(&arg.type_, ctx.custom_types),
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
                let native_type = map_runtime_native_ffi_type(&arg.type_, ctx.records, ctx.enums)
                    .expect("validated callback-compatible arg type");
                let dart_ffi_type = map_runtime_dart_ffi_type(&arg.type_, ctx.records, ctx.enums)
                    .expect("validated callback-compatible arg type");
                native_args.push(format!("{native_type} {arg_name}"));
                dart_ffi_args.push(format!("{dart_ffi_type} {arg_name}"));
                append_runtime_arg_marshalling(
                    &arg_name,
                    &arg.type_,
                    ctx.enums,
                    ctx.custom_types,
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
                function_symbol
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
                    let decode = render_plain_ffi_decode_expr(ret_type, &call, ctx.custom_types);
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
            .map(|t| map_uniffi_type_to_dart(t, ctx.custom_types))
            .unwrap_or_else(|| "void".to_string());
        let is_throwing = is_runtime_throwing_ffi_compatible_function(
            function,
            ctx.callback_interfaces,
            ctx.records,
            ctx.enums,
        );
        let native_return = function
            .return_type
            .as_ref()
            .map(|t| {
                if is_throwing {
                    Some("ffi.Pointer<Utf8>")
                } else {
                    map_runtime_native_ffi_type(t, ctx.records, ctx.enums)
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
                    map_runtime_dart_ffi_type(t, ctx.records, ctx.enums)
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
            emit_function_skip_warning(out, &function.name, &function.args, ctx.custom_types, "  ");
            continue;
        };
        let Some(dart_ffi_return) = dart_ffi_return else {
            emit_function_skip_warning(out, &function.name, &function.args, ctx.custom_types, "  ");
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
                map_uniffi_type_to_dart(&arg.type_, ctx.custom_types),
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
            let Some(native_type) = map_runtime_native_ffi_type(&arg.type_, ctx.records, ctx.enums)
            else {
                signature_compatible = false;
                break;
            };
            let Some(dart_ffi_type) = map_runtime_dart_ffi_type(&arg.type_, ctx.records, ctx.enums)
            else {
                signature_compatible = false;
                break;
            };
            native_args.push(format!("{native_type} {arg_name}"));
            dart_ffi_args.push(format!("{dart_ffi_type} {arg_name}"));
            append_runtime_arg_marshalling(
                &arg_name,
                &arg.type_,
                ctx.enums,
                ctx.custom_types,
                &mut pre_call,
                &mut post_call,
                &mut call_args,
            );
        }

        if !signature_compatible {
            emit_function_skip_warning(out, &function.name, &function.args, ctx.custom_types, "  ");
            continue;
        }

        if is_runtime_async_rust_future_compatible_function(
            function,
            ctx.callback_interfaces,
            ctx.records,
            ctx.enums,
        ) {
            let Some(async_spec) =
                async_rust_future_spec(function.return_type.as_ref(), ctx.records, ctx.enums)
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
                function_symbol
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
                if is_runtime_utf8_pointer_marshaled_type(ret_type, ctx.records, ctx.enums) {
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
                    let lifted = lift_custom_if_needed(
                        "resultPtr.toDartString()",
                        ret_type,
                        ctx.custom_types,
                    );
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str(&format!("            return {lifted};\n"));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if is_runtime_optional_string_type(ret_type) {
                    let lifted = lift_custom_if_needed(
                        "resultPtr.toDartString()",
                        ret_type,
                        ctx.custom_types,
                    );
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("            return null;\n");
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str(&format!("            return {lifted};\n"));
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
                } else if is_runtime_enum_type(ret_type, ctx.enums) {
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
                        "            return {}FfiCodec.decode(payload);\n",
                        to_upper_camel(enum_name)
                    ));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if is_runtime_object_type(ret_type) {
                    let lift = render_object_lift_expr_with_objects(
                        ret_type,
                        "resultValue",
                        ctx.local_module_path,
                        "this",
                        ctx.objects,
                    );
                    out.push_str(&format!("          return {lift};\n"));
                } else if is_runtime_optional_object_type(ret_type) {
                    let inner = match runtime_unwrapped_type(ret_type) {
                        Type::Optional { inner_type } => inner_type,
                        other => unreachable!("expected Optional or Sequence, got {other:?}"),
                    };
                    let lift = render_object_lift_expr_with_objects(
                        inner,
                        "resultValue",
                        ctx.local_module_path,
                        "this",
                        ctx.objects,
                    );
                    out.push_str("          if (resultValue == 0) {\n");
                    out.push_str("            return null;\n");
                    out.push_str("          }\n");
                    out.push_str(&format!("          return {lift};\n"));
                } else if is_runtime_optional_record_type(ret_type) {
                    let inner = match runtime_unwrapped_type(ret_type) {
                        Type::Optional { inner_type } => inner_type,
                        other => unreachable!("expected Optional or Sequence, got {other:?}"),
                    };
                    let record_name = record_name_from_type(inner).unwrap_or("Record");
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("            return null;\n");
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
                } else if is_runtime_optional_enum_type(ret_type) {
                    let inner = match runtime_unwrapped_type(ret_type) {
                        Type::Optional { inner_type } => inner_type,
                        other => unreachable!("expected Optional or Sequence, got {other:?}"),
                    };
                    let enum_name = enum_name_from_type(inner).unwrap_or("Enum");
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("            return null;\n");
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "            return {}FfiCodec.decode(payload);\n",
                        to_upper_camel(enum_name)
                    ));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if is_runtime_optional_primitive_type(ret_type) {
                    let decode = render_json_decode_expr("decoded", ret_type, ctx.custom_types);
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str("            final Object? decoded = jsonDecode(payload);\n");
                    out.push_str(&format!("            return {decode};\n"));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if is_runtime_sequence_json_type(ret_type) {
                    let inner_type = match runtime_unwrapped_type(ret_type) {
                        Type::Sequence { inner_type } => inner_type,
                        other => unreachable!("expected Optional or Sequence, got {other:?}"),
                    };
                    let decode = render_json_decode_expr("item", inner_type, ctx.custom_types);
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "            return (jsonDecode(payload) as List).map((item) => {decode}).toList();\n"
                    ));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if is_runtime_map_with_string_key_type(ret_type) {
                    let decode =
                        render_json_decode_expr("jsonDecode(payload)", ret_type, ctx.custom_types);
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!("            return {decode};\n"));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if is_runtime_map_type(ret_type) {
                    let decode = render_uniffi_binary_read_expression(
                        ret_type,
                        "mapReader",
                        ctx.enums,
                        ctx.custom_types,
                    );
                    out.push_str("          final _RustBuffer resultBuf = resultValue;\n");
                    out.push_str(
                        "          final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n",
                    );
                    out.push_str("          final int resultLen = resultBuf.len;\n");
                    out.push_str("          if (resultData == ffi.nullptr) {\n");
                    out.push_str("            _rustBytesFree(resultBuf);\n");
                    out.push_str(
                        "            final mapReader = _UniFfiBinaryReader(Uint8List(0));\n",
                    );
                    out.push_str(&format!("            return {decode};\n"));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final Uint8List resultBytes = Uint8List.fromList(resultData.asTypedList(resultLen));\n");
                    out.push_str(
                        "            final mapReader = _UniFfiBinaryReader(resultBytes);\n",
                    );
                    out.push_str(&format!("            return {decode};\n"));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustBytesFree(resultBuf);\n");
                    out.push_str("          }\n");
                } else if is_runtime_bytes_type(ret_type) {
                    out.push_str("          final _RustBuffer resultBuf = resultValue;\n");
                    out.push_str(
                        "          final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n",
                    );
                    out.push_str("          final int resultLen = resultBuf.len;\n");
                    out.push_str("          if (resultData == ffi.nullptr) {\n");
                    out.push_str("            if (resultLen == 0) {\n");
                    out.push_str("              _rustBytesFree(resultBuf);\n");
                    out.push_str("              return Uint8List(0);\n");
                    out.push_str("            }\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned invalid buffer for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str(
                        "            return Uint8List.fromList(resultData.asTypedList(resultLen));\n",
                    );
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustBytesFree(resultBuf);\n");
                    out.push_str("          }\n");
                } else if is_runtime_optional_bytes_type(ret_type) {
                    out.push_str("          final _RustBufferOpt resultOpt = resultValue;\n");
                    out.push_str("          if (resultOpt.isSome == 0) {\n");
                    out.push_str("            return null;\n");
                    out.push_str("          }\n");
                    out.push_str("          final _RustBuffer resultBuf = resultOpt.value;\n");
                    out.push_str(
                        "          final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n",
                    );
                    out.push_str("          final int resultLen = resultBuf.len;\n");
                    out.push_str("          if (resultData == ffi.nullptr) {\n");
                    out.push_str("            if (resultLen == 0) {\n");
                    out.push_str("              _rustBytesFree(resultBuf);\n");
                    out.push_str("              return Uint8List(0);\n");
                    out.push_str("            }\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned invalid optional buffer for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str(
                        "            return Uint8List.fromList(resultData.asTypedList(resultLen));\n",
                    );
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustBytesFree(resultBuf);\n");
                    out.push_str("          }\n");
                } else if is_runtime_sequence_bytes_type(ret_type) {
                    out.push_str("          final _RustBufferVec resultVec = resultValue;\n");
                    out.push_str(
                        "          final ffi.Pointer<_RustBuffer> resultData = resultVec.data;\n",
                    );
                    out.push_str("          final int resultLen = resultVec.len;\n");
                    out.push_str("          if (resultData == ffi.nullptr) {\n");
                    out.push_str("            if (resultLen == 0) {\n");
                    out.push_str("              _rustBytesVecFree(resultVec);\n");
                    out.push_str("              return <Uint8List>[];\n");
                    out.push_str("            }\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned invalid byte vector for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final out = <Uint8List>[];\n");
                    out.push_str("            for (var i = 0; i < resultLen; i++) {\n");
                    out.push_str("              final _RustBuffer item = (resultData + i).ref;\n");
                    out.push_str(
                        "              final ffi.Pointer<ffi.Uint8> itemData = item.data;\n",
                    );
                    out.push_str("              final int itemLen = item.len;\n");
                    out.push_str("              if (itemData == ffi.nullptr) {\n");
                    out.push_str("                if (itemLen == 0) {\n");
                    out.push_str("                  out.add(Uint8List(0));\n");
                    out.push_str("                  continue;\n");
                    out.push_str("                }\n");
                    out.push_str(&format!(
                        "                throw StateError('Rust returned invalid nested buffer for {}');\n",
                        function.name
                    ));
                    out.push_str("              }\n");
                    out.push_str("              try {\n");
                    out.push_str(
                        "                out.add(Uint8List.fromList(itemData.asTypedList(itemLen)));\n",
                    );
                    out.push_str("              } finally {\n");
                    out.push_str("                _rustBytesFree(item);\n");
                    out.push_str("              }\n");
                    out.push_str("            }\n");
                    out.push_str("            return out;\n");
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustBytesVecFree(resultVec);\n");
                    out.push_str("          }\n");
                } else if is_runtime_timestamp_type(ret_type) {
                    out.push_str(
                        "          return DateTime.fromMicrosecondsSinceEpoch(resultValue, isUtc: true);\n",
                    );
                } else if is_runtime_duration_type(ret_type) {
                    out.push_str("          return Duration(microseconds: resultValue);\n");
                } else {
                    let decode =
                        render_plain_ffi_decode_expr(ret_type, "resultValue", ctx.custom_types);
                    out.push_str(&format!("          return {decode};\n"));
                }
            } else {
                out.push_str("          return;\n");
            }
            out.push_str("        }\n");
            debug_assert!(
                function.throws_type.as_ref().and_then(throws_name_from_type).is_some()
                    || function.throws_type.is_none(),
                "throws_type passed is_runtime_throws_enum_type but throws_name_from_type returned None"
            );
            if let Some(throws_type) = function.throws_type.as_ref() {
                if let Some(throws_name) = throws_name_from_type(throws_type).map(to_upper_camel) {
                    // Free any bytes-like result buffer on the error path to prevent leaks.
                    if let Some(ret_type) = function.return_type.as_ref() {
                        if is_runtime_bytes_type(ret_type)
                            || is_runtime_non_string_map_type(ret_type)
                        {
                            out.push_str("        {\n");
                            out.push_str("          final _RustBuffer buf = resultValue;\n");
                            out.push_str(
                                "          if (buf.len > 0 && buf.data != ffi.nullptr) {\n",
                            );
                            out.push_str("            _rustBytesFree(buf);\n");
                            out.push_str("          }\n");
                            out.push_str("        }\n");
                        } else if is_runtime_optional_bytes_type(ret_type) {
                            out.push_str("        {\n");
                            out.push_str("          final _RustBufferOpt opt = resultValue;\n");
                            out.push_str("          if (opt.isSome != 0) {\n");
                            out.push_str("            final _RustBuffer buf = opt.value;\n");
                            out.push_str(
                                "            if (buf.len > 0 && buf.data != ffi.nullptr) {\n",
                            );
                            out.push_str("              _rustBytesFree(buf);\n");
                            out.push_str("            }\n");
                            out.push_str("          }\n");
                            out.push_str("        }\n");
                        } else if is_runtime_sequence_bytes_type(ret_type) {
                            out.push_str("        {\n");
                            out.push_str("          final _RustBufferVec vec = resultValue;\n");
                            out.push_str(
                                "          if (vec.len > 0 && vec.data != ffi.nullptr) {\n",
                            );
                            out.push_str("            _rustBytesVecFree(vec);\n");
                            out.push_str("          }\n");
                            out.push_str("        }\n");
                        } else if is_runtime_sequence_json_type(ret_type)
                            || is_runtime_map_with_string_key_type(ret_type)
                        {
                            out.push_str("        {\n");
                            out.push_str("          final ffi.Pointer<Utf8> ptr = resultPtr;\n");
                            out.push_str(
                                "          if (ptr != ffi.nullptr) _rustStringFree(ptr);\n",
                            );
                            out.push_str("        }\n");
                        }
                    }
                    out.push_str("        if (statusCode == _rustCallStatusError) {\n");
                    out.push_str(
                        "          final ffi.Pointer<Utf8> errorPtr = outStatusPtr.ref.errorBuf;\n",
                    );
                    out.push_str("          if (errorPtr != ffi.nullptr) {\n");
                    out.push_str("            try {\n");
                    out.push_str(
                        "              final String errorPayload = errorPtr.toDartString();\n",
                    );
                    if is_throws_object_type(throws_type) {
                        out.push_str(&format!(
                            "              throw {throws_name}._(this, (jsonDecode(errorPayload) as num).toInt());\n"
                        ));
                    } else {
                        out.push_str(&format!(
                            "              throw {}ExceptionFfiCodec.decode(jsonDecode(errorPayload));\n",
                            throws_name
                        ));
                    }
                    out.push_str("            } finally {\n");
                    out.push_str("              _rustStringFree(errorPtr);\n");
                    out.push_str("            }\n");
                    out.push_str("          }\n");
                    out.push_str(&format!(
                        "          throw StateError('Rust async error without payload for {}');\n",
                        function.name
                    ));
                    out.push_str("        }\n");
                }
            }
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
            function_symbol
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
            let Some(throws_type) = function.throws_type.as_ref() else {
                continue;
            };
            let Some(_throws_name) = throws_name_from_type(throws_type) else {
                continue;
            };
            let ok_decode = function
                .return_type
                .as_ref()
                .map(|t| render_json_decode_expr("okRaw", t, ctx.custom_types));
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
            out.push_str(&render_throws_expr(throws_type, "errRaw", "        "));
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
                    let lifted =
                        lift_custom_if_needed("resultPtr.toDartString()", type_, ctx.custom_types);
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
                    out.push_str(&format!("        return {lifted};\n"));
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustStringFree(resultPtr);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_optional_string_type(type_) => {
                    let lifted =
                        lift_custom_if_needed("resultPtr.toDartString()", type_, ctx.custom_types);
                    out.push_str(&format!(
                        "      final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("      if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("        return null;\n");
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str(&format!("        return {lifted};\n"));
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
                Some(type_) if is_runtime_enum_type(type_, ctx.enums) => {
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
                        "        return {}FfiCodec.decode(payload);\n",
                        to_upper_camel(enum_name)
                    ));
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustStringFree(resultPtr);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_object_type(type_) => {
                    let lift = render_object_lift_expr_with_objects(
                        type_,
                        &call_expr,
                        ctx.local_module_path,
                        "this",
                        ctx.objects,
                    );
                    out.push_str(&format!("      return {lift};\n"));
                }
                Some(type_) if is_runtime_optional_object_type(type_) => {
                    let inner = match runtime_unwrapped_type(type_) {
                        Type::Optional { inner_type } => inner_type,
                        other => unreachable!("expected Optional or Sequence, got {other:?}"),
                    };
                    out.push_str(&format!("      final int resultHandle = {call_expr};\n"));
                    out.push_str("      if (resultHandle == 0) {\n");
                    out.push_str("        return null;\n");
                    out.push_str("      }\n");
                    let lift = render_object_lift_expr_with_objects(
                        inner,
                        "resultHandle",
                        ctx.local_module_path,
                        "this",
                        ctx.objects,
                    );
                    out.push_str(&format!("      return {lift};\n"));
                }
                Some(type_) if is_runtime_optional_record_type(type_) => {
                    let inner = match runtime_unwrapped_type(type_) {
                        Type::Optional { inner_type } => inner_type,
                        other => unreachable!("expected Optional or Sequence, got {other:?}"),
                    };
                    let record_name = record_name_from_type(inner).unwrap_or("Record");
                    out.push_str(&format!(
                        "      final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("      if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("        return null;\n");
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
                Some(type_) if is_runtime_optional_enum_type(type_) => {
                    let inner = match runtime_unwrapped_type(type_) {
                        Type::Optional { inner_type } => inner_type,
                        other => unreachable!("expected Optional or Sequence, got {other:?}"),
                    };
                    let enum_name = enum_name_from_type(inner).unwrap_or("Enum");
                    out.push_str(&format!(
                        "      final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("      if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("        return null;\n");
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str("        final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "        return {}FfiCodec.decode(payload);\n",
                        to_upper_camel(enum_name)
                    ));
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustStringFree(resultPtr);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_optional_primitive_type(type_) => {
                    let decode = render_json_decode_expr("decoded", type_, ctx.custom_types);
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
                    out.push_str("        final Object? decoded = jsonDecode(payload);\n");
                    out.push_str(&format!("        return {decode};\n"));
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustStringFree(resultPtr);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_sequence_json_type(type_) => {
                    let inner_type = match runtime_unwrapped_type(type_) {
                        Type::Sequence { inner_type } => inner_type,
                        other => unreachable!("expected Optional or Sequence, got {other:?}"),
                    };
                    let decode = render_json_decode_expr("item", inner_type, ctx.custom_types);
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
                        "        return (jsonDecode(payload) as List).map((item) => {decode}).toList();\n"
                    ));
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustStringFree(resultPtr);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_map_with_string_key_type(type_) => {
                    let decode =
                        render_json_decode_expr("jsonDecode(payload)", type_, ctx.custom_types);
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
                    out.push_str(&format!("        return {decode};\n"));
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustStringFree(resultPtr);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_map_type(type_) => {
                    let decode = render_uniffi_binary_read_expression(
                        type_,
                        "mapReader",
                        ctx.enums,
                        ctx.custom_types,
                    );
                    out.push_str(&format!(
                        "      final _RustBuffer resultBuf = {call_expr};\n"
                    ));
                    out.push_str(
                        "      final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n",
                    );
                    out.push_str("      final int resultLen = resultBuf.len;\n");
                    out.push_str("      if (resultData == ffi.nullptr) {\n");
                    out.push_str("        _rustBytesFree(resultBuf);\n");
                    out.push_str("        final mapReader = _UniFfiBinaryReader(Uint8List(0));\n");
                    out.push_str(&format!("        return {decode};\n"));
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str("        final Uint8List resultBytes = Uint8List.fromList(resultData.asTypedList(resultLen));\n");
                    out.push_str("        final mapReader = _UniFfiBinaryReader(resultBytes);\n");
                    out.push_str(&format!("        return {decode};\n"));
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
