use super::*;

pub(super) fn render_function_stubs(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    ffi_class_name: &str,
    api_overrides: &ApiOverrides,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    if functions.is_empty()
        && objects.is_empty()
        && records.iter().all(|r| r.methods.is_empty())
        && enums.iter().all(|e| e.methods.is_empty())
    {
        return String::new();
    }

    let mut out = String::new();
    let has_runtime_functions = functions.iter().any(|f| {
        is_runtime_ffi_compatible_function(f, records, enums)
            || is_runtime_callback_compatible_function(f, callback_interfaces, records, enums)
            || has_runtime_callback_args_in_args(&f.args, callback_interfaces, records, enums)
    }) || !objects.is_empty()
        || records.iter().any(|r| !r.methods.is_empty())
        || enums.iter().any(|e| !e.methods.is_empty());
    out.push('\n');
    if has_runtime_functions {
        out.push_str(&format!("{ffi_class_name}? _defaultBindings;\n\n"));
        out.push_str(&format!(
            "{ffi_class_name} _bindings() => _defaultBindings ??= {ffi_class_name}();\n\n"
        ));
        out.push_str(
            "void configureDefaultBindings({ffi.DynamicLibrary? dynamicLibrary, String? libraryPath}) {\n",
        );
        out.push_str(&format!(
            "  _defaultBindings = {ffi_class_name}(dynamicLibrary: dynamicLibrary, libraryPath: libraryPath);\n"
        ));
        out.push_str("}\n\n");
        out.push_str("void resetDefaultBindings() {\n");
        out.push_str("  _defaultBindings = null;\n");
        out.push_str("}\n\n");
    }
    for f in functions {
        if api_overrides.excluded(&ApiOverrides::fn_key(&f.name)) {
            continue;
        }
        let public_fn_name = safe_dart_identifier(
            &api_overrides
                .renamed_or_default(&ApiOverrides::fn_key(&f.name), || to_lower_camel(&f.name)),
        );
        let internal_fn_name = safe_dart_identifier(&to_lower_camel(&f.name));
        let value_return_type = f
            .return_type
            .as_ref()
            .map(map_uniffi_type_to_dart)
            .unwrap_or_else(|| "void".to_string());
        let signature_return_type = if f.is_async {
            format!("Future<{value_return_type}>")
        } else {
            value_return_type.clone()
        };
        let args = render_callable_args_signature(&f.args, enums);
        let arg_names = render_callable_arg_names(&f.args);

        out.push_str(&render_doc_comment(f.docstring.as_deref(), ""));
        out.push_str(&format!(
            "{signature_return_type} {public_fn_name}({args}) {{\n"
        ));
        if is_runtime_ffi_compatible_function(f, records, enums)
            || is_runtime_callback_compatible_function(f, callback_interfaces, records, enums)
            || has_runtime_callback_args_in_args(&f.args, callback_interfaces, records, enums)
        {
            if f.is_async {
                if is_runtime_async_rust_future_compatible_function(
                    f,
                    callback_interfaces,
                    records,
                    enums,
                ) {
                    out.push_str(&format!(
                        "  return _bindings().{internal_fn_name}({arg_names});\n"
                    ));
                } else {
                    out.push_str(&format!(
                        "  return Future(() => _bindings().{internal_fn_name}({arg_names}));\n"
                    ));
                }
            } else if f.return_type.is_some() {
                out.push_str(&format!(
                    "  return _bindings().{internal_fn_name}({arg_names});\n"
                ));
            } else {
                out.push_str(&format!("  _bindings().{internal_fn_name}({arg_names});\n"));
            }
        } else if f.is_async {
            out.push_str("  return Future.error(UnimplementedError('TODO: bind to Rust FFI'));\n");
        } else {
            out.push_str("  throw UnimplementedError('TODO: bind to Rust FFI');\n");
        }
        out.push_str("}\n\n");
    }
    out
}
