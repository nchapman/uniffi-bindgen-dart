use uniffi_bindgen::interface::{ffi::FfiType, Type};

use super::config::CustomTypeConfig;
use super::*;
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_bound_methods(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    ffi_namespace: &str,
    local_module_path: &str,
    records: &[UdlRecord],
    enums: &[UdlEnum],
    custom_types: &HashMap<String, CustomTypeConfig>,
) -> String {
    let mut out = String::new();
    let mut runtime_functions = functions.to_vec();
    for record in records {
        for method in &record.methods {
            let mut args = vec![UdlArg {
                name: "self".to_string(),
                type_: Type::Record {
                    module_path: local_module_path.to_string(),
                    name: record.name.clone(),
                },
                docstring: None,
                default: None,
            }];
            args.extend(method.args.clone());
            runtime_functions.push(UdlFunction {
                name: format!(
                    "{}_{}",
                    dart_identifier(&record.name),
                    dart_identifier(&method.name)
                ),
                ffi_symbol: method.ffi_symbol.clone(),
                ffi_arg_types: method.ffi_arg_types.clone(),
                ffi_return_type: method.ffi_return_type.clone(),
                ffi_has_rust_call_status: method.ffi_has_rust_call_status,
                runtime_unsupported: method.runtime_unsupported.clone(),
                docstring: method.docstring.clone(),
                is_async: method.is_async,
                return_type: method.return_type.clone(),
                throws_type: method.throws_type.clone(),
                args,
            });
        }
    }
    for enum_ in enums {
        for method in &enum_.methods {
            let mut args = vec![UdlArg {
                name: "self".to_string(),
                type_: Type::Enum {
                    module_path: local_module_path.to_string(),
                    name: enum_.name.clone(),
                },
                docstring: None,
                default: None,
            }];
            args.extend(method.args.clone());
            runtime_functions.push(UdlFunction {
                name: format!(
                    "{}_{}",
                    dart_identifier(&enum_.name),
                    dart_identifier(&method.name)
                ),
                ffi_symbol: method.ffi_symbol.clone(),
                ffi_arg_types: method.ffi_arg_types.clone(),
                ffi_return_type: method.ffi_return_type.clone(),
                ffi_has_rust_call_status: method.ffi_has_rust_call_status,
                runtime_unsupported: method.runtime_unsupported.clone(),
                docstring: method.docstring.clone(),
                is_async: method.is_async,
                return_type: method.return_type.clone(),
                throws_type: method.throws_type.clone(),
                args,
            });
        }
    }
    let has_runtime_ffibuffer_fallback = runtime_functions.iter().any(|f| {
        f.runtime_unsupported.is_some()
            && (is_ffibuffer_eligible_function(f)
                || is_runtime_unsupported_async_ffibuffer_eligible_function(f))
    }) || objects.iter().any(|o| {
        o.constructors
            .iter()
            .any(|c| c.runtime_unsupported.is_some() && is_ffibuffer_eligible_object_constructor(c))
            || o.methods
                .iter()
                .any(|m| m.runtime_unsupported.is_some() && is_ffibuffer_eligible_object_member(m))
    });
    let callback_runtime_interfaces = callback_interfaces_used_for_runtime(
        &runtime_functions,
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
    let needs_string_free =
        needs_async_rust_future
            || functions.iter().any(|f| {
                f.runtime_unsupported.is_none()
                    && is_runtime_ffi_compatible_function(f, records, enums)
                    && (function_returns_runtime_string(f)
                        || f.return_type.as_ref().is_some_and(|t| {
                            is_runtime_utf8_pointer_marshaled_type(t, records, enums)
                        })
                        || is_runtime_throwing_ffi_compatible_function(
                            f,
                            callback_interfaces,
                            records,
                            enums,
                        )
                        || f.return_type
                            .as_ref()
                            .is_some_and(|t| is_runtime_record_or_enum_string_type(t, enums)))
            })
            || objects.iter().any(|o| {
                o.methods.iter().any(|m| {
                    m.runtime_unsupported.is_none()
                        && (m.return_type.as_ref().is_some_and(|t| {
                            is_runtime_utf8_pointer_marshaled_type(t, records, enums)
                        }) || (m.throws_type.is_some()
                            && m.return_type
                                .as_ref()
                                .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
                                .unwrap_or(true)
                            && m.args
                                .iter()
                                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums))))
                })
            })
            || records.iter().any(|r| {
                r.methods.iter().any(|m| {
                    m.runtime_unsupported.is_none()
                        && (m.return_type.as_ref().is_some_and(|t| {
                            is_runtime_utf8_pointer_marshaled_type(t, records, enums)
                        }) || (m.throws_type.is_some()
                            && m.return_type
                                .as_ref()
                                .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
                                .unwrap_or(true)
                            && m.args
                                .iter()
                                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums))))
                })
            })
            || enums.iter().any(|e| {
                e.methods.iter().any(|m| {
                    m.runtime_unsupported.is_none()
                        && (m.return_type.as_ref().is_some_and(|t| {
                            is_runtime_utf8_pointer_marshaled_type(t, records, enums)
                        }) || (m.throws_type.is_some()
                            && m.return_type
                                .as_ref()
                                .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
                                .unwrap_or(true)
                            && m.args
                                .iter()
                                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums))))
                })
            });
    let needs_bytes_free = functions.iter().any(|f| {
        is_runtime_ffi_compatible_function(f, records, enums)
            && (function_returns_runtime_bytes(f)
                || f.return_type
                    .as_ref()
                    .is_some_and(is_runtime_non_string_map_type))
    }) || objects.iter().any(|o| {
        o.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(|t| is_runtime_bytes_like_type(t) || is_runtime_non_string_map_type(t))
        })
    }) || records.iter().any(|r| {
        r.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(|t| is_runtime_bytes_like_type(t) || is_runtime_non_string_map_type(t))
        })
    }) || enums.iter().any(|e| {
        e.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(|t| is_runtime_bytes_like_type(t) || is_runtime_non_string_map_type(t))
        })
    });
    let needs_bytes_vec_free = functions.iter().any(|f| {
        is_runtime_ffi_compatible_function(f, records, enums)
            && f.return_type
                .as_ref()
                .is_some_and(is_runtime_sequence_bytes_type)
    }) || objects.iter().any(|o| {
        o.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_sequence_bytes_type)
        })
    }) || records.iter().any(|r| {
        r.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_sequence_bytes_type)
        })
    }) || enums.iter().any(|e| {
        e.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_sequence_bytes_type)
        })
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
    if has_runtime_ffibuffer_fallback {
        out.push('\n');
        out.push_str(&format!(
            "  late final _UniFfiRustBuffer Function(_UniFfiForeignBytes bytes, ffi.Pointer<_UniFfiRustCallStatus> outStatus) _uniFfiRustBufferFromBytes = _lib.lookupFunction<_UniFfiRustBuffer Function(_UniFfiForeignBytes bytes, ffi.Pointer<_UniFfiRustCallStatus> outStatus), _UniFfiRustBuffer Function(_UniFfiForeignBytes bytes, ffi.Pointer<_UniFfiRustCallStatus> outStatus)>('ffi_{ffi_namespace}_rustbuffer_from_bytes');\n"
        ));
        out.push_str(&format!(
            "  late final void Function(_UniFfiRustBuffer buf, ffi.Pointer<_UniFfiRustCallStatus> outStatus) _uniFfiRustBufferFree = _lib.lookupFunction<ffi.Void Function(_UniFfiRustBuffer buf, ffi.Pointer<_UniFfiRustCallStatus> outStatus), void Function(_UniFfiRustBuffer buf, ffi.Pointer<_UniFfiRustCallStatus> outStatus)>('ffi_{ffi_namespace}_rustbuffer_free');\n"
        ));
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
    for trait_object in objects.iter().filter(|o| o.has_callback_interface) {
        let vtable_name = trait_callback_vtable_struct_name(&trait_object.name);
        let init_field = trait_callback_init_field_name(&trait_object.name);
        let init_done_field = trait_callback_init_done_field_name(&trait_object.name);
        let vtable_field = trait_callback_vtable_field_name(&trait_object.name);
        let bridge_name = trait_callback_bridge_class_name(&trait_object.name);
        let init_symbol = trait_callback_init_symbol(&trait_object.name);
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

    for function in &runtime_functions {
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
                    .map(|t| map_uniffi_type_to_dart(t, custom_types))
                    .unwrap_or_else(|| "void".to_string());
                let signature_return_type = format!("Future<{value_return_type}>");
                let dart_sig = function
                    .args
                    .iter()
                    .map(|a| {
                        format!(
                            "{} {}",
                            map_uniffi_type_to_dart(&a.type_, custom_types),
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

                ffi_buffer::render_ffibuffer_async_ffi_lookups(
                    &mut out,
                    &ffi_buffer::FfiBufferAsyncSymbols {
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
                                &mut out,
                                arg,
                                *offset,
                                &escaped_reason,
                                &function.name,
                                enums,
                                custom_types,
                            );
                        }
                        _ => {
                            render_ffibuffer_primitive_arg_write(
                                &mut out,
                                arg,
                                ffi_type,
                                *offset,
                                &escaped_reason,
                                &function.name,
                                custom_types,
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
                render_ffibuffer_async_poll_loop(&mut out, &poll_field, &function.name);
                render_ffibuffer_async_complete_and_decode(
                    &mut out,
                    &complete_field,
                    &cancel_field,
                    &free_field,
                    &async_spec,
                    function.return_type.as_ref(),
                    function.throws_type.as_ref(),
                    &function.name,
                    local_module_path,
                    objects,
                    enums,
                    custom_types,
                );
                render_ffibuffer_outer_cleanup(&mut out);
                out.push_str("  }\n");
                continue;
            }
            if ffibuffer_eligible {
                let value_return_type = function
                    .return_type
                    .as_ref()
                    .map(|t| map_uniffi_type_to_dart(t, custom_types))
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
                            map_uniffi_type_to_dart(&a.type_, custom_types),
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
                                &mut out,
                                arg,
                                *offset,
                                &escaped_reason,
                                &function.name,
                                enums,
                                custom_types,
                            );
                        }
                        _ => {
                            render_ffibuffer_primitive_arg_write(
                                &mut out,
                                arg,
                                ffi_type,
                                *offset,
                                &escaped_reason,
                                &function.name,
                                custom_types,
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
                                    custom_types,
                                ),
                                Type::Bytes => {
                                    lift_custom_if_needed("retBytes", ret_type, custom_types)
                                }
                                Type::Record { name, .. } | Type::Enum { name, .. } => {
                                    format!("_uniffiDecode{}(retBytes)", to_upper_camel(name))
                                }
                                _ => render_uniffi_binary_read_expression(
                                    ret_type,
                                    "retReader",
                                    enums,
                                    custom_types,
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
                render_ffibuffer_outer_cleanup(&mut out);
                out.push_str("  }\n");
                continue;
            }

            let value_return_type = function
                .return_type
                .as_ref()
                .map(|t| map_uniffi_type_to_dart(t, custom_types))
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
                        map_uniffi_type_to_dart(&a.type_, custom_types),
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

        let is_runtime_supported = is_runtime_ffi_compatible_function(function, records, enums);
        let is_sync_callback_supported =
            is_runtime_callback_compatible_function(function, callback_interfaces, records, enums);
        let has_callback_args =
            has_runtime_callback_args_in_args(&function.args, callback_interfaces, records, enums);
        if !is_runtime_supported && !is_sync_callback_supported && !has_callback_args {
            emit_function_skip_warning(
                &mut out,
                &function.name,
                &function.args,
                custom_types,
                "  ",
            );
            continue;
        }
        let field_name = format!("_{}", method_name);
        let function_symbol = function.ffi_symbol.as_deref().unwrap_or(&function.name);
        if is_sync_callback_supported {
            let return_type = function
                .return_type
                .as_ref()
                .map(|t| map_uniffi_type_to_dart(t, custom_types))
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
                    map_uniffi_type_to_dart(&arg.type_, custom_types),
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
                    custom_types,
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
                    let decode = render_plain_ffi_decode_expr(ret_type, &call, custom_types);
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
            .map(|t| map_uniffi_type_to_dart(t, custom_types))
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
            emit_function_skip_warning(
                &mut out,
                &function.name,
                &function.args,
                custom_types,
                "  ",
            );
            continue;
        };
        let Some(dart_ffi_return) = dart_ffi_return else {
            emit_function_skip_warning(
                &mut out,
                &function.name,
                &function.args,
                custom_types,
                "  ",
            );
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
                map_uniffi_type_to_dart(&arg.type_, custom_types),
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
            append_runtime_arg_marshalling(
                &arg_name,
                &arg.type_,
                enums,
                custom_types,
                &mut pre_call,
                &mut post_call,
                &mut call_args,
            );
        }

        if !signature_compatible {
            emit_function_skip_warning(
                &mut out,
                &function.name,
                &function.args,
                custom_types,
                "  ",
            );
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
                if is_runtime_utf8_pointer_marshaled_type(ret_type, records, enums) {
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
                    let lifted =
                        lift_custom_if_needed("resultPtr.toDartString()", ret_type, custom_types);
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
                    let lifted =
                        lift_custom_if_needed("resultPtr.toDartString()", ret_type, custom_types);
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
                        local_module_path,
                        "this",
                        objects,
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
                        local_module_path,
                        "this",
                        objects,
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
                    let decode = render_json_decode_expr("decoded", ret_type, custom_types);
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
                    let decode = render_json_decode_expr("item", inner_type, custom_types);
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
                        render_json_decode_expr("jsonDecode(payload)", ret_type, custom_types);
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
                        enums,
                        custom_types,
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
                        render_plain_ffi_decode_expr(ret_type, "resultValue", custom_types);
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
                .map(|t| render_json_decode_expr("okRaw", t, custom_types));
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
                        lift_custom_if_needed("resultPtr.toDartString()", type_, custom_types);
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
                        lift_custom_if_needed("resultPtr.toDartString()", type_, custom_types);
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
                        local_module_path,
                        "this",
                        objects,
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
                        local_module_path,
                        "this",
                        objects,
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
                    let decode = render_json_decode_expr("decoded", type_, custom_types);
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
                    let decode = render_json_decode_expr("item", inner_type, custom_types);
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
                        render_json_decode_expr("jsonDecode(payload)", type_, custom_types);
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
                        enums,
                        custom_types,
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

    for object in objects {
        let object_name = to_upper_camel(&object.name);
        // For [Trait, WithForeign] objects, use the _Impl class for handle-based construction
        let object_construct_name = if object.has_callback_interface {
            format!("_{}Impl", object_name)
        } else {
            object_name.clone()
        };
        let object_lower = safe_dart_identifier(&to_lower_camel(&object.name));
        let object_symbol = dart_identifier(&object.name);
        // In library mode, ffi_free_symbol is always populated by the parser
        // (from `obj.ffi_object_free().name()`). The fallback to `{name}_free`
        // is only used in UDL mode where ffi_free_symbol is None.
        let free_symbol = object
            .ffi_free_symbol
            .clone()
            .unwrap_or_else(|| format!("{object_symbol}_free"));
        let free_field = format!("_{}Free", object_lower);
        out.push('\n');
        if object.ffi_free_symbol.is_some() {
            // Library mode: the Rust free function expects (handle, &mut RustCallStatus).
            // We look up the raw 2-param function and wrap it in a closure that
            // allocates/frees the RustCallStatus, keeping the `void Function(int)`
            // signature expected by FinalizerToken and close().
            let raw_free_field = format!("_{}FreeRaw", object_lower);
            out.push_str(&format!(
                "  late final void Function(int handle, ffi.Pointer<_UniFfiRustCallStatus> outStatus) {raw_free_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle, ffi.Pointer<_UniFfiRustCallStatus> outStatus), void Function(int handle, ffi.Pointer<_UniFfiRustCallStatus> outStatus)>('{free_symbol}');\n\
                 \x20 late final void Function(int handle) {free_field} = (int handle) {{\n\
                 \x20   final statusPtr = calloc<_UniFfiRustCallStatus>();\n\
                 \x20   statusPtr.ref.code = _uniFfiRustCallStatusSuccess;\n\
                 \x20   statusPtr.ref.errorBuf\n\
                 \x20     ..capacity = 0\n\
                 \x20     ..len = 0\n\
                 \x20     ..data = ffi.nullptr;\n\
                 \x20   {raw_free_field}(handle, statusPtr);\n\
                 \x20   calloc.free(statusPtr);\n\
                 \x20 }};\n"
            ));
        } else {
            // UDL mode: the scaffolding free function takes only (handle).
            out.push_str(&format!(
                "  late final void Function(int handle) {free_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle), void Function(int handle)>('{free_symbol}');\n"
            ));
        }

        // Library mode: generate a clone function that increments the handle's
        // Arc refcount.  Every method call must clone the handle before passing
        // it to the Rust scaffolding, because `try_lift` consumes the handle
        // via `Arc::from_raw()`.  Without cloning, the first method call would
        // free the underlying object, and subsequent calls would be
        // use-after-free.  This matches what Swift and Kotlin do (they call
        // `uniffiCloneHandle()` before every method invocation).
        let clone_field = if let Some(clone_symbol) = &object.ffi_clone_symbol {
            let field = format!("_{}Clone", object_lower);
            out.push('\n');
            out.push_str(&format!(
                "  late final int Function(int handle, ffi.Pointer<_UniFfiRustCallStatus> outStatus) {field} = _lib.lookupFunction<ffi.Uint64 Function(ffi.Uint64 handle, ffi.Pointer<_UniFfiRustCallStatus> outStatus), int Function(int handle, ffi.Pointer<_UniFfiRustCallStatus> outStatus)>('{clone_symbol}');\n"
            ));
            Some(field)
        } else {
            None
        };

        for ctor in &object.constructors {
            if let Some(reason) = ctor.runtime_unsupported.as_ref() {
                let ctor_camel = to_upper_camel(&ctor.name);
                let ctor_method = format!("{}Create{}", object_lower, ctor_camel);
                let dart_args = ctor
                    .args
                    .iter()
                    .map(|arg| {
                        let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                        format!(
                            "{} {arg_name}",
                            map_uniffi_type_to_dart(&arg.type_, custom_types)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let escaped_reason = reason.replace('\'', "\\'");
                let ffibuffer_eligible = is_ffibuffer_eligible_object_constructor(ctor);
                if ffibuffer_eligible {
                    let ctor_field = format!("_{}Ctor{}FfiBuffer", object_lower, ctor_camel);
                    let ctor_symbol = ctor.ffi_symbol.as_deref().unwrap_or(&ctor.name).to_string();
                    let ffibuffer_symbol = ffibuffer_symbol_name(&ctor_symbol);
                    let ffi_return_type = ctor.ffi_return_type.clone().or(Some(FfiType::Handle));
                    let Some(ffi_return_type) = ffi_return_type else {
                        out.push('\n');
                        out.push_str(&format!("  {object_name} {ctor_method}({dart_args}) {{\n"));
                        out.push_str(&format!(
                            "    throw UnsupportedError('{escaped_reason} ({})');\n",
                            ctor.name
                        ));
                        out.push_str("  }\n");
                        continue;
                    };
                    let Some(return_ffi_elements) = ffibuffer_element_count(&ffi_return_type)
                    else {
                        out.push('\n');
                        out.push_str(&format!("  {object_name} {ctor_method}({dart_args}) {{\n"));
                        out.push_str(&format!(
                            "    throw UnsupportedError('{escaped_reason} ({})');\n",
                            ctor.name
                        ));
                        out.push_str("  }\n");
                        continue;
                    };
                    let ffi_arg_types = if ctor.ffi_arg_types.len() == ctor.args.len() {
                        ctor.ffi_arg_types.clone()
                    } else {
                        ctor.args
                            .iter()
                            .filter_map(|a| ffibuffer_ffi_type_from_uniffi_type(&a.type_))
                            .collect::<Vec<_>>()
                    };
                    let mut arg_ffi_offsets = Vec::new();
                    let mut arg_cursor = 0usize;
                    let mut signature_compatible = ffi_arg_types.len() == ctor.args.len();
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
                        out.push('\n');
                        out.push_str(&format!("  {object_name} {ctor_method}({dart_args}) {{\n"));
                        out.push_str(&format!(
                            "    throw UnsupportedError('{escaped_reason} ({})');\n",
                            ctor.name
                        ));
                        out.push_str("  }\n");
                        continue;
                    }

                    out.push('\n');
                    out.push_str(&format!(
                        "  late final void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr) {ctor_field} = _lib.lookupFunction<ffi.Void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr), void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr)>('{ffibuffer_symbol}');\n"
                    ));
                    out.push('\n');
                    out.push_str(&format!("  {object_name} {ctor_method}({dart_args}) {{\n"));
                    out.push_str(&format!(
                        "    final ffi.Pointer<_UniFfiFfiBufferElement> argBuf = calloc<_UniFfiFfiBufferElement>({arg_cursor});\n"
                    ));
                    out.push_str(&format!(
                        "    final ffi.Pointer<_UniFfiFfiBufferElement> returnBuf = calloc<_UniFfiFfiBufferElement>({});\n",
                        return_ffi_elements + 4
                    ));
                    out.push_str("    final foreignArgPtrs = <ffi.Pointer<ffi.Uint8>>[];\n");
                    out.push_str(
                        "    final rustRetBufferPtrs = <ffi.Pointer<_UniFfiRustBuffer>>[];\n",
                    );
                    out.push_str("    try {\n");
                    for ((arg, ffi_type), offset) in ctor
                        .args
                        .iter()
                        .zip(ffi_arg_types.iter())
                        .zip(arg_ffi_offsets.iter())
                    {
                        match ffi_type {
                            FfiType::RustBuffer(_) => {
                                ffi_buffer::render_ffibuffer_rustbuffer_arg_serialization(
                                    &mut out,
                                    arg,
                                    *offset,
                                    &escaped_reason,
                                    &ctor.name,
                                    enums,
                                    custom_types,
                                );
                            }
                            _ => {
                                ffi_buffer::render_ffibuffer_primitive_arg_write(
                                    &mut out,
                                    arg,
                                    ffi_type,
                                    *offset,
                                    &escaped_reason,
                                    &ctor.name,
                                    custom_types,
                                );
                            }
                        }
                    }
                    out.push_str(&format!("      {ctor_field}(argBuf, returnBuf);\n"));
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
                    if let Some(throws_type) = ctor.throws_type.as_ref() {
                        if let Some(throws_name) =
                            throws_name_from_type(throws_type).map(to_upper_camel)
                        {
                            out.push_str(
                                "        if (statusCode == _uniFfiRustCallStatusError) {\n",
                            );
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
                    match ffi_return_type {
                        FfiType::Handle | FfiType::UInt64 | FfiType::Int64 => {
                            out.push_str("      final int handle = (returnBuf + 0).ref.u64;\n");
                            out.push_str(&format!(
                                "      return {object_construct_name}._(this, handle);\n"
                            ));
                        }
                        _ => {
                            out.push_str(&format!(
                                "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                ctor.name
                            ));
                        }
                    }
                    ffi_buffer::render_ffibuffer_outer_cleanup(&mut out);
                    out.push_str("  }\n");
                    continue;
                }
                out.push('\n');
                out.push_str(&format!("  {object_name} {ctor_method}({dart_args}) {{\n"));
                out.push_str(&format!(
                    "    throw UnsupportedError('{escaped_reason} ({})');\n",
                    ctor.name
                ));
                out.push_str("  }\n");
                continue;
            }
            if !ctor
                .args
                .iter()
                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums))
            {
                emit_constructor_skip_warning(
                    &mut out,
                    &object_name,
                    &ctor.name,
                    &ctor.args,
                    custom_types,
                    "  ",
                );
                continue;
            }
            let ctor_camel = to_upper_camel(&ctor.name);
            let ctor_field = format!("_{}Ctor{}", object_lower, ctor_camel);
            let ctor_method = format!("{}Create{}", object_lower, ctor_camel);
            let ctor_symbol = ctor
                .ffi_symbol
                .clone()
                .unwrap_or_else(|| format!("{}_{}", object_symbol, dart_identifier(&ctor.name)));
            let is_throwing = ctor.throws_type.is_some();
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
                    map_uniffi_type_to_dart(&arg.type_, custom_types)
                ));
                append_runtime_arg_marshalling(
                    &arg_name,
                    &arg.type_,
                    enums,
                    custom_types,
                    &mut pre_call,
                    &mut post_call,
                    &mut call_args,
                );
            }
            if !signature_compatible {
                emit_constructor_skip_warning(
                    &mut out,
                    &object_name,
                    &ctor.name,
                    &ctor.args,
                    custom_types,
                    "  ",
                );
                continue;
            }

            // Async constructor with real Rust-future poll/complete/free lifecycle
            if is_runtime_async_rust_future_compatible_constructor(
                ctor,
                callback_interfaces,
                records,
                enums,
            ) {
                let start_native_sig = format!("ffi.Uint64 Function({})", native_args.join(", "));
                let start_dart_sig = format!("int Function({})", dart_ffi_args.join(", "));
                let poll_field = format!("{ctor_field}RustFuturePoll");
                let cancel_field = format!("{ctor_field}RustFutureCancel");
                let complete_field = format!("{ctor_field}RustFutureComplete");
                let free_field = format!("{ctor_field}RustFutureFree");
                // Constructors always return a u64 handle
                let complete_symbol = "rust_future_complete_u64";
                let poll_symbol = "rust_future_poll_u64";
                let cancel_symbol = "rust_future_cancel_u64";
                let free_symbol = "rust_future_free_u64";
                let complete_native_sig =
                    "ffi.Uint64 Function(ffi.Uint64 handle, ffi.Pointer<_RustCallStatus> outStatus)";
                let complete_dart_sig =
                    "int Function(int handle, ffi.Pointer<_RustCallStatus> outStatus)";

                out.push('\n');
                out.push_str(&format!(
                    "  late final {start_dart_sig} {ctor_field} = _lib.lookupFunction<{start_native_sig}, {start_dart_sig}>('{ctor_symbol}');\n",
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
                    "  Future<{object_name}> {ctor_method}({}) async {{\n",
                    dart_args.join(", ")
                ));
                for line in &pre_call {
                    out.push_str(line);
                }
                out.push_str("    final int futureHandle;\n");
                if !post_call.is_empty() {
                    out.push_str("    try {\n");
                    out.push_str(&format!(
                        "      futureHandle = {ctor_field}({});\n",
                        call_args.join(", ")
                    ));
                    out.push_str("    } finally {\n");
                    for line in &post_call {
                        out.push_str(line);
                    }
                    out.push_str("    }\n");
                } else {
                    out.push_str(&format!(
                        "    futureHandle = {ctor_field}({});\n",
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
                    ctor_symbol
                ));
                out.push_str("      }\n");
                out.push_str(
                    "      final ffi.Pointer<_RustCallStatus> outStatusPtr = calloc<_RustCallStatus>();\n",
                );
                out.push_str("      try {\n");
                out.push_str(&format!(
                    "        final int resultValue = {complete_field}(futureHandle, outStatusPtr);\n"
                ));
                out.push_str("        final int statusCode = outStatusPtr.ref.code;\n");
                out.push_str("        if (statusCode == _rustCallStatusSuccess) {\n");
                out.push_str(&format!(
                    "          return {object_construct_name}._(this, resultValue);\n"
                ));
                out.push_str("        }\n");
                if let Some(throws_type) = ctor.throws_type.as_ref() {
                    if let Some(throws_name) =
                        throws_name_from_type(throws_type).map(to_upper_camel)
                    {
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
                            ctor_symbol
                        ));
                        out.push_str("        }\n");
                    }
                }
                out.push_str("        if (statusCode == _rustCallStatusCancelled) {\n");
                out.push_str(&format!(
                    "          throw StateError('Rust future was cancelled for {}');\n",
                    ctor_symbol
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
                    ctor_symbol
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
                let Some(throws_type) = ctor.throws_type.as_ref() else {
                    continue;
                };
                let Some(_throws_name) = throws_name_from_type(throws_type) else {
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
                out.push_str(&render_throws_expr(throws_type, "errRaw", "      "));
                out.push_str("    }\n");
                out.push_str("    final Object? okRaw = envelope['ok'];\n");
                out.push_str("    final int handle = (okRaw as num).toInt();\n");
                out.push_str(&format!(
                    "    return {object_construct_name}._(this, handle);\n"
                ));
            } else {
                out.push_str(&format!(
                    "    final int handle = {ctor_field}({});\n",
                    call_args.join(", ")
                ));
                out.push_str(&format!(
                    "    return {object_construct_name}._(this, handle);\n"
                ));
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
            if let Some(reason) = method.runtime_unsupported.as_ref() {
                let method_invoke =
                    format!("{}Invoke{}", object_lower, to_upper_camel(&method.name));
                let value_return_type = method
                    .return_type
                    .as_ref()
                    .map(|t| map_uniffi_type_to_dart(t, custom_types))
                    .unwrap_or_else(|| "void".to_string());
                let signature_return_type = if method.is_async {
                    format!("Future<{value_return_type}>")
                } else {
                    value_return_type
                };
                let mut dart_args = vec!["int handle".to_string()];
                dart_args.extend(method.args.iter().map(|arg| {
                    let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                    format!(
                        "{} {arg_name}",
                        map_uniffi_type_to_dart(&arg.type_, custom_types)
                    )
                }));
                let escaped_reason = reason.replace('\'', "\\'");
                let ffibuffer_eligible = is_ffibuffer_eligible_object_member(method);
                if ffibuffer_eligible {
                    let method_camel = to_upper_camel(&method.name);
                    let method_field = format!("_{}{}FfiBuffer", object_lower, method_camel);
                    let method_symbol = method
                        .ffi_symbol
                        .as_deref()
                        .unwrap_or(&method.name)
                        .to_string();
                    let ffibuffer_symbol = ffibuffer_symbol_name(&method_symbol);
                    let ffi_return_type = method.ffi_return_type.clone().or_else(|| {
                        method
                            .return_type
                            .as_ref()
                            .and_then(ffibuffer_ffi_type_from_uniffi_type)
                    });
                    // For void-returning methods, return_ffi_elements is 0 (no return
                    // value slots); the return buffer only holds the RustCallStatus
                    // fields (4 elements).
                    let return_ffi_elements = match &ffi_return_type {
                        Some(fft) => {
                            let Some(count) = ffibuffer_element_count(fft) else {
                                out.push('\n');
                                out.push_str(&format!(
                                    "  {signature_return_type} {method_invoke}({}) {{\n",
                                    dart_args.join(", ")
                                ));
                                out.push_str(&format!(
                                    "    throw UnsupportedError('{escaped_reason} ({})');\n",
                                    method.name
                                ));
                                out.push_str("  }\n");
                                continue;
                            };
                            count
                        }
                        None => 0,
                    };
                    let ffi_arg_types = if method.ffi_arg_types.len() == method.args.len() + 1 {
                        method.ffi_arg_types.clone()
                    } else {
                        let mut inferred = vec![FfiType::Handle];
                        inferred.extend(
                            method
                                .args
                                .iter()
                                .filter_map(|a| ffibuffer_ffi_type_from_uniffi_type(&a.type_)),
                        );
                        inferred
                    };
                    let mut arg_ffi_offsets = Vec::new();
                    let mut arg_cursor = 0usize;
                    let mut signature_compatible = ffi_arg_types.len() == method.args.len() + 1;
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
                        out.push('\n');
                        out.push_str(&format!(
                            "  {signature_return_type} {method_invoke}({}) {{\n",
                            dart_args.join(", ")
                        ));
                        out.push_str(&format!(
                            "    throw UnsupportedError('{escaped_reason} ({})');\n",
                            method.name
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
                        "  {signature_return_type} {method_invoke}({}) {{\n",
                        dart_args.join(", ")
                    ));
                    out.push_str(&format!(
                        "    final ffi.Pointer<_UniFfiFfiBufferElement> argBuf = calloc<_UniFfiFfiBufferElement>({arg_cursor});\n"
                    ));
                    out.push_str(&format!(
                        "    final ffi.Pointer<_UniFfiFfiBufferElement> returnBuf = calloc<_UniFfiFfiBufferElement>({});\n",
                        return_ffi_elements + 4
                    ));
                    out.push_str("    final foreignArgPtrs = <ffi.Pointer<ffi.Uint8>>[];\n");
                    out.push_str(
                        "    final rustRetBufferPtrs = <ffi.Pointer<_UniFfiRustBuffer>>[];\n",
                    );
                    out.push_str("    try {\n");

                    // Clone the handle before passing it to the method.
                    // UniFFI's `try_lift` consumes the handle via
                    // `Arc::from_raw()`, so without cloning the object
                    // would be freed after the first method call.
                    if let Some(clone_fn) = &clone_field {
                        out.push_str("      final int clonedHandle;\n");
                        out.push_str("      {\n");
                        out.push_str(
                            "        final cloneStatusPtr = calloc<_UniFfiRustCallStatus>();\n",
                        );
                        out.push_str("        try {\n");
                        out.push_str(
                            "          cloneStatusPtr.ref.code = _uniFfiRustCallStatusSuccess;\n",
                        );
                        out.push_str("          cloneStatusPtr.ref.errorBuf\n");
                        out.push_str("            ..capacity = 0\n");
                        out.push_str("            ..len = 0\n");
                        out.push_str("            ..data = ffi.nullptr;\n");
                        out.push_str(&format!(
                            "          clonedHandle = {clone_fn}(handle, cloneStatusPtr);\n"
                        ));
                        out.push_str("          if (cloneStatusPtr.ref.code != _uniFfiRustCallStatusSuccess) {\n");
                        out.push_str("            throw StateError('UniFFI clone failed with status ${cloneStatusPtr.ref.code}');\n");
                        out.push_str("          }\n");
                        out.push_str("        } finally {\n");
                        out.push_str("          calloc.free(cloneStatusPtr);\n");
                        out.push_str("        }\n");
                        out.push_str("      }\n");
                    }

                    if let Some(handle_ffi_type) = ffi_arg_types.first() {
                        if let Some(handle_field) = ffibuffer_primitive_union_field(handle_ffi_type)
                        {
                            let handle_expr = if clone_field.is_some() {
                                "clonedHandle"
                            } else {
                                "handle"
                            };
                            if handle_field == "ptr" {
                                out.push_str(&format!(
                                    "      (argBuf + {}).ref.ptr = {handle_expr}.cast<ffi.Void>();\n",
                                    arg_ffi_offsets[0]
                                ));
                            } else {
                                out.push_str(&format!(
                                    "      (argBuf + {}).ref.{handle_field} = {handle_expr};\n",
                                    arg_ffi_offsets[0]
                                ));
                            }
                        } else {
                            out.push_str(&format!(
                                "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                method.name
                            ));
                        }
                    }

                    for ((arg, ffi_type), offset) in method
                        .args
                        .iter()
                        .zip(ffi_arg_types.iter().skip(1))
                        .zip(arg_ffi_offsets.iter().skip(1))
                    {
                        match ffi_type {
                            FfiType::RustBuffer(_) => {
                                render_ffibuffer_rustbuffer_arg_serialization(
                                    &mut out,
                                    arg,
                                    *offset,
                                    &escaped_reason,
                                    &method.name,
                                    enums,
                                    custom_types,
                                );
                            }
                            _ => {
                                render_ffibuffer_primitive_arg_write(
                                    &mut out,
                                    arg,
                                    ffi_type,
                                    *offset,
                                    &escaped_reason,
                                    &method.name,
                                    custom_types,
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
                    if let Some(throws_type) = method.throws_type.as_ref() {
                        if let Some(throws_name) =
                            throws_name_from_type(throws_type).map(to_upper_camel)
                        {
                            out.push_str(
                                "        if (statusCode == _uniFfiRustCallStatusError) {\n",
                            );
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

                    match method.return_type.as_ref() {
                        None => out.push_str("      return;\n"),
                        Some(Type::Boolean) => {
                            out.push_str("      return (returnBuf + 0).ref.i8 == 1;\n");
                        }
                        Some(ret_type) if is_runtime_object_type(ret_type) => {
                            let lift = render_object_lift_expr_with_objects(
                                ret_type,
                                "(returnBuf + 0).ref.u64",
                                local_module_path,
                                "this",
                                objects,
                            );
                            out.push_str(&format!("      return {lift};\n"));
                        }
                        Some(ret_type) if is_runtime_optional_object_type(ret_type) => {
                            let inner = match runtime_unwrapped_type(ret_type) {
                                Type::Optional { inner_type } => inner_type,
                                other => {
                                    unreachable!("expected Optional or Sequence, got {other:?}")
                                }
                            };
                            out.push_str(
                                "      final int resultHandle = (returnBuf + 0).ref.u64;\n",
                            );
                            out.push_str("      if (resultHandle == 0) {\n");
                            out.push_str("        return null;\n");
                            out.push_str("      }\n");
                            let lift = render_object_lift_expr_with_objects(
                                inner,
                                "resultHandle",
                                local_module_path,
                                "this",
                                objects,
                            );
                            out.push_str(&format!("      return {lift};\n"));
                        }
                        Some(ret_type) if is_runtime_optional_primitive_type(ret_type) => {
                            // Optional primitives are JSON-encoded as Pointer<Utf8>.
                            let decode = render_json_decode_expr("decoded", ret_type, custom_types);
                            out.push_str("      final ffi.Pointer<Utf8> resultPtr = (returnBuf + 0).ref.pointer.cast<Utf8>();\n");
                            out.push_str("      if (resultPtr == ffi.nullptr) {\n");
                            out.push_str(&format!(
                                "        throw StateError('Rust returned null pointer for {}');\n",
                                method.name
                            ));
                            out.push_str("      }\n");
                            out.push_str("      try {\n");
                            out.push_str(
                                "        final String payload = resultPtr.toDartString();\n",
                            );
                            out.push_str("        final Object? decoded = jsonDecode(payload);\n");
                            out.push_str(&format!("        return {decode};\n"));
                            out.push_str("      } finally {\n");
                            out.push_str("        _rustStringFree(resultPtr);\n");
                            out.push_str("      }\n");
                        }
                        Some(ret_type) => match ffi_return_type.as_ref() {
                            Some(FfiType::RustBuffer(_)) => {
                                let is_map_type =
                                    matches!(runtime_unwrapped_type(ret_type), Type::Map { .. });
                                let decode_expr = match runtime_unwrapped_type(ret_type) {
                                    Type::Record { name, .. } | Type::Enum { name, .. } => {
                                        format!("_uniffiDecode{}(retBytes)", to_upper_camel(name))
                                    }
                                    Type::Map { .. } => render_uniffi_binary_read_expression(
                                        ret_type,
                                        "retReader",
                                        enums,
                                        custom_types,
                                    ),
                                    _ => {
                                        out.push_str(&format!(
                                            "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                            method.name
                                        ));
                                        String::new()
                                    }
                                };
                                if !decode_expr.is_empty() {
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
                                    if is_map_type {
                                        out.push_str("      final _UniFfiBinaryReader retReader = _UniFfiBinaryReader(retBytes);\n");
                                    }
                                    out.push_str(&format!("      return {decode_expr};\n"));
                                }
                            }
                            _ => {
                                let Some(union_field) = ffi_return_type
                                    .as_ref()
                                    .and_then(ffibuffer_primitive_union_field)
                                else {
                                    out.push_str(&format!(
                                        "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                        method.name
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

                    render_ffibuffer_outer_cleanup(&mut out);
                    out.push_str("  }\n");
                    continue;
                }
                // Check if this is an async method eligible for the rust-future
                // polling path (same mechanism used for async top-level functions).
                // Throwing async methods are not yet supported on this path;
                // they fall through to the UnsupportedError stub below.
                let async_method_eligible =
                    ffi_buffer::is_runtime_unsupported_async_ffibuffer_eligible_method(method);
                if async_method_eligible {
                    let method_camel = to_upper_camel(&method.name);
                    let method_field = format!("_{}{}FfiBuffer", object_lower, method_camel);
                    let method_symbol = method
                        .ffi_symbol
                        .as_deref()
                        .unwrap_or(&method.name)
                        .to_string();
                    let ffibuffer_symbol = ffibuffer_symbol_name(&method_symbol);
                    // The ffi_buffer start call for async methods returns a
                    // future handle (u64).  The RustCallStatus is embedded in
                    // the return buffer (4 elements), and the future handle
                    // occupies 1 element, so returnBuf has 5 elements total.
                    let ffi_start_return_type = FfiType::UInt64;
                    let Some(return_ffi_elements) = ffibuffer_element_count(&ffi_start_return_type)
                    else {
                        out.push('\n');
                        out.push_str(&format!(
                            "  {signature_return_type} {method_invoke}({}) async {{\n",
                            dart_args.join(", ")
                        ));
                        out.push_str(&format!(
                            "    throw UnsupportedError('{escaped_reason} ({})');\n",
                            method.name
                        ));
                        out.push_str("  }\n");
                        continue;
                    };
                    let Some(async_spec) =
                        async_rust_future_spec_from_uniffi_return_type(method.return_type.as_ref())
                    else {
                        out.push('\n');
                        out.push_str(&format!(
                            "  {signature_return_type} {method_invoke}({}) async {{\n",
                            dart_args.join(", ")
                        ));
                        out.push_str(&format!(
                            "    throw UnsupportedError('unsupported async return type ({})');\n",
                            method.name
                        ));
                        out.push_str("  }\n");
                        continue;
                    };
                    // Compute argument FFI offsets (handle is first arg).
                    let ffi_arg_types = if method.ffi_arg_types.len() == method.args.len() + 1 {
                        method.ffi_arg_types.clone()
                    } else {
                        let mut inferred = vec![FfiType::Handle];
                        inferred.extend(
                            method
                                .args
                                .iter()
                                .filter_map(|a| ffibuffer_ffi_type_from_uniffi_type(&a.type_)),
                        );
                        inferred
                    };
                    let mut arg_ffi_offsets = Vec::new();
                    let mut arg_cursor = 0usize;
                    let mut signature_compatible = ffi_arg_types.len() == method.args.len() + 1;
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
                        out.push('\n');
                        out.push_str(&format!(
                            "  {signature_return_type} {method_invoke}({}) async {{\n",
                            dart_args.join(", ")
                        ));
                        out.push_str(&format!(
                            "    throw UnsupportedError('{escaped_reason} ({})');\n",
                            method.name
                        ));
                        out.push_str("  }\n");
                        continue;
                    }
                    let start_return_union_field =
                        ffibuffer_primitive_union_field(&ffi_start_return_type).unwrap_or("u64");

                    let poll_field = format!("{method_field}RustFuturePoll");
                    let cancel_field = format!("{method_field}RustFutureCancel");
                    let complete_field = format!("{method_field}RustFutureComplete");
                    let free_field_async = format!("{method_field}RustFutureFree");
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
                    let free_symbol_async =
                        format!("ffi_{ffi_namespace}_rust_future_free_{}", async_spec.suffix);
                    let complete_native_sig = format!(
                        "{} Function(ffi.Uint64 handle, ffi.Pointer<_UniFfiRustCallStatus> outStatus)",
                        async_spec.complete_native_type
                    );
                    let complete_dart_sig = format!(
                        "{} Function(int handle, ffi.Pointer<_UniFfiRustCallStatus> outStatus)",
                        async_spec.complete_dart_type
                    );

                    // Emit function lookups.
                    ffi_buffer::render_ffibuffer_async_ffi_lookups(
                        &mut out,
                        &ffi_buffer::FfiBufferAsyncSymbols {
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
                            free_field: &free_field_async,
                            free_symbol: &free_symbol_async,
                        },
                    );

                    // Emit the async method body.
                    out.push('\n');
                    out.push_str(&format!(
                        "  {signature_return_type} {method_invoke}({}) async {{\n",
                        dart_args.join(", ")
                    ));
                    out.push_str(&format!(
                        "    final ffi.Pointer<_UniFfiFfiBufferElement> argBuf = calloc<_UniFfiFfiBufferElement>({arg_cursor});\n"
                    ));
                    out.push_str(&format!(
                        "    final ffi.Pointer<_UniFfiFfiBufferElement> returnBuf = calloc<_UniFfiFfiBufferElement>({});\n",
                        return_ffi_elements + 4
                    ));
                    out.push_str("    final foreignArgPtrs = <ffi.Pointer<ffi.Uint8>>[];\n");
                    out.push_str(
                        "    final rustRetBufferPtrs = <ffi.Pointer<_UniFfiRustBuffer>>[];\n",
                    );
                    out.push_str("    try {\n");

                    // Clone the handle before passing it.
                    if let Some(clone_fn) = &clone_field {
                        out.push_str("      final int clonedHandle;\n");
                        out.push_str("      {\n");
                        out.push_str(
                            "        final cloneStatusPtr = calloc<_UniFfiRustCallStatus>();\n",
                        );
                        out.push_str("        try {\n");
                        out.push_str(
                            "          cloneStatusPtr.ref.code = _uniFfiRustCallStatusSuccess;\n",
                        );
                        out.push_str("          cloneStatusPtr.ref.errorBuf\n");
                        out.push_str("            ..capacity = 0\n");
                        out.push_str("            ..len = 0\n");
                        out.push_str("            ..data = ffi.nullptr;\n");
                        out.push_str(&format!(
                            "          clonedHandle = {clone_fn}(handle, cloneStatusPtr);\n"
                        ));
                        out.push_str("          if (cloneStatusPtr.ref.code != _uniFfiRustCallStatusSuccess) {\n");
                        out.push_str("            throw StateError('UniFFI clone failed with status ${cloneStatusPtr.ref.code}');\n");
                        out.push_str("          }\n");
                        out.push_str("        } finally {\n");
                        out.push_str("          calloc.free(cloneStatusPtr);\n");
                        out.push_str("        }\n");
                        out.push_str("      }\n");
                    }

                    // Write handle into arg buffer.
                    let handle_expr = if clone_field.is_some() {
                        "clonedHandle"
                    } else {
                        "handle"
                    };
                    out.push_str(&format!(
                        "      (argBuf + {}).ref.u64 = {handle_expr};\n",
                        arg_ffi_offsets[0]
                    ));

                    // Write remaining args.
                    for ((arg, ffi_type), offset) in method
                        .args
                        .iter()
                        .zip(ffi_arg_types.iter().skip(1))
                        .zip(arg_ffi_offsets.iter().skip(1))
                    {
                        match ffi_type {
                            FfiType::RustBuffer(_) => {
                                render_ffibuffer_rustbuffer_arg_serialization(
                                    &mut out,
                                    arg,
                                    *offset,
                                    &escaped_reason,
                                    &method.name,
                                    enums,
                                    custom_types,
                                );
                            }
                            _ => {
                                render_ffibuffer_primitive_arg_write(
                                    &mut out,
                                    arg,
                                    ffi_type,
                                    *offset,
                                    &escaped_reason,
                                    &method.name,
                                    custom_types,
                                );
                            }
                        }
                    }

                    // Call the ffi_buffer start function.
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
                    out.push_str(&format!(
                        "      final int futureHandle = (returnBuf + 0).ref.{start_return_union_field};\n"
                    ));

                    // Poll loop.
                    render_ffibuffer_async_poll_loop(&mut out, &poll_field, &method.name);

                    // Complete + decode + cancel/free.
                    render_ffibuffer_async_complete_and_decode(
                        &mut out,
                        &complete_field,
                        &cancel_field,
                        &free_field_async,
                        &async_spec,
                        method.return_type.as_ref(),
                        method.throws_type.as_ref(),
                        &method.name,
                        local_module_path,
                        objects,
                        enums,
                        custom_types,
                    );

                    // Cleanup.
                    render_ffibuffer_outer_cleanup(&mut out);
                    out.push_str("  }\n");
                    continue;
                }
                out.push('\n');
                if method.is_async {
                    out.push_str(&format!(
                        "  {signature_return_type} {method_invoke}({}) async {{\n",
                        dart_args.join(", ")
                    ));
                } else {
                    out.push_str(&format!(
                        "  {signature_return_type} {method_invoke}({}) {{\n",
                        dart_args.join(", ")
                    ));
                }
                out.push_str(&format!(
                    "    throw UnsupportedError('{escaped_reason} ({})');\n",
                    method.name
                ));
                out.push_str("  }\n");
                continue;
            }
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
                emit_method_skip_warning(
                    &mut out,
                    &object_name,
                    &method.name,
                    &method.args,
                    custom_types,
                    "  ",
                );
                continue;
            }
            let method_camel = to_upper_camel(&method.name);
            let method_field = format!("_{}{}", object_lower, method_camel);
            let method_invoke = format!("{}Invoke{}", object_lower, method_camel);
            let method_symbol = method
                .ffi_symbol
                .clone()
                .unwrap_or_else(|| format!("{}_{}", object_symbol, dart_identifier(&method.name)));
            let is_throwing = method.throws_type.is_some();

            // Note: this direct-call path only runs when runtime_unsupported is
            // None, which currently only occurs in UDL mode (where
            // ffi_clone_symbol is None).  If library-mode methods ever reach
            // this path, clone_field must be injected into call_args here.
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
                    map_uniffi_type_to_dart(&arg.type_, custom_types)
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
                        custom_types,
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
                        custom_types,
                        &mut pre_call,
                        &mut post_call,
                        &mut call_args,
                    );
                }
            }
            if !signature_compatible {
                emit_method_skip_warning(
                    &mut out,
                    &object_name,
                    &method.name,
                    &method.args,
                    custom_types,
                    "  ",
                );
                continue;
            }
            let return_type = method
                .return_type
                .as_ref()
                .map(|t| map_uniffi_type_to_dart(t, custom_types))
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
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(|t| is_runtime_utf8_pointer_marshaled_type(t, records, enums))
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
                out.push_str("        final int statusCode = outStatusPtr.ref.code;\n");
                out.push_str("        if (statusCode == _rustCallStatusSuccess) {\n");
                if method.return_type.is_none() {
                    out.push_str("          return;\n");
                } else if let Some(ret_type) = method
                    .return_type
                    .as_ref()
                    .filter(|t| is_runtime_string_type(t))
                {
                    let lifted =
                        lift_custom_if_needed("resultPtr.toDartString()", ret_type, custom_types);
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str(&format!("            return {lifted};\n"));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if let Some(ret_type) = method
                    .return_type
                    .as_ref()
                    .filter(|t| is_runtime_optional_string_type(t))
                {
                    let lifted =
                        lift_custom_if_needed("resultPtr.toDartString()", ret_type, custom_types);
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("            return null;\n");
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str(&format!("            return {lifted};\n"));
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
                        "            return {}FfiCodec.decode(payload);\n",
                        to_upper_camel(enum_name)
                    ));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_sequence_json_type)
                {
                    let inner_type = method
                        .return_type
                        .as_ref()
                        .and_then(|t| match runtime_unwrapped_type(t) {
                            Type::Sequence { inner_type } => Some(inner_type.as_ref()),
                            _ => None,
                        })
                        .expect("validated sequence type");
                    let decode = render_json_decode_expr("item", inner_type, custom_types);
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        method_symbol
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
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_map_with_string_key_type)
                {
                    let decode = method
                        .return_type
                        .as_ref()
                        .map(|t| render_json_decode_expr("jsonDecode(payload)", t, custom_types))
                        .unwrap_or_else(|| "null".to_string());
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!("            return {decode};\n"));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if method.return_type.as_ref().is_some_and(is_runtime_map_type) {
                    let decode = method
                        .return_type
                        .as_ref()
                        .map(|t| {
                            render_uniffi_binary_read_expression(
                                t,
                                "mapReader",
                                enums,
                                custom_types,
                            )
                        })
                        .unwrap_or_else(|| "null".to_string());
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
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_bytes_type)
                {
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
                        method_symbol
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str(
                        "            return Uint8List.fromList(resultData.asTypedList(resultLen));\n",
                    );
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustBytesFree(resultBuf);\n");
                    out.push_str("          }\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_optional_bytes_type)
                {
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
                        method_symbol
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str(
                        "            return Uint8List.fromList(resultData.asTypedList(resultLen));\n",
                    );
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustBytesFree(resultBuf);\n");
                    out.push_str("          }\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_sequence_bytes_type)
                {
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
                        method_symbol
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
                        method_symbol
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
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_optional_object_type)
                {
                    let inner = method
                        .return_type
                        .as_ref()
                        .and_then(|t| match runtime_unwrapped_type(t) {
                            Type::Optional { inner_type } => Some(inner_type.as_ref()),
                            _ => None,
                        })
                        .expect("validated optional object type");
                    let lift = render_object_lift_expr_with_objects(
                        inner,
                        "resultValue",
                        local_module_path,
                        "this",
                        objects,
                    );
                    out.push_str("          if (resultValue == 0) {\n");
                    out.push_str("            return null;\n");
                    out.push_str("          }\n");
                    out.push_str(&format!("          return {lift};\n"));
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_optional_record_type)
                {
                    let inner = method
                        .return_type
                        .as_ref()
                        .and_then(|t| match runtime_unwrapped_type(t) {
                            Type::Optional { inner_type } => Some(inner_type.as_ref()),
                            _ => None,
                        })
                        .expect("validated optional record type");
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
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_optional_enum_type)
                {
                    let inner = method
                        .return_type
                        .as_ref()
                        .and_then(|t| match runtime_unwrapped_type(t) {
                            Type::Optional { inner_type } => Some(inner_type.as_ref()),
                            _ => None,
                        })
                        .expect("validated optional enum type");
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
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_optional_primitive_type)
                {
                    let decode = method
                        .return_type
                        .as_ref()
                        .map(|t| render_json_decode_expr("decoded", t, custom_types))
                        .unwrap_or_else(|| "null".to_string());
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str("            final Object? decoded = jsonDecode(payload);\n");
                    out.push_str(&format!("            return {decode};\n"));
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
                    let decode =
                        render_plain_ffi_decode_expr(ret_type, "resultValue", custom_types);
                    out.push_str(&format!("          return {decode};\n"));
                }
                out.push_str("        }\n");
                debug_assert!(
                    method.throws_type.as_ref().and_then(throws_name_from_type).is_some()
                        || method.throws_type.is_none(),
                    "throws_type passed is_runtime_throws_enum_type but throws_name_from_type returned None"
                );
                if let Some(throws_type) = method.throws_type.as_ref() {
                    if let Some(throws_name) =
                        throws_name_from_type(throws_type).map(to_upper_camel)
                    {
                        // Free any bytes-like result buffer on the error path to prevent leaks.
                        if let Some(ret_type) = method.return_type.as_ref() {
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
                                out.push_str(
                                    "          final ffi.Pointer<Utf8> ptr = resultPtr;\n",
                                );
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
                            method_symbol
                        ));
                        out.push_str("        }\n");
                    }
                }
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
                let Some(throws_type) = method.throws_type.as_ref() else {
                    continue;
                };
                let Some(_throws_name) = throws_name_from_type(throws_type) else {
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
                out.push_str(&render_throws_expr(throws_type, "errRaw", "      "));
                out.push_str("    }\n");
                if let Some(ret_type) = method.return_type.as_ref() {
                    out.push_str("    final Object? okRaw = envelope['ok'];\n");
                    let decode = render_json_decode_expr("okRaw", ret_type, custom_types);
                    out.push_str(&format!("    return {decode};\n"));
                } else {
                    out.push_str("    return;\n");
                }
            } else if let Some(ret) = &method.return_type {
                let call_expr = format!("{method_field}({})", call_args.join(", "));
                if is_runtime_string_type(ret) {
                    let lifted =
                        lift_custom_if_needed("resultPtr.toDartString()", ret, custom_types);
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
                    out.push_str(&format!("      return {lifted};\n"));
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustStringFree(resultPtr);\n");
                    out.push_str("    }\n");
                } else if is_runtime_optional_string_type(ret) {
                    let lifted =
                        lift_custom_if_needed("resultPtr.toDartString()", ret, custom_types);
                    out.push_str(&format!(
                        "    final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("      return null;\n");
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str(&format!("      return {lifted};\n"));
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
                        "      return {}FfiCodec.decode(payload);\n",
                        to_upper_camel(enum_name)
                    ));
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustStringFree(resultPtr);\n");
                    out.push_str("    }\n");
                } else if is_runtime_object_type(ret) {
                    let lift = render_object_lift_expr_with_objects(
                        ret,
                        &call_expr,
                        local_module_path,
                        "_bindings()",
                        objects,
                    );
                    out.push_str(&format!("    return {lift};\n"));
                } else if is_runtime_optional_object_type(ret) {
                    let inner = match runtime_unwrapped_type(ret) {
                        Type::Optional { inner_type } => inner_type,
                        other => unreachable!("expected Optional or Sequence, got {other:?}"),
                    };
                    let lift = render_object_lift_expr_with_objects(
                        inner,
                        "resultHandle",
                        local_module_path,
                        "_bindings()",
                        objects,
                    );
                    out.push_str(&format!("    final int resultHandle = {call_expr};\n"));
                    out.push_str("    if (resultHandle == 0) {\n");
                    out.push_str("      return null;\n");
                    out.push_str("    }\n");
                    out.push_str(&format!("    return {lift};\n"));
                } else if is_runtime_optional_record_type(ret) {
                    let inner = match runtime_unwrapped_type(ret) {
                        Type::Optional { inner_type } => inner_type,
                        other => unreachable!("expected Optional or Sequence, got {other:?}"),
                    };
                    let record_name = record_name_from_type(inner).unwrap_or("Record");
                    out.push_str(&format!(
                        "    final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("      return null;\n");
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
                } else if is_runtime_optional_enum_type(ret) {
                    let inner = match runtime_unwrapped_type(ret) {
                        Type::Optional { inner_type } => inner_type,
                        other => unreachable!("expected Optional or Sequence, got {other:?}"),
                    };
                    let enum_name = enum_name_from_type(inner).unwrap_or("Enum");
                    out.push_str(&format!(
                        "    final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("      return null;\n");
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str("      final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "      return {}FfiCodec.decode(payload);\n",
                        to_upper_camel(enum_name)
                    ));
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustStringFree(resultPtr);\n");
                    out.push_str("    }\n");
                } else if is_runtime_optional_primitive_type(ret) {
                    let decode = render_json_decode_expr("decoded", ret, custom_types);
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
                    out.push_str("      final Object? decoded = jsonDecode(payload);\n");
                    out.push_str(&format!("      return {decode};\n"));
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustStringFree(resultPtr);\n");
                    out.push_str("    }\n");
                } else if is_runtime_sequence_json_type(ret) {
                    let inner_type = match runtime_unwrapped_type(ret) {
                        Type::Sequence { inner_type } => inner_type,
                        other => unreachable!("expected Optional or Sequence, got {other:?}"),
                    };
                    let decode = render_json_decode_expr("item", inner_type, custom_types);
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
                        "      return (jsonDecode(payload) as List).map((item) => {decode}).toList();\n"
                    ));
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustStringFree(resultPtr);\n");
                    out.push_str("    }\n");
                } else if is_runtime_map_with_string_key_type(ret) {
                    let decode = render_json_decode_expr("jsonDecode(payload)", ret, custom_types);
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
                    out.push_str(&format!("      return {decode};\n"));
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustStringFree(resultPtr);\n");
                    out.push_str("    }\n");
                } else if is_runtime_map_type(ret) {
                    let decode =
                        render_uniffi_binary_read_expression(ret, "mapReader", enums, custom_types);
                    out.push_str(&format!("    final _RustBuffer resultBuf = {call_expr};\n"));
                    out.push_str("    final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n");
                    out.push_str("    final int resultLen = resultBuf.len;\n");
                    out.push_str("    if (resultData == ffi.nullptr) {\n");
                    out.push_str("      _rustBytesFree(resultBuf);\n");
                    out.push_str("      final mapReader = _UniFfiBinaryReader(Uint8List(0));\n");
                    out.push_str(&format!("      return {decode};\n"));
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str("      final Uint8List resultBytes = Uint8List.fromList(resultData.asTypedList(resultLen));\n");
                    out.push_str("      final mapReader = _UniFfiBinaryReader(resultBytes);\n");
                    out.push_str(&format!("      return {decode};\n"));
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustBytesFree(resultBuf);\n");
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
                } else if is_runtime_optional_bytes_type(ret) {
                    out.push_str(&format!(
                        "    final _RustBufferOpt resultOpt = {call_expr};\n"
                    ));
                    out.push_str("    if (resultOpt.isSome == 0) {\n");
                    out.push_str("      return null;\n");
                    out.push_str("    }\n");
                    out.push_str("    final _RustBuffer resultBuf = resultOpt.value;\n");
                    out.push_str("    final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n");
                    out.push_str("    final int resultLen = resultBuf.len;\n");
                    out.push_str("    if (resultData == ffi.nullptr) {\n");
                    out.push_str("      if (resultLen == 0) {\n");
                    out.push_str("        _rustBytesFree(resultBuf);\n");
                    out.push_str("        return Uint8List(0);\n");
                    out.push_str("      }\n");
                    out.push_str(&format!(
                        "      throw StateError('Rust returned invalid optional buffer for {}');\n",
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
                } else if is_runtime_sequence_bytes_type(ret) {
                    out.push_str(&format!(
                        "    final _RustBufferVec resultVec = {call_expr};\n"
                    ));
                    out.push_str(
                        "    final ffi.Pointer<_RustBuffer> resultData = resultVec.data;\n",
                    );
                    out.push_str("    final int resultLen = resultVec.len;\n");
                    out.push_str("    if (resultData == ffi.nullptr) {\n");
                    out.push_str("      if (resultLen == 0) {\n");
                    out.push_str("        _rustBytesVecFree(resultVec);\n");
                    out.push_str("        return <Uint8List>[];\n");
                    out.push_str("      }\n");
                    out.push_str(&format!(
                        "      throw StateError('Rust returned invalid byte vector for {}');\n",
                        method_symbol
                    ));
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str("      final out = <Uint8List>[];\n");
                    out.push_str("      for (var i = 0; i < resultLen; i++) {\n");
                    out.push_str("        final _RustBuffer item = (resultData + i).ref;\n");
                    out.push_str("        final ffi.Pointer<ffi.Uint8> itemData = item.data;\n");
                    out.push_str("        final int itemLen = item.len;\n");
                    out.push_str("        if (itemData == ffi.nullptr) {\n");
                    out.push_str("          if (itemLen == 0) {\n");
                    out.push_str("            out.add(Uint8List(0));\n");
                    out.push_str("            continue;\n");
                    out.push_str("          }\n");
                    out.push_str(&format!(
                        "          throw StateError('Rust returned invalid nested buffer for {}');\n",
                        method_symbol
                    ));
                    out.push_str("        }\n");
                    out.push_str("        try {\n");
                    out.push_str(
                        "          out.add(Uint8List.fromList(itemData.asTypedList(itemLen)));\n",
                    );
                    out.push_str("        } finally {\n");
                    out.push_str("          _rustBytesFree(item);\n");
                    out.push_str("        }\n");
                    out.push_str("      }\n");
                    out.push_str("      return out;\n");
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustBytesVecFree(resultVec);\n");
                    out.push_str("    }\n");
                } else {
                    let decode = render_plain_ffi_decode_expr(ret, &call_expr, custom_types);
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
