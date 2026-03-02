use uniffi_bindgen::interface::Type;

use super::*;

pub(super) fn render_callback_interfaces(callback_interfaces: &[UdlCallbackInterface]) -> String {
    if callback_interfaces.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for callback_interface in callback_interfaces {
        let class_name = to_upper_camel(&callback_interface.name);
        out.push_str(&render_doc_comment(
            callback_interface.docstring.as_deref(),
            "",
        ));
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
            out.push_str(&render_doc_comment(method.docstring.as_deref(), "  "));
            out.push_str(&format!(
                "  {signature_return_type} {method_name}({args});\n"
            ));
        }
        out.push_str("}\n\n");
    }
    out
}

pub(super) fn render_callback_bridges(
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
                let return_field =
                    render_callback_async_result_return_field(return_type, records, enums)
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
        let has_async_methods = callback_interface.methods.iter().any(|m| m.is_async);
        if has_async_methods {
            out.push_str("  final Map<int, bool> _droppedFutures = <int, bool>{};\n");
            out.push_str("  int _nextDroppedFutureHandle = 1;\n\n");
            out.push_str("  int beginDroppedFutureTracking() {\n");
            out.push_str("    final int handle = _nextDroppedFutureHandle++;\n");
            out.push_str("    _droppedFutures[handle] = false;\n");
            out.push_str("    return handle;\n");
            out.push_str("  }\n\n");
            out.push_str("  void markDroppedFuture(int handle) {\n");
            out.push_str("    if (_droppedFutures.containsKey(handle)) {\n");
            out.push_str("      _droppedFutures[handle] = true;\n");
            out.push_str("    }\n");
            out.push_str("  }\n\n");
            out.push_str("  bool isDroppedFuture(int handle) {\n");
            out.push_str("    return _droppedFutures[handle] ?? true;\n");
            out.push_str("  }\n\n");
            out.push_str("  void finishDroppedFuture(int handle) {\n");
            out.push_str("    _droppedFutures.remove(handle);\n");
            out.push_str("  }\n\n");
        }
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
        out.push_str("      throw StateError('Invalid callback handle: $handle');\n");
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
        if has_async_methods {
            out.push_str(
                "  static final ffi.NativeCallable<ffi.Void Function(ffi.Uint64 handle)> _futureDroppedNative = ffi.NativeCallable<ffi.Void Function(ffi.Uint64 handle)>.isolateLocal((int handle) {\n",
            );
            out.push_str("    instance.markDroppedFuture(handle);\n");
            out.push_str("  });\n\n");
        }

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
                let async_result_uses_utf8_ptr = method
                    .return_type
                    .as_ref()
                    .is_some_and(|t| is_runtime_utf8_pointer_marshaled_type(t, records, enums));
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
                out.push_str(
                    "    final int droppedHandle = instance.beginDroppedFutureTracking();\n",
                );
                out.push_str("    if (uniffiOutDroppedCallback != ffi.nullptr) {\n");
                out.push_str("      uniffiOutDroppedCallback.ref\n");
                out.push_str("        ..handle = droppedHandle\n");
                out.push_str("        ..callback = _futureDroppedNative.nativeFunction;\n");
                out.push_str("    }\n");
                out.push_str("    if (callback == null) {\n");
                out.push_str(&format!(
                    "      final ffi.Pointer<{result_struct_name}> resultPtr = calloc<{result_struct_name}>();\n"
                ));
                if let Some(return_type) = method.return_type.as_ref() {
                    let default_value =
                        callback_async_default_return_expr(return_type, records, enums);
                    out.push_str(&format!(
                        "      resultPtr.ref.returnValue = {default_value};\n"
                    ));
                }
                out.push_str("      resultPtr.ref.callStatus\n");
                out.push_str("        ..code = _rustCallStatusUnexpectedError\n");
                out.push_str("        ..errorBuf = 'Invalid callback handle'.toNativeUtf8();\n");
                out.push_str(
                    "      final bool dropped = instance.isDroppedFuture(droppedHandle);\n",
                );
                out.push_str("      if (!dropped) {\n");
                out.push_str("        complete(callbackData, resultPtr.ref);\n");
                out.push_str("      } else {\n");
                out.push_str("        if (resultPtr.ref.callStatus.errorBuf != ffi.nullptr) {\n");
                out.push_str("          calloc.free(resultPtr.ref.callStatus.errorBuf);\n");
                out.push_str("        }\n");
                if async_result_uses_utf8_ptr {
                    out.push_str("        if (resultPtr.ref.returnValue != ffi.nullptr) {\n");
                    out.push_str("          calloc.free(resultPtr.ref.returnValue);\n");
                    out.push_str("        }\n");
                }
                out.push_str("      }\n");
                out.push_str("      instance.finishDroppedFuture(droppedHandle);\n");
                out.push_str("      calloc.free(resultPtr);\n");
                out.push_str("      return;\n");
                out.push_str("    }\n");
                out.push_str("    () async {\n");
                out.push_str(&format!(
                    "      final ffi.Pointer<{result_struct_name}> resultPtr = calloc<{result_struct_name}>();\n"
                ));
                if let Some(return_type) = method.return_type.as_ref() {
                    let default_value =
                        callback_async_default_return_expr(return_type, records, enums);
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
                out.push_str("      } catch (err) {\n");
                if method.throws_type.is_some() {
                    if let Some(exception_name) = method
                        .throws_type
                        .as_ref()
                        .and_then(enum_name_from_type)
                        .map(|name| format!("{}Exception", to_upper_camel(name)))
                    {
                        out.push_str(&format!("        if (err is {exception_name}) {{\n"));
                        out.push_str("          resultPtr.ref.callStatus\n");
                        out.push_str("            ..code = _rustCallStatusError\n");
                        out.push_str(&format!(
                            "            ..errorBuf = {exception_name}FfiCodec.encode(err).toNativeUtf8();\n"
                        ));
                        out.push_str("        } else {\n");
                        out.push_str("          resultPtr.ref.callStatus\n");
                        out.push_str("            ..code = _rustCallStatusUnexpectedError\n");
                        out.push_str("            ..errorBuf = err.toString().toNativeUtf8();\n");
                        out.push_str("        }\n");
                    } else {
                        out.push_str("        resultPtr.ref.callStatus\n");
                        out.push_str("          ..code = _rustCallStatusUnexpectedError\n");
                        out.push_str("          ..errorBuf = err.toString().toNativeUtf8();\n");
                    }
                } else {
                    out.push_str("        resultPtr.ref.callStatus\n");
                    out.push_str("          ..code = _rustCallStatusUnexpectedError\n");
                    out.push_str("          ..errorBuf = err.toString().toNativeUtf8();\n");
                }
                out.push_str("      } finally {\n");
                out.push_str(
                    "        final bool dropped = instance.isDroppedFuture(droppedHandle);\n",
                );
                out.push_str("        if (!dropped) {\n");
                out.push_str("          complete(callbackData, resultPtr.ref);\n");
                out.push_str("        } else {\n");
                out.push_str("          if (resultPtr.ref.callStatus.errorBuf != ffi.nullptr) {\n");
                out.push_str("            calloc.free(resultPtr.ref.callStatus.errorBuf);\n");
                out.push_str("          }\n");
                if async_result_uses_utf8_ptr {
                    out.push_str("          if (resultPtr.ref.returnValue != ffi.nullptr) {\n");
                    out.push_str("            calloc.free(resultPtr.ref.returnValue);\n");
                    out.push_str("          }\n");
                }
                out.push_str("        }\n");
                out.push_str("        instance.finishDroppedFuture(droppedHandle);\n");
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
                out.push_str("        ..errorBuf = 'Invalid callback handle'.toNativeUtf8();\n");
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
                out.push_str("    } catch (err) {\n");
                if method.throws_type.is_some() {
                    if let Some(exception_name) = method
                        .throws_type
                        .as_ref()
                        .and_then(enum_name_from_type)
                        .map(|name| format!("{}Exception", to_upper_camel(name)))
                    {
                        out.push_str(&format!("      if (err is {exception_name}) {{\n"));
                        out.push_str("        outStatus.ref\n");
                        out.push_str("          ..code = _rustCallStatusError\n");
                        out.push_str(&format!(
                            "          ..errorBuf = {exception_name}FfiCodec.encode(err).toNativeUtf8();\n"
                        ));
                        out.push_str("      } else {\n");
                        out.push_str("        outStatus.ref\n");
                        out.push_str("          ..code = _rustCallStatusUnexpectedError\n");
                        out.push_str("          ..errorBuf = err.toString().toNativeUtf8();\n");
                        out.push_str("      }\n");
                    } else {
                        out.push_str("      outStatus.ref\n");
                        out.push_str("        ..code = _rustCallStatusUnexpectedError\n");
                        out.push_str("        ..errorBuf = err.toString().toNativeUtf8();\n");
                    }
                } else {
                    out.push_str("      outStatus.ref\n");
                    out.push_str("        ..code = _rustCallStatusUnexpectedError\n");
                    out.push_str("        ..errorBuf = err.toString().toNativeUtf8();\n");
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

pub(super) fn has_runtime_callback_support(
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
        || records.iter().any(|r| {
            r.methods.iter().any(|m| {
                has_runtime_callback_args_in_args(&m.args, callback_interfaces, records, enums)
            })
        })
        || enums.iter().any(|e| {
            e.methods.iter().any(|m| {
                has_runtime_callback_args_in_args(&m.args, callback_interfaces, records, enums)
            })
        })
}

pub(super) fn is_runtime_callback_compatible_function(
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

pub(super) fn callback_interfaces_used_for_runtime<'a>(
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

pub(super) fn runtime_args_compatible_with_optional_callbacks(
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

pub(super) fn has_runtime_callback_args_in_args(
    args: &[UdlArg],
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    runtime_args_compatible_with_optional_callbacks(args, callback_interfaces, records, enums)
        .unwrap_or(false)
}

pub(super) fn callback_interface_name_from_type(type_: &Type) -> Option<&str> {
    match type_ {
        Type::CallbackInterface { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

pub(super) fn is_runtime_callback_interface_compatible(
    callback_interface: &UdlCallbackInterface,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    callback_interface
        .methods
        .iter()
        .all(|method| is_runtime_callback_method_compatible(method, records, enums))
}

pub(super) fn is_runtime_callback_method_compatible(
    method: &UdlCallbackMethod,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    method
        .throws_type
        .as_ref()
        .map(|t| {
            is_runtime_ffi_compatible_type(t, records, enums)
                && is_runtime_throws_enum_type(t, enums)
        })
        .unwrap_or(true)
        && method
            .return_type
            .as_ref()
            .map(|t| {
                if method.is_async {
                    is_runtime_callback_async_return_type_compatible(t, records, enums)
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

pub(super) fn is_runtime_callback_function_return_compatible_type(type_: &Type) -> bool {
    let type_ = runtime_unwrapped_type(type_);
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

pub(super) fn is_runtime_callback_method_type_compatible(
    type_: &Type,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    is_runtime_callback_function_return_compatible_type(type_)
        || is_runtime_string_type(type_)
        || is_runtime_optional_string_type(type_)
        || is_runtime_object_type(type_)
        || is_runtime_optional_object_type(type_)
        || records
            .iter()
            .any(|r| record_name_from_type(type_) == Some(r.name.as_str()))
        || is_runtime_enum_type(type_, enums)
        || is_runtime_sequence_json_type(type_)
        || is_runtime_bytes_type(type_)
}

pub(super) fn is_runtime_callback_async_return_type_compatible(
    type_: &Type,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    is_runtime_callback_method_type_compatible(type_, records, enums)
}

pub(super) fn callback_bridge_class_name(callback_name: &str) -> String {
    format!("_{}CallbackBridge", to_upper_camel(callback_name))
}

pub(super) fn callback_vtable_struct_name(callback_name: &str) -> String {
    format!("_{}VTable", to_upper_camel(callback_name))
}

pub(super) fn callback_init_symbol(callback_name: &str) -> String {
    format!("{}_callback_init", callback_name.to_ascii_lowercase())
}

pub(super) fn callback_init_field_name(callback_name: &str) -> String {
    safe_dart_identifier(&format!("_{}CallbackInit", to_lower_camel(callback_name)))
}

pub(super) fn callback_init_done_field_name(callback_name: &str) -> String {
    safe_dart_identifier(&format!(
        "_{}CallbackInitDone",
        to_lower_camel(callback_name)
    ))
}

pub(super) fn callback_vtable_field_name(callback_name: &str) -> String {
    safe_dart_identifier(&format!("_{}CallbackVTable", to_lower_camel(callback_name)))
}

pub(super) fn callback_async_result_struct_name(callback_name: &str, method_name: &str) -> String {
    format!(
        "_{}{}AsyncResult",
        to_upper_camel(callback_name),
        to_upper_camel(method_name)
    )
}

pub(super) fn render_callback_async_result_return_field(
    type_: &Type,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> Option<String> {
    let type_ = runtime_unwrapped_type(type_);
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
        Type::String => Some("  external ffi.Pointer<Utf8> returnValue;\n\n".to_string()),
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
            Some("  external ffi.Pointer<Utf8> returnValue;\n\n".to_string())
        }
        Type::Record { .. }
            if records
                .iter()
                .any(|r| record_name_from_type(type_) == Some(r.name.as_str())) =>
        {
            Some("  external ffi.Pointer<Utf8> returnValue;\n\n".to_string())
        }
        Type::Enum { .. } if is_runtime_enum_type(type_, enums) => {
            Some("  external ffi.Pointer<Utf8> returnValue;\n\n".to_string())
        }
        Type::Object { .. } => Some("  @ffi.Uint64()\n  external int returnValue;\n\n".to_string()),
        Type::Optional { inner_type } if is_runtime_object_type(inner_type) => {
            Some("  @ffi.Uint64()\n  external int returnValue;\n\n".to_string())
        }
        Type::Sequence { .. } if is_runtime_sequence_json_type(type_) => {
            Some("  external ffi.Pointer<Utf8> returnValue;\n\n".to_string())
        }
        Type::Bytes => Some("  external ffi.Pointer<Utf8> returnValue;\n\n".to_string()),
        _ => None,
    }
}

pub(super) fn callback_async_default_return_expr(
    type_: &Type,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> &'static str {
    let type_ = runtime_unwrapped_type(type_);
    match type_ {
        Type::Float32 | Type::Float64 => "0.0",
        Type::String => "ffi.nullptr",
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => "ffi.nullptr",
        Type::Record { .. }
            if records
                .iter()
                .any(|r| record_name_from_type(type_) == Some(r.name.as_str())) =>
        {
            "ffi.nullptr"
        }
        Type::Enum { .. } if is_runtime_enum_type(type_, enums) => "ffi.nullptr",
        Type::Object { .. } => "0",
        Type::Optional { inner_type } if is_runtime_object_type(inner_type) => "0",
        Type::Sequence { .. } if is_runtime_sequence_json_type(type_) => "ffi.nullptr",
        Type::Bytes => "ffi.nullptr",
        _ => "0",
    }
}

pub(super) fn render_callback_arg_decode_expr(
    type_: &Type,
    arg_name: &str,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let type_ = runtime_unwrapped_type(type_);
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
                "{arg_name} == ffi.nullptr ? (throw StateError('Rust passed null enum callback arg')) : {}FfiCodec.decode({arg_name}.toDartString())",
                to_upper_camel(enum_name)
            )
        }
        Type::Sequence { inner_type } if is_runtime_sequence_json_type(type_) => {
            let inner_decode = render_json_decode_expr("item", inner_type);
            format!(
                "{arg_name} == ffi.nullptr ? (throw StateError('Rust passed null sequence callback arg')) : (jsonDecode({arg_name}.toDartString()) as List).map((item) => {inner_decode}).toList()"
            )
        }
        Type::Object { name, .. } => {
            format!(
                "{}FfiCodec.lift({arg_name})",
                to_upper_camel(name)
            )
        }
        Type::Optional { inner_type } if is_runtime_object_type(inner_type) => {
            let name = object_name_from_type(inner_type)
                .expect("is_runtime_object_type guarantees Object inner");
            format!(
                "{arg_name} == 0 ? null : {}FfiCodec.lift({arg_name})",
                to_upper_camel(name)
            )
        }
        Type::Bytes => {
            format!(
                "{arg_name} == ffi.nullptr ? (throw StateError('Rust passed null bytes callback arg')) : base64Decode({arg_name}.toDartString())"
            )
        }
        Type::Timestamp => {
            format!("DateTime.fromMicrosecondsSinceEpoch({arg_name}, isUtc: true)")
        }
        Type::Duration => format!("Duration(microseconds: {arg_name})"),
        _ => arg_name.to_string(),
    }
}

pub(super) fn render_callback_return_encode_expr(
    type_: &Type,
    value_expr: &str,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let type_ = runtime_unwrapped_type(type_);
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
                "{}FfiCodec.encode({value_expr}).toNativeUtf8()",
                to_upper_camel(enum_name)
            )
        }
        Type::Sequence { inner_type } if is_runtime_sequence_json_type(type_) => {
            let inner_encode = render_json_encode_expr("item", inner_type);
            format!(
                "jsonEncode({value_expr}.map((item) => {inner_encode}).toList()).toNativeUtf8()"
            )
        }
        Type::Object { name, .. } => {
            format!("{}FfiCodec.lower({value_expr})", to_upper_camel(name))
        }
        Type::Optional { inner_type } if is_runtime_object_type(inner_type) => {
            let name = object_name_from_type(inner_type)
                .expect("is_runtime_object_type guarantees Object inner");
            format!(
                "{value_expr} == null ? 0 : {}FfiCodec.lower({value_expr})",
                to_upper_camel(name)
            )
        }
        Type::Bytes => {
            format!("base64Encode({value_expr}).toNativeUtf8()")
        }
        Type::Timestamp => format!("{value_expr}.toUtc().microsecondsSinceEpoch"),
        Type::Duration => format!("{value_expr}.inMicroseconds"),
        Type::Boolean => format!("{value_expr} ? 1 : 0"),
        _ => value_expr.to_string(),
    }
}
