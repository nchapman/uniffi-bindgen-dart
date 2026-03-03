use uniffi_bindgen::interface::Type;

use super::super::*;

/// Merge record and enum methods into the top-level function list by
/// wrapping each method as a `UdlFunction` with a synthetic `self` argument.
pub(super) fn merge_record_enum_methods(
    functions: &[UdlFunction],
    records: &[UdlRecord],
    enums: &[UdlEnum],
    local_module_path: &str,
) -> Vec<UdlFunction> {
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
    runtime_functions
}

/// Returns true when any function or object member is `runtime_unsupported`
/// but still eligible for the ffi-buffer code path.
pub(super) fn has_runtime_ffibuffer_fallback(
    runtime_functions: &[UdlFunction],
    objects: &[UdlObject],
) -> bool {
    runtime_functions.iter().any(|f| {
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
    })
}

/// Returns true when the generated bindings need a `_rustStringFree` lookup.
#[allow(clippy::too_many_arguments)]
pub(super) fn needs_runtime_string_free(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
    needs_async: bool,
) -> bool {
    needs_async
        || functions.iter().any(|f| {
            f.runtime_unsupported.is_none()
                && is_runtime_ffi_compatible_function(f, records, enums)
                && (function_returns_runtime_string(f)
                    || f.return_type
                        .as_ref()
                        .is_some_and(|t| is_runtime_utf8_pointer_marshaled_type(t, records, enums))
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
                    && (m
                        .return_type
                        .as_ref()
                        .is_some_and(|t| is_runtime_utf8_pointer_marshaled_type(t, records, enums))
                        || (m.throws_type.is_some()
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
                    && (m
                        .return_type
                        .as_ref()
                        .is_some_and(|t| is_runtime_utf8_pointer_marshaled_type(t, records, enums))
                        || (m.throws_type.is_some()
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
                    && (m
                        .return_type
                        .as_ref()
                        .is_some_and(|t| is_runtime_utf8_pointer_marshaled_type(t, records, enums))
                        || (m.throws_type.is_some()
                            && m.return_type
                                .as_ref()
                                .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
                                .unwrap_or(true)
                            && m.args
                                .iter()
                                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums))))
            })
        })
}

/// Returns true when the generated bindings need a `_rustBytesFree` lookup.
pub(super) fn needs_runtime_bytes_free(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    functions.iter().any(|f| {
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
    })
}

/// Returns true when the generated bindings need a `_rustBytesVecFree` lookup.
pub(super) fn needs_runtime_bytes_vec_free(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    functions.iter().any(|f| {
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
    })
}

/// Emit the initial `late final` FFI lookup fields: string_free, bytes_free,
/// bytes_vec_free, rustbuffer_from_bytes/free, callback init fields, and
/// trait callback init fields.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_ffi_lookup_fields(
    out: &mut String,
    ffi_namespace: &str,
    objects: &[UdlObject],
    callback_runtime_interfaces: &[&UdlCallbackInterface],
    needs_string_free: bool,
    needs_bytes_free: bool,
    needs_bytes_vec_free: bool,
    has_runtime_ffibuffer_fallback: bool,
) {
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
    for callback_interface in callback_runtime_interfaces {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use uniffi_bindgen::interface::Type;

    fn make_method(name: &str, ffi_symbol: Option<&str>) -> UdlObjectMethod {
        UdlObjectMethod {
            name: name.to_string(),
            ffi_symbol: ffi_symbol.map(String::from),
            ffi_arg_types: vec![],
            ffi_return_type: None,
            ffi_has_rust_call_status: false,
            runtime_unsupported: None,
            docstring: None,
            is_async: false,
            return_type: None,
            throws_type: None,
            args: vec![],
        }
    }

    fn make_record(name: &str, methods: Vec<UdlObjectMethod>) -> UdlRecord {
        UdlRecord {
            name: name.to_string(),
            docstring: None,
            fields: vec![],
            methods,
            traits: vec![],
        }
    }

    fn make_enum(name: &str, methods: Vec<UdlObjectMethod>) -> UdlEnum {
        UdlEnum {
            name: name.to_string(),
            docstring: None,
            is_error: false,
            is_non_exhaustive: false,
            has_discr_type: false,
            variants: vec![],
            methods,
            traits: vec![],
        }
    }

    #[test]
    fn merge_record_enum_methods_empty() {
        let result = merge_record_enum_methods(&[], &[], &[], "crate::my_mod");
        assert!(result.is_empty());
    }

    #[test]
    fn merge_record_methods_adds_self_arg() {
        let methods = vec![make_method("get_value", None)];
        let records = vec![make_record("MyRecord", methods)];
        let result = merge_record_enum_methods(&[], &records, &[], "crate::my_mod");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "myrecord_get_value");
        assert_eq!(result[0].args.len(), 1);
        assert_eq!(result[0].args[0].name, "self");
        assert!(
            matches!(result[0].args[0].type_, Type::Record { ref name, .. } if name == "MyRecord")
        );
    }

    #[test]
    fn merge_enum_methods_adds_self_arg() {
        let methods = vec![make_method("label", None)];
        let enums = vec![make_enum("Color", methods)];
        let result = merge_record_enum_methods(&[], &[], &enums, "crate::my_mod");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "color_label");
        assert_eq!(result[0].args[0].name, "self");
        assert!(matches!(result[0].args[0].type_, Type::Enum { ref name, .. } if name == "Color"));
    }

    #[test]
    fn merge_preserves_original_functions() {
        let original = vec![UdlFunction {
            name: "add".to_string(),
            ffi_symbol: None,
            ffi_arg_types: vec![],
            ffi_return_type: None,
            ffi_has_rust_call_status: false,
            runtime_unsupported: None,
            docstring: None,
            is_async: false,
            return_type: None,
            throws_type: None,
            args: vec![],
        }];
        let methods = vec![make_method("get_value", None)];
        let records = vec![make_record("Rec", methods)];
        let result = merge_record_enum_methods(&original, &records, &[], "crate::m");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "add");
        assert_eq!(result[1].name, "rec_get_value");
    }

    #[test]
    fn has_ffibuffer_fallback_false_for_empty() {
        assert!(!has_runtime_ffibuffer_fallback(&[], &[]));
    }

    #[test]
    fn has_ffibuffer_fallback_true_for_eligible_unsupported_function() {
        let functions = vec![UdlFunction {
            name: "test".to_string(),
            ffi_symbol: Some("uniffi_test".to_string()),
            ffi_arg_types: vec![],
            ffi_return_type: None,
            ffi_has_rust_call_status: false,
            runtime_unsupported: Some("not supported".to_string()),
            docstring: None,
            is_async: false,
            return_type: None,
            throws_type: None,
            args: vec![],
        }];
        assert!(has_runtime_ffibuffer_fallback(&functions, &[]));
    }

    #[test]
    fn needs_bytes_free_false_for_empty() {
        assert!(!needs_runtime_bytes_free(&[], &[], &[], &[]));
    }

    #[test]
    fn needs_bytes_vec_free_false_for_empty() {
        assert!(!needs_runtime_bytes_vec_free(&[], &[], &[], &[]));
    }

    #[test]
    fn needs_string_free_true_when_async() {
        assert!(needs_runtime_string_free(&[], &[], &[], &[], &[], true));
    }

    #[test]
    fn needs_string_free_false_for_empty() {
        assert!(!needs_runtime_string_free(&[], &[], &[], &[], &[], false));
    }

    // -- Helper builders for positive-path tests --

    fn make_function(name: &str, return_type: Option<Type>, args: Vec<UdlArg>) -> UdlFunction {
        UdlFunction {
            name: name.to_string(),
            ffi_symbol: None,
            ffi_arg_types: vec![],
            ffi_return_type: None,
            ffi_has_rust_call_status: false,
            runtime_unsupported: None,
            docstring: None,
            is_async: false,
            return_type,
            throws_type: None,
            args,
        }
    }

    fn make_object(
        name: &str,
        constructors: Vec<UdlObjectConstructor>,
        methods: Vec<UdlObjectMethod>,
    ) -> UdlObject {
        UdlObject {
            name: name.to_string(),
            docstring: None,
            is_error: false,
            has_callback_interface: false,
            ffi_free_symbol: None,
            ffi_clone_symbol: None,
            constructors,
            methods,
            trait_methods: UdlObjectTraitMethods::default(),
        }
    }

    fn make_constructor(name: &str, ffi_symbol: Option<&str>) -> UdlObjectConstructor {
        UdlObjectConstructor {
            name: name.to_string(),
            ffi_symbol: ffi_symbol.map(String::from),
            ffi_arg_types: vec![],
            ffi_return_type: None,
            ffi_has_rust_call_status: false,
            runtime_unsupported: None,
            docstring: None,
            is_async: false,
            args: vec![],
            throws_type: None,
        }
    }

    // -- Positive-path: needs_runtime_string_free --

    #[test]
    fn needs_string_free_true_for_function_returning_string() {
        let fns = vec![make_function("get_name", Some(Type::String), vec![])];
        assert!(needs_runtime_string_free(&fns, &[], &[], &[], &[], false));
    }

    #[test]
    fn needs_string_free_true_for_object_method_returning_string() {
        let method = UdlObjectMethod {
            return_type: Some(Type::String),
            ..make_method("get_label", None)
        };
        let objects = vec![make_object("Widget", vec![], vec![method])];
        assert!(needs_runtime_string_free(
            &[],
            &objects,
            &[],
            &[],
            &[],
            false
        ));
    }

    // -- Positive-path: needs_runtime_bytes_free --

    #[test]
    fn needs_bytes_free_true_for_function_returning_bytes() {
        let fns = vec![make_function("get_data", Some(Type::Bytes), vec![])];
        assert!(needs_runtime_bytes_free(&fns, &[], &[], &[]));
    }

    #[test]
    fn needs_bytes_free_true_for_object_method_returning_bytes() {
        let method = UdlObjectMethod {
            return_type: Some(Type::Bytes),
            ..make_method("read_payload", None)
        };
        let objects = vec![make_object("Stream", vec![], vec![method])];
        assert!(needs_runtime_bytes_free(&[], &objects, &[], &[]));
    }

    // -- Positive-path: needs_runtime_bytes_vec_free --

    #[test]
    fn needs_bytes_vec_free_true_for_function_returning_sequence_bytes() {
        let fns = vec![make_function(
            "get_chunks",
            Some(Type::Sequence {
                inner_type: Box::new(Type::Bytes),
            }),
            vec![],
        )];
        assert!(needs_runtime_bytes_vec_free(&fns, &[], &[], &[]));
    }

    // -- Positive-path: has_runtime_ffibuffer_fallback via object constructor --

    #[test]
    fn has_ffibuffer_fallback_true_for_eligible_unsupported_constructor() {
        let ctor = UdlObjectConstructor {
            runtime_unsupported: Some("complex args".to_string()),
            ..make_constructor("new", Some("uniffi_widget_new"))
        };
        let objects = vec![make_object("Widget", vec![ctor], vec![])];
        assert!(has_runtime_ffibuffer_fallback(&[], &objects));
    }
}
